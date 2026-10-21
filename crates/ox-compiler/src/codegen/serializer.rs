//! Direct binary serializer generator.
//!
//! Encodes standard JavaScript objects into the packed binary struct format
//! in a single straight-line pass without JSON serialization or intermediary allocations.

use crate::ir::{PrimitiveType, StructIR, TypeIR};

/// Emits the binary encoder function for a StructIR.
pub fn emit_struct_serializer(struct_ir: &StructIR) -> String {
    let fn_name = format!("encode{}Binary", struct_ir.name);
    let mut fixed_writes = Vec::new();
    let mut dynamic_calcs = Vec::new();
    let mut dynamic_writes = Vec::new();

    for field in struct_ir.fields.values() {
        let name = &field.name;
        let off = field.offset;

        match &field.ty {
            TypeIR::Primitive(p) => match p {
                PrimitiveType::Bool => {
                    fixed_writes.push(format!(
                        "  view.setUint8(baseOffset + {}, obj.{} ? 1 : 0);",
                        off, name
                    ));
                }
                PrimitiveType::F32
                | PrimitiveType::F64
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64 => {
                    fixed_writes.push(format!(
                        "  view.{}(baseOffset + {}, obj.{}, true);",
                        p.dataview_setter(),
                        off,
                        name
                    ));
                }
                PrimitiveType::U8 | PrimitiveType::I8 => {
                    fixed_writes.push(format!(
                        "  view.{}(baseOffset + {}, obj.{});",
                        p.dataview_setter(),
                        off,
                        name
                    ));
                }
            },
            TypeIR::Enum(_) => {
                let map_name = format!("{}_{}_ENUM_MAP", struct_ir.name, name);
                fixed_writes.push(format!(
                    "  view.setUint8(baseOffset + {}, {}[obj.{}] ?? 0);",
                    off, map_name, name
                ));
            }
            TypeIR::String => {
                let bytes_var = format!("{}_bytes", name);
                dynamic_calcs.push(format!(
                    "  const {} = typeof obj.{} === 'string' ? _sharedTextEncoder.encode(obj.{}) : new Uint8Array(0);",
                    bytes_var, name, name
                ));
                dynamic_writes.push(format!(
                    r#"  view.setUint32(baseOffset + {off}, tailOffset, true);
  view.setUint32(baseOffset + {off_len}, {bytes}.byteLength, true);
  u8.set({bytes}, baseOffset + tailOffset);
  tailOffset += {bytes}.byteLength;"#,
                    off = off,
                    off_len = off + 4,
                    bytes = bytes_var
                ));
            }
            TypeIR::Array(inner) => {
                let elem_size = inner.fixed_header_size();
                let arr_var = format!("{}_arr", name);
                dynamic_calcs.push(format!(
                    "  const {} = Array.isArray(obj.{}) ? obj.{} : [];",
                    arr_var, name, name
                ));
                match inner.as_ref() {
                    TypeIR::Primitive(PrimitiveType::F64) => {
                        dynamic_writes.push(format!(
                            r#"  view.setUint32(baseOffset + {off}, tailOffset, true);
  view.setUint32(baseOffset + {off_cnt}, {arr}.length, true);
  for (let i = 0; i < {arr}.length; i++) {{
    view.setFloat64(baseOffset + tailOffset + (i << 3), {arr}[i], true);
  }}
  tailOffset += ({arr}.length << 3);"#,
                            off = off,
                            off_cnt = off + 4,
                            arr = arr_var
                        ));
                    }
                    TypeIR::Primitive(PrimitiveType::U32) => {
                        dynamic_writes.push(format!(
                            r#"  view.setUint32(baseOffset + {off}, tailOffset, true);
  view.setUint32(baseOffset + {off_cnt}, {arr}.length, true);
  for (let i = 0; i < {arr}.length; i++) {{
    view.setUint32(baseOffset + tailOffset + (i << 2), {arr}[i], true);
  }}
  tailOffset += ({arr}.length << 2);"#,
                            off = off,
                            off_cnt = off + 4,
                            arr = arr_var
                        ));
                    }
                    _ => {
                        dynamic_writes.push(format!(
                            r#"  view.setUint32(baseOffset + {off}, tailOffset, true);
  view.setUint32(baseOffset + {off_cnt}, {arr}.length, true);
  tailOffset += Math.imul({arr}.length, {elem_size});"#,
                            off = off,
                            off_cnt = off + 4,
                            arr = arr_var,
                            elem_size = elem_size
                        ));
                    }
                }
            }
            TypeIR::Struct(sub) => {
                let sub_fn = format!("encode{}Binary", sub.name);
                fixed_writes.push(format!(
                    "  {}(obj.{}, view, baseOffset + {});",
                    sub_fn, name, off
                ));
            }
            TypeIR::DiscriminatedUnion(_) => {}
        }
    }

    // Generate enum reverse maps
    let mut enum_maps = Vec::new();
    for field in struct_ir.fields.values() {
        if let TypeIR::Enum(e) = &field.ty {
            let map_name = format!("{}_{}_ENUM_MAP", struct_ir.name, field.name);
            let pairs: Vec<String> = e
                .variants
                .iter()
                .map(|(k, v)| format!("\"{}\": {}", k, v))
                .collect();
            enum_maps.push(format!("const {} = {{ {} }};", map_name, pairs.join(", ")));
        }
    }

    let enum_decl = if enum_maps.is_empty() {
        "".to_string()
    } else {
        format!("{}\n\n", enum_maps.join("\n"))
    };

    let dyn_calc_block = if dynamic_calcs.is_empty() {
        "".to_string()
    } else {
        format!("{}\n", dynamic_calcs.join("\n"))
    };

    let dyn_write_block = if dynamic_writes.is_empty() {
        "".to_string()
    } else {
        format!("\n{}", dynamic_writes.join("\n"))
    };

    format!(
        r#"{enum_decl}export function {fn_name}(obj, targetBuffer = null, baseOffset = 0) {{
{dyn_calc_block}  let tailOffset = {header_size};
  const u8 = targetBuffer instanceof Uint8Array ? targetBuffer : new Uint8Array(65536);
  const view = new DataView(u8.buffer, u8.byteOffset, u8.byteLength);

{fixed_writes}{dyn_write_block}

  return u8.subarray(baseOffset, baseOffset + tailOffset);
}}
"#,
        enum_decl = enum_decl,
        fn_name = fn_name,
        dyn_calc_block = dyn_calc_block,
        header_size = struct_ir.fixed_size,
        fixed_writes = fixed_writes.join("\n"),
        dyn_write_block = dyn_write_block
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{EnumIR, FieldIR};
    use indexmap::IndexMap;

    #[test]
    fn test_emit_serializer() {
        let mut fields = IndexMap::new();
        fields.insert(
            "price".to_string(),
            FieldIR {
                name: "price".to_string(),
                ty: TypeIR::Primitive(PrimitiveType::F64),
                optional: false,
                offset: 0,
                size: 8,
                alignment: 8,
            },
        );
        fields.insert(
            "symbol".to_string(),
            FieldIR {
                name: "symbol".to_string(),
                ty: TypeIR::String,
                optional: false,
                offset: 8,
                size: 8,
                alignment: 4,
            },
        );

        let mut exchange_variants = IndexMap::new();
        exchange_variants.insert("NYSE".to_string(), 0);
        exchange_variants.insert("NASDAQ".to_string(), 1);
        fields.insert(
            "exchange".to_string(),
            FieldIR {
                name: "exchange".to_string(),
                ty: TypeIR::Enum(EnumIR {
                    name: Some("Exchange".to_string()),
                    variants: exchange_variants,
                }),
                optional: false,
                offset: 16,
                size: 1,
                alignment: 1,
            },
        );

        let struct_ir = StructIR {
            name: "MarketTick".to_string(),
            fields,
            fixed_size: 24,
            alignment: 8,
            padding: Vec::new(),
        };

        let js = emit_struct_serializer(&struct_ir);
        println!("{}", js);

        assert!(js.contains("export function encodeMarketTickBinary(obj"));
        assert!(js.contains("view.setFloat64(baseOffset + 0, obj.price, true);"));
        assert!(js.contains("u8.subarray(baseOffset, baseOffset + tailOffset)"));
    }
}
