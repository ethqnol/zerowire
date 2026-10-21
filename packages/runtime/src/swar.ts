//! Runtime SWAR utility constants and ASCII byte scanning helpers.

// 0x80808080 checks bit 7 (ASCII high bit) of all 4 bytes in a 32-bit word
export const SWAR_ASCII_MASK_U32 = 0x80808080;

// 0x8080808080808080n checks bit 7 of all 8 bytes in a 64-bit word
export const SWAR_ASCII_MASK_U64 = 0x8080808080808080n;

/**
 * Checks whether an 8-byte slice in a DataView contains only ASCII characters.
 */
export function isAsciiWord64(view: DataView, offset: number): boolean {
  return (view.getBigUint64(offset, true) & SWAR_ASCII_MASK_U64) === 0n;
}

/**
 * Checks whether a 4-byte slice in a DataView contains only ASCII characters.
 */
export function isAsciiWord32(view: DataView, offset: number): boolean {
  return (view.getUint32(offset, true) & SWAR_ASCII_MASK_U32) === 0;
}
