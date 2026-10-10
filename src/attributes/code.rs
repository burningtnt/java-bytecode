//! `Code` attribute (JVMS §4.7.3).

use core::ops::{Deref, DerefMut};

use super::{Attribute, AttributeBody, parse_attributes, write_attributes};
use crate::bytecode::{self, Instruction};
use crate::constant_pool::{ConstantPool, ConstantRef};
use crate::error::Result;
use crate::io::{Reader, Writer};

/// A `Code` exception-table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceptionTableEntry<'a> {
    /// Start of the protected range, as a bytecode offset.
    pub start_pc: u16,
    /// End of the protected range, as a bytecode offset.
    pub end_pc: u16,
    /// Handler entry point, as a bytecode offset.
    pub handler_pc: u16,
    /// Caught class index, or 0 for `finally`/`catch (Throwable)`.
    pub catch_type: ConstantRef<'a>,
}

/// The raw bytecode storage of a [`CodeAttribute`].
///
/// It borrows the original bytes until the code is edited, after which it owns
/// the re-encoded bytes.
#[derive(Debug, Clone)]
enum CodeBytes<'a> {
    Borrowed(&'a [u8]),
    Owned(Box<[u8]>),
}

impl CodeBytes<'_> {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Borrowed(bytes) => bytes,
            Self::Owned(bytes) => bytes,
        }
    }
}

impl PartialEq for CodeBytes<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for CodeBytes<'_> {}

/// `Code` attribute (JVMS §4.7.3).
///
/// The raw bytecode is private; use [`CodeAttribute::instructions_mut`] to edit
/// it, which accounts for the constant-pool scope.
///
/// To create a `Code` attribute from scratch, use [`CodeAttribute::new`] (an
/// empty body) or [`CodeAttribute::from_instructions`], then optionally attach
/// it with [`CodeAttribute::into_attribute`]. See the crate-level docs and
/// `tests/create.rs`.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeAttribute<'a> {
    /// Maximum operand stack depth.
    pub max_stack: u16,
    /// Number of local variable slots.
    pub max_locals: u16,
    code: CodeBytes<'a>,
    /// Exception handler table.
    pub exception_table: Vec<ExceptionTableEntry<'a>>,
    /// Code sub-attributes.
    pub attributes: Vec<Attribute<'a>>,
}

impl<'a> CodeAttribute<'a> {
    /// Parses the attribute body, resolving nested attributes against `pool`.
    pub fn parse(reader: &mut Reader<'a>, pool: &ConstantPool<'a>) -> Result<Self> {
        let max_stack = reader.read_u2()?;
        let max_locals = reader.read_u2()?;
        let code_length = reader.read_u4()? as usize;
        let code = CodeBytes::Borrowed(reader.read_bytes(code_length)?);

        let exception_table_length = reader.read_u2()?;
        let mut exception_table = Vec::with_capacity(exception_table_length as usize);
        for _ in 0..exception_table_length {
            exception_table.push(ExceptionTableEntry {
                start_pc: reader.read_u2()?,
                end_pc: reader.read_u2()?,
                handler_pc: reader.read_u2()?,
                catch_type: ConstantRef::new(reader.read_u2()?),
            });
        }

        let attribute_count = reader.read_u2()?;
        let attributes = parse_attributes(reader, pool, attribute_count)?;

        Ok(Self {
            max_stack,
            max_locals,
            code,
            exception_table,
            attributes,
        })
    }

    /// Creates an empty `Code` attribute with no instructions and no exception
    /// handlers.
    ///
    /// `max_stack` is the maximum operand-stack depth; `max_locals` is the
    /// number of local-variable slots, which must include `this` for instance
    /// methods. Populate the body with [`CodeAttribute::set_instructions`] or
    /// [`CodeAttribute::instructions_mut`].
    #[must_use]
    pub fn new(max_stack: u16, max_locals: u16) -> Self {
        Self {
            max_stack,
            max_locals,
            code: CodeBytes::Owned(Box::<[u8]>::default()),
            exception_table: Vec::new(),
            attributes: Vec::new(),
        }
    }

    /// Builds a `Code` attribute by encoding `instructions`.
    ///
    /// This is the from-scratch counterpart of
    /// [`CodeAttribute::instructions_mut`]: it lets you assemble a bytecode
    /// body as a list of [`Instruction`]s without hand-writing the attribute
    /// bytes. The instructions' constant-pool indices must belong to the pool
    /// of the class this attribute will be attached to; the shared lifetime
    /// `'a` enforces this.
    ///
    /// # Errors
    ///
    /// Fails if an instruction cannot be encoded (for example an `ldc` whose
    /// constant-pool index does not fit in one byte).
    pub fn from_instructions(
        max_stack: u16,
        max_locals: u16,
        instructions: impl IntoIterator<Item = Instruction<'a>>,
    ) -> Result<Self> {
        let mut code = Self::new(max_stack, max_locals);
        code.set_instructions(instructions)?;
        Ok(code)
    }

    /// Encodes `instructions` and installs them, replacing any existing
    /// bytecode.
    ///
    /// The exception table and sub-attributes are left untouched. This is the
    /// encoding counterpart of [`Instructions::apply`].
    ///
    /// # Errors
    ///
    /// As for [`CodeAttribute::from_instructions`].
    pub fn set_instructions(
        &mut self,
        instructions: impl IntoIterator<Item = Instruction<'a>>,
    ) -> Result<()> {
        let instructions: Vec<Instruction<'a>> = instructions.into_iter().collect();
        let encoded = bytecode::encode_impl(&instructions)?;
        self.code = CodeBytes::Owned(encoded.into_boxed_slice());
        Ok(())
    }

    /// Wraps this attribute into an [`Attribute`] whose name is `name_index`
    /// (the interned `Utf8` `"Code"`).
    #[must_use]
    pub fn into_attribute(self, name_index: ConstantRef<'a>) -> Attribute<'a> {
        Attribute {
            name_index,
            body: AttributeBody::Code(self),
        }
    }

    /// Returns the raw bytecode bytes.
    ///
    /// Interpreting them requires the (unsafe) [`bytecode::decode`]; prefer
    /// [`CodeAttribute::instructions_mut`].
    #[must_use]
    pub fn code(&self) -> &[u8] {
        self.code.as_slice()
    }

    /// Decodes the bytecode into an owned instruction list.
    ///
    /// This is the read-only counterpart of
    /// [`CodeAttribute::instructions_mut`]. It allocates a fresh
    /// `Vec<Instruction<'a>>`; to edit the code and write it back, use
    /// [`CodeAttribute::instructions_mut`] instead.
    pub fn instructions(&self) -> Result<Vec<Instruction<'a>>> {
        bytecode::decode_impl(self.code.as_slice())
    }

    /// Decodes the bytecode into a mutable [`Instructions`] view.
    ///
    /// The returned view dereferences to `Vec<Instruction<'a>>`; edit it and
    /// call [`Instructions::apply`] to re-encode and write the bytes back.
    ///
    /// This is the safe path for modifying bytecode.
    pub fn instructions_mut(&mut self) -> Result<Instructions<'a, '_>> {
        let instructions = bytecode::decode_impl(self.code.as_slice())?;
        Ok(Instructions {
            code: &mut self.code,
            instructions,
        })
    }

    /// Replaces the raw bytecode with borrowed `code`.
    ///
    /// The bytes are installed verbatim; nothing is decoded, validated or
    /// re-encoded.
    ///
    /// # Safety
    ///
    /// `code` must be a valid JVM bytecode stream and every constant-pool index
    /// it contains must be valid **for this class file's constant pool**.
    /// Passing bytes with out-of-range indices or from another class leaves the
    /// file malformed. Prefer [`CodeAttribute::instructions_mut`] plus
    /// [`Instructions::apply`], which re-encode with the correct scope.
    #[allow(unsafe_code)]
    pub unsafe fn set_code(&mut self, code: &'a [u8]) {
        self.code = CodeBytes::Borrowed(code);
    }

    /// Replaces the raw bytecode with owned `code`.
    ///
    /// The bytes are installed verbatim; nothing is decoded, validated or
    /// re-encoded.
    ///
    /// # Safety
    ///
    /// `code` must be a valid JVM bytecode stream and every constant-pool index
    /// it contains must be valid **for this class file's constant pool**.
    /// Passing bytes with out-of-range indices or from another class leaves the
    /// file malformed. Prefer [`CodeAttribute::instructions_mut`] plus
    /// [`Instructions::apply`], which re-encode with the correct scope.
    #[allow(unsafe_code)]
    pub unsafe fn set_code_owned(&mut self, code: Box<[u8]>) {
        self.code = CodeBytes::Owned(code);
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2(self.max_stack);
        writer.write_u2(self.max_locals);
        let code = self.code.as_slice();
        if writer.has_remap() {
            let instructions = bytecode::decode_impl(code)?;
            let mut code_writer = writer.fork();
            bytecode::encode_into_impl(&mut code_writer, &instructions)?;
            let bytes = code_writer.into_bytes();
            writer.write_u4_len(bytes.len())?;
            writer.write_bytes(&bytes);
        } else {
            writer.write_u4_len(code.len())?;
            writer.write_bytes(code);
        }

        writer.write_u2_len(self.exception_table.len())?;
        for entry in &self.exception_table {
            writer.write_u2(entry.start_pc);
            writer.write_u2(entry.end_pc);
            writer.write_u2(entry.handler_pc);
            writer.write_cp_u2(entry.catch_type.get());
        }

        write_attributes(writer, &self.attributes)
    }
}

/// A mutable view of a method's decoded instructions.
///
/// It dereferences to `Vec<Instruction<'a>>`, so the instruction list can be
/// edited directly. Call [`Instructions::apply`] to re-encode the instructions
/// and store the resulting bytes back into the [`CodeAttribute`].
///
/// The first lifetime `'a` is the constant-pool scope carried by the
/// instructions (needed to intern or reference constants); the second is the
/// borrow of the owning `CodeAttribute`.
#[derive(Debug)]
pub struct Instructions<'a, 'code> {
    code: &'code mut CodeBytes<'a>,
    instructions: Vec<Instruction<'a>>,
}

impl<'a, 'code> Instructions<'a, 'code> {
    /// Encodes the instructions and writes the bytes back into the `Code`
    /// attribute.
    ///
    /// # Errors
    ///
    /// Fails if an instruction cannot be encoded (for example an `ldc` whose
    /// constant-pool index does not fit in one byte).
    pub fn apply(&mut self) -> Result<()> {
        let encoded = bytecode::encode_impl(&self.instructions)?;
        *self.code = CodeBytes::Owned(encoded.into_boxed_slice());
        Ok(())
    }

    /// Returns the instructions without applying them.
    #[must_use]
    pub fn as_slice(&self) -> &[Instruction<'a>] {
        &self.instructions
    }

    /// Consumes the view and returns the instructions.
    #[must_use]
    pub fn into_vec(self) -> Vec<Instruction<'a>> {
        self.instructions
    }
}

impl<'a> Deref for Instructions<'a, '_> {
    type Target = Vec<Instruction<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.instructions
    }
}

impl DerefMut for Instructions<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.instructions
    }
}

#[cfg(test)]
mod tests {
    use super::{AttributeBody, CodeAttribute};
    use crate::Instruction;
    use crate::bytecode::Opcode;
    use crate::constant_pool::ConstantRef;
    use crate::error::Error;

    #[test]
    fn new_creates_an_empty_attribute() {
        let code = CodeAttribute::new(2, 4);
        assert_eq!(code.max_stack, 2);
        assert_eq!(code.max_locals, 4);
        assert!(code.code().is_empty());
        assert!(code.instructions().unwrap().is_empty());
        assert!(code.exception_table.is_empty());
        assert!(code.attributes.is_empty());
    }

    #[test]
    fn from_instructions_encodes_and_decodes() {
        let instructions = [
            Instruction::None(Opcode::Iconst0),
            Instruction::None(Opcode::Ireturn),
        ];
        let code = CodeAttribute::from_instructions(1, 3, instructions).unwrap();
        assert_eq!(code.code(), &[0x03, 0xAC]);
        let opcodes: Vec<_> = code
            .instructions()
            .unwrap()
            .iter()
            .map(Instruction::opcode)
            .collect();
        assert_eq!(opcodes, [Opcode::Iconst0, Opcode::Ireturn]);
    }

    #[test]
    fn set_instructions_replaces_the_body() {
        let mut code = CodeAttribute::new(0, 0);
        code.set_instructions([Instruction::None(Opcode::Return)])
            .unwrap();
        assert_eq!(code.code(), &[0xB1]);

        code.set_instructions([
            Instruction::None(Opcode::Nop),
            Instruction::None(Opcode::Return),
        ])
        .unwrap();
        assert_eq!(code.code(), &[0x00, 0xB1]);
    }

    #[test]
    fn set_instructions_reports_encode_errors() {
        let mut code = CodeAttribute::new(1, 0);
        // `ldc` cannot reference a constant-pool index above 255.
        let result =
            code.set_instructions([Instruction::Constant(Opcode::Ldc, ConstantRef::new(300))]);
        assert!(matches!(result, Err(Error::ValueTooLarge { .. })));
    }

    #[test]
    fn into_attribute_wraps_the_body() {
        let name = ConstantRef::new(1);
        let attribute = CodeAttribute::new(0, 0).into_attribute(name);
        assert_eq!(attribute.name_index, name);
        assert!(matches!(attribute.body, AttributeBody::Code(_)));
    }
}
