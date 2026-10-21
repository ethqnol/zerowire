//! Zero-copy Flyweight Class generator backed by DataView.
//!
//! Emits Flyweight classes with lazy field accessors:
//! - Primitives read directly via monomorphic DataView getters.
//! - Strings use inline ASCII loop (String.fromCharCode) for < 48 bytes to bypass
//!   V8 TextDecoder C++ binding overhead, falling back to singleton TextDecoder
//!   when >= 48 bytes or multi-byte UTF-8 detected via SWAR.
//! - Typed arrays returned directly over existing ArrayBuffer slice without heap allocations.

use crate::ir::{PrimitiveType, StructIR, TypeIR};

/// Emits the Flyweight proxy class for a StructIR.
pub fn emit_flyweight_class(struct_ir: &StructIR) -> String {
    let class_name = format!("{}View", struct_ir.name);
    let mut getters = Vec::new();

    for field in struct_ir.fields.values() {
        let name = &field.name;
        let off = field.offset;

        match &field.ty {
            TypeIR::Primitive(p) => match p {
                PrimitiveType::Bool => {
                    getters.push(format!(
                        r#"  get {name}() {{
    return this._view.getUint8(this._offset + {off}) !== 0;
  }}"#
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
                    getters.push(format!(
                        r#"  get {name}() {{
    return this._view.{getter}(this._offset + {off}, true);
  }}"#,
                        getter = p.dataview_getter()
                    ));
                }
                PrimitiveType::U8 | PrimitiveType::I8 => {
                    getters.push(format!(
                        r#"  get {name}() {{
    return this._view.{getter}(this._offset + {off});
  }}"#,
                        getter = p.dataview_getter()
                    ));
                }
            },
            TypeIR::Enum(_) => {
                let table_name = format!("{}_{}_ENUM_TABLE", struct_ir.name, name);
                getters.push(format!(
                    r#"  get {name}() {{
    const raw = this._view.getUint8(this._offset + {off});
    return {table_name}[raw] ?? "";
  }}"#
                ));
            }
            TypeIR::String => {
                getters.push(format!(
                    r#"  get {name}() {{
    const off = this._view.getUint32(this._offset + {off}, true);
    const len = this._view.getUint32(this._offset + {off_len}, true);
    if (len === 0) return "";
    const start = this._offset + off;
    if (len < 48) {{
      let isAscii = true;
      let i = 0;
      while (i + 4 <= len) {{
        // SWAR high-bit check: 0x80808080 checks bit 7 of 4 ASCII bytes
        if ((this._view.getUint32(start + i, true) & 0x80808080) !== 0) {{
          isAscii = false;
          break;
        }}
        i += 4;
      }}
      if (isAscii) {{
        while (i < len) {{
          if ((this._u8[start + i] & 0x80) !== 0) {{
            isAscii = false;
            break;
          }}
          i++;
        }}
      }}
      if (isAscii) {{
        let s = "";
        for (let j = 0; j < len; j++) {{
          s += String.fromCharCode(this._u8[start + j]);
        }}
        return s;
      }}
    }}
    return _sharedTextDecoder.decode(new Uint8Array(this._view.buffer, this._view.byteOffset + start, len));
  }}"#,
                    off_len = off + 4
                ));
            }
            TypeIR::Array(inner) => {
                match inner.as_ref() {
                    TypeIR::Primitive(PrimitiveType::F64) => {
                        getters.push(format!(
                            r#"  get {name}() {{
    const off = this._view.getUint32(this._offset + {off}, true);
    const count = this._view.getUint32(this._offset + {off_cnt}, true);
    return new Float64Array(this._view.buffer, this._view.byteOffset + this._offset + off, count);
  }}"#,
                            off_cnt = off + 4
                        ));
                    }
                    TypeIR::Primitive(PrimitiveType::U32) => {
                        getters.push(format!(
                            r#"  get {name}() {{
    const off = this._view.getUint32(this._offset + {off}, true);
    const count = this._view.getUint32(this._offset + {off_cnt}, true);
    return new Uint32Array(this._view.buffer, this._view.byteOffset + this._offset + off, count);
  }}"#,
                            off_cnt = off + 4
                        ));
                    }
                    TypeIR::Primitive(PrimitiveType::U8) => {
                        getters.push(format!(
                            r#"  get {name}() {{
    const off = this._view.getUint32(this._offset + {off}, true);
    const count = this._view.getUint32(this._offset + {off_cnt}, true);
    return new Uint8Array(this._view.buffer, this._view.byteOffset + this._offset + off, count);
  }}"#,
                            off_cnt = off + 4
                        ));
                    }
                    _ => {
                        // General array descriptor
                        getters.push(format!(
                            r#"  get {name}() {{
    const off = this._view.getUint32(this._offset + {off}, true);
    const count = this._view.getUint32(this._offset + {off_cnt}, true);
    return {{ offset: this._offset + off, count }};
  }}"#,
                            off_cnt = off + 4
                        ));
                    }
                }
            }
            TypeIR::Struct(sub) => {
                let sub_view = format!("{}View", sub.name);
                getters.push(format!(
                    r#"  get {name}() {{
    return new {sub_view}(this._view, this._offset + {off});
  }}"#
                ));
            }
            TypeIR::DiscriminatedUnion(u) => {
                let union_view = format!("{}View", u.name);
                getters.push(format!(
                    r#"  get {name}() {{
    return new {union_view}(this._view, this._offset + {off});
  }}"#
                ));
            }
        }
    }

    // Generate enum table declarations
    let mut enum_tables = Vec::new();
    for field in struct_ir.fields.values() {
        if let TypeIR::Enum(e) = &field.ty {
            let table_name = format!("{}_{}_ENUM_TABLE", struct_ir.name, field.name);
            let entries: Vec<String> = e.variants.keys().map(|k| format!("\"{}\"", k)).collect();
            enum_tables.push(format!("const {} = [{}];", table_name, entries.join(", ")));
        }
    }

    let enum_decl = if enum_tables.is_empty() {
        "".to_string()
    } else {
        format!("{}\n\n", enum_tables.join("\n"))
    };

    format!(
        r#"{enum_decl}export class {class_name} {{
  constructor(view, offset = 0) {{
    this._view = view;
    this._u8 = new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
    this._offset = offset;
  }}

{getters}

  toBinaryBuffer() {{
    return new Uint8Array(this._view.buffer, this._view.byteOffset + this._offset);
  }}
}}
"#,
        enum_decl = enum_decl,
        class_name = class_name,
        getters = getters.join("\n\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{EnumIR, FieldIR};
    use indexmap::IndexMap;

    #[test]
    fn test_emit_flyweight_class() {
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
        exchange_variants.insert("CME".to_string(), 2);
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

        let js = emit_flyweight_class(&struct_ir);
        println!("{}", js);

        assert!(js.contains("export class MarketTickView"));
        assert!(js.contains("get price()"));
        assert!(js.contains("get symbol()"));
        assert!(js.contains("get exchange()"));
        assert!(js.contains("String.fromCharCode"));
        assert!(js.contains("0x80808080"));
    }
}
