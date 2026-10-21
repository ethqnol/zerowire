//! JavaScript and WebAssembly code generators for zero-copy binary validation,
//! SWAR key matching, and flyweight proxy views.

pub mod codec;
pub mod flyweight;
pub mod serializer;
pub mod swar;
pub mod validator;

pub use codec::*;
pub use flyweight::*;
pub use serializer::*;
pub use swar::*;
pub use validator::*;
