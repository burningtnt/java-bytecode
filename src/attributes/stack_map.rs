//! `StackMapTable` attribute (JVMS §4.7.4).

use crate::constant_pool::ConstantRef;
use crate::error::{Error, Result};
use crate::io::{Reader, Writer};

/// A `verification_type_info` entry (JVMS §4.7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationType<'a> {
    /// `Top` (tag 0).
    Top,
    /// `Integer` (tag 1).
    Integer,
    /// `Float` (tag 2).
    Float,
    /// `Double` (tag 3).
    Double,
    /// `Long` (tag 4).
    Long,
    /// `Null` (tag 5).
    Null,
    /// `UninitializedThis` (tag 6).
    UninitializedThis,
    /// `Object` (tag 7).
    Object {
        /// Index of the class in the constant pool.
        cpool_index: ConstantRef<'a>,
    },
    /// `Uninitialized` (tag 8).
    Uninitialized {
        /// Bytecode offset of the `new` instruction.
        offset: u16,
    },
}

impl<'a> VerificationType<'a> {
    /// Parses one verification type.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let tag_offset = reader.position();
        let tag = reader.read_u1()?;
        let value = match tag {
            0 => Self::Top,
            1 => Self::Integer,
            2 => Self::Float,
            3 => Self::Double,
            4 => Self::Long,
            5 => Self::Null,
            6 => Self::UninitializedThis,
            7 => Self::Object {
                cpool_index: ConstantRef::new(reader.read_u2()?),
            },
            8 => Self::Uninitialized {
                offset: reader.read_u2()?,
            },
            _ => {
                return Err(Error::InvalidVerificationTypeTag {
                    tag,
                    offset: tag_offset,
                });
            }
        };
        Ok(value)
    }

    /// Writes this verification type.
    pub fn write_to(&self, writer: &mut Writer) {
        match self {
            Self::Top => writer.write_u1(0),
            Self::Integer => writer.write_u1(1),
            Self::Float => writer.write_u1(2),
            Self::Double => writer.write_u1(3),
            Self::Long => writer.write_u1(4),
            Self::Null => writer.write_u1(5),
            Self::UninitializedThis => writer.write_u1(6),
            Self::Object { cpool_index } => {
                writer.write_u1(7);
                writer.write_cp_u2(cpool_index.get());
            }
            Self::Uninitialized { offset } => {
                writer.write_u1(8);
                writer.write_u2(*offset);
            }
        }
    }
}

/// A `stack_map_frame` entry (JVMS §4.7.4).
///
/// Compact and extended encodings of the same frame are kept distinct so that
/// a round-trip preserves the original bytes.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum StackMapFrame<'a> {
    /// `same_frame` (frame types 0–63).
    Same {
        /// Offset delta (0–63).
        offset_delta: u16,
    },
    /// `same_locals_1_stack_item_frame` (frame types 64–127).
    SameLocals1StackItem {
        /// Offset delta (0–63).
        offset_delta: u16,
        /// The single stack item.
        stack: VerificationType<'a>,
    },
    /// `same_locals_1_stack_item_frame_extended` (frame type 247).
    SameLocals1StackItemExtended {
        /// Offset delta.
        offset_delta: u16,
        /// The single stack item.
        stack: VerificationType<'a>,
    },
    /// `chop_frame` (frame types 248–250).
    Chop {
        /// Offset delta.
        offset_delta: u16,
        /// Number of locals to remove (1–3).
        k: u8,
    },
    /// `same_frame_extended` (frame type 251).
    SameExtended {
        /// Offset delta.
        offset_delta: u16,
    },
    /// `append_frame` (frame types 252–254).
    Append {
        /// Offset delta.
        offset_delta: u16,
        /// Locals (1–3 entries).
        locals: Vec<VerificationType<'a>>,
    },
    /// `full_frame` (frame type 255).
    Full {
        /// Offset delta.
        offset_delta: u16,
        /// Locals.
        locals: Vec<VerificationType<'a>>,
        /// Stack items.
        stack: Vec<VerificationType<'a>>,
    },
}

impl<'a> StackMapFrame<'a> {
    /// Parses one frame.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let frame_type = reader.read_u1()?;
        let frame = match frame_type {
            0..=63 => Self::Same {
                offset_delta: u16::from(frame_type),
            },
            64..=127 => Self::SameLocals1StackItem {
                offset_delta: u16::from(frame_type - 64),
                stack: VerificationType::parse(reader)?,
            },
            128..=246 => {
                return Err(Error::InvalidVerificationTypeTag {
                    tag: frame_type,
                    offset: reader.position() - 1,
                });
            }
            247 => Self::SameLocals1StackItemExtended {
                offset_delta: reader.read_u2()?,
                stack: VerificationType::parse(reader)?,
            },
            248..=250 => Self::Chop {
                offset_delta: reader.read_u2()?,
                k: 251 - frame_type,
            },
            251 => Self::SameExtended {
                offset_delta: reader.read_u2()?,
            },
            252..=254 => {
                let offset_delta = reader.read_u2()?;
                let count = usize::from(frame_type - 251);
                let mut locals = Vec::with_capacity(count);
                for _ in 0..count {
                    locals.push(VerificationType::parse(reader)?);
                }
                Self::Append {
                    offset_delta,
                    locals,
                }
            }
            255 => {
                let offset_delta = reader.read_u2()?;
                let local_count = reader.read_u2()?;
                let mut locals = Vec::with_capacity(local_count as usize);
                for _ in 0..local_count {
                    locals.push(VerificationType::parse(reader)?);
                }
                let stack_count = reader.read_u2()?;
                let mut stack = Vec::with_capacity(stack_count as usize);
                for _ in 0..stack_count {
                    stack.push(VerificationType::parse(reader)?);
                }
                Self::Full {
                    offset_delta,
                    locals,
                    stack,
                }
            }
        };
        Ok(frame)
    }

    /// Writes this frame.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        match self {
            Self::Same { offset_delta } => {
                writer.write_u1(compact_u7(*offset_delta, "same_frame offset_delta")?);
            }
            Self::SameLocals1StackItem {
                offset_delta,
                stack,
            } => {
                writer.write_u1(
                    64 + compact_u7(*offset_delta, "same_locals_1_stack_item offset_delta")?,
                );
                stack.write_to(writer);
            }
            Self::SameLocals1StackItemExtended {
                offset_delta,
                stack,
            } => {
                writer.write_u1(247);
                writer.write_u2(*offset_delta);
                stack.write_to(writer);
            }
            Self::Chop { offset_delta, k } => {
                if !(1..=3).contains(k) {
                    return Err(Error::ValueTooLarge {
                        value: u64::from(*k),
                        target: "chop_frame k (1..=3)",
                    });
                }
                writer.write_u1(251 - k);
                writer.write_u2(*offset_delta);
            }
            Self::SameExtended { offset_delta } => {
                writer.write_u1(251);
                writer.write_u2(*offset_delta);
            }
            Self::Append {
                offset_delta,
                locals,
            } => {
                if !(1..=3).contains(&locals.len()) {
                    return Err(Error::ValueTooLarge {
                        value: locals.len() as u64,
                        target: "append_frame locals (1..=3)",
                    });
                }
                writer.write_u1(251 + locals.len() as u8);
                writer.write_u2(*offset_delta);
                for local in locals {
                    local.write_to(writer);
                }
            }
            Self::Full {
                offset_delta,
                locals,
                stack,
            } => {
                writer.write_u1(255);
                writer.write_u2(*offset_delta);
                writer.write_u2_len(locals.len())?;
                for local in locals {
                    local.write_to(writer);
                }
                writer.write_u2_len(stack.len())?;
                for item in stack {
                    item.write_to(writer);
                }
            }
        }
        Ok(())
    }
}

fn compact_u7(value: u16, target: &'static str) -> Result<u8> {
    u8::try_from(value)
        .ok()
        .filter(|v| *v <= 63)
        .ok_or(Error::ValueTooLarge {
            value: u64::from(value),
            target,
        })
}

/// `StackMapTable` attribute (JVMS §4.7.4).
#[derive(Debug, Clone, PartialEq)]
pub struct StackMapTableAttribute<'a> {
    /// The frames.
    pub entries: Vec<StackMapFrame<'a>>,
}

impl<'a> StackMapTableAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let count = reader.read_u2()?;
        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            entries.push(StackMapFrame::parse(reader)?);
        }
        Ok(Self { entries })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2_len(self.entries.len())?;
        for entry in &self.entries {
            entry.write_to(writer)?;
        }
        Ok(())
    }
}
