//! `MethodParameters` attribute (JVMS §4.7.24).

use crate::access_flags::MethodParameterAccessFlags;
use crate::constant_pool::ConstantRef;
use crate::error::Result;
use crate::io::{Reader, Writer};

/// A `MethodParameters` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodParameter<'a> {
    /// Index of the `Utf8` parameter name, or 0.
    pub name_index: ConstantRef<'a>,
    /// Parameter access flags.
    pub access_flags: MethodParameterAccessFlags,
}

/// `MethodParameters` attribute (JVMS §4.7.24).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodParametersAttribute<'a> {
    /// The parameters.
    pub parameters: Vec<MethodParameter<'a>>,
}

impl<'a> MethodParametersAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let count = reader.read_u1()?;
        let mut parameters = Vec::with_capacity(count as usize);
        for _ in 0..count {
            parameters.push(MethodParameter {
                name_index: ConstantRef::new(reader.read_u2()?),
                access_flags: MethodParameterAccessFlags::from_bits_retain(reader.read_u2()?),
            });
        }
        Ok(Self { parameters })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        let count = u8::try_from(self.parameters.len()).map_err(|_| {
            crate::error::Error::ValueTooLarge {
                value: self.parameters.len() as u64,
                target: "u8 (parameters_count)",
            }
        })?;
        writer.write_u1(count);
        for parameter in &self.parameters {
            writer.write_cp_u2(parameter.name_index.get());
            writer.write_u2(parameter.access_flags.bits());
        }
        Ok(())
    }
}
