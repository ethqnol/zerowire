#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ox_compiler::codegen::codec::compile_type_to_codec;
use ox_compiler::ir::TypeIR;
use ox_compiler::layout::packer::generate_layout_diagram;
use ox_compiler::parser::extractor::AstTypeContext;

/// Output result returned to JavaScript / TypeScript bundlers from AOT compilation.
#[napi(object)]
pub struct CompileResult {
    /// Generated JavaScript codec bundle code
    pub js_code: String,
    /// Resolved target type name
    pub target_type: String,
    /// Fixed struct header size in bytes
    pub fixed_header_size: u32,
    /// Byte alignment requirement
    pub alignment: u32,
    /// ASCII memory layout diagram
    pub layout_diagram: String,
    /// JSON manifest of the lowered TypeIR
    pub json_manifest: String,
}

/// Compile a TypeScript source containing interfaces, type aliases, or macro markers
/// into a zero-allocation, branchless wire codec.
#[napi]
pub fn compile_wire_codec(
    source: String,
    target_type: Option<String>,
) -> napi::Result<CompileResult> {
    let mut ctx = AstTypeContext::new();
    ctx.parse_source(&source).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to parse TypeScript source: {}", e),
        )
    })?;

    // Determine target type name: explicit argument, macro marker, or first interface
    let resolved_target_name = target_type
        .or_else(|| ctx.macro_target_type.clone())
        .or_else(|| ctx.raw_interfaces.keys().next().cloned())
        .or_else(|| ctx.raw_aliases.keys().next().cloned())
        .ok_or_else(|| {
            Error::new(
                Status::GenericFailure,
                "No TypeScript interface, type alias, or compileWireCodec<T>() macro target found in source",
            )
        })?;

    let type_ir = ctx.resolve_type(&resolved_target_name).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to resolve type '{}': {}", resolved_target_name, e),
        )
    })?;

    let js_code = compile_type_to_codec(&type_ir).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!(
                "Failed to compile codec for '{}': {}",
                resolved_target_name, e
            ),
        )
    })?;

    let (fixed_header_size, alignment, layout_diagram) = match &type_ir {
        TypeIR::Struct(s) => (s.fixed_size, s.alignment, generate_layout_diagram(s)),
        TypeIR::DiscriminatedUnion(u) => (u.fixed_size(), u.alignment(), String::new()),
        _ => (
            type_ir.fixed_header_size(),
            type_ir.alignment(),
            String::new(),
        ),
    };

    let json_manifest = serde_json::to_string_pretty(&type_ir).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to serialize TypeIR manifest: {}", e),
        )
    })?;

    Ok(CompileResult {
        js_code,
        target_type: resolved_target_name,
        fixed_header_size,
        alignment,
        layout_diagram,
        json_manifest,
    })
}

#[napi]
pub fn napi_version() -> String {
    ox_compiler::version().to_string()
}
