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
//!    bytecode into an [`Instructions<'a, '_>`](crate::Instructions) view scoped
//!    to the class's constant pool. Use its `insert`/`remove`/`replace`/`splice`
//!    methods (which keep branch and switch offsets consistent), then call
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
//! ## What the editing API handles
//!
//! [`crate::CodeAttribute::instructions_mut`] returns a view whose
//! `insert`/`remove`/`replace`/`splice` methods edit the internal instruction
//! list and rebase every branch and switch offset so that existing targets keep
//! pointing at the same logical instruction. Because switch padding depends on
//! the instruction's position (see [`Instruction::encoded_len`]), inserting or
//! removing bytes can change a switch's encoded length; that is accounted for.
//!
//! Relative offsets carried by instructions you supply are interpreted in the
//! original (pre-edit) layout — the replacement sequence is treated as laid out
//! at the splice point — so a newly inserted branch may reference an
//! instruction that already existed. An offset that pointed at an instruction
//! inside the removed range is moved forward to the first instruction after the
//! replacement.
//!
//! ## What you must still recompute
//!
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
//! The following reads an existing class and prepends a call to a static hook to
//! `ControlFlow.dense(int)`. That method starts with `iload_1; tableswitch`, so
//! inserting three bytes shifts the switch and grows its alignment padding by
//! one; `insert` rebases the switch's case and default offsets so they still
//! reach the same case bodies.
//!
//! ```
//! use java_bytecode::{ClassFile, Constant, Instruction, Opcode};
//!
//! let input: &[u8] = include_bytes!(concat!(
//!     env!("CARGO_MANIFEST_DIR"),
//!     "/tests/fixtures/classes/main/example/ControlFlow.class"
//! ));
//! let mut class = ClassFile::parse(input).unwrap();
//!
//! // 1. Intern `example/Hook.entry()V` and build a Methodref to it.
//! let class_name = class.constant_pool.intern_utf8("example/Hook").unwrap();
//! let class_ref = class
//!     .constant_pool
//!     .intern(Constant::Class { name_index: class_name })
//!     .unwrap();
//! let name = class.constant_pool.intern_utf8("entry").unwrap();
//! let descriptor = class.constant_pool.intern_utf8("()V").unwrap();
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
//! // 2. Record the tableswitch's default offset before editing.
//! let default_before = {
//!     let before = class
//!         .find_method("dense", "(I)I")
//!         .unwrap()
//!         .code()
//!         .unwrap()
//!         .instructions()
//!         .unwrap();
//!     match &before[1] {
//!         Instruction::TableSwitch { default, .. } => *default,
//!         other => panic!("expected a tableswitch, got {:?}", other.opcode()),
//!     }
//! };
//!
//! // 3. Insert the hook at the head; branch/switch offsets are rebased.
//! {
//!     let code = class
//!         .find_method_mut("dense", "(I)I")
//!         .unwrap()
//!         .code_mut()
//!         .unwrap();
//!     let mut instructions = code.instructions_mut().unwrap();
//!     instructions
//!         .insert(0, Instruction::ConstantWide(Opcode::Invokestatic, method_ref))
//!         .unwrap();
//!     instructions.apply().unwrap();
//! }
//!
//! // 4. Serialize and confirm the hook and the rebased switch target.
//! let output = class.to_bytes().unwrap();
//! let reparsed = ClassFile::parse(&output).unwrap();
//! let loaded = reparsed
//!     .find_method("dense", "(I)I")
//!     .unwrap()
//!     .code()
//!     .unwrap()
//!     .instructions()
//!     .unwrap();
//! assert_eq!(loaded[0].opcode(), Opcode::Invokestatic);
//! let default_after = match &loaded[2] {
//!     Instruction::TableSwitch { default, .. } => *default,
//!     other => panic!("expected a tableswitch, got {:?}", other.opcode()),
//! };
//! // The switch moved and its padding grew by one byte, so the default target
//! // had to be rebased to keep reaching the same case body.
//! assert_eq!(default_after, default_before + 1);
//! ```

pub mod opcode;

pub use opcode::{Opcode, OperandKind};

use core::ops::Range;

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

/// Computes the start program counter of each instruction and the total length.
fn layout(instructions: &[Instruction<'_>]) -> (Vec<usize>, usize) {
    let mut pcs = Vec::with_capacity(instructions.len());
    let mut pc = 0usize;
    for instruction in instructions {
        pcs.push(pc);
        pc += instruction.encoded_len(pc);
    }
    (pcs, pc)
}

/// Pushes the absolute target program counters carried by `instruction`.
///
/// Targets are stored on the wire relative to the instruction's own offset;
/// this resolves them to absolute offsets in `[0, code_len]`. Instructions
/// without a branch target push nothing.
fn resolve_targets(instruction: &Instruction<'_>, pc: usize, out: &mut Vec<i64>) {
    let base = pc as i64;
    match instruction {
        Instruction::Branch(_, offset) => {
            out.push(base + i64::from(*offset));
        }
        Instruction::BranchWide(_, offset) => {
            out.push(base + i64::from(*offset));
        }
        Instruction::TableSwitch {
            default, offsets, ..
        } => {
            out.push(base + i64::from(*default));
            out.extend(offsets.iter().map(|offset| base + i64::from(*offset)));
        }
        Instruction::LookupSwitch { default, pairs, .. } => {
            out.push(base + i64::from(*default));
            out.extend(pairs.iter().map(|(_, offset)| base + i64::from(*offset)));
        }
        _ => {}
    }
}

/// Writes remapped absolute target offsets back into `instruction`.
///
/// `targets` must be in the same order that [`resolve_targets`] produced them.
/// An offset that does not fit its field is reported as
/// [`Error::RelativeOffsetOutOfRange`].
fn write_targets(instruction: &mut Instruction<'_>, pc: usize, targets: &[i64]) -> Result<()> {
    let base = pc as i64;
    let fit_i16 = |offset: i64| {
        i16::try_from(offset).map_err(|_| Error::RelativeOffsetOutOfRange {
            offset,
            target: "i16 branch offset",
        })
    };
    let fit_i32 = |offset: i64| {
        i32::try_from(offset).map_err(|_| Error::RelativeOffsetOutOfRange {
            offset,
            target: "i32 branch offset",
        })
    };
    match instruction {
        Instruction::Branch(_, offset) => *offset = fit_i16(targets[0] - base)?,
        Instruction::BranchWide(_, offset) => *offset = fit_i32(targets[0] - base)?,
        Instruction::TableSwitch {
            default, offsets, ..
        } => {
            *default = fit_i32(targets[0] - base)?;
            for (slot, target) in offsets.iter_mut().zip(&targets[1..]) {
                *slot = fit_i32(*target - base)?;
            }
        }
        Instruction::LookupSwitch { default, pairs, .. } => {
            *default = fit_i32(targets[0] - base)?;
            for ((_, slot), target) in pairs.iter_mut().zip(&targets[1..]) {
                *slot = fit_i32(*target - base)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Replaces `instructions[range]` with `replacement`, rebasing every branch and
/// switch offset so that existing targets keep pointing at the same logical
/// instruction.
///
/// Relative offsets carried by `replacement` are interpreted in the *original*
/// layout: the replacement sequence is treated as laid out starting at the
/// splice point, so a supplied branch may reference an instruction that was
/// already present before the edit. Offsets that pointed at an instruction
/// inside `range` are moved forward to the first instruction after the
/// replacement.
///
/// Returns the instructions that were removed. On error `instructions` is left
/// unchanged.
pub(crate) fn splice<'a>(
    instructions: &mut Vec<Instruction<'a>>,
    range: Range<usize>,
    replacement: Vec<Instruction<'a>>,
) -> Result<Vec<Instruction<'a>>> {
    let len = instructions.len();
    let Range { start, end } = range;
    if start > end || end > len {
        return Err(Error::InstructionIndexOutOfRange {
            index: start.max(end),
            len,
        });
    }

    let (old_pcs, old_len) = layout(instructions);
    let removed_count = end - start;
    let replacement_len = replacement.len();

    // Lay the replacement out from the splice point in the original coordinate
    // system so its own relative offsets can be resolved.
    let edit_start = old_pcs.get(start).copied().unwrap_or(old_len) as i64;
    let mut replacement_pcs = Vec::with_capacity(replacement_len);
    let mut cursor = edit_start as usize;
    for instruction in &replacement {
        replacement_pcs.push(cursor);
        cursor += instruction.encoded_len(cursor);
    }

    // Resolve every target before restructuring.
    let mut old_targets: Vec<Vec<i64>> = Vec::with_capacity(len);
    for (index, instruction) in instructions.iter().enumerate() {
        let mut targets = Vec::new();
        resolve_targets(instruction, old_pcs[index], &mut targets);
        old_targets.push(targets);
    }
    let mut replacement_targets: Vec<Vec<i64>> = Vec::with_capacity(replacement_len);
    for (index, instruction) in replacement.iter().enumerate() {
        let mut targets = Vec::new();
        resolve_targets(instruction, replacement_pcs[index], &mut targets);
        replacement_targets.push(targets);
    }

    // Assemble the new instruction/target sequence.
    let mut combined: Vec<(Instruction<'a>, Vec<i64>)> =
        Vec::with_capacity(len - removed_count + replacement_len);
    for (index, targets) in old_targets.iter_mut().enumerate().take(start) {
        combined.push((instructions[index].clone(), core::mem::take(targets)));
    }
    for (instruction, targets) in replacement.into_iter().zip(replacement_targets) {
        combined.push((instruction, targets));
    }
    for (index, targets) in old_targets.iter_mut().enumerate().skip(end) {
        combined.push((instructions[index].clone(), core::mem::take(targets)));
    }

    // Recompute positions. Switch padding depends on the pc, so a surviving
    // instruction's length can change; mapping target *instructions* (not raw
    // byte deltas) keeps this correct.
    let mut new_pcs = Vec::with_capacity(combined.len());
    let mut new_len = 0usize;
    for (instruction, _) in &combined {
        new_pcs.push(new_len);
        new_len += instruction.encoded_len(new_len);
    }

    // Map a pre-edit absolute pc to a post-edit absolute pc. A target on an
    // instruction boundary follows that instruction; a target inside the
    // removed range moves to the first instruction after the replacement; a
    // target inside an instruction (malformed input) is shifted by the byte
    // delta as a best effort.
    let byte_delta = new_len as i64 - old_len as i64;
    let map_pc = |t: i64| -> i64 {
        let boundary = if t >= 0 {
            old_pcs.binary_search(&(t as usize)).ok()
        } else {
            None
        };
        if let Some(old_index) = boundary {
            let new_index = if old_index < start {
                old_index
            } else if old_index < end {
                start + replacement_len
            } else {
                old_index - removed_count + replacement_len
            };
            return new_pcs
                .get(new_index)
                .map_or(new_len as i64, |&pc| pc as i64);
        }
        (t + byte_delta).clamp(0, new_len as i64)
    };

    for (index, (instruction, targets)) in combined.iter_mut().enumerate() {
        for target in targets.iter_mut() {
            *target = map_pc(*target);
        }
        write_targets(instruction, new_pcs[index], targets)?;
    }

    let removed = instructions[start..end].to_vec();
    *instructions = combined
        .into_iter()
        .map(|(instruction, _)| instruction)
        .collect();
    Ok(removed)
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
    use super::{Instruction, decode_impl, encode_impl, encode_into_impl, splice};
    use crate::bytecode::{Opcode, OperandKind};
    use crate::constant_pool::ConstantRef;
    use crate::error::Error;
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

    fn nop<'a>() -> Instruction<'a> {
        Instruction::None(Opcode::Nop)
    }

    fn ret<'a>() -> Instruction<'a> {
        Instruction::None(Opcode::Return)
    }

    #[test]
    fn splice_insert_before_target_shifts_branch() {
        // goto -> [2]; nop; return
        let mut instructions = vec![Instruction::Branch(Opcode::Goto, 4), nop(), ret()];
        splice(&mut instructions, 1..1, vec![nop()]).expect("splice");
        assert_eq!(instructions[0], Instruction::Branch(Opcode::Goto, 5));
    }

    #[test]
    fn splice_insert_after_target_leaves_branch() {
        // goto -> [1]; return
        let mut instructions = vec![Instruction::Branch(Opcode::Goto, 3), ret()];
        splice(&mut instructions, 2..2, vec![nop()]).expect("splice");
        assert_eq!(instructions[0], Instruction::Branch(Opcode::Goto, 3));
    }

    #[test]
    fn splice_remove_target_redirects_to_next() {
        // goto -> [2]; nop; nop; return
        let mut instructions = vec![Instruction::Branch(Opcode::Goto, 4), nop(), nop(), ret()];
        let removed = splice(&mut instructions, 1..3, Vec::new()).expect("splice");
        assert_eq!(removed.len(), 2);
        // The removed range held the target, so the branch now reaches the
        // instruction that followed it.
        assert_eq!(instructions[0], Instruction::Branch(Opcode::Goto, 3));
    }

    #[test]
    fn splice_replace_target_redirects_after_replacement() {
        // goto -> [1]; ldc_w; return
        let mut instructions = vec![
            Instruction::Branch(Opcode::Goto, 3),
            Instruction::ConstantWide(Opcode::LdcW, ConstantRef::new(1)),
            ret(),
        ];
        splice(&mut instructions, 1..2, vec![nop(), nop()]).expect("splice");
        assert_eq!(instructions[0], Instruction::Branch(Opcode::Goto, 5));
    }

    #[test]
    fn splice_supplied_branch_references_original_code() {
        // nop; nop; return  -- a supplied goto whose offset (2) targets the
        // original `return` is rebased to the post-edit layout.
        let mut instructions = vec![nop(), nop(), ret()];
        let goto = Instruction::Branch(Opcode::Goto, 2);
        splice(&mut instructions, 0..0, vec![goto]).expect("splice");
        assert_eq!(instructions[0], Instruction::Branch(Opcode::Goto, 5));
    }

    #[test]
    fn splice_remaps_switch_targets() {
        // nop; tableswitch { default -> [2]; case 0 -> [2]; case 1 -> [3] }; return; return
        let switch = Instruction::TableSwitch {
            default: 23,
            low: 0,
            high: 1,
            offsets: vec![23, 24],
            padding: Vec::new(),
        };
        let mut instructions = vec![nop(), switch, ret(), ret()];
        splice(&mut instructions, 0..0, vec![nop()]).expect("splice");
        let new_pcs = {
            let mut pcs = Vec::new();
            let mut pc = 0usize;
            for instruction in &instructions {
                pcs.push(pc);
                pc += instruction.encoded_len(pc);
            }
            pcs
        };
        // The switch is now at index 2; its targets (default + two cases) must
        // land on the two trailing `return`s.
        let base = new_pcs[2] as i64;
        let Instruction::TableSwitch {
            default, offsets, ..
        } = &instructions[2]
        else {
            panic!("expected a tableswitch");
        };
        let targets: Vec<i64> = std::iter::once(i64::from(*default))
            .chain(offsets.iter().map(|offset| i64::from(*offset)))
            .map(|offset| base + offset)
            .collect();
        assert_eq!(
            targets,
            vec![new_pcs[3] as i64, new_pcs[3] as i64, new_pcs[4] as i64]
        );
    }

    #[test]
    fn splice_overflow_is_an_error_and_leaves_input_unchanged() {
        // Inserting the filler between the `goto` and its target pushes the
        // relative offset past `i16::MAX`.
        let mut instructions = vec![Instruction::Branch(Opcode::Goto, 3), ret()];
        let filler: Vec<Instruction<'static>> = vec![nop(); 40_000];
        let result = splice(&mut instructions, 1..1, filler);
        assert!(matches!(
            result,
            Err(Error::RelativeOffsetOutOfRange { .. })
        ));
        assert_eq!(instructions.len(), 2);
    }

    #[test]
    fn splice_reports_out_of_range() {
        let mut instructions = vec![nop(), ret()];
        assert!(matches!(
            splice(&mut instructions, 0..5, Vec::new()),
            Err(Error::InstructionIndexOutOfRange { index: 5, len: 2 })
        ));
        assert_eq!(instructions.len(), 2);
    }

    #[test]
    fn splice_delete_to_end_clamps_target() {
        // goto -> [1]; nop  -- deleting the target leaves only the goto, whose
        // target is clamped to the end of the code.
        let mut instructions = vec![Instruction::Branch(Opcode::Goto, 3), nop()];
        splice(&mut instructions, 1..2, Vec::new()).expect("splice");
        assert_eq!(instructions, vec![Instruction::Branch(Opcode::Goto, 3)]);
    }
}
