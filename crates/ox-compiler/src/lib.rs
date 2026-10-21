//! `ox-compiler`: Ahead-of-Time TypeScript type-to-wire compiler and zero-copy binary codec.
//!
//! Powered by `oxc_parser`, `oxc_resolver`, and `oxc_span`.

pub mod error;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
