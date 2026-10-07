//! Debug attributes (JVMS §4.7.12, §4.7.13, §4.7.14).

use crate::constant_pool::ConstantRef;
use crate::error::Result;
use crate::io::{Reader, Writer};

/// A `LineNumberTable` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineNumber {
    /// Bytecode offset.
    pub start_pc: u16,
    /// Source line number.
    pub line_number: u16,
}

/// `LineNumberTable` attribute (JVMS §4.7.12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineNumberTableAttribute {
    /// The table entries.
    pub line_number_table: Vec<LineNumber>,
}

impl LineNumberTableAttribute {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'_>) -> Result<Self> {
        let count = reader.read_u2()?;
        let mut line_number_table = Vec::with_capacity(count as usize);
        for _ in 0..count {
            line_number_table.push(LineNumber {
                start_pc: reader.read_u2()?,
                line_number: reader.read_u2()?,
            });
        }
        Ok(Self { line_number_table })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2_len(self.line_number_table.len())?;
        for entry in &self.line_number_table {
            writer.write_u2(entry.start_pc);
            writer.write_u2(entry.line_number);
        }
        Ok(())
    }
}

/// A `LocalVariableTable` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalVariable<'a> {
    /// Start of the variable's scope, as a bytecode offset.
    pub start_pc: u16,
    /// Length of the variable's scope.
    pub length: u16,
    /// Index of the `Utf8` variable name.
    pub name_index: ConstantRef<'a>,
    /// Index of the `Utf8` field descriptor.
    pub descriptor_index: ConstantRef<'a>,
    /// Local variable slot index.
    pub index: u16,
}

/// `LocalVariableTable` attribute (JVMS §4.7.13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVariableTableAttribute<'a> {
    /// The table entries.
    pub local_variable_table: Vec<LocalVariable<'a>>,
}

impl<'a> LocalVariableTableAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let count = reader.read_u2()?;
        let mut local_variable_table = Vec::with_capacity(count as usize);
        for _ in 0..count {
            local_variable_table.push(LocalVariable {
                start_pc: reader.read_u2()?,
                length: reader.read_u2()?,
                name_index: ConstantRef::new(reader.read_u2()?),
                descriptor_index: ConstantRef::new(reader.read_u2()?),
                index: reader.read_u2()?,
            });
        }
        Ok(Self {
            local_variable_table,
        })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2_len(self.local_variable_table.len())?;
        for entry in &self.local_variable_table {
            writer.write_u2(entry.start_pc);
            writer.write_u2(entry.length);
            writer.write_cp_u2(entry.name_index.get());
            writer.write_cp_u2(entry.descriptor_index.get());
            writer.write_u2(entry.index);
        }
        Ok(())
    }
}

/// A `LocalVariableTypeTable` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalVariableType<'a> {
    /// Start of the variable's scope, as a bytecode offset.
    pub start_pc: u16,
    /// Length of the variable's scope.
    pub length: u16,
    /// Index of the `Utf8` variable name.
    pub name_index: ConstantRef<'a>,
    /// Index of the `Utf8` field signature.
    pub signature_index: ConstantRef<'a>,
    /// Local variable slot index.
    pub index: u16,
}

/// `LocalVariableTypeTable` attribute (JVMS §4.7.14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVariableTypeTableAttribute<'a> {
    /// The table entries.
    pub local_variable_type_table: Vec<LocalVariableType<'a>>,
}

impl<'a> LocalVariableTypeTableAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let count = reader.read_u2()?;
        let mut local_variable_type_table = Vec::with_capacity(count as usize);
        for _ in 0..count {
            local_variable_type_table.push(LocalVariableType {
                start_pc: reader.read_u2()?,
                length: reader.read_u2()?,
                name_index: ConstantRef::new(reader.read_u2()?),
                signature_index: ConstantRef::new(reader.read_u2()?),
                index: reader.read_u2()?,
            });
        }
        Ok(Self {
            local_variable_type_table,
        })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2_len(self.local_variable_type_table.len())?;
        for entry in &self.local_variable_type_table {
            writer.write_u2(entry.start_pc);
            writer.write_u2(entry.length);
            writer.write_cp_u2(entry.name_index.get());
            writer.write_cp_u2(entry.signature_index.get());
            writer.write_u2(entry.index);
        }
        Ok(())
    }
}
