//! WebAssembly v128 SIMD and scalar structural JSON scanner.
//!
//! Scans raw UTF-8 byte buffers for JSON structural delimiters (quotes, colons,
//! commas, braces, brackets), masks characters inside strings via prefix-XOR,
//! and writes structural token tape entries at maximum memory bandwidth.

/// Structural token identifiers stored in the top 4 bits of each tape entry.
pub const TOKEN_QUOTE: u32 = 0x1;
pub const TOKEN_COLON: u32 = 0x2;
pub const TOKEN_COMMA: u32 = 0x3;
pub const TOKEN_LBRACE: u32 = 0x4;
pub const TOKEN_RBRACE: u32 = 0x5;
pub const TOKEN_LBRACKET: u32 = 0x6;
pub const TOKEN_RBRACKET: u32 = 0x7;

/// ASCII byte constants
pub const BYTE_QUOTE: u8 = 0x22; // '"'
pub const BYTE_BACKSLASH: u8 = 0x5C; // '\\'
pub const BYTE_COLON: u8 = 0x3A; // ':'
pub const BYTE_COMMA: u8 = 0x2C; // ','
pub const BYTE_LBRACE: u8 = 0x7B; // '{'
pub const BYTE_RBRACE: u8 = 0x7D; // '}'
pub const BYTE_LBRACKET: u8 = 0x5B; // '['
pub const BYTE_RBRACKET: u8 = 0x5D; // ']'

/// Encodes a token and byte offset into a 32-bit tape word.
/// High 4 bits: token type (0x1..0x7). Low 28 bits: byte offset.
#[inline(always)]
pub const fn make_tape_entry(token_type: u32, offset: usize) -> u32 {
    (token_type << 28) | ((offset as u32) & 0x0FFF_FFFF)
}

/// Decodes token type and offset from a 32-bit tape word.
#[inline(always)]
pub const fn decode_tape_entry(entry: u32) -> (u32, usize) {
    let token_type = entry >> 28;
    let offset = (entry & 0x0FFF_FFFF) as usize;
    (token_type, offset)
}

/// Structural scanning engine.
pub struct StructuralScanner;

impl StructuralScanner {
    /// Scans a byte buffer and populates `tape` with structural token entries.
    /// Returns the number of tape entries written.
    #[cfg(target_arch = "wasm32")]
    pub fn scan(buf: &[u8], tape: &mut [u32]) -> usize {
        use core::arch::wasm32::*;

        let len = buf.len();
        let max_tape = tape.len();
        let mut tape_idx = 0;
        let mut offset = 0;
        let mut in_string = false;

        // Splat ASCII constants into 128-bit SIMD registers
        let v_quote = i8x16_splat(BYTE_QUOTE as i8);
        let v_colon = i8x16_splat(BYTE_COLON as i8);
        let v_comma = i8x16_splat(BYTE_COMMA as i8);
        let v_lbrace = i8x16_splat(BYTE_LBRACE as i8);
        let v_rbrace = i8x16_splat(BYTE_RBRACE as i8);
        let v_lbracket = i8x16_splat(BYTE_LBRACKET as i8);
        let v_rbracket = i8x16_splat(BYTE_RBRACKET as i8);

        let ptr = buf.as_ptr();

        // 16-byte SIMD chunk loop
        while offset + 16 <= len && tape_idx + 16 <= max_tape {
            // SAFETY: Loop condition `offset + 16 <= len` guarantees that reading 16 bytes
            // starting at `ptr.add(offset)` is strictly within buffer boundaries.
            let chunk = unsafe { v128_load(ptr.add(offset) as *const v128) };

            // Find quote positions
            let quote_mask = i8x16_bitmask(i8x16_eq(chunk, v_quote)) as u32;

            // Prefix-XOR to compute in-string mask across 16 bytes
            let mut string_mask = quote_mask;
            string_mask ^= string_mask << 1;
            string_mask ^= string_mask << 2;
            string_mask ^= string_mask << 4;
            string_mask ^= string_mask << 8;
            if in_string {
                string_mask ^= 0xFFFF;
            }
            in_string = (string_mask & 0x8000) != 0;

            // Match structural characters outside strings
            let non_string_mask = (!string_mask) & 0xFFFF;

            let colon_mask = (i8x16_bitmask(i8x16_eq(chunk, v_colon)) as u32) & non_string_mask;
            let comma_mask = (i8x16_bitmask(i8x16_eq(chunk, v_comma)) as u32) & non_string_mask;
            let lbrace_mask = (i8x16_bitmask(i8x16_eq(chunk, v_lbrace)) as u32) & non_string_mask;
            let rbrace_mask = (i8x16_bitmask(i8x16_eq(chunk, v_rbrace)) as u32) & non_string_mask;
            let lbracket_mask =
                (i8x16_bitmask(i8x16_eq(chunk, v_lbracket)) as u32) & non_string_mask;
            let rbracket_mask =
                (i8x16_bitmask(i8x16_eq(chunk, v_rbracket)) as u32) & non_string_mask;

            let all_structural =
                colon_mask | comma_mask | lbrace_mask | rbrace_mask | lbracket_mask | rbracket_mask;

            if all_structural != 0 {
                for bit in 0..16 {
                    let bit_val = 1 << bit;
                    if (all_structural & bit_val) != 0 {
                        let token = if (colon_mask & bit_val) != 0 {
                            TOKEN_COLON
                        } else if (comma_mask & bit_val) != 0 {
                            TOKEN_COMMA
                        } else if (lbrace_mask & bit_val) != 0 {
                            TOKEN_LBRACE
                        } else if (rbrace_mask & bit_val) != 0 {
                            TOKEN_RBRACE
                        } else if (lbracket_mask & bit_val) != 0 {
                            TOKEN_LBRACKET
                        } else {
                            TOKEN_RBRACKET
                        };
                        tape[tape_idx] = make_tape_entry(token, offset + bit);
                        tape_idx += 1;
                    }
                }
            }

            offset += 16;
        }

        // Scalar fallback for remaining tail bytes (< 16 bytes)
        while offset < len && tape_idx < max_tape {
            let b = buf[offset];
            if b == BYTE_QUOTE {
                in_string = !in_string;
            } else if !in_string {
                let token = match b {
                    BYTE_COLON => Some(TOKEN_COLON),
                    BYTE_COMMA => Some(TOKEN_COMMA),
                    BYTE_LBRACE => Some(TOKEN_LBRACE),
                    BYTE_RBRACE => Some(TOKEN_RBRACE),
                    BYTE_LBRACKET => Some(TOKEN_LBRACKET),
                    BYTE_RBRACKET => Some(TOKEN_RBRACKET),
                    _ => None,
                };
                if let Some(t) = token {
                    tape[tape_idx] = make_tape_entry(t, offset);
                    tape_idx += 1;
                }
            }
            offset += 1;
        }

        tape_idx
    }

    /// Scalar implementation for host testing and non-WASM targets.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn scan(buf: &[u8], tape: &mut [u32]) -> usize {
        let max_tape = tape.len();
        let mut tape_idx = 0;
        let mut in_string = false;
        let mut escaped = false;

        for (offset, &b) in buf.iter().enumerate() {
            if tape_idx >= max_tape {
                break;
            }
            if escaped {
                escaped = false;
                continue;
            }
            if b == BYTE_BACKSLASH && in_string {
                escaped = true;
                continue;
            }
            if b == BYTE_QUOTE {
                in_string = !in_string;
                continue;
            }
            if !in_string {
                let token = match b {
                    BYTE_COLON => Some(TOKEN_COLON),
                    BYTE_COMMA => Some(TOKEN_COMMA),
                    BYTE_LBRACE => Some(TOKEN_LBRACE),
                    BYTE_RBRACE => Some(TOKEN_RBRACE),
                    BYTE_LBRACKET => Some(TOKEN_LBRACKET),
                    BYTE_RBRACKET => Some(TOKEN_RBRACKET),
                    _ => None,
                };
                if let Some(t) = token {
                    tape[tape_idx] = make_tape_entry(t, offset);
                    tape_idx += 1;
                }
            }
        }

        tape_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_json_structural_tokens() {
        let json = br#"{"symbol":"BTC:USD","price":65000.5,"active":true}"#;
        let mut tape = [0u32; 64];
        let count = StructuralScanner::scan(json, &mut tape);

        // Expected tokens outside strings:
        // offset 0: '{'
        // offset 9: ':'
        // (Notice: ':' inside "BTC:USD" at offset 14 is ignored because it is inside quotes)
        // offset 19: ','
        // offset 27: ':'
        // offset 35: ','
        // offset 44: ':'
        // offset 49: '}'
        assert_eq!(count, 7);

        let (t0, off0) = decode_tape_entry(tape[0]);
        assert_eq!(t0, TOKEN_LBRACE);
        assert_eq!(off0, 0);

        let (t1, off1) = decode_tape_entry(tape[1]);
        assert_eq!(t1, TOKEN_COLON);
        assert_eq!(off1, 9);

        let (t2, off2) = decode_tape_entry(tape[2]);
        assert_eq!(t2, TOKEN_COMMA);
        assert_eq!(off2, 19);

        let (t_last, off_last) = decode_tape_entry(tape[count - 1]);
        assert_eq!(t_last, TOKEN_RBRACE);
        assert_eq!(off_last, 49);
    }
}
