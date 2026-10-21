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

pub mod scanner;

pub use scanner::*;

/// Exported C-ABI for JavaScript WASM instantiation.
///
/// Scans `buf_ptr[0..buf_len]` and writes structural tokens to `tape_ptr[0..max_tape]`.
/// Returns the number of tape tokens written.
///
/// # Safety
///
/// Caller must ensure that:
/// - `buf_ptr` points to a valid readable byte buffer of length `buf_len`.
/// - `tape_ptr` points to a valid writable aligned `u32` buffer of length `max_tape`.
#[no_mangle]
pub unsafe extern "C" fn scan_json_structural(
    buf_ptr: *const u8,
    buf_len: usize,
    tape_ptr: *mut u32,
    max_tape: usize,
) -> usize {
    if buf_ptr.is_null() || tape_ptr.is_null() || buf_len == 0 || max_tape == 0 {
        return 0;
    }
    // SAFETY: buf_ptr and tape_ptr are verified non-null, with valid lengths passed from caller.
    let buf = core::slice::from_raw_parts(buf_ptr, buf_len);
    let tape = core::slice::from_raw_parts_mut(tape_ptr, max_tape);
    StructuralScanner::scan(buf, tape)
}

#[no_mangle]
pub extern "C" fn simd_version() -> u32 {
    1
}
