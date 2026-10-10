//! `java-bytecode` — read, edit and write Java class files.
//!
//! The entry point is [`ClassFile`], which borrows the underlying bytes
//! (`ClassFile<'a>`) and keeps all variable-length payloads as `&'a [u8]`.
//! Ownership of every byte of data stays with the caller for the entire
//! lifetime of the parsed value, so parsing never copies the input and editing
//! never allocates new string or byte data behind your back.
//!
//! The crate covers the class-file structures of JVMS chapter 4: the constant
//! pool, fields, methods, all thirty predefined attributes (including
//! annotations, type annotations and `StackMapTable`), and the full instruction
//! set of chapter 6. It performs structural parsing but no bytecode
//! verification.
//!
//! # Reading
//!
//! ```
//! # fn main() -> Result<(), java_bytecode::Error> {
//! # let bytes: &[u8] = &[];
//! # if bytes.is_empty() { return Ok(()); }
//! let class = java_bytecode::ClassFile::parse(bytes)?;
//! println!("{} v{}.{}", class.this_class_name()?, class.major_version, class.minor_version);
//! # Ok(())
//! # }
//! ```
//!
//! # Editing
//!
//! Because the model is borrowed, new strings and bytes are passed in by the
//! caller as `&'a` values. The default serialization ([`ClassFile::to_bytes`])
//! preserves constant-pool indices; [`ClassFile::to_bytes_rebuilt`] compacts
//! and deduplicates the pool, rewriting every reference (including inside
//! bytecode).
//!
//! ```
//! use java_bytecode::ClassFile;
//!
//! fn rename_first_method<'a>(
//!     bytes: &'a [u8],
//!     new_name: &'a str,
//! ) -> Result<Vec<u8>, java_bytecode::Error> {
//!     let mut class = ClassFile::parse(bytes)?;
//!     let index = class.constant_pool.intern_utf8(new_name)?;
//!     if let Some(method) = class.methods.first_mut() {
//!         method.name_index = index;
//!     }
//!     class.to_bytes()
//! }
//! ```
//!
//! # Creating a class from scratch
//!
//! There is no dedicated builder: a [`ClassFile`] is assembled from its public
//! fields. Fill the [`ConstantPool`] first (its `intern`/`intern_utf8` methods
//! return the [`ConstantRef`]s every other structure refers to), then build the
//! class, its methods and their attributes as plain struct literals.
//!
//! [`CodeAttribute`] is the one exception: its bytecode field is private, so a
//! `Code` attribute is built from an instruction list with
//! [`CodeAttribute::from_instructions`] and attached with
//! [`CodeAttribute::into_attribute`] (or edited in place through
//! [`CodeAttribute::instructions_mut`]). Every borrowed payload must outlive the
//! class file; `'static` strings and literals satisfy that automatically.
//!
//! ```
//! use java_bytecode::{
//!     ClassAccessFlags, ClassFile, CodeAttribute, Constant, ConstantPool, Instruction,
//!     MethodAccessFlags, MethodInfo, Opcode,
//! };
//!
//! fn build() -> Result<Vec<u8>, java_bytecode::Error> {
//!     let mut pool = ConstantPool::new();
//!
//!     let this_name = pool.intern_utf8("example/Sender")?;
//!     let this_class = pool.intern(Constant::Class { name_index: this_name })?;
//!     let super_name = pool.intern_utf8("java/lang/Object")?;
//!     let super_class = pool.intern(Constant::Class { name_index: super_name })?;
//!     let send_name = pool.intern_utf8("send")?;
//!     let send_descriptor =
//!         pool.intern_utf8("(Ljava/nio/ByteBuffer;Ljava/net/SocketAddress;)I")?;
//!     let code_name = pool.intern_utf8("Code")?;
//!
//!     // Build the method body from instructions: `iconst_0; ireturn`.
//!     let code = CodeAttribute::from_instructions(
//!         1, // max_stack
//!         3, // max_locals: `this` + two arguments
//!         [Instruction::None(Opcode::Iconst0), Instruction::None(Opcode::Ireturn)],
//!     )?;
//!
//!     let send = MethodInfo {
//!         access_flags: MethodAccessFlags::PUBLIC,
//!         name_index: send_name,
//!         descriptor_index: send_descriptor,
//!         attributes: vec![code.into_attribute(code_name)],
//!     };
//!
//!     let class = ClassFile {
//!         minor_version: 0,
//!         major_version: 52,
//!         constant_pool: pool,
//!         access_flags: ClassAccessFlags::PUBLIC | ClassAccessFlags::SUPER,
//!         this_class,
//!         super_class,
//!         interfaces: Vec::new(),
//!         fields: Vec::new(),
//!         methods: vec![send],
//!         attributes: Vec::new(),
//!     };
//!
//!     class.to_bytes()
//! }
//!
//! # build().unwrap();
//! ```
//!
//! A method that must not carry code (`abstract` or `native`) is created the same
//! way with an empty `attributes` vector, so no synthetic `Code` attribute is
//! needed. See `tests/create.rs` for these cases.
//!
//! # Modifying bytecode (instrumentation)
//!
//! Editing a method's code goes through
//! [`CodeAttribute::instructions_mut`], which returns an [`Instructions`] view
//! scoped to the class's constant pool. It dereferences to
//! `Vec<Instruction<'a>>`; edit the vector and call [`Instructions::apply`] to
//! re-encode and write the bytes back. New constants are added with
//! [`ConstantPool::intern`] / [`ConstantPool::intern_utf8`]. You remain
//! responsible for rebasing branch offsets, the exception table and the
//! `StackMapTable` when the code size changes.
//!
//! The [`bytecode`] module documents the full workflow, including a complete,
//! runnable instrumentation example.
//!
//! [`ConstantPool::intern`]: ConstantPool::intern
//! [`ConstantPool::intern_utf8`]: ConstantPool::intern_utf8
//!
//! See `examples/inspect.rs` and `examples/corpus.rs` for runnable tools.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]

pub mod access_flags;
pub mod attributes;
pub mod bytecode;
pub mod class_file;
pub mod constant_pool;
pub mod descriptor;
pub mod error;
pub mod field;
pub mod io;
pub mod method;
pub mod mutf8;
pub mod names;
pub mod rebuild;

pub use access_flags::{
    ClassAccessFlags, FieldAccessFlags, InnerClassAccessFlags, MethodAccessFlags,
    MethodParameterAccessFlags, ModuleAccessFlags, ModuleExportsFlags, ModuleRequiresFlags,
};
pub use attributes::{
    Annotation, AnnotationDefaultAttribute, AnnotationsAttribute, Attribute, AttributeBody,
    BootstrapMethod, BootstrapMethodsAttribute, CodeAttribute, ConstantValueAttribute,
    ElementValue, ElementValuePair, EnclosingMethodAttribute, ExceptionTableEntry,
    ExceptionsAttribute, InnerClass, InnerClassesAttribute, Instructions, LineNumber,
    LineNumberTableAttribute, LocalVarTargetEntry, LocalVariable, LocalVariableTableAttribute,
    LocalVariableType, LocalVariableTypeTableAttribute, MethodParameter, MethodParametersAttribute,
    ModuleAttribute, ModuleExports, ModuleMainClassAttribute, ModuleOpens, ModulePackagesAttribute,
    ModuleProvides, ModuleRequires, NestHostAttribute, NestMembersAttribute,
    ParameterAnnotationsAttribute, PermittedSubclassesAttribute, RecordAttribute, RecordComponent,
    SignatureAttribute, SourceDebugExtensionAttribute, SourceFileAttribute, StackMapFrame,
    StackMapTableAttribute, TargetInfo, TypeAnnotation, TypeAnnotationsAttribute, TypePath,
    TypePathEntry, VerificationType,
};
pub use bytecode::{Instruction, Opcode, OperandKind};
pub use class_file::{ClassFile, MAGIC};
pub use constant_pool::{Constant, ConstantPool, ConstantRef, ConstantTag};
pub use descriptor::{BaseType, FieldType, MethodDescriptor};
pub use error::{Error, Result};
pub use field::FieldInfo;
pub use method::MethodInfo;
pub use mutf8::Mutf8;
