//! `BootstrapMethods` attribute (JVMS §4.7.23).

use crate::constant_pool::ConstantRef;
use crate::error::Result;
use crate::io::{Reader, Writer};

/// A `BootstrapMethods` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapMethod<'a> {
    /// Index of the bootstrap method handle.
    pub bootstrap_method_ref: ConstantRef<'a>,
    /// Indices of the static bootstrap arguments.
    pub bootstrap_arguments: Vec<ConstantRef<'a>>,
}

/// `BootstrapMethods` attribute (JVMS §4.7.23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapMethodsAttribute<'a> {
    /// The bootstrap methods.
    pub bootstrap_methods: Vec<BootstrapMethod<'a>>,
}

impl<'a> BootstrapMethodsAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let count = reader.read_u2()?;
        let mut bootstrap_methods = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let bootstrap_method_ref = ConstantRef::new(reader.read_u2()?);
            let argument_count = reader.read_u2()?;
            let mut bootstrap_arguments = Vec::with_capacity(argument_count as usize);
            for _ in 0..argument_count {
                bootstrap_arguments.push(ConstantRef::new(reader.read_u2()?));
            }
            bootstrap_methods.push(BootstrapMethod {
                bootstrap_method_ref,
                bootstrap_arguments,
            });
        }
        Ok(Self { bootstrap_methods })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2_len(self.bootstrap_methods.len())?;
        for method in &self.bootstrap_methods {
            writer.write_cp_u2(method.bootstrap_method_ref.get());
            writer.write_u2_len(method.bootstrap_arguments.len())?;
            for argument in &method.bootstrap_arguments {
                writer.write_cp_u2(argument.get());
            }
        }
        Ok(())
    }
}
