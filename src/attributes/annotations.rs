//! Annotation building blocks (JVMS §4.7.16, §4.7.16.1).
//!
//! Annotation structures reference the constant pool exclusively through
//! [`ConstantRef`] indices.

use crate::constant_pool::ConstantRef;
use crate::error::{Error, Result};
use crate::io::{Reader, Writer};

/// One `element_value_pair`.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementValuePair<'a> {
    /// Index of the `Utf8` element name.
    pub element_name_index: ConstantRef<'a>,
    /// The element value.
    pub value: ElementValue<'a>,
}

/// A parsed `annotation` structure (JVMS §4.7.16).
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation<'a> {
    /// Index of the `Utf8` annotation type descriptor.
    pub type_index: ConstantRef<'a>,
    /// The element-value pairs.
    pub element_value_pairs: Vec<ElementValuePair<'a>>,
}

impl<'a> Annotation<'a> {
    /// Parses one annotation.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
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
            type_index,
            element_value_pairs,
        })
    }

    /// Writes this annotation.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_cp_u2(self.type_index.get());
        writer.write_u2_len(self.element_value_pairs.len())?;
        for pair in &self.element_value_pairs {
            writer.write_cp_u2(pair.element_name_index.get());
            pair.value.write_to(writer)?;
        }
        Ok(())
    }
}

/// An `element_value` union (JVMS §4.7.16.1).
#[derive(Debug, Clone, PartialEq)]
pub enum ElementValue<'a> {
    /// A primitive or `String` constant; `tag` is one of
    /// `B C D F I J S Z s`.
    Const {
        /// The tag byte.
        tag: u8,
        /// Index of the constant.
        const_value_index: ConstantRef<'a>,
    },
    /// An enum constant (`tag = e`).
    Enum {
        /// Index of the `Utf8` enum type descriptor.
        type_name_index: ConstantRef<'a>,
        /// Index of the `Utf8` constant name.
        const_name_index: ConstantRef<'a>,
    },
    /// A class literal (`tag = c`).
    Class {
        /// Index of the `Utf8` class descriptor.
        class_info_index: ConstantRef<'a>,
    },
    /// A nested annotation (`tag = @`).
    Annotation(Box<Annotation<'a>>),
    /// An array (`tag = [`).
    Array(Vec<ElementValue<'a>>),
}

impl<'a> ElementValue<'a> {
    /// Parses one element value.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let tag_offset = reader.position();
        let tag = reader.read_u1()?;
        let value = match tag {
            b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b's' => Self::Const {
                tag,
                const_value_index: ConstantRef::new(reader.read_u2()?),
            },
            b'e' => Self::Enum {
                type_name_index: ConstantRef::new(reader.read_u2()?),
                const_name_index: ConstantRef::new(reader.read_u2()?),
            },
            b'c' => Self::Class {
                class_info_index: ConstantRef::new(reader.read_u2()?),
            },
            b'@' => Self::Annotation(Box::new(Annotation::parse(reader)?)),
            b'[' => {
                let count = reader.read_u2()?;
                let mut values = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    values.push(Self::parse(reader)?);
                }
                Self::Array(values)
            }
            _ => {
                return Err(Error::InvalidElementValueTag {
                    tag,
                    offset: tag_offset,
                });
            }
        };
        Ok(value)
    }

    /// Writes this element value.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        match self {
            Self::Const {
                tag,
                const_value_index,
            } => {
                writer.write_u1(*tag);
                writer.write_cp_u2(const_value_index.get());
            }
            Self::Enum {
                type_name_index,
                const_name_index,
            } => {
                writer.write_u1(b'e');
                writer.write_cp_u2(type_name_index.get());
                writer.write_cp_u2(const_name_index.get());
            }
            Self::Class { class_info_index } => {
                writer.write_u1(b'c');
                writer.write_cp_u2(class_info_index.get());
            }
            Self::Annotation(annotation) => {
                writer.write_u1(b'@');
                annotation.write_to(writer)?;
            }
            Self::Array(values) => {
                writer.write_u1(b'[');
                writer.write_u2_len(values.len())?;
                for value in values {
                    value.write_to(writer)?;
                }
            }
        }
        Ok(())
    }
}
