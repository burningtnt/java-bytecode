//! Bytecode instruction decoding and encoding (JVMS §6.5).
//!
//! This module is the building block for editing a method's code. It decodes a
//! raw `Code` byte array into a [`Vec<Instruction>`] and encodes a list of
//! instructions back into bytes, preserving branch offsets and switch padding
//! exactly.
//!
//! # Modifying bytecode (instrumentation)
//!
//! The crate deliberately does not ship an assembler with label resolution, so
//! the steps are explicit. A typical instrumentation pass looks like this:
//!
//! 1. **Find the target `Code` attribute.** Use [`ClassFile::find_method_mut`]
//!    (or [`ClassFile::find_field_mut`]) together with
//!    [`MethodInfo::code_mut`], or walk the `methods`/`attributes` fields.
//! 2. **Intern the constants you reference.** Adding a call, a string or a
//!    class starts with [`ConstantPool::intern`] / [`ConstantPool::intern_utf8`],
//!    which deduplicate. The returned [`ConstantRef`] is what you embed in the
//!    new instructions.
//! 3. **Edit through [`crate::CodeAttribute::instructions_mut`].** It decodes the
//!    bytecode into an [`Instructions<'a, '_>`](crate::Instructions) view that
//!    dereferences to `Vec<Instruction<'a>>`, keeping the class's constant-pool
//!    scope `'a`. Mutate the vector, then call
//!    [`crate::Instructions::apply`](crate::Instructions::apply) to re-encode the
//!    instructions and write the bytes back into the `Code` attribute.
//! 4. **Fix the metadata the verifier reads:** `max_stack`, `max_locals`, the
//!    exception table program counters, and — when control flow changes — the
//!    `StackMapTable` (see below).
//! 5. **Serialize** with [`ClassFile::to_bytes`] (index-stable, recommended
//!    after interning) or [`ClassFile::to_bytes_rebuilt`] (compacts the pool).
//!
//! [`ClassFile::find_method_mut`]: crate::ClassFile::find_method_mut
//! [`ClassFile::find_field_mut`]: crate::ClassFile::find_field_mut
//! [`MethodInfo::code_mut`]: crate::MethodInfo::code_mut
//! [`ConstantPool::intern`]: crate::ConstantPool::intern
//! [`ConstantPool::intern_utf8`]: crate::ConstantPool::intern_utf8
//! [`ClassFile::to_bytes`]: crate::ClassFile::to_bytes
//! [`ClassFile::to_bytes_rebuilt`]: crate::ClassFile::to_bytes_rebuilt
//!
//! ## Why `decode` and `encode` are `unsafe`
//!
//! [`decode`] and [`encode`] have no access to a constant pool, so they cannot
//! check that the embedded constant-pool indices are valid or which class they
//! belong to. They are therefore `unsafe`, and the returned instructions carry
//! an unconstrained lifetime. [`crate::CodeAttribute::instructions_mut`] performs the
//! same decode internally with the correct scope and is the safe path.
//!
//! Likewise, [`crate::CodeAttribute::set_code`] and
//! [`crate::CodeAttribute::set_code_owned`] let you install raw bytes verbatim; they
//! are `unsafe` for the same reason and should be avoided in favour of
//! [`crate::CodeAttribute::instructions_mut`] + [`crate::Instructions::apply`].
//!
//! ## What you must recompute
//!
//! Decoding and encoding are byte-exact but *do not* relaunch offsets for you.
//! Relative branch offsets and switch targets are stored exactly as they appear
//! on the wire, so inserting or removing instructions invalidates them. After a
//! size-changing edit you are responsible for:
//!
//! * **Branch and switch targets.** Convert each relative offset to an absolute
//!   target (`target = instruction_pc + relative_offset`) before editing, then
//!   back to a relative offset after. Note that switch padding depends on the
//!   instruction's position (see [`Instruction::encoded_len`]), so relative
//!   lengths can change when alignment changes.
//! * **The exception table** (`start_pc`, `end_pc`, `handler_pc`).
//! * **Debug tables** (`LineNumberTable`, `LocalVariableTable`,
//!   `LocalVariableTypeTable`). Stale values are tolerated by the JVM but are
//!   misleading, so update them if you can.
//! * **`StackMapTable`.** The verifier derives its types from these frames. If
//!   you only prepend straight-line code to a branch-free method, no new frames
//!   are needed; if you change control flow, the frames must be recomputed,
//!   which this crate does not do.
//! * **`max_stack` and `max_locals`.** Increase `max_stack` when you push more
//!   values (e.g. before an `invoke`), and `max_locals` if you introduce new
//!   local slots.
//!
//! ## Rebuild vs. index-stable output
//!
//! [`ClassFile::to_bytes`] preserves constant-pool indices, so an index you
//! interened (like the method reference below) stays valid and appears verbatim
//! in the written bytes. [`ClassFile::to_bytes_rebuilt`] instead renumbers every
//! reference — including the operands inside `Code` — so the indices in the
//! output differ from the ones in memory. Prefer the former while you are still
//! reasoning about indices, and the latter for a compact final artifact.
//!
//! ## Worked example
//!
//! The following prepends a call to a static hook at the start of a branch-free
//! method. Because the method is just `return`, there are no branch offsets,
//! exception entries or stack-map frames to fix, and `invokestatic` with the
//! descriptor `()V` neither pops nor pushes, so `max_stack` is unchanged.
//!
//! ```
//! use java_bytecode::{ClassFile, Constant, Instruction, Mutf8, Opcode};
//!
//! let input: &[u8] = include_bytes!(concat!(
//!     env!("CARGO_MANIFEST_DIR"),
//!     "/tests/fixtures/classes/main/example/Simple.class"
//! ));
//! let mut class = ClassFile::parse(input).unwrap();
//!
//! // 1. Intern `example/Hook.entry()V` and build a Methodref to it.
//! let class_name = class.constant_pool.intern_utf8(Mutf8::try_from("example/Hook").unwrap()).unwrap();
//! let class_ref = class
//!     .constant_pool
//!     .intern(Constant::Class { name_index: class_name })
//!     .unwrap();
//! let name = class.constant_pool.intern_utf8(Mutf8::try_from("entry").unwrap()).unwrap();
//! let descriptor = class.constant_pool.intern_utf8(Mutf8::try_from("()V").unwrap()).unwrap();
//! let name_and_type = class
//!     .constant_pool
//!     .intern(Constant::NameAndType {
//!         name_index: name,
//!         descriptor_index: descriptor,
//!     })
//!     .unwrap();
//! let method_ref = class
//!     .constant_pool
//!     .intern(Constant::Methodref {
//!         class_index: class_ref,
//!         name_and_type_index: name_and_type,
//!     })
//!     .unwrap();
//!
//! // 2. Edit through the scoped instructions view, then write it back.
//! {
//!     let code = class
//!         .find_method_mut("voidMethod", "()V")
//!         .unwrap()
//!         .code_mut()
//!         .unwrap();
//!     let mut instructions = code.instructions_mut().unwrap();
//!     instructions.insert(0, Instruction::ConstantWide(Opcode::Invokestatic, method_ref));
//!     instructions.apply().unwrap();
//! }
//!
//! // 3. Serialize (index-stable) and confirm the hook is present.
//! let output = class.to_bytes().unwrap();
//! let mut reparsed = ClassFile::parse(&output).unwrap();
//! let loaded = reparsed
//!     .find_method_mut("voidMethod", "()V")
//!     .unwrap()
//!     .code_mut()
//!     .unwrap()
//!     .instructions_mut()
//!     .unwrap()
//!     .into_vec();
//! assert_eq!(loaded[0].opcode(), Opcode::Invokestatic);
//! ```

pub mod opcode;

pub use opcode::{Opcode, OperandKind};

use crate::constant_pool::ConstantRef;
use crate::error::{Error, Result};
use crate::io::{Reader, Writer};

/// A decoded JVM instruction.
///
/// Branch offsets are stored exactly as they appear on the wire, i.e. relative
/// to the offset of the instruction itself. `tableswitch`/`lookupswitch`
/// padding bytes are preserved so that decoding then encoding a method yields
/// the original bytes.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Instruction<'a> {
    /// An instruction with no operands.
    None(Opcode),
    /// `bipush`.
    Byte(Opcode, i8),
    /// `sipush`.
    Short(Opcode, i16),
    /// An instruction with a one-byte local variable index.
    Local(Opcode, u8),
    /// `ldc` (one-byte constant pool index).
    Constant(Opcode, ConstantRef<'a>),
    /// An instruction with a two-byte constant pool index.
    ConstantWide(Opcode, ConstantRef<'a>),
    /// An instruction with a signed 16-bit relative branch offset.
    Branch(Opcode, i16),
    /// `goto_w` / `jsr_w` (signed 32-bit relative branch offset).
    BranchWide(Opcode, i32),
    /// `iinc`.
    Iinc {
        /// Local variable index.
        index: u8,
        /// Signed increment.
        amount: i8,
    },
    /// `wide iinc`.
    IincWide {
        /// Local variable index.
        index: u16,
        /// Signed increment.
        amount: i16,
    },
    /// `wide <load/store/ret>`.
    WideLocal {
        /// The modified opcode.
        opcode: Opcode,
        /// Local variable index.
        index: u16,
    },
    /// `invokeinterface`.
    InvokeInterface {
        /// Constant pool index of the interface method.
        index: ConstantRef<'a>,
        /// Argument slot count.
        count: u8,
    },
    /// `invokedynamic`.
    InvokeDynamic {
        /// Constant pool index.
        index: ConstantRef<'a>,
        /// The two reserved bytes (normally zero), preserved verbatim.
        reserved: u16,
    },
    /// `newarray`.
    NewArray {
        /// Array type code (4–11).
        atype: u8,
    },
    /// `multianewarray`.
    MultiANewArray {
        /// Constant pool index.
        index: ConstantRef<'a>,
        /// Number of dimensions.
        dimensions: u8,
    },
    /// `tableswitch`.
    TableSwitch {
        /// Default target, relative to the switch offset.
        default: i32,
        /// Lowest match value.
        low: i32,
        /// Highest match value.
        high: i32,
        /// Per-case targets, relative to the switch offset.
        offsets: Vec<i32>,
        /// The alignment padding bytes that preceded the table.
        padding: Vec<u8>,
    },
    /// `lookupswitch`.
    LookupSwitch {
        /// Default target, relative to the switch offset.
        default: i32,
        /// `(match, target)` pairs, targets relative to the switch offset.
        pairs: Vec<(i32, i32)>,
        /// The alignment padding bytes that preceded the pairs.
        padding: Vec<u8>,
    },
}

impl<'a> Instruction<'a> {
    /// Returns the leading opcode byte of this instruction.
    #[must_use]
    pub const fn opcode(&self) -> Opcode {
        match self {
            Self::None(op)
            | Self::Byte(op, _)
            | Self::Short(op, _)
            | Self::Local(op, _)
            | Self::Constant(op, _)
            | Self::ConstantWide(op, _)
            | Self::Branch(op, _)
            | Self::BranchWide(op, _) => *op,
            Self::Iinc { .. } => Opcode::Iinc,
            Self::IincWide { .. } | Self::WideLocal { .. } => Opcode::Wide,
            Self::InvokeInterface { .. } => Opcode::Invokeinterface,
            Self::InvokeDynamic { .. } => Opcode::Invokedynamic,
            Self::NewArray { .. } => Opcode::Newarray,
            Self::MultiANewArray { .. } => Opcode::Multianewarray,
            Self::TableSwitch { .. } => Opcode::TableSwitch,
            Self::LookupSwitch { .. } => Opcode::LookupSwitch,
        }
    }

    /// The number of bytes this instruction occupies when `pc` bytes of code
    /// precede it.
    #[must_use]
    pub fn encoded_len(&self, pc: usize) -> usize {
        match self {
            Self::None(_) => 1,
            Self::Byte(_, _) => 2,
            Self::Short(_, _) => 3,
            Self::Local(_, _) | Self::Constant(_, _) | Self::NewArray { .. } => 2,
            Self::ConstantWide(_, _) | Self::Branch(_, _) | Self::Iinc { .. } => 3,
            Self::BranchWide(_, _) | Self::InvokeInterface { .. } | Self::InvokeDynamic { .. } => 5,
            Self::WideLocal { .. } => 4,
            Self::IincWide { .. } => 6,
            Self::MultiANewArray { .. } => 4,
            Self::TableSwitch {
                offsets, padding, ..
            } => 1 + padding_len(pc, padding) + 12 + offsets.len() * 4,
            Self::LookupSwitch { pairs, padding, .. } => {
                1 + padding_len(pc, padding) + 8 + pairs.len() * 8
            }
        }
    }
}

fn required_padding(pc: usize) -> usize {
    (3 - (pc & 3)) & 3
}

fn padding_len(pc: usize, stored: &[u8]) -> usize {
    let required = required_padding(pc);
    if stored.len() == required {
        stored.len()
    } else {
        required
    }
}

/// Decodes a bytecode array into instructions.
///
/// # Safety
///
/// This function has no access to the constant pool, so it cannot check that
/// the embedded constant-pool indices are valid, nor which pool they belong to.
/// The returned [`Instruction`]s carry an unconstrained lifetime `'a`; the
/// caller must ensure `'a` is the lifetime of the constant pool those indices
/// refer to. Prefer [`crate::CodeAttribute::instructions_mut`](crate::CodeAttribute::instructions_mut),
/// which performs the decode with the correct scope.
#[allow(unsafe_code)]
pub unsafe fn decode<'a>(code: &[u8]) -> Result<Vec<Instruction<'a>>> {
    decode_impl(code)
}

/// Crate-internal decode that is known to be used with the right pool scope.
pub(crate) fn decode_impl<'a>(code: &[u8]) -> Result<Vec<Instruction<'a>>> {
    let mut reader = Reader::new(code);
    let mut instructions = Vec::new();
    while !reader.is_empty() {
        let pc = reader.position();
        let opcode_offset = reader.position();
        let opcode = Opcode::from_u8(reader.read_u1()?).ok_or(Error::InvalidOpcode {
            opcode: code[opcode_offset],
            offset: opcode_offset,
        })?;
        instructions.push(decode_one(&mut reader, opcode, pc)?);
    }
    Ok(instructions)
}

fn decode_one<'a>(reader: &mut Reader<'_>, opcode: Opcode, pc: usize) -> Result<Instruction<'a>> {
    let instruction = match opcode.operand_kind() {
        OperandKind::None => Instruction::None(opcode),
        OperandKind::Byte => Instruction::Byte(opcode, reader.read_u1()? as i8),
        OperandKind::Short => Instruction::Short(opcode, reader.read_u2()? as i16),
        OperandKind::Local => Instruction::Local(opcode, reader.read_u1()?),
        OperandKind::Ldc => {
            Instruction::Constant(opcode, ConstantRef::new(u16::from(reader.read_u1()?)))
        }
        OperandKind::ConstantPool => {
            Instruction::ConstantWide(opcode, ConstantRef::new(reader.read_u2()?))
        }
        OperandKind::Branch => Instruction::Branch(opcode, reader.read_u2()? as i16),
        OperandKind::BranchWide => Instruction::BranchWide(opcode, reader.read_u4()? as i32),
        OperandKind::NewArray => Instruction::NewArray {
            atype: reader.read_u1()?,
        },
        OperandKind::Iinc => Instruction::Iinc {
            index: reader.read_u1()?,
            amount: reader.read_u1()? as i8,
        },
        OperandKind::InvokeInterface => {
            let index = ConstantRef::new(reader.read_u2()?);
            let count = reader.read_u1()?;
            // The fourth operand byte is reserved and must be zero.
            let _reserved = reader.read_u1()?;
            Instruction::InvokeInterface { index, count }
        }
        OperandKind::InvokeDynamic => Instruction::InvokeDynamic {
            index: ConstantRef::new(reader.read_u2()?),
            reserved: reader.read_u2()?,
        },
        OperandKind::MultiANewArray => Instruction::MultiANewArray {
            index: ConstantRef::new(reader.read_u2()?),
            dimensions: reader.read_u1()?,
        },
        OperandKind::TableSwitch => {
            let pad = required_padding(pc);
            let padding = reader.read_bytes(pad)?.to_vec();
            let default = reader.read_i4()?;
            let low = reader.read_i4()?;
            let high = reader.read_i4()?;
            let count = (i64::from(high) - i64::from(low) + 1).max(0) as usize;
            let mut offsets = Vec::with_capacity(count);
            for _ in 0..count {
                offsets.push(reader.read_i4()?);
            }
            Instruction::TableSwitch {
                default,
                low,
                high,
                offsets,
                padding,
            }
        }
        OperandKind::LookupSwitch => {
            let pad = required_padding(pc);
            let padding = reader.read_bytes(pad)?.to_vec();
            let default = reader.read_i4()?;
            let pair_count = reader.read_i4()?.max(0) as usize;
            let mut pairs = Vec::with_capacity(pair_count);
            for _ in 0..pair_count {
                let key = reader.read_i4()?;
                let offset = reader.read_i4()?;
                pairs.push((key, offset));
            }
            Instruction::LookupSwitch {
                default,
                pairs,
                padding,
            }
        }
        OperandKind::Wide => {
            let modified = reader.read_u1()?;
            let modified_opcode = Opcode::from_u8(modified).ok_or(Error::InvalidOpcode {
                opcode: modified,
                offset: reader.position() - 1,
            })?;
            if modified_opcode == Opcode::Iinc {
                Instruction::IincWide {
                    index: reader.read_u2()?,
                    amount: reader.read_u2()? as i16,
                }
            } else {
                Instruction::WideLocal {
                    opcode: modified_opcode,
                    index: reader.read_u2()?,
                }
            }
        }
    };
    Ok(instruction)
}

/// Encodes `instructions` into a bytecode array.
///
/// # Safety
///
/// This function has no access to the constant pool, so it cannot check that
/// the constant-pool indices carried by the instructions are valid for the
/// class the bytes will be written into. The caller must ensure they are.
/// Prefer [`crate::CodeAttribute::instructions_mut`](crate::CodeAttribute::instructions_mut),
/// whose [`apply`](crate::Instructions::apply) encodes with the correct scope.
#[allow(unsafe_code)]
pub unsafe fn encode<'a>(instructions: &[Instruction<'a>]) -> Result<Vec<u8>> {
    encode_impl(instructions)
}

/// Crate-internal encode that is known to be used with the right pool scope.
pub(crate) fn encode_impl(instructions: &[Instruction<'_>]) -> Result<Vec<u8>> {
    let mut writer = Writer::new();
    encode_into_impl(&mut writer, instructions)?;
    Ok(writer.into_bytes())
}

/// Encodes `instructions` into an existing writer.
///
/// When the writer carries a constant-pool remapping, constant-pool operand
/// indices are translated through it.
///
/// # Safety
///
/// As for [`encode`]: the caller must ensure the instructions' constant-pool
/// indices are valid for the target class file.
#[allow(unsafe_code)]
pub unsafe fn encode_into<'a>(writer: &mut Writer, instructions: &[Instruction<'a>]) -> Result<()> {
    encode_into_impl(writer, instructions)
}

/// Crate-internal encode into an existing writer.
pub(crate) fn encode_into_impl(
    writer: &mut Writer,
    instructions: &[Instruction<'_>],
) -> Result<()> {
    for instruction in instructions {
        encode_one(writer, instruction)?;
    }
    Ok(())
}

fn encode_one(writer: &mut Writer, instruction: &Instruction<'_>) -> Result<()> {
    let pc = writer.len();
    match instruction {
        Instruction::None(op) => writer.write_u1(op.as_u8()),
        Instruction::Byte(op, value) => {
            writer.write_u1(op.as_u8());
            writer.write_u1(*value as u8);
        }
        Instruction::Short(op, value) => {
            writer.write_u1(op.as_u8());
            writer.write_u2(*value as u16);
        }
        Instruction::Local(op, index) => {
            writer.write_u1(op.as_u8());
            writer.write_u1(*index);
        }
        Instruction::Constant(op, index) => {
            writer.write_u1(op.as_u8());
            let raw = u8::try_from(index.get()).map_err(|_| Error::ValueTooLarge {
                value: u64::from(index.get()),
                target: "u8 (ldc constant pool index)",
            })?;
            writer.write_cp_u1(raw)?;
        }
        Instruction::ConstantWide(op, index) => {
            writer.write_u1(op.as_u8());
            writer.write_cp_u2(index.get());
        }
        Instruction::Branch(op, offset) => {
            writer.write_u1(op.as_u8());
            writer.write_u2(*offset as u16);
        }
        Instruction::BranchWide(op, offset) => {
            writer.write_u1(op.as_u8());
            writer.write_u4(*offset as u32);
        }
        Instruction::Iinc { index, amount } => {
            writer.write_u1(Opcode::Iinc.as_u8());
            writer.write_u1(*index);
            writer.write_u1(*amount as u8);
        }
        Instruction::IincWide { index, amount } => {
            writer.write_u1(Opcode::Wide.as_u8());
            writer.write_u1(Opcode::Iinc.as_u8());
            writer.write_u2(*index);
            writer.write_u2(*amount as u16);
        }
        Instruction::WideLocal { opcode, index } => {
            writer.write_u1(Opcode::Wide.as_u8());
            writer.write_u1(opcode.as_u8());
            writer.write_u2(*index);
        }
        Instruction::InvokeInterface { index, count } => {
            writer.write_u1(Opcode::Invokeinterface.as_u8());
            writer.write_cp_u2(index.get());
            writer.write_u1(*count);
            writer.write_u1(0);
        }
        Instruction::InvokeDynamic { index, reserved } => {
            writer.write_u1(Opcode::Invokedynamic.as_u8());
            writer.write_cp_u2(index.get());
            writer.write_u2(*reserved);
        }
        Instruction::NewArray { atype } => {
            writer.write_u1(Opcode::Newarray.as_u8());
            writer.write_u1(*atype);
        }
        Instruction::MultiANewArray { index, dimensions } => {
            writer.write_u1(Opcode::Multianewarray.as_u8());
            writer.write_cp_u2(index.get());
            writer.write_u1(*dimensions);
        }
        Instruction::TableSwitch {
            default,
            low,
            high,
            offsets,
            padding,
        } => {
            writer.write_u1(Opcode::TableSwitch.as_u8());
            write_padding(writer, pc, padding);
            writer.write_i4(*default);
            writer.write_i4(*low);
            writer.write_i4(*high);
            for offset in offsets {
                writer.write_i4(*offset);
            }
        }
        Instruction::LookupSwitch {
            default,
            pairs,
            padding,
        } => {
            writer.write_u1(Opcode::LookupSwitch.as_u8());
            write_padding(writer, pc, padding);
            writer.write_i4(*default);
            writer.write_i4(pairs.len() as i32);
            for (key, offset) in pairs {
                writer.write_i4(*key);
                writer.write_i4(*offset);
            }
        }
    }
    Ok(())
}

fn write_padding(writer: &mut Writer, pc: usize, stored: &[u8]) {
    let required = required_padding(pc);
    if stored.len() == required {
        writer.write_bytes(stored);
    } else {
        for _ in 0..required {
            writer.write_u1(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Instruction, decode_impl, encode_impl, encode_into_impl};
    use crate::bytecode::{Opcode, OperandKind};
    use crate::constant_pool::ConstantRef;
    use crate::io::Writer;

    #[test]
    fn opcode_round_trips() {
        for byte in 0u8..=255 {
            if let Some(opcode) = Opcode::from_u8(byte) {
                assert_eq!(opcode.as_u8(), byte);
            }
        }
    }

    #[test]
    fn decodes_and_encodes_simple_sequence() {
        // iconst_1; bipush 42; istore_1; iload_1; ireturn
        let code = [0x04, 0x10, 0x2a, 0x3c, 0x1b, 0xac];
        let instructions = decode_impl(&code).unwrap();
        assert_eq!(instructions.len(), 5);
        assert_eq!(instructions[1], Instruction::Byte(Opcode::Bipush, 42));
        assert_eq!(encode_impl(&instructions).unwrap(), code);
    }

    #[test]
    fn decodes_wide_iinc() {
        // wide iinc 256 300
        let code = [0xc4, 0x84, 0x01, 0x00, 0x01, 0x2c];
        let instructions = decode_impl(&code).unwrap();
        assert_eq!(
            instructions[0],
            Instruction::IincWide {
                index: 256,
                amount: 300
            }
        );
        assert_eq!(encode_impl(&instructions).unwrap(), code);
    }

    #[test]
    fn tableswitch_padding_round_trips() {
        // tableswitch at offset 0 needs 3 padding bytes.
        let code = [
            0xaa, 0x00, 0x00, 0x00, // padding
            0x00, 0x00, 0x00, 0x10, // default = 16
            0x00, 0x00, 0x00, 0x00, // low = 0
            0x00, 0x00, 0x00, 0x02, // high = 2
            0x00, 0x00, 0x00, 0x20, // target 0
            0x00, 0x00, 0x00, 0x24, // target 1
            0x00, 0x00, 0x00, 0x28, // target 2
        ];
        let instructions = decode_impl(&code).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(encode_impl(&instructions).unwrap(), code);
    }

    #[test]
    fn lookupswitch_round_trips() {
        let code = [
            0xab, 0x00, 0x00, 0x00, // padding (pc = 0 -> 3 bytes)
            0x00, 0x00, 0x00, 0x0c, // default
            0x00, 0x00, 0x00, 0x02, // npairs
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x10, // key 1 -> 16
            0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x20, // key 5 -> 32
        ];
        let instructions = decode_impl(&code).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(encode_impl(&instructions).unwrap(), code);
    }

    #[test]
    fn operand_kinds_are_consistent() {
        assert_eq!(Opcode::Nop.operand_kind(), OperandKind::None);
        assert_eq!(Opcode::Getstatic.operand_kind(), OperandKind::ConstantPool);
        assert_eq!(Opcode::Ifeq.operand_kind(), OperandKind::Branch);
    }

    /// Builds a minimal, valid instruction for any (non-`wide`) opcode.
    fn sample_instruction<'a>(op: Opcode) -> Instruction<'a> {
        match op.operand_kind() {
            OperandKind::None => Instruction::None(op),
            OperandKind::Byte => Instruction::Byte(op, -5),
            OperandKind::Short => Instruction::Short(op, -1234),
            OperandKind::Local => Instruction::Local(op, 7),
            OperandKind::Ldc => Instruction::Constant(op, ConstantRef::new(7)),
            OperandKind::ConstantPool => Instruction::ConstantWide(op, ConstantRef::new(777)),
            OperandKind::Branch => Instruction::Branch(op, 12),
            OperandKind::BranchWide => Instruction::BranchWide(op, 123_456),
            OperandKind::NewArray => Instruction::NewArray { atype: 10 },
            OperandKind::Iinc => Instruction::Iinc {
                index: 3,
                amount: -2,
            },
            OperandKind::InvokeInterface => Instruction::InvokeInterface {
                index: ConstantRef::new(42),
                count: 3,
            },
            OperandKind::InvokeDynamic => Instruction::InvokeDynamic {
                index: ConstantRef::new(42),
                reserved: 0,
            },
            OperandKind::MultiANewArray => Instruction::MultiANewArray {
                index: ConstantRef::new(42),
                dimensions: 2,
            },
            // Decoding at pc = 0 gives three padding bytes for both switches.
            OperandKind::TableSwitch => Instruction::TableSwitch {
                default: 12,
                low: 0,
                high: 2,
                offsets: vec![20, 24, 28],
                padding: vec![0, 0, 0],
            },
            OperandKind::LookupSwitch => Instruction::LookupSwitch {
                default: 12,
                pairs: vec![(1, 16), (5, 32)],
                padding: vec![0, 0, 0],
            },
            // `wide` is a prefix, not a standalone instruction.
            OperandKind::Wide => unreachable!("wide is tested separately"),
        }
    }

    #[test]
    fn every_opcode_round_trips() {
        let mut covered = 0usize;
        for byte in 0u8..=0xFF {
            let Some(op) = Opcode::from_u8(byte) else {
                continue;
            };
            if op.operand_kind() == OperandKind::Wide {
                continue;
            }
            let instruction = sample_instruction(op);
            let expected = std::slice::from_ref(&instruction);
            let bytes = encode_impl(expected).expect("encode");
            let decoded = decode_impl(&bytes).expect("decode");
            assert_eq!(
                decoded.as_slice(),
                expected,
                "opcode {op:?} ({}) did not round-trip",
                op.mnemonic()
            );
            covered += 1;
        }
        assert!(
            covered >= 200,
            "expected to cover most opcodes, got {covered}"
        );
    }

    #[test]
    fn decodes_wide_local() {
        // wide iload 300
        let code = [0xc4, 0x15, 0x01, 0x2c];
        let instructions = decode_impl(&code).unwrap();
        assert_eq!(
            instructions[0],
            Instruction::WideLocal {
                opcode: Opcode::Iload,
                index: 300,
            }
        );
        assert_eq!(encode_impl(&instructions).unwrap(), code);
    }

    #[test]
    fn switch_padding_matches_alignment() {
        for nops in 0usize..4 {
            let mut sequence: Vec<Instruction<'static>> =
                std::iter::repeat_n(Instruction::None(Opcode::Nop), nops).collect();
            sequence.push(Instruction::TableSwitch {
                default: 0,
                low: 0,
                high: 1,
                offsets: vec![4, 8],
                padding: Vec::new(),
            });
            sequence.push(Instruction::LookupSwitch {
                default: 0,
                pairs: vec![(1, 4)],
                padding: Vec::new(),
            });

            let bytes = encode_impl(&sequence).expect("encode");
            let padding = (3 - (nops & 3)) & 3;
            // The bytes following each switch opcode must align to a multiple
            // of four relative to the start of the code array.
            assert_eq!((nops + 1 + padding) % 4, 0);

            let decoded = decode_impl(&bytes).expect("decode");
            assert_eq!(encode_impl(&decoded).expect("re-encode"), bytes);
        }
    }

    #[test]
    fn encoded_len_matches_actual() {
        let sequence = [
            Instruction::None(Opcode::Nop),
            Instruction::Byte(Opcode::Bipush, 1),
            Instruction::ConstantWide(Opcode::Getstatic, ConstantRef::new(2)),
            Instruction::TableSwitch {
                default: 0,
                low: 0,
                high: 0,
                offsets: vec![4],
                padding: Vec::new(),
            },
        ];
        let bytes = encode_impl(&sequence).expect("encode");
        let mut pc = 0usize;
        for instruction in &sequence {
            pc += instruction.encoded_len(pc);
        }
        assert_eq!(pc, bytes.len());
    }

    #[test]
    fn encode_into_remaps_constant_pool_only() {
        let instructions = [
            Instruction::ConstantWide(Opcode::LdcW, ConstantRef::new(5)),
            Instruction::Local(Opcode::Iload, 5),
            Instruction::Branch(Opcode::Goto, 5),
            Instruction::InvokeInterface {
                index: ConstantRef::new(5),
                count: 3,
            },
            Instruction::InvokeDynamic {
                index: ConstantRef::new(5),
                reserved: 0,
            },
        ];
        let mut remap: Vec<u16> = (0..6).collect();
        remap[5] = 9;

        let mut writer = Writer::with_remap(remap);
        encode_into_impl(&mut writer, &instructions).expect("encode");
        let bytes = writer.into_bytes();

        assert_eq!(&bytes[0..3], &[0x13, 0x00, 0x09]); // ldc_w index remapped
        assert_eq!(&bytes[3..5], &[0x15, 0x05]); // iload local index kept
        assert_eq!(&bytes[5..8], &[0xa7, 0x00, 0x05]); // goto offset kept
        assert_eq!(&bytes[8..13], &[0xb9, 0x00, 0x09, 0x03, 0x00]); // invokeinterface
        assert_eq!(&bytes[13..18], &[0xba, 0x00, 0x09, 0x00, 0x00]); // invokedynamic
    }
}
