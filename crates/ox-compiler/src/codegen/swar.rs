//! SIMD Within A Register (SWAR) string and key matcher generator.
//!
//! Converts ASCII string literals and JSON keys into 64-bit and 32-bit little-endian
//! integer constants for branchless, zero-allocation multi-byte verification.

use serde::{Deserialize, Serialize};

/// High-bit mask for detecting non-ASCII (multi-byte UTF-8) bytes in a 32-bit word.
// Mask 0x80808080 checks bit 7 of all 4 bytes simultaneously
pub const SWAR_ASCII_MASK_U32: u32 = 0x80808080;

/// High-bit mask for detecting non-ASCII (multi-byte UTF-8) bytes in a 64-bit word.
// Mask 0x8080808080808080 checks bit 7 of all 8 bytes simultaneously
pub const SWAR_ASCII_MASK_U64: u64 = 0x8080_8080_8080_8080;

/// Represents a single SWAR word read and comparison chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwarChunk {
    /// 8-byte 64-bit integer comparison (`view.getBigUint64(pos + offset, true) === 0x...n`)
    U64 {
        offset: usize,
        value: u64,
        ascii_comment: String,
    },
    /// 4-byte 32-bit integer comparison (`view.getUint32(pos + offset, true) === 0x...`)
    U32 {
        offset: usize,
        value: u32,
        ascii_comment: String,
    },
    /// 2-byte 16-bit integer comparison (`view.getUint16(pos + offset, true) === 0x...`)
    U16 {
        offset: usize,
        value: u16,
        ascii_comment: String,
    },
    /// 1-byte 8-bit integer comparison (`view.getUint8(pos + offset) === 0x...`)
    U8 {
        offset: usize,
        value: u8,
        ascii_comment: String,
    },
}

/// Decompose an ASCII string into an optimal sequence of 64-bit, 32-bit, 16-bit, and 8-bit SWAR chunks.
pub fn encode_swar_chunks(s: &str) -> Vec<SwarChunk> {
    let bytes = s.as_bytes();
    let mut chunks = Vec::new();
    let mut i = 0;
    let n = bytes.len();

    // 8-byte chunks (64-bit words)
    while i + 8 <= n {
        let slice = &bytes[i..i + 8];
        let mut buf = [0u8; 8];
        buf.copy_from_slice(slice);
        let val = u64::from_le_bytes(buf);
        let ascii_repr = String::from_utf8_lossy(slice).replace('"', "\\\"");
        chunks.push(SwarChunk::U64 {
            offset: i,
            value: val,
            ascii_comment: format!("\"{}\" -> ASCII {:?}", ascii_repr, slice),
        });
        i += 8;
    }

    // 4-byte chunk (32-bit word)
    if i + 4 <= n {
        let slice = &bytes[i..i + 4];
        let mut buf = [0u8; 4];
        buf.copy_from_slice(slice);
        let val = u32::from_le_bytes(buf);
        let ascii_repr = String::from_utf8_lossy(slice).replace('"', "\\\"");
        chunks.push(SwarChunk::U32 {
            offset: i,
            value: val,
            ascii_comment: format!("\"{}\" -> ASCII {:?}", ascii_repr, slice),
        });
        i += 4;
    }

    // 2-byte chunk (16-bit word)
    if i + 2 <= n {
        let slice = &bytes[i..i + 2];
        let mut buf = [0u8; 2];
        buf.copy_from_slice(slice);
        let val = u16::from_le_bytes(buf);
        let ascii_repr = String::from_utf8_lossy(slice).replace('"', "\\\"");
        chunks.push(SwarChunk::U16 {
            offset: i,
            value: val,
            ascii_comment: format!("\"{}\" -> ASCII {:?}", ascii_repr, slice),
        });
        i += 2;
    }

    // Remaining single byte (8-bit)
    if i < n {
        let slice = &bytes[i..i + 1];
        let val = slice[0];
        let ascii_repr = String::from_utf8_lossy(slice).replace('"', "\\\"");
        chunks.push(SwarChunk::U8 {
            offset: i,
            value: val,
            ascii_comment: format!("\"{}\" -> ASCII {:?}", ascii_repr, slice),
        });
    }

    chunks
}

/// Emits JavaScript boolean expression checking all SWAR chunks in sequence against a DataView.
///
/// ### Codegen Example Output:
/// ```javascript
/// (view.getBigUint64(pos + 0, true) === 0x617473656d697422n && /* "timesta" */
///  view.getUint32(pos + 8, true) === 0x3a22706d) /* "mp\":" */
/// ```
pub fn emit_js_swar_check(chunks: &[SwarChunk], view_var: &str, pos_var: &str) -> String {
    if chunks.is_empty() {
        return "true".to_string();
    }

    let checks: Vec<String> = chunks
        .iter()
        .map(|chunk| match chunk {
            SwarChunk::U64 {
                offset,
                value,
                ascii_comment,
            } => {
                let off_expr = if *offset == 0 {
                    pos_var.to_string()
                } else {
                    format!("{} + {}", pos_var, offset)
                };
                format!(
                    "({}.getBigUint64({}, true) === 0x{:016x}n /* {} */)",
                    view_var, off_expr, value, ascii_comment
                )
            }
            SwarChunk::U32 {
                offset,
                value,
                ascii_comment,
            } => {
                let off_expr = if *offset == 0 {
                    pos_var.to_string()
                } else {
                    format!("{} + {}", pos_var, offset)
                };
                format!(
                    "({}.getUint32({}, true) === 0x{:08x} /* {} */)",
                    view_var, off_expr, value, ascii_comment
                )
            }
            SwarChunk::U16 {
                offset,
                value,
                ascii_comment,
            } => {
                let off_expr = if *offset == 0 {
                    pos_var.to_string()
                } else {
                    format!("{} + {}", pos_var, offset)
                };
                format!(
                    "({}.getUint16({}, true) === 0x{:04x} /* {} */)",
                    view_var, off_expr, value, ascii_comment
                )
            }
            SwarChunk::U8 {
                offset,
                value,
                ascii_comment,
            } => {
                let off_expr = if *offset == 0 {
                    pos_var.to_string()
                } else {
                    format!("{} + {}", pos_var, offset)
                };
                format!(
                    "({}.getUint8({}) === 0x{:02x} /* {} */)",
                    view_var, off_expr, value, ascii_comment
                )
            }
        })
        .collect();

    checks.join(" &&\n    ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_timestamp_key_swar() {
        // Matches PLAN.md section 4.3 example:
        // `"timestamp":` (12 bytes)
        // [0..8]  "timesta" -> 0x617473656d697422n
        // [8..12] "mp\":"   -> 0x3a22706d
        let key = "\"timestamp\":";
        let chunks = encode_swar_chunks(key);
        assert_eq!(chunks.len(), 2);

        if let SwarChunk::U64 { value, .. } = chunks[0] {
            assert_eq!(value, 0x617473656d697422);
        } else {
            panic!("Expected U64 chunk");
        }

        if let SwarChunk::U32 { value, .. } = chunks[1] {
            assert_eq!(value, 0x3a22706d);
        } else {
            panic!("Expected U32 chunk");
        }

        let js_code = emit_js_swar_check(&chunks, "view", "pos");
        assert!(js_code.contains("0x617473656d697422n"));
        assert!(js_code.contains("0x3a22706d"));
    }
}
