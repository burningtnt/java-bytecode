//! `Record` attribute (JVMS §4.7.30).

use super::{Attribute, parse_attributes, write_attributes};
use crate::constant_pool::{ConstantPool, ConstantRef};
use crate::error::Result;
use crate::io::{Reader, Writer};

/// A record component.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordComponent<'a> {
    /// Index of the `Utf8` component name.
    pub name_index: ConstantRef<'a>,
    /// Index of the `Utf8` component descriptor.
    pub descriptor_index: ConstantRef<'a>,
    /// Component attributes.
    pub attributes: Vec<Attribute<'a>>,
}

/// `Record` attribute (JVMS §4.7.30).
#[derive(Debug, Clone, PartialEq)]
pub struct RecordAttribute<'a> {
    /// The record components.
    pub components: Vec<RecordComponent<'a>>,
}

impl<'a> RecordAttribute<'a> {
    /// Parses the attribute body, resolving component attributes against `pool`.
    pub fn parse(reader: &mut Reader<'a>, pool: &ConstantPool<'a>) -> Result<Self> {
        let count = reader.read_u2()?;
        let mut components = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let name_index = ConstantRef::new(reader.read_u2()?);
            let descriptor_index = ConstantRef::new(reader.read_u2()?);
            let attribute_count = reader.read_u2()?;
            let attributes = parse_attributes(reader, pool, attribute_count)?;
            components.push(RecordComponent {
                name_index,
                descriptor_index,
                attributes,
            });
        }
        Ok(Self { components })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2_len(self.components.len())?;
        for component in &self.components {
            writer.write_cp_u2(component.name_index.get());
            writer.write_cp_u2(component.descriptor_index.get());
            write_attributes(writer, &component.attributes)?;
        }
        Ok(())
    }
}
