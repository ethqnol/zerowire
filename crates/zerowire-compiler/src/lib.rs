//! `zerowire-compiler`: Ahead-of-Time TypeScript type-to-wire compiler and zero-copy binary codec.
//!
//! Powered by `oxc_parser`, `oxc_resolver`, and `oxc_span`.

pub mod codegen;
pub mod error;
pub mod ir;
pub mod layout;
pub mod mphf;
pub mod parser;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
