//! Canonical type intermediate representation (`TypeIR`).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Physical numeric or boolean primitive width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrimitiveType {
    /// 8-bit unsigned integer (1 byte, alignment 1)
    U8,
    /// 16-bit unsigned integer (2 bytes, alignment 2)
    U16,
    /// 32-bit unsigned integer (4 bytes, alignment 4)
    U32,
    /// 64-bit unsigned integer (8 bytes, alignment 8)
    U64,
    /// 8-bit signed integer (1 byte, alignment 1)
    I8,
    /// 16-bit signed integer (2 bytes, alignment 2)
    I16,
    /// 32-bit signed integer (4 bytes, alignment 4)
    I32,
    /// 64-bit signed integer (8 bytes, alignment 8)
    I64,
    /// 32-bit IEEE-754 float (4 bytes, alignment 4)
    F32,
    /// 64-bit IEEE-754 float (8 bytes, alignment 8)
    F64,
    /// 8-bit boolean (0 = false, 1 = true) (1 byte, alignment 1)
    Bool,
}

impl PrimitiveType {
    /// Returns the byte size of this primitive.
    #[inline]
    pub const fn byte_size(self) -> u32 {
        match self {
            Self::U8 | Self::I8 | Self::Bool => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::U64 | Self::I64 | Self::F64 => 8,
        }
    }

    /// Returns the alignment requirement in bytes.
    #[inline]
    pub const fn alignment(self) -> u32 {
        self.byte_size()
    }

    /// Returns the JavaScript DataView getter name (e.g. `getUint32`).
    #[inline]
    pub const fn dataview_getter(self) -> &'static str {
        match self {
            Self::U8 => "getUint8",
            Self::U16 => "getUint16",
            Self::U32 => "getUint32",
            Self::U64 => "getBigUint64",
            Self::I8 => "getInt8",
            Self::I16 => "getInt16",
            Self::I32 => "getInt32",
            Self::I64 => "getBigInt64",
            Self::F32 => "getFloat32",
            Self::F64 => "getFloat64",
            Self::Bool => "getUint8",
        }
    }

    /// Returns the JavaScript DataView setter name (e.g. `setUint32`).
    #[inline]
    pub const fn dataview_setter(self) -> &'static str {
        match self {
            Self::U8 => "setUint8",
            Self::U16 => "setUint16",
            Self::U32 => "setUint32",
            Self::U64 => "setBigUint64",
            Self::I8 => "setInt8",
            Self::I16 => "setInt16",
            Self::I32 => "setInt32",
            Self::I64 => "setBigInt64",
            Self::F32 => "setFloat32",
            Self::F64 => "setFloat64",
            Self::Bool => "setUint8",
        }
    }

    /// Returns whether this primitive requires little-endian flag in DataView calls.
    #[inline]
    pub const fn requires_endian_arg(self) -> bool {
        self.byte_size() > 1
    }
}

/// Canonical Intermediate Representation of a TypeScript Type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeIR {
    /// Primitive numeric or boolean type
    Primitive(PrimitiveType),

    /// Dynamic UTF-8 string, stored as (u32 offset, u32 byte_length) in fixed header
    String,

    /// Dynamic homogenous array, stored as (u32 offset, u32 element_count) in fixed header
    Array(Box<TypeIR>),

    /// Fixed-size struct / interface
    Struct(StructIR),

    /// String literal enum (e.g. `"BUY" | "SELL" | "HOLD"`) represented as u8 index
    Enum(EnumIR),

    /// Tagged / Discriminated union of structs dispatched via MPHF
    DiscriminatedUnion(DiscriminatedUnionIR),
}

impl TypeIR {
    /// Returns the alignment requirement in bytes.
    pub fn alignment(&self) -> u32 {
        match self {
            Self::Primitive(p) => p.alignment(),
            // Dynamic slice descriptor in fixed header consists of two u32s (offset, len) -> align 4
            Self::String | Self::Array(_) => 4,
            Self::Struct(s) => s.alignment,
            Self::Enum(_) => 1,
            Self::DiscriminatedUnion(u) => u.alignment(),
        }
    }

    /// Returns the fixed header size in bytes.
    /// Dynamic types have fixed descriptor size = 8 bytes (`u32 offset`, `u32 length`).
    pub fn fixed_header_size(&self) -> u32 {
        match self {
            Self::Primitive(p) => p.byte_size(),
            Self::String | Self::Array(_) => 8,
            Self::Struct(s) => s.fixed_size,
            Self::Enum(_) => 1,
            Self::DiscriminatedUnion(u) => u.fixed_size(),
        }
    }

    /// Returns whether this type is dynamic or contains dynamic tail elements.
    pub fn is_dynamic(&self) -> bool {
        match self {
            Self::Primitive(_) | Self::Enum(_) => false,
            Self::String | Self::Array(_) => true,
            Self::Struct(s) => s.fields.values().any(|f| f.ty.is_dynamic()),
            Self::DiscriminatedUnion(u) => u.variants.values().any(|v| v.is_dynamic()),
        }
    }
}

/// Represents a single field within a packed struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldIR {
    /// Property name in TypeScript
    pub name: String,
    /// Property type
    pub ty: TypeIR,
    /// Whether the property is optional (e.g. `foo?: string`)
    pub optional: bool,
    /// Byte offset within the fixed struct header (assigned by packing algorithm)
    pub offset: u32,
    /// Size in bytes occupied in the fixed struct header
    pub size: u32,
    /// Alignment requirement in bytes
    pub alignment: u32,
}

/// Represents a packed struct / interface layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructIR {
    /// Type / interface name
    pub name: String,
    /// Struct fields ordered by packing algorithm (descending alignment)
    pub fields: IndexMap<String, FieldIR>,
    /// Total fixed header size in bytes (including tail padding to struct alignment)
    pub fixed_size: u32,
    /// Struct alignment in bytes (maximum alignment across all fields)
    pub alignment: u32,
    /// Explicit padding segments inserted for alignment
    pub padding: Vec<PaddingSegment>,
}

impl StructIR {
    /// Returns whether this struct contains any dynamic fields.
    pub fn is_dynamic(&self) -> bool {
        self.fields.values().any(|f| f.ty.is_dynamic())
    }
}

/// Explicit padding inserted between struct fields or at the end of the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaddingSegment {
    /// Start byte offset
    pub offset: u32,
    /// Number of zeroed padding bytes
    pub length: u32,
}

/// Represents a string literal enum (e.g., `"BUY" | "SELL" | "HOLD"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumIR {
    /// Optional enum or type name
    pub name: Option<String>,
    /// Map from string literal to assigned u8 ordinal index (0..N-1)
    pub variants: IndexMap<String, u8>,
}

/// Represents a tagged / discriminated union.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscriminatedUnionIR {
    /// Union type name
    pub name: String,
    /// Field name used as discriminator (e.g. `"type"` or `"kind"`)
    pub discriminator_field: String,
    /// Variants keyed by discriminator string value
    pub variants: IndexMap<String, StructIR>,
    /// Perfect hash multiplier discovered at compile time
    pub mphf_multiplier: u32,
    /// Table capacity $M$ (smallest power of 2 >= variant count)
    pub mphf_capacity: u32,
    /// Right shift $R = 32 - \log_2(M)$
    pub mphf_shift: u32,
}

impl DiscriminatedUnionIR {
    /// Maximum alignment among all variants.
    pub fn alignment(&self) -> u32 {
        self.variants.values().map(|v| v.alignment).max().unwrap_or(8)
    }

    /// Maximum fixed header size among all variants.
    pub fn fixed_size(&self) -> u32 {
        self.variants.values().map(|v| v.fixed_size).max().unwrap_or(0)
    }

    /// Whether any variant has dynamic elements.
    pub fn is_dynamic(&self) -> bool {
        self.variants.values().any(|v| v.is_dynamic())
    }
}
