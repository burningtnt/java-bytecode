//! Runtime annotation attributes (JVMS §4.7.16–§4.7.22).

use super::annotations::{Annotation, ElementValue, ElementValuePair};
use crate::constant_pool::ConstantRef;
use crate::error::{Error, Result};
use crate::io::{Reader, Writer};

/// `RuntimeVisibleAnnotations` / `RuntimeInvisibleAnnotations`
/// (JVMS §4.7.16, §4.7.17).
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationsAttribute<'a> {
    /// The annotations.
    pub annotations: Vec<Annotation<'a>>,
}

impl<'a> AnnotationsAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let count = reader.read_u2()?;
        let mut annotations = Vec::with_capacity(count as usize);
        for _ in 0..count {
            annotations.push(Annotation::parse(reader)?);
        }
        Ok(Self { annotations })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2_len(self.annotations.len())?;
        for annotation in &self.annotations {
            annotation.write_to(writer)?;
        }
        Ok(())
    }
}

/// `RuntimeVisibleParameterAnnotations` / `RuntimeInvisibleParameterAnnotations`
/// (JVMS §4.7.18, §4.7.19).
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterAnnotationsAttribute<'a> {
    /// One annotation list per parameter.
    pub parameters: Vec<Vec<Annotation<'a>>>,
}

impl<'a> ParameterAnnotationsAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let num_parameters = reader.read_u1()?;
        let mut parameters = Vec::with_capacity(num_parameters as usize);
        for _ in 0..num_parameters {
            let count = reader.read_u2()?;
            let mut annotations = Vec::with_capacity(count as usize);
            for _ in 0..count {
                annotations.push(Annotation::parse(reader)?);
            }
            parameters.push(annotations);
        }
        Ok(Self { parameters })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        let count = u8::try_from(self.parameters.len()).map_err(|_| Error::ValueTooLarge {
            value: self.parameters.len() as u64,
            target: "u8 (num_parameters)",
        })?;
        writer.write_u1(count);
        for annotations in &self.parameters {
            writer.write_u2_len(annotations.len())?;
            for annotation in annotations {
                annotation.write_to(writer)?;
            }
        }
        Ok(())
    }
}

/// `AnnotationDefault` (JVMS §4.7.22).
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationDefaultAttribute<'a> {
    /// The default value.
    pub default_value: ElementValue<'a>,
}

impl<'a> AnnotationDefaultAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        Ok(Self {
            default_value: ElementValue::parse(reader)?,
        })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        self.default_value.write_to(writer)
    }
}

/// A `type_path` structure (JVMS §4.7.20.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypePath {
    /// The path elements.
    pub path: Vec<TypePathEntry>,
}

impl TypePath {
    fn parse(reader: &mut Reader<'_>) -> Result<Self> {
        let length = reader.read_u1()?;
        let mut path = Vec::with_capacity(length as usize);
        for _ in 0..length {
            path.push(TypePathEntry {
                type_path_kind: reader.read_u1()?,
                type_argument_index: reader.read_u1()?,
            });
        }
        Ok(Self { path })
    }

    fn write_to(&self, writer: &mut Writer) -> Result<()> {
        let length = u8::try_from(self.path.len()).map_err(|_| Error::ValueTooLarge {
            value: self.path.len() as u64,
            target: "u8 (path_length)",
        })?;
        writer.write_u1(length);
        for entry in &self.path {
            writer.write_u1(entry.type_path_kind);
            writer.write_u1(entry.type_argument_index);
        }
        Ok(())
    }
}

/// A `type_path` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypePathEntry {
    /// `0` = array, `1` = nested, `2` = wildcard, `3` = type argument.
    pub type_path_kind: u8,
    /// Type-argument index (only meaningful for kind 3).
    pub type_argument_index: u8,
}

/// A `localvar_target` table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalVarTargetEntry {
    /// Start of the range, as a bytecode offset.
    pub start_pc: u16,
    /// Length of the range.
    pub length: u16,
    /// Local variable slot index.
    pub index: u16,
}

/// The `target_info` union (JVMS §4.7.20.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetInfo {
    /// `0x00`, `0x01` — `type_parameter_target`.
    TypeParameter {
        /// Index of the type parameter.
        type_parameter_index: u8,
    },
    /// `0x10` — `supertype_target`.
    Supertype {
        /// Index into the interfaces/superclass list.
        supertype_index: u16,
    },
    /// `0x11`, `0x12` — `type_parameter_bound_target`.
    TypeParameterBound {
        /// Index of the type parameter.
        type_parameter_index: u8,
        /// Index of the bound.
        bound_index: u8,
    },
    /// `0x13`, `0x14`, `0x15` — `empty_target`.
    Empty,
    /// `0x16` — `formal_parameter_target`.
    FormalParameter {
        /// Index of the formal parameter.
        formal_parameter_index: u8,
    },
    /// `0x17` — `throws_target`.
    Throws {
        /// Index into the exception table.
        throws_type_index: u16,
    },
    /// `0x40`, `0x41` — `localvar_target`.
    LocalVar {
        /// The table.
        table: Vec<LocalVarTargetEntry>,
    },
    /// `0x42` — `catch_target`.
    Catch {
        /// Index into the exception table.
        exception_table_index: u16,
    },
    /// `0x43`–`0x46` — `offset_target`.
    Offset {
        /// Bytecode offset.
        offset: u16,
    },
    /// `0x47`–`0x4B` — `type_argument_target`.
    TypeArgument {
        /// Bytecode offset.
        offset: u16,
        /// Index of the type argument.
        type_argument_index: u8,
    },
}

impl TargetInfo {
    fn parse(reader: &mut Reader<'_>, target_type: u8, offset: usize) -> Result<Self> {
        let info = match target_type {
            0x00 | 0x01 => Self::TypeParameter {
                type_parameter_index: reader.read_u1()?,
            },
            0x10 => Self::Supertype {
                supertype_index: reader.read_u2()?,
            },
            0x11 | 0x12 => Self::TypeParameterBound {
                type_parameter_index: reader.read_u1()?,
                bound_index: reader.read_u1()?,
            },
            0x13..=0x15 => Self::Empty,
            0x16 => Self::FormalParameter {
                formal_parameter_index: reader.read_u1()?,
            },
            0x17 => Self::Throws {
                throws_type_index: reader.read_u2()?,
            },
            0x40 | 0x41 => {
                let count = reader.read_u2()?;
                let mut table = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    table.push(LocalVarTargetEntry {
                        start_pc: reader.read_u2()?,
                        length: reader.read_u2()?,
                        index: reader.read_u2()?,
                    });
                }
                Self::LocalVar { table }
            }
            0x42 => Self::Catch {
                exception_table_index: reader.read_u2()?,
            },
            0x43..=0x46 => Self::Offset {
                offset: reader.read_u2()?,
            },
            0x47..=0x4B => Self::TypeArgument {
                offset: reader.read_u2()?,
                type_argument_index: reader.read_u1()?,
            },
            _ => {
                return Err(Error::InvalidTargetType {
                    target_type,
                    offset,
                });
            }
        };
        Ok(info)
    }

    fn write_to(&self, writer: &mut Writer) -> Result<()> {
        match self {
            Self::TypeParameter {
                type_parameter_index,
            } => writer.write_u1(*type_parameter_index),
            Self::Supertype { supertype_index } => writer.write_u2(*supertype_index),
            Self::TypeParameterBound {
                type_parameter_index,
                bound_index,
            } => {
                writer.write_u1(*type_parameter_index);
                writer.write_u1(*bound_index);
            }
            Self::Empty => {}
            Self::FormalParameter {
                formal_parameter_index,
            } => writer.write_u1(*formal_parameter_index),
            Self::Throws { throws_type_index } => writer.write_u2(*throws_type_index),
            Self::LocalVar { table } => {
                writer.write_u2_len(table.len())?;
                for entry in table {
                    writer.write_u2(entry.start_pc);
                    writer.write_u2(entry.length);
                    writer.write_u2(entry.index);
                }
            }
            Self::Catch {
                exception_table_index,
            } => writer.write_u2(*exception_table_index),
            Self::Offset { offset } => writer.write_u2(*offset),
            Self::TypeArgument {
                offset,
                type_argument_index,
            } => {
                writer.write_u2(*offset);
                writer.write_u1(*type_argument_index);
            }
        }
        Ok(())
    }
}

/// A `type_annotation` structure (JVMS §4.7.20).
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnnotation<'a> {
    /// The target type byte.
    pub target_type: u8,
    /// The decoded target.
    pub target_info: TargetInfo,
    /// The type path.
    pub target_path: TypePath,
    /// Index of the `Utf8` annotation type descriptor.
    pub type_index: ConstantRef<'a>,
    /// The element-value pairs.
    pub element_value_pairs: Vec<ElementValuePair<'a>>,
}

impl<'a> TypeAnnotation<'a> {
    /// Parses one type annotation.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let target_offset = reader.position();
        let target_type = reader.read_u1()?;
        let target_info = TargetInfo::parse(reader, target_type, target_offset)?;
        let target_path = TypePath::parse(reader)?;
        let type_index = ConstantRef::new(reader.read_u2()?);
        let count = reader.read_u2()?;
        let mut element_value_pairs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            element_value_pairs.push(ElementValuePair {
                element_name_index: ConstantRef::new(reader.read_u2()?),
                value: ElementValue::parse(reader)?,
            });
        }
        Ok(Self {
            target_type,
            target_info,
            target_path,
            type_index,
            element_value_pairs,
        })
    }

    /// Writes this type annotation.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u1(self.target_type);
        self.target_info.write_to(writer)?;
        self.target_path.write_to(writer)?;
        writer.write_cp_u2(self.type_index.get());
        writer.write_u2_len(self.element_value_pairs.len())?;
        for pair in &self.element_value_pairs {
            writer.write_cp_u2(pair.element_name_index.get());
            pair.value.write_to(writer)?;
        }
        Ok(())
    }
}

/// `RuntimeVisibleTypeAnnotations` / `RuntimeInvisibleTypeAnnotations`
/// (JVMS §4.7.20, §4.7.21).
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnnotationsAttribute<'a> {
    /// The type annotations.
    pub annotations: Vec<TypeAnnotation<'a>>,
}

impl<'a> TypeAnnotationsAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let count = reader.read_u2()?;
        let mut annotations = Vec::with_capacity(count as usize);
        for _ in 0..count {
            annotations.push(TypeAnnotation::parse(reader)?);
        }
        Ok(Self { annotations })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2_len(self.annotations.len())?;
        for annotation in &self.annotations {
            annotation.write_to(writer)?;
        }
        Ok(())
    }
}
