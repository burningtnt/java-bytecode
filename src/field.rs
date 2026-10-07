//! Fields (JVMS §4.5).

use crate::access_flags::FieldAccessFlags;
use crate::attributes::{Attribute, parse_attributes, write_attributes};
use crate::constant_pool::{ConstantPool, ConstantRef};
use crate::error::Result;
use crate::io::{Reader, Writer};

/// A `field_info` structure.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInfo<'a> {
    /// Field access and property flags.
    pub access_flags: FieldAccessFlags,
    /// Index of the `Utf8` field name.
    pub name_index: ConstantRef<'a>,
    /// Index of the `Utf8` field descriptor.
    pub descriptor_index: ConstantRef<'a>,
    /// Field attributes.
    pub attributes: Vec<Attribute<'a>>,
}

impl<'a> FieldInfo<'a> {
    /// Parses one field.
    pub fn parse(reader: &mut Reader<'a>, pool: &ConstantPool<'a>) -> Result<Self> {
        let access_flags = FieldAccessFlags::from_bits_retain(reader.read_u2()?);
        let name_index = ConstantRef::new(reader.read_u2()?);
        let descriptor_index = ConstantRef::new(reader.read_u2()?);
        let attribute_count = reader.read_u2()?;
        let attributes = parse_attributes(reader, pool, attribute_count)?;
        Ok(Self {
            access_flags,
            name_index,
            descriptor_index,
            attributes,
        })
    }

    /// Serializes this field.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2(self.access_flags.bits());
        writer.write_cp_u2(self.name_index.get());
        writer.write_cp_u2(self.descriptor_index.get());
        write_attributes(writer, &self.attributes)
    }

    /// Resolves this field's name against `pool`.
    pub fn name(&self, pool: &ConstantPool<'a>) -> Result<String> {
        pool.utf8_str(self.name_index)
    }

    /// Resolves this field's descriptor against `pool`.
    pub fn descriptor(&self, pool: &ConstantPool<'a>) -> Result<String> {
        pool.utf8_str(self.descriptor_index)
    }
}
