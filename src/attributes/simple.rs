//! Small fixed-shape attributes (JVMS §4.7.2, §4.7.5, §4.7.7, §4.7.9,
//! §4.7.10, §4.7.11, §4.7.26, §4.7.27, §4.7.28, §4.7.29, §4.7.31).

use crate::constant_pool::ConstantRef;
use crate::error::Result;
use crate::io::{Reader, Writer};
use crate::mutf8::Mutf8;

fn read_ref_list<'a>(reader: &mut Reader<'a>) -> Result<Vec<ConstantRef<'a>>> {
    let count = reader.read_u2()?;
    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(ConstantRef::new(reader.read_u2()?));
    }
    Ok(values)
}

fn write_ref_list(writer: &mut Writer, values: &[ConstantRef<'_>]) -> Result<()> {
    writer.write_u2_len(values.len())?;
    for value in values {
        writer.write_cp_u2(value.get());
    }
    Ok(())
}

macro_rules! u2_attribute {
    ($name:ident, $field:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name<'a> {
            #[doc = "Referenced constant pool index."]
            pub $field: ConstantRef<'a>,
        }

        impl<'a> $name<'a> {
            /// Parses the attribute body.
            pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
                Ok(Self {
                    $field: ConstantRef::new(reader.read_u2()?),
                })
            }

            /// Writes the attribute body.
            pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
                writer.write_cp_u2(self.$field.get());
                Ok(())
            }
        }
    };
}

u2_attribute!(
    ConstantValueAttribute,
    constantvalue_index,
    "`ConstantValue` attribute (JVMS §4.7.2)."
);
u2_attribute!(
    SignatureAttribute,
    signature_index,
    "`Signature` attribute (JVMS §4.7.9)."
);
u2_attribute!(
    SourceFileAttribute,
    sourcefile_index,
    "`SourceFile` attribute (JVMS §4.7.10)."
);
u2_attribute!(
    NestHostAttribute,
    host_class_index,
    "`NestHost` attribute (JVMS §4.7.28)."
);
u2_attribute!(
    ModuleMainClassAttribute,
    main_class_index,
    "`ModuleMainClass` attribute (JVMS §4.7.27)."
);

/// `SourceDebugExtension` attribute (JVMS §4.7.11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDebugExtensionAttribute<'a> {
    /// Raw debug extension bytes (a modified UTF-8 string).
    pub debug_extension: Mutf8<'a>,
}

impl<'a> SourceDebugExtensionAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        Ok(Self {
            debug_extension: Mutf8::new(reader.read_bytes(reader.remaining())?),
        })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_bytes(self.debug_extension.as_bytes());
        Ok(())
    }
}

/// `Exceptions` attribute (JVMS §4.7.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionsAttribute<'a> {
    /// Indices of the declared exception classes.
    pub exception_index_table: Vec<ConstantRef<'a>>,
}

impl<'a> ExceptionsAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        Ok(Self {
            exception_index_table: read_ref_list(reader)?,
        })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        write_ref_list(writer, &self.exception_index_table)
    }
}

/// `EnclosingMethod` attribute (JVMS §4.7.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnclosingMethodAttribute<'a> {
    /// Index of the enclosing class.
    pub class_index: ConstantRef<'a>,
    /// Index of the enclosing method's name-and-type, or 0.
    pub method_index: ConstantRef<'a>,
}

impl<'a> EnclosingMethodAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        Ok(Self {
            class_index: ConstantRef::new(reader.read_u2()?),
            method_index: ConstantRef::new(reader.read_u2()?),
        })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_cp_u2(self.class_index.get());
        writer.write_cp_u2(self.method_index.get());
        Ok(())
    }
}

macro_rules! class_list_attribute {
    ($name:ident, $field:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name<'a> {
            /// Referenced class indices.
            pub $field: Vec<ConstantRef<'a>>,
        }

        impl<'a> $name<'a> {
            /// Parses the attribute body.
            pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
                Ok(Self {
                    $field: read_ref_list(reader)?,
                })
            }

            /// Writes the attribute body.
            pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
                write_ref_list(writer, &self.$field)
            }
        }
    };
}

class_list_attribute!(
    NestMembersAttribute,
    classes,
    "`NestMembers` attribute (JVMS §4.7.29)."
);
class_list_attribute!(
    PermittedSubclassesAttribute,
    classes,
    "`PermittedSubclasses` attribute (JVMS §4.7.31)."
);
class_list_attribute!(
    ModulePackagesAttribute,
    package_index,
    "`ModulePackages` attribute (JVMS §4.7.26)."
);
