//! Intermediate Representation (`TypeIR`) for Ahead-of-Time wire and binary compilation.
//!
//! Provides a canonical algebraic representation of TypeScript types,
//! normalized primitive widths (u8..u64, i8..i64, f32, f64, bool),
//! packed struct layouts, and tagged union metadata.

pub mod types;

pub use types::*;
