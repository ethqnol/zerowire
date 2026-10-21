//! Straight-line branchless bitwise binary buffer validator generator.
//!
//! Emits monomorphic, V8 TurboFan-optimized JavaScript functions that perform
//! a single entry bounds check, speculatively load fixed struct headers,
//! and accumulate validation errors into a single 32-bit bitmask with zero branching.

use crate::ir::{PrimitiveType, StructIR, TypeIR};

/// Emits the complete straight-line JavaScript binary validator function for a StructIR.
///
/// ### Codegen Example Output:
/// ```javascript
/// export function validateTradeExecutionBinary(view, baseOffset = 0) {
///   const byteLen = view.byteLength;
///   if (baseOffset + 32 > byteLen) return 0;
///   const timestamp = view.getFloat64(baseOffset + 0, true);
///   const tradeId = view.getUint32(baseOffset + 8, true);
///   const symbol_off = view.getUint32(baseOffset + 12, true);
///   const symbol_len = view.getUint32(baseOffset + 16, true);
///   const fills_off = view.getUint32(baseOffset + 20, true);
///   const fills_cnt = view.getUint32(baseOffset + 24, true);
///   const side = view.getUint8(baseOffset + 28);
///   const isMaker = view.getUint8(baseOffset + 29);
///   const errMask =
///     (side >= 2 ? 1 : 0) |
///     (isMaker & ~1) |
///     (timestamp !== timestamp ? 1 : 0) |
///     ((symbol_off + symbol_len > byteLen || symbol_off + symbol_len < symbol_off) ? 1 : 0) |
///     ((fills_cnt > 536870911 || fills_off + (fills_cnt << 3) > byteLen || fills_off + (fills_cnt << 3) < fills_off) ? 1 : 0);
///   return errMask === 0 ? 1 : 0;
/// }
/// ```
pub fn emit_struct_validator(struct_ir: &StructIR) -> String {
    let fn_name = format!("validate{}Binary", struct_ir.name);
    let mut loads = Vec::new();
    let mut err_checks = Vec::new();

    for field in struct_ir.fields.values() {
        let name = &field.name;
        let off = field.offset;

        match &field.ty {
            TypeIR::Primitive(p) => {
                let endian_arg = if p.requires_endian_arg() {
                    ", true"
                } else {
                    ""
                };
                loads.push(format!(
                    "  const {} = view.{}(baseOffset + {}{});",
                    name,
                    p.dataview_getter(),
                    off,
                    endian_arg
                ));

                match p {
                    PrimitiveType::Bool => {
                        err_checks.push(format!("({} & ~1)", name));
                    }
                    PrimitiveType::F32 | PrimitiveType::F64 => {
                        // NaN check
                        err_checks.push(format!("({} !== {} ? 1 : 0)", name, name));
                    }
                    _ => {}
                }
            }
            TypeIR::Enum(e) => {
                loads.push(format!(
                    "  const {} = view.getUint8(baseOffset + {});",
                    name, off
                ));
                let variant_count = e.variants.len() as u32;
                if variant_count.is_power_of_two() && variant_count <= 256 {
                    let mask = !(variant_count - 1) & 0xFF;
                    err_checks.push(format!("({} & 0x{:02x})", name, mask));
                } else {
                    err_checks.push(format!("({} >= {} ? 1 : 0)", name, variant_count));
                }
            }
            TypeIR::String => {
                let off_var = format!("{}_off", name);
                let len_var = format!("{}_len", name);
                loads.push(format!(
                    "  const {} = view.getUint32(baseOffset + {}, true);",
                    off_var, off
                ));
                loads.push(format!(
                    "  const {} = view.getUint32(baseOffset + {}, true);",
                    len_var,
                    off + 4
                ));
                // Overflow-safe bounds check: off + len must not exceed byteLen and must not overflow u32
                err_checks.push(format!(
                    "(({} + {} > byteLen || {} + {} < {}) ? 1 : 0)",
                    off_var, len_var, off_var, len_var, off_var
                ));
            }
            TypeIR::Array(inner) => {
                let off_var = format!("{}_off", name);
                let cnt_var = format!("{}_cnt", name);
                let elem_size = inner.fixed_header_size();
                loads.push(format!(
                    "  const {} = view.getUint32(baseOffset + {}, true);",
                    off_var, off
                ));
                loads.push(format!(
                    "  const {} = view.getUint32(baseOffset + {}, true);",
                    cnt_var,
                    off + 4
                ));

                let max_count = u32::MAX.checked_div(elem_size).unwrap_or(u32::MAX);
                let size_calc = match elem_size {
                    1 => cnt_var.clone(),
                    2 => format!("({} << 1)", cnt_var),
                    4 => format!("({} << 2)", cnt_var),
                    8 => format!("({} << 3)", cnt_var),
                    other => format!("Math.imul({}, {})", cnt_var, other),
                };

                err_checks.push(format!(
                    "(({} > {} || {} + {} > byteLen || {} + {} < {}) ? 1 : 0)",
                    cnt_var, max_count, off_var, size_calc, off_var, size_calc, off_var
                ));
            }
            TypeIR::Struct(sub) => {
                let sub_fn = format!("validate{}Binary", sub.name);
                err_checks.push(format!(
                    "({}(view, baseOffset + {}) === 0 ? 1 : 0)",
                    sub_fn, off
                ));
            }
            TypeIR::DiscriminatedUnion(u) => {
                let disc_sub_fn = format!("validate{}Binary", u.name);
                err_checks.push(format!(
                    "({}(view, baseOffset + {}) === 0 ? 1 : 0)",
                    disc_sub_fn, off
                ));
            }
        }
    }

    let err_mask_expr = if err_checks.is_empty() {
        "0".to_string()
    } else {
        err_checks.join(" |\n    ")
    };

    format!(
        r#"export function {fn_name}(view, baseOffset = 0) {{
  const byteLen = view.byteLength;
  if (baseOffset + {header_size} > byteLen) return 0;
{loads}
  const errMask =
    {err_mask_expr};
  return errMask === 0 ? 1 : 0;
}}
"#,
        fn_name = fn_name,
        header_size = struct_ir.fixed_size,
        loads = loads.join("\n"),
        err_mask_expr = err_mask_expr
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{EnumIR, FieldIR};
    use indexmap::IndexMap;

    #[test]
    fn test_emit_trade_execution_validator() {
        let mut fields = IndexMap::new();
        fields.insert(
            "timestamp".to_string(),
            FieldIR {
                name: "timestamp".to_string(),
                ty: TypeIR::Primitive(PrimitiveType::F64),
                optional: false,
                offset: 0,
                size: 8,
                alignment: 8,
            },
        );
        fields.insert(
            "tradeId".to_string(),
            FieldIR {
                name: "tradeId".to_string(),
                ty: TypeIR::Primitive(PrimitiveType::U32),
                optional: false,
                offset: 8,
                size: 4,
                alignment: 4,
            },
        );
        fields.insert(
            "symbol".to_string(),
            FieldIR {
                name: "symbol".to_string(),
                ty: TypeIR::String,
                optional: false,
                offset: 12,
                size: 8,
                alignment: 4,
            },
        );

        let mut side_variants = IndexMap::new();
        side_variants.insert("BUY".to_string(), 0);
        side_variants.insert("SELL".to_string(), 1);
        fields.insert(
            "side".to_string(),
            FieldIR {
                name: "side".to_string(),
                ty: TypeIR::Enum(EnumIR {
                    name: Some("Side".to_string()),
                    variants: side_variants,
                }),
                optional: false,
                offset: 20,
                size: 1,
                alignment: 1,
            },
        );

        let struct_ir = StructIR {
            name: "TradeExecution".to_string(),
            fields,
            fixed_size: 24,
            alignment: 8,
            padding: Vec::new(),
        };

        let js = emit_struct_validator(&struct_ir);
        println!("{}", js);

        assert!(js.contains("export function validateTradeExecutionBinary(view, baseOffset = 0)"));
        assert!(js.contains("if (baseOffset + 24 > byteLen) return 0;"));
        assert!(js.contains("view.getFloat64(baseOffset + 0, true)"));
        assert!(js.contains("view.getUint32(baseOffset + 8, true)"));
        assert!(js.contains("errMask === 0 ? 1 : 0"));
    }
}
