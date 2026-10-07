//! Methods (JVMS §4.6).

use crate::access_flags::MethodAccessFlags;
use crate::attributes::{Attribute, parse_attributes, write_attributes};
use crate::constant_pool::{ConstantPool, ConstantRef};
use crate::error::Result;
use crate::io::{Reader, Writer};

/// A `method_info` structure.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodInfo<'a> {
    /// Method access and property flags.
    pub access_flags: MethodAccessFlags,
    /// Index of the `Utf8` method name.
    pub name_index: ConstantRef<'a>,
    /// Index of the `Utf8` method descriptor.
    pub descriptor_index: ConstantRef<'a>,
    /// Method attributes.
    pub attributes: Vec<Attribute<'a>>,
}

impl<'a> MethodInfo<'a> {
    /// Parses one method.
    pub fn parse(reader: &mut Reader<'a>, pool: &ConstantPool<'a>) -> Result<Self> {
        let access_flags = MethodAccessFlags::from_bits_retain(reader.read_u2()?);
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

    /// Serializes this method.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2(self.access_flags.bits());
        writer.write_cp_u2(self.name_index.get());
        writer.write_cp_u2(self.descriptor_index.get());
        write_attributes(writer, &self.attributes)
    }

    /// Resolves this method's name against `pool`.
    pub fn name(&self, pool: &ConstantPool<'a>) -> Result<String> {
        pool.utf8_str(self.name_index)
    }

    /// Resolves this method's descriptor against `pool`.
    pub fn descriptor(&self, pool: &ConstantPool<'a>) -> Result<String> {
        pool.utf8_str(self.descriptor_index)
    }

    /// Returns the first `Code` attribute of this method, if any.
    #[must_use]
    pub fn code(&self) -> Option<&crate::attributes::CodeAttribute<'a>> {
        self.attributes
            .iter()
            .find_map(|attribute| match &attribute.body {
                crate::attributes::AttributeBody::Code(code) => Some(code),
                _ => None,
            })
    }

    /// Returns the first `Code` attribute of this method, if any.
    ///
    /// Use [`CodeAttribute::instructions_mut`](crate::CodeAttribute::instructions_mut)
    /// to edit the bytecode; see the [`bytecode`](crate::bytecode) module for
    /// the instrumentation workflow.
    #[must_use]
    pub fn code_mut(&mut self) -> Option<&mut crate::attributes::CodeAttribute<'a>> {
        self.attributes
            .iter_mut()
            .find_map(|attribute| match &mut attribute.body {
                crate::attributes::AttributeBody::Code(code) => Some(code),
                _ => None,
            })
    }
}
