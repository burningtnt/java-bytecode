//! Error and result types.

/// Convenience alias used throughout the crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Errors produced while reading or writing a class file.
///
/// Parsing is deliberately permissive about the *semantics* of a class file
/// (this crate performs no bytecode verification), but it never panics on
/// malformed input: truncation, invalid tags and out-of-range values are all
/// reported as variants of this type.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The input ended before the requested number of bytes could be read.
    #[error(
        "unexpected end of input at offset {offset}: needed {needed} byte(s), {remaining} remaining"
    )]
    UnexpectedEof {
        /// Offset into the input at which the read started.
        offset: usize,
        /// Number of bytes the read wanted.
        needed: usize,
        /// Number of bytes that were actually available.
        remaining: usize,
    },

    /// The four-byte class file magic was not `0xCAFEBABE`.
    #[error("invalid class file magic 0x{found:08X} (expected 0xCAFEBABE)")]
    InvalidMagic {
        /// The magic value that was found.
        found: u32,
    },

    /// A constant pool entry declared an unknown tag byte.
    #[error("invalid constant pool tag {tag} at offset {offset}")]
    InvalidConstantTag {
        /// The offending tag value.
        tag: u8,
        /// Offset of the tag byte.
        offset: usize,
    },

    /// A constant pool index did not point at a valid entry.
    #[error("constant pool index {index} is out of range (pool has {len} entries)")]
    ConstantIndexOutOfRange {
        /// The invalid index.
        index: u16,
        /// Number of entries in the pool.
        len: usize,
    },

    /// A [`Constant::Invalid`](crate::Constant::Invalid) placeholder was
    /// serialized. It is a bookkeeping value for unused slots and must not
    /// appear on the wire.
    #[error("cannot serialize Constant::Invalid (an unusable constant-pool slot)")]
    InvalidConstant,

    /// A `CONSTANT_Utf8` entry was not valid Java modified UTF-8.
    #[error("invalid modified UTF-8 at offset {offset}: {reason}")]
    InvalidModifiedUtf8 {
        /// Offset of the offending byte.
        offset: usize,
        /// Human readable reason.
        reason: &'static str,
    },

    /// A field or method descriptor could not be parsed.
    #[error("invalid descriptor at offset {offset}: {reason}")]
    InvalidDescriptor {
        /// Offset of the offending byte.
        offset: usize,
        /// Human readable reason.
        reason: &'static str,
    },

    /// The declared `attribute_length` did not match the bytes consumed by the
    /// attribute body.
    #[error("attribute length mismatch: declared {declared} byte(s) but body consumed {consumed}")]
    AttributeLengthMismatch {
        /// The length declared in the attribute header.
        declared: u32,
        /// Number of bytes actually consumed.
        consumed: usize,
    },

    /// A `RuntimeVisibleTypeAnnotations` entry declared an unknown
    /// `target_type`.
    #[error("invalid type-annotation target_type 0x{target_type:02X} at offset {offset}")]
    InvalidTargetType {
        /// The offending target type value.
        target_type: u8,
        /// Offset of the byte.
        offset: usize,
    },

    /// An `element_value` declared an unknown tag byte.
    #[error("invalid element_value tag 0x{tag:02X} at offset {offset}")]
    InvalidElementValueTag {
        /// The offending tag value.
        tag: u8,
        /// Offset of the tag byte.
        offset: usize,
    },

    /// A `StackMapTable` verification type declared an unknown tag byte.
    #[error("invalid verification_type tag {tag} at offset {offset}")]
    InvalidVerificationTypeTag {
        /// The offending tag value.
        tag: u8,
        /// Offset of the tag byte.
        offset: usize,
    },

    /// A bytecode stream contained an undefined opcode.
    #[error("invalid opcode 0x{opcode:02X} at bytecode offset {offset}")]
    InvalidOpcode {
        /// The offending opcode byte.
        opcode: u8,
        /// Bytecode offset of the opcode.
        offset: usize,
    },

    /// A value did not fit in its fixed-width field (e.g. more constants than
    /// `u16::MAX`).
    #[error("value {value} does not fit in {target}")]
    ValueTooLarge {
        /// The value that was too large.
        value: u64,
        /// The name of the target type.
        target: &'static str,
    },

    /// Bytes remained after a structure was fully parsed.
    #[error("{remaining} trailing byte(s) after parsing {context}")]
    TrailingBytes {
        /// Number of unconsumed bytes.
        remaining: usize,
        /// The structure being parsed.
        context: &'static str,
    },

    /// An instruction index or range used while editing bytecode was outside
    /// the method body.
    #[error("instruction index {index} is out of range (code has {len} instruction(s))")]
    InstructionIndexOutOfRange {
        /// The offending index (or the larger bound of an invalid range).
        index: usize,
        /// Number of instructions in the body.
        len: usize,
    },

    /// Rebasing a branch or switch offset after an edit produced a value that
    /// does not fit the instruction's field.
    #[error("relative offset {offset} does not fit in {target}")]
    RelativeOffsetOutOfRange {
        /// The offset that did not fit.
        offset: i64,
        /// The name of the target field.
        target: &'static str,
    },
}
