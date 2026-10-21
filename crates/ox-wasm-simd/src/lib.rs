//! WebAssembly v128 SIMD structural JSON scanner and tape generator.
//!
//! Provides zero-allocation structural scanning over raw byte buffers using
//! WebAssembly `v128` SIMD intrinsics (`i8x16`).

#![cfg_attr(all(target_arch = "wasm32", not(test)), no_std)]

#[cfg(all(target_arch = "wasm32", not(test)))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

/// Structural character bitmask indices or token indicators
pub const TOKEN_QUOTE: u32 = 0x01;
pub const TOKEN_COLON: u32 = 0x02;
pub const TOKEN_COMMA: u32 = 0x03;
pub const TOKEN_LBRACE: u32 = 0x04;
pub const TOKEN_RBRACE: u32 = 0x05;
pub const TOKEN_LBRACKET: u32 = 0x06;
pub const TOKEN_RBRACKET: u32 = 0x07;

#[no_mangle]
pub extern "C" fn simd_version() -> u32 {
    1
}
