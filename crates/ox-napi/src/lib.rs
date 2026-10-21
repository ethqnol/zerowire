#![deny(clippy::all)]

use napi_derive::napi;

#[napi]
pub fn napi_version() -> String {
    ox_compiler::version().to_string()
}
