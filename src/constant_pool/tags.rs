//! Constant pool tag metadata.

/// A constant pool tag value (JVMS §4.4, Table 4.4-A).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstantTag {
    /// tag 1 — `CONSTANT_Utf8`.
    Utf8,
    /// tag 3 — `CONSTANT_Integer`.
    Integer,
    /// tag 4 — `CONSTANT_Float`.
    Float,
    /// tag 5 — `CONSTANT_Long`.
    Long,
    /// tag 6 — `CONSTANT_Double`.
    Double,
    /// tag 7 — `CONSTANT_Class`.
    Class,
    /// tag 8 — `CONSTANT_String`.
    String,
    /// tag 9 — `CONSTANT_Fieldref`.
    Fieldref,
    /// tag 10 — `CONSTANT_Methodref`.
    Methodref,
    /// tag 11 — `CONSTANT_InterfaceMethodref`.
    InterfaceMethodref,
    /// tag 12 — `CONSTANT_NameAndType`.
    NameAndType,
    /// tag 15 — `CONSTANT_MethodHandle`.
    MethodHandle,
    /// tag 16 — `CONSTANT_MethodType`.
    MethodType,
    /// tag 17 — `CONSTANT_Dynamic`.
    Dynamic,
    /// tag 18 — `CONSTANT_InvokeDynamic`.
    InvokeDynamic,
    /// tag 19 — `CONSTANT_Module`.
    Module,
    /// tag 20 — `CONSTANT_Package`.
    Package,
}

impl ConstantTag {
    /// Returns the tag for a wire byte, or `None` if it is not a known tag.
    #[must_use]
    pub const fn from_u8(tag: u8) -> Option<Self> {
        Some(match tag {
            1 => Self::Utf8,
            3 => Self::Integer,
            4 => Self::Float,
            5 => Self::Long,
            6 => Self::Double,
            7 => Self::Class,
            8 => Self::String,
            9 => Self::Fieldref,
            10 => Self::Methodref,
            11 => Self::InterfaceMethodref,
            12 => Self::NameAndType,
            15 => Self::MethodHandle,
            16 => Self::MethodType,
            17 => Self::Dynamic,
            18 => Self::InvokeDynamic,
            19 => Self::Module,
            20 => Self::Package,
            _ => return None,
        })
    }

    /// Returns the wire tag byte.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Utf8 => 1,
            Self::Integer => 3,
            Self::Float => 4,
            Self::Long => 5,
            Self::Double => 6,
            Self::Class => 7,
            Self::String => 8,
            Self::Fieldref => 9,
            Self::Methodref => 10,
            Self::InterfaceMethodref => 11,
            Self::NameAndType => 12,
            Self::MethodHandle => 15,
            Self::MethodType => 16,
            Self::Dynamic => 17,
            Self::InvokeDynamic => 18,
            Self::Module => 19,
            Self::Package => 20,
        }
    }

    /// The first class-file major version in which this tag is valid
    /// (Table 4.4-B). Provided for diagnostics; this crate does not enforce it.
    #[must_use]
    pub const fn first_major_version(self) -> u16 {
        match self {
            Self::MethodHandle | Self::MethodType | Self::InvokeDynamic => 51,
            Self::Dynamic => 55,
            Self::Module | Self::Package => 53,
            _ => 45,
        }
    }
}
