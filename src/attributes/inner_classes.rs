//! `InnerClasses` attribute (JVMS §4.7.6).

use crate::access_flags::InnerClassAccessFlags;
use crate::constant_pool::ConstantRef;
use crate::error::Result;
use crate::io::{Reader, Writer};

/// An `InnerClasses` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InnerClass<'a> {
    /// Index of the inner class.
    pub inner_class_info_index: ConstantRef<'a>,
    /// Index of the enclosing class, or 0.
    pub outer_class_info_index: ConstantRef<'a>,
    /// Index of the inner class's simple name, or 0 for anonymous.
    pub inner_name_index: ConstantRef<'a>,
    /// Inner class access flags.
    pub inner_class_access_flags: InnerClassAccessFlags,
}

/// `InnerClasses` attribute (JVMS §4.7.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InnerClassesAttribute<'a> {
    /// The class entries.
    pub classes: Vec<InnerClass<'a>>,
}

impl<'a> InnerClassesAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let count = reader.read_u2()?;
        let mut classes = Vec::with_capacity(count as usize);
        for _ in 0..count {
            classes.push(InnerClass {
                inner_class_info_index: ConstantRef::new(reader.read_u2()?),
                outer_class_info_index: ConstantRef::new(reader.read_u2()?),
                inner_name_index: ConstantRef::new(reader.read_u2()?),
                inner_class_access_flags: InnerClassAccessFlags::from_bits_retain(
                    reader.read_u2()?,
                ),
            });
        }
        Ok(Self { classes })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2_len(self.classes.len())?;
        for entry in &self.classes {
            writer.write_cp_u2(entry.inner_class_info_index.get());
            writer.write_cp_u2(entry.outer_class_info_index.get());
            writer.write_cp_u2(entry.inner_name_index.get());
            writer.write_u2(entry.inner_class_access_flags.bits());
        }
        Ok(())
    }
}
