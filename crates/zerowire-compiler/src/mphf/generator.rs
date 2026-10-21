//! Compile-time Minimal Perfect Hash Function (MPHF) synthesis.
//!
//! Synthesizes branchless O(1) jump table indices over discriminator string literals:
//! h(w, L) = ((w ^ L) * C) >>> R (mod M)
//! where:
//! - w = first 4 bytes of literal as little-endian u32
//! - L = byte length of literal
//! - C = compile-time discovered 32-bit odd multiplier
//! - R = 32 - log2(M)
//! - M = power of 2 >= variant count

use crate::error::CompileError;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Extracts the first 4 bytes of a string literal as a little-endian u32.
/// Strings shorter than 4 bytes are zero-padded.
#[inline]
pub fn extract_first_u32(s: &str) -> u32 {
    let bytes = s.as_bytes();
    let mut buf = [0u8; 4];
    let len = bytes.len().min(4);
    buf[..len].copy_from_slice(&bytes[..len]);
    u32::from_le_bytes(buf)
}

/// Represents an entry in the generated MPHF dispatch table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MphfSlot {
    /// Expected literal string
    pub key: String,
    /// Slot index in [0, capacity)
    pub slot: u32,
    /// Little-endian u32 word of first 4 bytes
    pub first_u32: u32,
    /// Total byte length of literal
    pub byte_len: u32,
    /// Precomputed 64-bit SWAR prefix if key length >= 8
    pub swar_prefix_u64: Option<u64>,
}

/// Complete compile-time Minimal Perfect Hash Table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MphfTable {
    /// 32-bit odd multiplier C
    pub multiplier: u32,
    /// Table capacity M (power of 2)
    pub capacity: u32,
    /// Unsigned right-shift R = 32 - log2(capacity)
    pub shift: u32,
    /// Ordered slots from index 0 to capacity - 1 (unoccupied slots are None)
    pub slots: Vec<Option<MphfSlot>>,
    /// Map from key to assigned slot index
    pub key_to_slot: IndexMap<String, u32>,
}

impl MphfTable {
    /// Computes the slot index for a given 4-byte word and length.
    #[inline]
    pub fn compute_slot(&self, first_u32: u32, byte_len: u32) -> u32 {
        let x = first_u32 ^ byte_len;
        (x.wrapping_mul(self.multiplier)) >> self.shift
    }

    /// Evaluates the slot index directly for a string slice.
    #[inline]
    pub fn lookup(&self, s: &str) -> Option<&MphfSlot> {
        let w = extract_first_u32(s);
        let len = s.len() as u32;
        let slot_idx = self.compute_slot(w, len);
        if let Some(Some(slot)) = self.slots.get(slot_idx as usize) {
            // Full-word confirmation check to reject out-of-set keys
            if slot.key == s {
                return Some(slot);
            }
        }
        None
    }

    /// Emits a JavaScript expression computing the slot index:
    /// `(Math.imul(w ^ len, 0xHEX) >>> SHIFT)`
    pub fn emit_js_index_expr(&self, word_expr: &str, len_expr: &str) -> String {
        format!(
            "(Math.imul({} ^ {}, 0x{:08x}) >>> {})",
            word_expr, len_expr, self.multiplier, self.shift
        )
    }
}

/// Synthesize a Minimal Perfect Hash Function for a set of distinct string keys.
pub fn synthesize_mphf(keys: &[&str]) -> Result<MphfTable, CompileError> {
    if keys.is_empty() {
        return Err(CompileError::MphfError {
            message: "Cannot synthesize MPHF for empty key set".to_string(),
        });
    }

    // Check for duplicate keys
    let mut unique_keys = Vec::new();
    for &k in keys {
        if unique_keys.contains(&k) {
            return Err(CompileError::MphfError {
                message: format!("Duplicate key in MPHF synthesis: '{}'", k),
            });
        }
        unique_keys.push(k);
    }

    let n = unique_keys.len();
    // Capacity M starts at smallest power of 2 >= n
    let mut capacity = n.next_power_of_two() as u32;

    let items: Vec<KeyData> = unique_keys
        .iter()
        .map(|&k| {
            let w = extract_first_u32(k);
            let len = k.len() as u32;
            let x = w ^ len;
            KeyData { key: k, w, len, x }
        })
        .collect();

    // Candidate odd multipliers to search.
    // We start with well-known golden ratio constants and pseudorandom odd 32-bit integers.
    let base_multipliers: [u32; 16] = [
        0x9e3779b9, // 2654435769 (Fibonacci / Knuth golden ratio)
        0x15a4e35,  // Sample constant from PLAN.md
        0x45d9f3b,  // Fast hash constant
        0x27d4eb2d, // LCG multiplier
        0x85ebca6b, // MurmurHash3 fmix32 constant 1
        0xc2b2ae35, // MurmurHash3 fmix32 constant 2
        0x21f0aaad, // SplitMix32 constant
        0x7feb352d, // High-entropy prime
        0x845a2697, // High-entropy odd
        0x65b93d71, // High-entropy odd
        0x9b3a1523, // High-entropy odd
        0x5c4b1d39, // High-entropy odd
        0x1a87e5b3, // High-entropy odd
        0xe6cd328b, // High-entropy odd
        0x3c6ef372 | 1,
        0xa0761d64 | 1,
    ];

    // Attempt up to 3 capacity expansions (M, 2M, 4M)
    for _ in 0..3 {
        let shift = 32 - capacity.trailing_zeros();

        // 1. Try base multipliers
        for &mult in &base_multipliers {
            if let Some(table) = try_build_table(&items, mult, capacity, shift) {
                return Ok(table);
            }
        }

        // 2. Systematic LCG search for up to 65,536 iterations
        let mut seed: u32 = 0x1234567 | 1;
        for _ in 0..65536 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223) | 1;
            if let Some(table) = try_build_table(&items, seed, capacity, shift) {
                return Ok(table);
            }
        }

        // Expand capacity to next power of 2 if density was too high
        capacity *= 2;
    }

    Err(CompileError::MphfError {
        message: format!(
            "Failed to synthesize collision-free MPHF for keys: {:?}",
            keys
        ),
    })
}

fn try_build_table(
    items: &[KeyData],
    multiplier: u32,
    capacity: u32,
    shift: u32,
) -> Option<MphfTable> {
    let mut slots: Vec<Option<MphfSlot>> = vec![None; capacity as usize];
    let mut key_to_slot = IndexMap::with_capacity(items.len());

    for item in items {
        let slot_idx = ((item.x.wrapping_mul(multiplier)) >> shift) as usize;
        if slot_idx >= capacity as usize || slots[slot_idx].is_some() {
            // Collision detected
            return None;
        }

        // Compute 64-bit SWAR prefix if key is at least 8 bytes
        let swar_prefix_u64 = if item.key.len() >= 8 {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&item.key.as_bytes()[..8]);
            Some(u64::from_le_bytes(buf))
        } else {
            None
        };

        let slot = MphfSlot {
            key: item.key.to_string(),
            slot: slot_idx as u32,
            first_u32: item.w,
            byte_len: item.len,
            swar_prefix_u64,
        };

        slots[slot_idx] = Some(slot);
        key_to_slot.insert(item.key.to_string(), slot_idx as u32);
    }

    Some(MphfTable {
        multiplier,
        capacity,
        shift,
        slots,
        key_to_slot,
    })
}

struct KeyData<'a> {
    key: &'a str,
    w: u32,
    len: u32,
    x: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mphf_synthesis_wire_messages() {
        // Tagged union from PLAN.md:
        // SUBSCRIBE, UNSUBSCRIBE, PING, EXECUTION
        let keys = ["SUBSCRIBE", "UNSUBSCRIBE", "PING", "EXECUTION"];
        let table = synthesize_mphf(&keys).expect("MPHF synthesis should succeed");

        assert_eq!(table.key_to_slot.len(), 4);
        assert!(table.capacity >= 4);

        // Verify all keys map to distinct valid slots
        for &k in &keys {
            let slot = table.lookup(k).expect("Key should resolve in table");
            assert_eq!(slot.key, k);
        }

        // Out-of-set key must return None (even if hashing lands in a slot)
        assert!(table.lookup("UNKNOWN").is_none());
        assert!(table.lookup("PING2").is_none());
        assert!(table.lookup("").is_none());
    }

    #[test]
    fn test_mphf_order_types() {
        let keys = [
            "LIMIT",
            "MARKET",
            "STOP_LOSS",
            "TAKE_PROFIT",
            "TRAILING_STOP",
        ];
        let table = synthesize_mphf(&keys).expect("MPHF synthesis should succeed");

        for &k in &keys {
            let slot = table.lookup(k).expect("Key should resolve in table");
            assert_eq!(slot.key, k);
        }
    }
}
