//! Automatic Struct Packing & Alignment calculation.
//!
//! Organizes struct fields by descending alignment (8B -> 4B -> 2B -> 1B)
//! to guarantee zero unaligned memory access penalties and eliminate padding holes.

use crate::error::CompileError;
use crate::ir::{FieldIR, PaddingSegment, StructIR, TypeIR};
use indexmap::IndexMap;
use std::fmt::Write;

/// Pack an unordered list of field definitions into an optimal, aligned StructIR layout.
pub fn pack_struct(
    name: &str,
    raw_fields: Vec<(String, TypeIR, bool)>,
) -> Result<StructIR, CompileError> {
    if raw_fields.is_empty() {
        return Ok(StructIR {
            name: name.to_string(),
            fields: IndexMap::new(),
            fixed_size: 0,
            alignment: 1,
            padding: Vec::new(),
        });
    }

    // Step 1: Precompute size and alignment for each field
    struct IntermediateField {
        name: String,
        ty: TypeIR,
        optional: bool,
        size: u32,
        alignment: u32,
        original_idx: usize,
    }

    let mut fields: Vec<IntermediateField> = raw_fields
        .into_iter()
        .enumerate()
        .map(|(idx, (field_name, ty, optional))| {
            let size = ty.fixed_header_size();
            let alignment = ty.alignment();
            IntermediateField {
                name: field_name,
                ty,
                optional,
                size,
                alignment,
                original_idx: idx,
            }
        })
        .collect();

    // Step 2: Sort primarily by descending alignment (8B -> 4B -> 2B -> 1B),
    // and secondarily by original declaration order for determinism and natural grouping.
    fields.sort_by(|a, b| {
        b.alignment
            .cmp(&a.alignment)
            .then_with(|| a.original_idx.cmp(&b.original_idx))
    });

    let max_alignment = fields.iter().map(|f| f.alignment).max().unwrap_or(1);
    let mut current_offset: u32 = 0;
    let mut packed_fields = IndexMap::with_capacity(fields.len());
    let mut padding_segments = Vec::new();

    // Step 3: Compute aligned offsets and detect necessary padding gaps
    for f in fields {
        let align_rem = current_offset % f.alignment;
        if align_rem != 0 {
            let pad = f.alignment - align_rem;
            padding_segments.push(PaddingSegment {
                offset: current_offset,
                length: pad,
            });
            current_offset += pad;
        }

        let field_offset = current_offset;
        current_offset += f.size;

        packed_fields.insert(
            f.name.clone(),
            FieldIR {
                name: f.name,
                ty: f.ty,
                optional: f.optional,
                offset: field_offset,
                size: f.size,
                alignment: f.alignment,
            },
        );
    }

    // Step 4: Pad tail to ensure total struct size is a multiple of maximum alignment
    let tail_rem = current_offset % max_alignment;
    if tail_rem != 0 {
        let tail_pad = max_alignment - tail_rem;
        padding_segments.push(PaddingSegment {
            offset: current_offset,
            length: tail_pad,
        });
        current_offset += tail_pad;
    }

    Ok(StructIR {
        name: name.to_string(),
        fields: packed_fields,
        fixed_size: current_offset,
        alignment: max_alignment,
        padding: padding_segments,
    })
}

/// Generates a standardized ASCII memory layout diagram for documentation and tests.
pub fn generate_layout_diagram(layout: &StructIR) -> String {
    let mut out = String::new();
    writeln!(
        &mut out,
        "Struct Layout: {} (Size: {}B, Alignment: {}B)",
        layout.name, layout.fixed_size, layout.alignment
    )
    .unwrap();
    writeln!(
        &mut out,
        "Byte Offset   Field            Type / Role                  Size (Bytes)"
    )
    .unwrap();
    writeln!(
        &mut out,
        "------------------------------------------------------------------------"
    )
    .unwrap();

    // Build timeline of events (fields and padding)
    #[derive(Debug)]
    enum Entry<'a> {
        Field(&'a FieldIR),
        Pad(u32, u32),
    }

    let mut entries = Vec::new();
    for f in layout.fields.values() {
        entries.push(Entry::Field(f));
    }
    for p in &layout.padding {
        entries.push(Entry::Pad(p.offset, p.length));
    }
    entries.sort_by_key(|e| match e {
        Entry::Field(f) => f.offset,
        Entry::Pad(off, _) => *off,
    });

    for entry in entries {
        match entry {
            Entry::Field(f) => {
                let start = f.offset;
                let end = f.offset + f.size - 1;
                let offset_str = if f.size == 1 {
                    format!("[0x{:02X}]", start)
                } else {
                    format!("[0x{:02X}..0x{:02X}]", start, end)
                };

                let type_desc = match &f.ty {
                    TypeIR::Primitive(p) => format!("{:?}", p),
                    TypeIR::String => "string (u32 offset, u32 len)".to_string(),
                    TypeIR::Array(_) => "array (u32 offset, u32 count)".to_string(),
                    TypeIR::Enum(e) => format!("enum ({} variants)", e.variants.len()),
                    TypeIR::Struct(s) => format!("struct {} ({}B)", s.name, s.fixed_size),
                    TypeIR::DiscriminatedUnion(u) => {
                        format!("union {} ({} variants)", u.name, u.variants.len())
                    }
                };

                writeln!(
                    &mut out,
                    "{:<14}{:<17}{:<29}{}B",
                    offset_str, f.name, type_desc, f.size
                )
                .unwrap();
            }
            Entry::Pad(off, len) => {
                let offset_str = if len == 1 {
                    format!("[0x{:02X}]", off)
                } else {
                    format!("[0x{:02X}..0x{:02X}]", off, off + len - 1)
                };
                writeln!(
                    &mut out,
                    "{:<14}{:<17}{:<29}{}B",
                    offset_str, "<padding>", "Zero-filled alignment pad", len
                )
                .unwrap();
            }
        }
    }

    writeln!(
        &mut out,
        "------------------------------------------------------------------------"
    )
    .unwrap();
    writeln!(
        &mut out,
        "Total Fixed Header Size (H) = {} Bytes (0x{:02X})",
        layout.fixed_size, layout.fixed_size
    )
    .unwrap();

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::PrimitiveType;

    #[test]
    fn test_trade_execution_packing() {
        // Corresponds to TradeExecution example in PLAN.md
        // timestamp: f64 (8B)
        // tradeId: u32 (4B)
        // side: "BUY" | "SELL" (u8, 1B)
        // isMaker: bool (1B)
        // symbol: string (8B dynamic descriptor)
        // fills: number[] (8B dynamic descriptor)
        let mut enum_variants = IndexMap::new();
        enum_variants.insert("BUY".to_string(), 0);
        enum_variants.insert("SELL".to_string(), 1);

        let fields = vec![
            (
                "timestamp".to_string(),
                TypeIR::Primitive(PrimitiveType::F64),
                false,
            ),
            (
                "tradeId".to_string(),
                TypeIR::Primitive(PrimitiveType::U32),
                false,
            ),
            (
                "side".to_string(),
                TypeIR::Enum(crate::ir::EnumIR {
                    name: Some("Side".to_string()),
                    variants: enum_variants,
                }),
                false,
            ),
            (
                "isMaker".to_string(),
                TypeIR::Primitive(PrimitiveType::Bool),
                false,
            ),
            ("symbol".to_string(), TypeIR::String, false),
            (
                "fills".to_string(),
                TypeIR::Array(Box::new(TypeIR::Primitive(PrimitiveType::F64))),
                false,
            ),
        ];

        let packed = pack_struct("TradeExecution", fields).expect("packing should succeed");

        // Verify sorted fields:
        // timestamp (8B, align 8) -> offset 0
        // tradeId (4B, align 4) -> offset 8
        // symbol (8B descriptor, align 4) -> offset 12
        // fills (8B descriptor, align 4) -> offset 20
        // side (1B, align 1) -> offset 28
        // isMaker (1B, align 1) -> offset 29
        // padding: 2 bytes at offset 30..32
        // Total fixed size: 32 bytes
        assert_eq!(packed.fixed_size, 32);
        assert_eq!(packed.alignment, 8);
        assert_eq!(packed.fields["timestamp"].offset, 0);
        assert_eq!(packed.fields["tradeId"].offset, 8);
        assert_eq!(packed.fields["symbol"].offset, 12);
        assert_eq!(packed.fields["fills"].offset, 20);
        assert_eq!(packed.fields["side"].offset, 28);
        assert_eq!(packed.fields["isMaker"].offset, 29);

        assert_eq!(packed.padding.len(), 1);
        assert_eq!(packed.padding[0].offset, 30);
        assert_eq!(packed.padding[0].length, 2);

        let diagram = generate_layout_diagram(&packed);
        println!("{}", diagram);
        assert!(diagram.contains("Total Fixed Header Size (H) = 32 Bytes (0x20)"));
    }
}
