//! TypeScript AST type extractor using `oxc_parser` and `oxc_allocator`.
//!
//! Traverses TypeScript ASTs to extract interfaces, type aliases, branded types,
//! string literal enums, and discriminated unions, lowering them into canonical `TypeIR`.

use crate::error::CompileError;
use crate::ir::{DiscriminatedUnionIR, EnumIR, PrimitiveType, TypeIR};
use crate::layout::packer::pack_struct;
use crate::mphf::generator::synthesize_mphf;
use indexmap::IndexMap;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::{SourceType, Span};

/// Context holding parsed declarations and extracted type representations.
#[derive(Debug, Default)]
pub struct AstTypeContext {
    /// Raw interface field definitions: name -> [(field_name, raw_type, optional)]
    pub raw_interfaces: IndexMap<String, Vec<(String, RawFieldType, bool)>>,
    /// Extracted type aliases
    pub raw_aliases: IndexMap<String, RawType>,
    /// Fully resolved TypeIR cache
    pub resolved_types: IndexMap<String, TypeIR>,
    /// Target type requested via compile macro (e.g. `compileWireCodec<T>()`)
    pub macro_target_type: Option<String>,
}

/// Unresolved raw field type prior to recursive type resolution.
#[derive(Debug, Clone)]
pub enum RawFieldType {
    Primitive(PrimitiveType),
    Named(String),
    String,
    Array(Box<RawFieldType>),
    StringLiteralEnum(Vec<String>),
    InlineObject(Vec<(String, RawFieldType, bool)>),
    Union(Vec<RawFieldType>),
}

/// Unresolved raw type alias.
#[derive(Debug, Clone)]
pub enum RawType {
    Primitive(PrimitiveType),
    Named(String),
    String,
    Array(Box<RawFieldType>),
    StringLiteralEnum(Vec<String>),
    InlineObject(Vec<(String, RawFieldType, bool)>),
    Union(Vec<RawFieldType>),
    Branded { base: PrimitiveType, brand: String },
}

impl AstTypeContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse TypeScript source code and populate the type context.
    pub fn parse_source(&mut self, source: &str) -> Result<(), CompileError> {
        let allocator = Allocator::default();
        let source_type = SourceType::ts();
        let parser_ret = Parser::new(&allocator, source, source_type).parse();

        if !parser_ret.diagnostics.is_empty() {
            let first_err = &parser_ret.diagnostics[0];
            return Err(CompileError::SyntaxError {
                message: first_err.to_string(),
                span: Span::default(),
            });
        }

        // Walk top-level statements
        for stmt in &parser_ret.program.body {
            self.visit_statement(stmt)?;
        }

        Ok(())
    }

    fn visit_statement<'a>(&mut self, stmt: &Statement<'a>) -> Result<(), CompileError> {
        match stmt {
            Statement::TSInterfaceDeclaration(itf) => {
                self.extract_interface(itf)?;
            }
            Statement::TSTypeAliasDeclaration(alias) => {
                self.extract_type_alias(alias)?;
            }
            Statement::ExportDeclaration(export_decl) => {
                match &export_decl.declaration {
                    Declaration::TSInterfaceDeclaration(itf) => {
                        self.extract_interface(itf)?;
                    }
                    Declaration::TSTypeAliasDeclaration(alias) => {
                        self.extract_type_alias(alias)?;
                    }
                    Declaration::VariableDeclaration(var_decl) => {
                        self.extract_variable_declaration(var_decl)?;
                    }
                    _ => {}
                }
            }
            Statement::VariableDeclaration(var_decl) => {
                self.extract_variable_declaration(var_decl)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn extract_variable_declaration<'a>(
        &mut self,
        var_decl: &VariableDeclaration<'a>,
    ) -> Result<(), CompileError> {
        for declarator in &var_decl.declarations {
            if let Some(init) = &declarator.init {
                if let Expression::CallExpression(call) = init {
                    if let Expression::Identifier(ident) = &call.callee {
                        if ident.name == "compileWireCodec"
                            || ident.name == "compileCodec"
                            || ident.name == "createFlyweight"
                        {
                            if let Some(type_args) = &call.type_arguments {
                                if let Some(first_param) = type_args.params.first() {
                                    if let TSType::TSTypeReference(type_ref) = first_param {
                                        if let TSTypeName::IdentifierReference(type_ident) =
                                            &type_ref.type_name
                                        {
                                            self.macro_target_type =
                                                Some(type_ident.name.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn extract_interface<'a>(&mut self, itf: &TSInterfaceDeclaration<'a>) -> Result<(), CompileError> {
        let name = itf.id.name.to_string();
        let mut fields = Vec::new();

        for member in &itf.body.body {
            if let TSSignature::TSPropertySignature(prop) = member {
                let field_name = match &prop.key {
                    PropertyKey::StaticIdentifier(ident) => ident.name.to_string(),
                    PropertyKey::StringLiteral(lit) => lit.value.as_str().unwrap_or("").to_string(),
                    _ => continue,
                };

                let raw_type = if let Some(type_annot) = &prop.type_annotation {
                    self.extract_ts_type(&type_annot.type_annotation)?
                } else {
                    RawFieldType::Primitive(PrimitiveType::F64)
                };

                fields.push((field_name, raw_type, prop.optional));
            }
        }

        self.raw_interfaces.insert(name, fields);
        Ok(())
    }

    fn extract_type_alias<'a>(&mut self, alias: &TSTypeAliasDeclaration<'a>) -> Result<(), CompileError> {
        let name = alias.id.name.to_string();
        let raw = self.extract_type_alias_body(&alias.type_annotation)?;
        self.raw_aliases.insert(name, raw);
        Ok(())
    }

    fn extract_type_alias_body<'a>(&self, ts_type: &TSType<'a>) -> Result<RawType, CompileError> {
        // Check for branded type pattern: `number & { __brand: "u32" }` or `number & Brand<"u32">`
        if let TSType::TSIntersectionType(intersection) = ts_type {
            if let Some(branded) = self.try_extract_brand(intersection)? {
                return Ok(branded);
            }
        }

        match ts_type {
            TSType::TSUnionType(union) => {
                // Check if all variants are string literals
                let mut string_literals = Vec::new();
                let mut all_literals = true;
                for member in &union.types {
                    if let TSType::TSLiteralType(lit) = member {
                        if let TSLiteral::StringLiteral(s) = &lit.literal {
                            string_literals.push(s.value.as_str().unwrap_or("").to_string());
                        } else {
                            all_literals = false;
                            break;
                        }
                    } else {
                        all_literals = false;
                        break;
                    }
                }

                if all_literals && !string_literals.is_empty() {
                    return Ok(RawType::StringLiteralEnum(string_literals));
                }

                // General union of types (e.g. tagged union)
                let mut variants = Vec::new();
                for member in &union.types {
                    let raw_field = self.extract_ts_type(member)?;
                    variants.push(raw_field);
                }
                Ok(RawType::Union(variants))
            }
            _ => {
                let raw_field = self.extract_ts_type(ts_type)?;
                Ok(match raw_field {
                    RawFieldType::Primitive(p) => RawType::Primitive(p),
                    RawFieldType::Named(n) => RawType::Named(n),
                    RawFieldType::String => RawType::String,
                    RawFieldType::Array(inner) => RawType::Array(inner),
                    RawFieldType::StringLiteralEnum(e) => RawType::StringLiteralEnum(e),
                    RawFieldType::InlineObject(o) => RawType::InlineObject(o),
                    RawFieldType::Union(u) => RawType::Union(u),
                })
            }
        }
    }

    fn try_extract_brand<'a>(
        &self,
        intersection: &TSIntersectionType<'a>,
    ) -> Result<Option<RawType>, CompileError> {
        let mut base_primitive = None;
        let mut brand_tag = None;

        for member in &intersection.types {
            match member {
                TSType::TSNumberKeyword(_) => {
                    base_primitive = Some(PrimitiveType::F64);
                }
                TSType::TSBooleanKeyword(_) => {
                    base_primitive = Some(PrimitiveType::Bool);
                }
                TSType::TSTypeLiteral(lit) => {
                    for prop in &lit.members {
                        if let TSSignature::TSPropertySignature(p) = prop {
                            let key_opt = match &p.key {
                                PropertyKey::StaticIdentifier(i) => Some(i.name.as_str()),
                                PropertyKey::StringLiteral(s) => s.value.as_str(),
                                _ => None,
                            };
                            if let Some(key) = key_opt {
                                if key == "__brand" || key == "_brand" {
                                    if let Some(annot) = &p.type_annotation {
                                        if let TSType::TSLiteralType(lit) = &annot.type_annotation {
                                            if let TSLiteral::StringLiteral(s) = &lit.literal {
                                                brand_tag = Some(s.value.as_str().unwrap_or("").to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                TSType::TSTypeReference(type_ref) => {
                    if let TSTypeName::IdentifierReference(ident) = &type_ref.type_name {
                        if ident.name == "Brand" {
                            if let Some(args) = &type_ref.type_arguments {
                                if let Some(first) = args.params.first() {
                                    if let TSType::TSLiteralType(lit) = first {
                                        if let TSLiteral::StringLiteral(s) = &lit.literal {
                                            brand_tag = Some(s.value.as_str().unwrap_or("").to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if let (Some(base), Some(brand)) = (base_primitive, brand_tag) {
            // Narrow physical primitive based on brand name
            let narrowed = match brand.as_str() {
                "u8" => PrimitiveType::U8,
                "u16" => PrimitiveType::U16,
                "u32" => PrimitiveType::U32,
                "u64" => PrimitiveType::U64,
                "i8" => PrimitiveType::I8,
                "i16" => PrimitiveType::I16,
                "i32" => PrimitiveType::I32,
                "i64" => PrimitiveType::I64,
                "f32" => PrimitiveType::F32,
                "f64" => PrimitiveType::F64,
                _ => base,
            };
            return Ok(Some(RawType::Branded {
                base: narrowed,
                brand,
            }));
        }

        Ok(None)
    }

    fn extract_ts_type<'a>(&self, ts_type: &TSType<'a>) -> Result<RawFieldType, CompileError> {
        match ts_type {
            TSType::TSNumberKeyword(_) => Ok(RawFieldType::Primitive(PrimitiveType::F64)),
            TSType::TSBooleanKeyword(_) => Ok(RawFieldType::Primitive(PrimitiveType::Bool)),
            TSType::TSStringKeyword(_) => Ok(RawFieldType::String),
            TSType::TSBigIntKeyword(_) => Ok(RawFieldType::Primitive(PrimitiveType::U64)),
            TSType::TSTypeReference(type_ref) => {
                if let TSTypeName::IdentifierReference(ident) = &type_ref.type_name {
                    let name = ident.name.as_str();
                    match name {
                        // Standard physical primitive nicknames
                        "u8" => Ok(RawFieldType::Primitive(PrimitiveType::U8)),
                        "u16" => Ok(RawFieldType::Primitive(PrimitiveType::U16)),
                        "u32" => Ok(RawFieldType::Primitive(PrimitiveType::U32)),
                        "u64" => Ok(RawFieldType::Primitive(PrimitiveType::U64)),
                        "i8" => Ok(RawFieldType::Primitive(PrimitiveType::I8)),
                        "i16" => Ok(RawFieldType::Primitive(PrimitiveType::I16)),
                        "i32" => Ok(RawFieldType::Primitive(PrimitiveType::I32)),
                        "i64" => Ok(RawFieldType::Primitive(PrimitiveType::I64)),
                        "f32" => Ok(RawFieldType::Primitive(PrimitiveType::F32)),
                        "f64" => Ok(RawFieldType::Primitive(PrimitiveType::F64)),
                        "Array" => {
                            if let Some(args) = &type_ref.type_arguments {
                                if let Some(first) = args.params.first() {
                                    let inner = self.extract_ts_type(first)?;
                                    return Ok(RawFieldType::Array(Box::new(inner)));
                                }
                            }
                            Ok(RawFieldType::Array(Box::new(RawFieldType::Primitive(
                                PrimitiveType::F64,
                            ))))
                        }
                        other => Ok(RawFieldType::Named(other.to_string())),
                    }
                } else {
                    Ok(RawFieldType::Named("unknown".to_string()))
                }
            }
            TSType::TSArrayType(arr) => {
                let inner = self.extract_ts_type(&arr.element_type)?;
                Ok(RawFieldType::Array(Box::new(inner)))
            }
            TSType::TSUnionType(union) => {
                let mut string_literals = Vec::new();
                let mut all_literals = true;
                for member in &union.types {
                    if let TSType::TSLiteralType(lit) = member {
                        if let TSLiteral::StringLiteral(s) = &lit.literal {
                            string_literals.push(s.value.as_str().unwrap_or("").to_string());
                        } else {
                            all_literals = false;
                            break;
                        }
                    } else {
                        all_literals = false;
                        break;
                    }
                }

                if all_literals && !string_literals.is_empty() {
                    Ok(RawFieldType::StringLiteralEnum(string_literals))
                } else {
                    let mut variants = Vec::new();
                    for member in &union.types {
                        variants.push(self.extract_ts_type(member)?);
                    }
                    Ok(RawFieldType::Union(variants))
                }
            }
            TSType::TSTypeLiteral(lit) => {
                let mut fields = Vec::new();
                for member in &lit.members {
                    if let TSSignature::TSPropertySignature(prop) = member {
                        let field_name = match &prop.key {
                            PropertyKey::StaticIdentifier(ident) => ident.name.to_string(),
                            PropertyKey::StringLiteral(lit) => {
                                lit.value.as_str().unwrap_or("").to_string()
                            }
                            _ => continue,
                        };
                        let raw = if let Some(annot) = &prop.type_annotation {
                            self.extract_ts_type(&annot.type_annotation)?
                        } else {
                            RawFieldType::Primitive(PrimitiveType::F64)
                        };
                        fields.push((field_name, raw, prop.optional));
                    }
                }
                Ok(RawFieldType::InlineObject(fields))
            }
            _ => Ok(RawFieldType::Primitive(PrimitiveType::F64)),
        }
    }

    /// Resolves a named type into its canonical TypeIR representation.
    pub fn resolve_type(&mut self, name: &str) -> Result<TypeIR, CompileError> {
        if let Some(cached) = self.resolved_types.get(name) {
            return Ok(cached.clone());
        }

        // 1. Check if it's an interface
        if let Some(raw_fields) = self.raw_interfaces.get(name).cloned() {
            let mut resolved_fields = Vec::new();
            for (fname, rtype, optional) in raw_fields {
                let field_ir = self.resolve_raw_field(&rtype)?;
                resolved_fields.push((fname, field_ir, optional));
            }
            let packed = pack_struct(name, resolved_fields)?;
            let ir = TypeIR::Struct(packed);
            self.resolved_types.insert(name.to_string(), ir.clone());
            return Ok(ir);
        }

        // 2. Check if it's a type alias
        if let Some(raw_alias) = self.raw_aliases.get(name).cloned() {
            let ir = match raw_alias {
                RawType::Primitive(p) => TypeIR::Primitive(p),
                RawType::Branded { base, .. } => TypeIR::Primitive(base),
                RawType::String => TypeIR::String,
                RawType::Array(inner) => {
                    let inner_ir = self.resolve_raw_field(&inner)?;
                    TypeIR::Array(Box::new(inner_ir))
                }
                RawType::StringLiteralEnum(variants) => {
                    let mut variant_map = IndexMap::new();
                    for (i, v) in variants.into_iter().enumerate() {
                        variant_map.insert(v, i as u8);
                    }
                    TypeIR::Enum(EnumIR {
                        name: Some(name.to_string()),
                        variants: variant_map,
                    })
                }
                RawType::InlineObject(fields) => {
                    let mut resolved_fields = Vec::new();
                    for (fname, rtype, optional) in fields {
                        let field_ir = self.resolve_raw_field(&rtype)?;
                        resolved_fields.push((fname, field_ir, optional));
                    }
                    let packed = pack_struct(name, resolved_fields)?;
                    TypeIR::Struct(packed)
                }
                RawType::Union(variants) => self.resolve_union(name, &variants)?,
                RawType::Named(ref target) => self.resolve_type(target)?,
            };

            self.resolved_types.insert(name.to_string(), ir.clone());
            return Ok(ir);
        }

        // 3. Check primitive names directly
        match name {
            "u8" => Ok(TypeIR::Primitive(PrimitiveType::U8)),
            "u16" => Ok(TypeIR::Primitive(PrimitiveType::U16)),
            "u32" => Ok(TypeIR::Primitive(PrimitiveType::U32)),
            "u64" => Ok(TypeIR::Primitive(PrimitiveType::U64)),
            "i8" => Ok(TypeIR::Primitive(PrimitiveType::I8)),
            "i16" => Ok(TypeIR::Primitive(PrimitiveType::I16)),
            "i32" => Ok(TypeIR::Primitive(PrimitiveType::I32)),
            "i64" => Ok(TypeIR::Primitive(PrimitiveType::I64)),
            "f32" => Ok(TypeIR::Primitive(PrimitiveType::F32)),
            "f64" | "number" => Ok(TypeIR::Primitive(PrimitiveType::F64)),
            "bool" | "boolean" => Ok(TypeIR::Primitive(PrimitiveType::Bool)),
            "string" => Ok(TypeIR::String),
            _ => Err(CompileError::ResolutionError {
                message: format!("Type '{}' not found in AST context", name),
                span: None,
            }),
        }
    }

    fn resolve_raw_field(&mut self, raw: &RawFieldType) -> Result<TypeIR, CompileError> {
        match raw {
            RawFieldType::Primitive(p) => Ok(TypeIR::Primitive(*p)),
            RawFieldType::Named(n) => self.resolve_type(n),
            RawFieldType::String => Ok(TypeIR::String),
            RawFieldType::Array(inner) => {
                let inner_ir = self.resolve_raw_field(inner)?;
                Ok(TypeIR::Array(Box::new(inner_ir)))
            }
            RawFieldType::StringLiteralEnum(variants) => {
                let mut map = IndexMap::new();
                for (i, v) in variants.iter().enumerate() {
                    map.insert(v.clone(), i as u8);
                }
                Ok(TypeIR::Enum(EnumIR {
                    name: None,
                    variants: map,
                }))
            }
            RawFieldType::InlineObject(fields) => {
                let mut resolved_fields = Vec::new();
                for (fname, rtype, optional) in fields {
                    let field_ir = self.resolve_raw_field(rtype)?;
                    resolved_fields.push((fname.clone(), field_ir, *optional));
                }
                let packed = pack_struct("AnonymousStruct", resolved_fields)?;
                Ok(TypeIR::Struct(packed))
            }
            RawFieldType::Union(variants) => self.resolve_union("AnonymousUnion", variants),
        }
    }

    fn resolve_union(
        &mut self,
        union_name: &str,
        variants: &[RawFieldType],
    ) -> Result<TypeIR, CompileError> {
        let mut struct_variants = IndexMap::new();
        let mut candidate_discriminator: Option<String> = None;

        for (idx, v) in variants.iter().enumerate() {
            let variant_ir = self.resolve_raw_field(v)?;
            let struct_ir = match variant_ir {
                TypeIR::Struct(s) => s,
                _ => {
                    return Err(CompileError::UnsupportedType {
                        message: "Non-object unions not supported in discriminated union lowering"
                            .to_string(),
                        span: Span::default(),
                    })
                }
            };

            if candidate_discriminator.is_none() {
                let d = if struct_ir.fields.contains_key("type") {
                    "type".to_string()
                } else if struct_ir.fields.contains_key("kind") {
                    "kind".to_string()
                } else {
                    struct_ir
                        .fields
                        .keys()
                        .next()
                        .cloned()
                        .unwrap_or_else(|| "type".to_string())
                };
                candidate_discriminator = Some(d);
            }

            let variant_tag = struct_ir.name.clone();
            let tag_key = if variant_tag.is_empty() || variant_tag == "AnonymousStruct" {
                format!("Variant_{}", idx)
            } else {
                variant_tag
            };

            struct_variants.insert(tag_key, struct_ir);
        }

        let disc_field = candidate_discriminator.unwrap_or_else(|| "type".to_string());
        let variant_keys: Vec<&str> = struct_variants.keys().map(|s| s.as_str()).collect();
        let mphf = synthesize_mphf(&variant_keys)?;

        Ok(TypeIR::DiscriminatedUnion(DiscriminatedUnionIR {
            name: union_name.to_string(),
            discriminator_field: disc_field,
            variants: struct_variants,
            mphf_multiplier: mphf.multiplier,
            mphf_capacity: mphf.capacity,
            mphf_shift: mphf.shift,
        }))
    }
}

/// Helper function to parse TypeScript code and extract a specific named type.
pub fn extract_type_from_source(source: &str, type_name: &str) -> Result<TypeIR, CompileError> {
    let mut ctx = AstTypeContext::new();
    ctx.parse_source(source)?;
    ctx.resolve_type(type_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_trade_execution_interface() {
        let ts_code = r#"
            type u32 = number & { __brand: "u32" };

            export interface TradeExecution {
                timestamp: number;
                tradeId: u32;
                side: "BUY" | "SELL";
                isMaker: boolean;
                symbol: string;
                fills: number[];
            }

            export const TradeCodec = compileWireCodec<TradeExecution>();
        "#;

        let mut ctx = AstTypeContext::new();
        ctx.parse_source(ts_code).expect("Parsing should succeed");

        assert_eq!(ctx.macro_target_type, Some("TradeExecution".to_string()));

        let target_type = ctx
            .resolve_type("TradeExecution")
            .expect("Resolving TradeExecution should succeed");

        if let TypeIR::Struct(s) = target_type {
            assert_eq!(s.name, "TradeExecution");
            assert_eq!(s.fixed_size, 32);
            assert_eq!(s.alignment, 8);
            assert_eq!(s.fields["timestamp"].offset, 0);
            assert_eq!(s.fields["tradeId"].offset, 8);
            assert_eq!(s.fields["symbol"].offset, 12);
            assert_eq!(s.fields["fills"].offset, 20);
            assert_eq!(s.fields["side"].offset, 28);
            assert_eq!(s.fields["isMaker"].offset, 29);
        } else {
            panic!("Expected StructIR");
        }
    }
}
