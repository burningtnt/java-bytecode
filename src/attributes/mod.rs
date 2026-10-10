//! Attributes (JVMS §4.7).
//!
//! An attribute is a `(name_index, body)` pair. Known attribute names are
//! decoded into typed bodies ([`AttributeBody::Code`], [`AttributeBody::Module`],
//! …); anything else is preserved verbatim as [`AttributeBody::Raw`]. Unknown
//! attributes and unknown flag bits therefore round-trip byte for byte.

mod annotation_attrs;
mod annotations;
mod bootstrap_methods;
mod code;
mod debug;
mod inner_classes;
mod method_parameters;
mod module;
mod record;
mod simple;
mod stack_map;

pub use annotation_attrs::{
    AnnotationDefaultAttribute, AnnotationsAttribute, LocalVarTargetEntry,
    ParameterAnnotationsAttribute, TargetInfo, TypeAnnotation, TypeAnnotationsAttribute, TypePath,
    TypePathEntry,
};
pub use annotations::{Annotation, ElementValue, ElementValuePair};
pub use bootstrap_methods::{BootstrapMethod, BootstrapMethodsAttribute};
pub use code::{CodeAttribute, ExceptionTableEntry, Instructions};
pub use debug::{
    LineNumber, LineNumberTableAttribute, LocalVariable, LocalVariableTableAttribute,
    LocalVariableType, LocalVariableTypeTableAttribute,
};
pub use inner_classes::{InnerClass, InnerClassesAttribute};
pub use method_parameters::{MethodParameter, MethodParametersAttribute};
pub use module::{ModuleAttribute, ModuleExports, ModuleOpens, ModuleProvides, ModuleRequires};
pub use record::{RecordAttribute, RecordComponent};
pub use simple::{
    ConstantValueAttribute, EnclosingMethodAttribute, ExceptionsAttribute,
    ModuleMainClassAttribute, ModulePackagesAttribute, NestHostAttribute, NestMembersAttribute,
    PermittedSubclassesAttribute, SignatureAttribute, SourceDebugExtensionAttribute,
    SourceFileAttribute,
};
pub use stack_map::{StackMapFrame, StackMapTableAttribute, VerificationType};

use crate::constant_pool::{ConstantPool, ConstantRef};
use crate::error::Result;
use crate::io::{Reader, Writer};
use crate::mutf8::Mutf8;

/// An `attribute_info` structure (JVMS §4.7).
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute<'a> {
    /// Index of the `CONSTANT_Utf8` attribute name.
    pub name_index: ConstantRef<'a>,
    /// The attribute body.
    pub body: AttributeBody<'a>,
}

/// The interpreted (or uninterpreted) body of an [`Attribute`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AttributeBody<'a> {
    /// Uninterpreted attribute bytes.
    Raw(&'a [u8]),
    /// `ConstantValue`.
    ConstantValue(ConstantValueAttribute<'a>),
    /// `Code`.
    Code(CodeAttribute<'a>),
    /// `Exceptions`.
    Exceptions(ExceptionsAttribute<'a>),
    /// `InnerClasses`.
    InnerClasses(InnerClassesAttribute<'a>),
    /// `EnclosingMethod`.
    EnclosingMethod(EnclosingMethodAttribute<'a>),
    /// `Synthetic` (empty body).
    Synthetic,
    /// `Deprecated` (empty body).
    Deprecated,
    /// `Signature`.
    Signature(SignatureAttribute<'a>),
    /// `SourceFile`.
    SourceFile(SourceFileAttribute<'a>),
    /// `SourceDebugExtension`.
    SourceDebugExtension(SourceDebugExtensionAttribute<'a>),
    /// `LineNumberTable`.
    LineNumberTable(LineNumberTableAttribute),
    /// `LocalVariableTable`.
    LocalVariableTable(LocalVariableTableAttribute<'a>),
    /// `LocalVariableTypeTable`.
    LocalVariableTypeTable(LocalVariableTypeTableAttribute<'a>),
    /// `BootstrapMethods`.
    BootstrapMethods(BootstrapMethodsAttribute<'a>),
    /// `MethodParameters`.
    MethodParameters(MethodParametersAttribute<'a>),
    /// `Module`.
    Module(ModuleAttribute<'a>),
    /// `ModulePackages`.
    ModulePackages(ModulePackagesAttribute<'a>),
    /// `ModuleMainClass`.
    ModuleMainClass(ModuleMainClassAttribute<'a>),
    /// `NestHost`.
    NestHost(NestHostAttribute<'a>),
    /// `NestMembers`.
    NestMembers(NestMembersAttribute<'a>),
    /// `Record`.
    Record(RecordAttribute<'a>),
    /// `PermittedSubclasses`.
    PermittedSubclasses(PermittedSubclassesAttribute<'a>),
    /// `RuntimeVisibleAnnotations` / `RuntimeInvisibleAnnotations`.
    Annotations(AnnotationsAttribute<'a>),
    /// `RuntimeVisibleParameterAnnotations` / `RuntimeInvisibleParameterAnnotations`.
    ParameterAnnotations(ParameterAnnotationsAttribute<'a>),
    /// `RuntimeVisibleTypeAnnotations` / `RuntimeInvisibleTypeAnnotations`.
    TypeAnnotations(TypeAnnotationsAttribute<'a>),
    /// `AnnotationDefault`.
    AnnotationDefault(AnnotationDefaultAttribute<'a>),
    /// `StackMapTable`.
    StackMapTable(StackMapTableAttribute<'a>),
}

impl<'a> Attribute<'a> {
    /// Creates a raw attribute.
    #[must_use]
    pub const fn raw(name_index: ConstantRef<'a>, info: &'a [u8]) -> Self {
        Self {
            name_index,
            body: AttributeBody::Raw(info),
        }
    }

    /// Parses one attribute from `reader`, resolving its name and any nested
    /// attributes against `pool`.
    pub fn parse(reader: &mut Reader<'a>, pool: &ConstantPool<'a>) -> Result<Self> {
        let name_index = ConstantRef::new(reader.read_u2()?);
        let length = reader.read_u4()? as usize;
        let bytes = reader.read_bytes(length)?;
        let name = pool.utf8(name_index).unwrap_or(Mutf8::new(&[]));
        let body = AttributeBody::parse(name, bytes, pool)?;
        Ok(Self { name_index, body })
    }

    /// Resolves the attribute name against `pool`.
    pub fn resolved_name(&self, pool: &ConstantPool<'a>) -> Result<String> {
        pool.utf8_str(self.name_index)
    }

    /// Serializes this attribute, including its header.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_cp_u2(self.name_index.get());
        let mut body = writer.fork();
        self.body.encode(&mut body)?;
        let bytes = body.into_bytes();
        writer.write_u4_len(bytes.len())?;
        writer.write_bytes(&bytes);
        Ok(())
    }
}

impl<'a> AttributeBody<'a> {
    /// Decodes a body given the attribute `name`.
    ///
    /// This interprets the raw `info` bytes of an attribute as read from a class
    /// file. To build a typed body from scratch, prefer the type's own
    /// constructor (for example [`CodeAttribute::from_instructions`]); fall back
    /// to [`AttributeBody::parse`] only when you already have `info` bytes.
    pub fn parse(
        name: Mutf8<'a>,
        bytes: &'a [u8],
        pool: &ConstantPool<'a>,
    ) -> Result<AttributeBody<'a>> {
        let body = match name.as_bytes() {
            b"ConstantValue" => AttributeBody::ConstantValue(parse_body(
                bytes,
                "ConstantValue",
                ConstantValueAttribute::parse,
            )?),
            b"Code" => {
                AttributeBody::Code(parse_body_pool(bytes, "Code", pool, CodeAttribute::parse)?)
            }
            b"Exceptions" => AttributeBody::Exceptions(parse_body(
                bytes,
                "Exceptions",
                ExceptionsAttribute::parse,
            )?),
            b"InnerClasses" => AttributeBody::InnerClasses(parse_body(
                bytes,
                "InnerClasses",
                InnerClassesAttribute::parse,
            )?),
            b"EnclosingMethod" => AttributeBody::EnclosingMethod(parse_body(
                bytes,
                "EnclosingMethod",
                EnclosingMethodAttribute::parse,
            )?),
            b"Synthetic" => AttributeBody::Synthetic,
            b"Deprecated" => AttributeBody::Deprecated,
            b"Signature" => {
                AttributeBody::Signature(parse_body(bytes, "Signature", SignatureAttribute::parse)?)
            }
            b"SourceFile" => AttributeBody::SourceFile(parse_body(
                bytes,
                "SourceFile",
                SourceFileAttribute::parse,
            )?),
            b"SourceDebugExtension" => AttributeBody::SourceDebugExtension(parse_body(
                bytes,
                "SourceDebugExtension",
                SourceDebugExtensionAttribute::parse,
            )?),
            b"LineNumberTable" => AttributeBody::LineNumberTable(parse_body(
                bytes,
                "LineNumberTable",
                LineNumberTableAttribute::parse,
            )?),
            b"LocalVariableTable" => AttributeBody::LocalVariableTable(parse_body(
                bytes,
                "LocalVariableTable",
                LocalVariableTableAttribute::parse,
            )?),
            b"LocalVariableTypeTable" => AttributeBody::LocalVariableTypeTable(parse_body(
                bytes,
                "LocalVariableTypeTable",
                LocalVariableTypeTableAttribute::parse,
            )?),
            b"BootstrapMethods" => AttributeBody::BootstrapMethods(parse_body(
                bytes,
                "BootstrapMethods",
                BootstrapMethodsAttribute::parse,
            )?),
            b"MethodParameters" => AttributeBody::MethodParameters(parse_body(
                bytes,
                "MethodParameters",
                MethodParametersAttribute::parse,
            )?),
            b"Module" => {
                AttributeBody::Module(parse_body(bytes, "Module", ModuleAttribute::parse)?)
            }
            b"ModulePackages" => AttributeBody::ModulePackages(parse_body(
                bytes,
                "ModulePackages",
                ModulePackagesAttribute::parse,
            )?),
            b"ModuleMainClass" => AttributeBody::ModuleMainClass(parse_body(
                bytes,
                "ModuleMainClass",
                ModuleMainClassAttribute::parse,
            )?),
            b"NestHost" => {
                AttributeBody::NestHost(parse_body(bytes, "NestHost", NestHostAttribute::parse)?)
            }
            b"NestMembers" => AttributeBody::NestMembers(parse_body(
                bytes,
                "NestMembers",
                NestMembersAttribute::parse,
            )?),
            b"Record" => AttributeBody::Record(parse_body_pool(
                bytes,
                "Record",
                pool,
                RecordAttribute::parse,
            )?),
            b"PermittedSubclasses" => AttributeBody::PermittedSubclasses(parse_body(
                bytes,
                "PermittedSubclasses",
                PermittedSubclassesAttribute::parse,
            )?),
            b"RuntimeVisibleAnnotations" | b"RuntimeInvisibleAnnotations" => {
                AttributeBody::Annotations(parse_body(
                    bytes,
                    "Annotations",
                    AnnotationsAttribute::parse,
                )?)
            }
            b"RuntimeVisibleParameterAnnotations" | b"RuntimeInvisibleParameterAnnotations" => {
                AttributeBody::ParameterAnnotations(parse_body(
                    bytes,
                    "ParameterAnnotations",
                    ParameterAnnotationsAttribute::parse,
                )?)
            }
            b"RuntimeVisibleTypeAnnotations" | b"RuntimeInvisibleTypeAnnotations" => {
                AttributeBody::TypeAnnotations(parse_body(
                    bytes,
                    "TypeAnnotations",
                    TypeAnnotationsAttribute::parse,
                )?)
            }
            b"AnnotationDefault" => AttributeBody::AnnotationDefault(parse_body(
                bytes,
                "AnnotationDefault",
                AnnotationDefaultAttribute::parse,
            )?),
            b"StackMapTable" => AttributeBody::StackMapTable(parse_body(
                bytes,
                "StackMapTable",
                StackMapTableAttribute::parse,
            )?),
            _ => AttributeBody::Raw(bytes),
        };
        Ok(body)
    }

    /// Appends the encoded body to `out`.
    pub fn encode(&self, writer: &mut Writer) -> Result<()> {
        match self {
            Self::Raw(info) => writer.write_bytes(info),
            Self::ConstantValue(a) => a.write_to(writer)?,
            Self::Code(a) => a.write_to(writer)?,
            Self::Exceptions(a) => a.write_to(writer)?,
            Self::InnerClasses(a) => a.write_to(writer)?,
            Self::EnclosingMethod(a) => a.write_to(writer)?,
            Self::Synthetic | Self::Deprecated => {}
            Self::Signature(a) => a.write_to(writer)?,
            Self::SourceFile(a) => a.write_to(writer)?,
            Self::SourceDebugExtension(a) => a.write_to(writer)?,
            Self::LineNumberTable(a) => a.write_to(writer)?,
            Self::LocalVariableTable(a) => a.write_to(writer)?,
            Self::LocalVariableTypeTable(a) => a.write_to(writer)?,
            Self::BootstrapMethods(a) => a.write_to(writer)?,
            Self::MethodParameters(a) => a.write_to(writer)?,
            Self::Module(a) => a.write_to(writer)?,
            Self::ModulePackages(a) => a.write_to(writer)?,
            Self::ModuleMainClass(a) => a.write_to(writer)?,
            Self::NestHost(a) => a.write_to(writer)?,
            Self::NestMembers(a) => a.write_to(writer)?,
            Self::Record(a) => a.write_to(writer)?,
            Self::PermittedSubclasses(a) => a.write_to(writer)?,
            Self::Annotations(a) => a.write_to(writer)?,
            Self::ParameterAnnotations(a) => a.write_to(writer)?,
            Self::TypeAnnotations(a) => a.write_to(writer)?,
            Self::AnnotationDefault(a) => a.write_to(writer)?,
            Self::StackMapTable(a) => a.write_to(writer)?,
        }
        Ok(())
    }
}

impl<'a> AttributeBody<'a> {
    /// Returns the `Code` attribute body, if this is one.
    #[must_use]
    pub fn as_code(&self) -> Option<&CodeAttribute<'a>> {
        match self {
            Self::Code(code) => Some(code),
            _ => None,
        }
    }

    /// Returns the `Module` attribute body, if this is one.
    #[must_use]
    pub fn as_module(&self) -> Option<&ModuleAttribute<'a>> {
        match self {
            Self::Module(module) => Some(module),
            _ => None,
        }
    }

    /// Returns the `BootstrapMethods` attribute body, if this is one.
    #[must_use]
    pub fn as_bootstrap_methods(&self) -> Option<&BootstrapMethodsAttribute<'a>> {
        match self {
            Self::BootstrapMethods(methods) => Some(methods),
            _ => None,
        }
    }
}

fn parse_body<'a, T, F>(bytes: &'a [u8], context: &'static str, f: F) -> Result<T>
where
    F: FnOnce(&mut Reader<'a>) -> Result<T>,
{
    let mut reader = Reader::new(bytes);
    let value = f(&mut reader)?;
    reader.finish(context)?;
    Ok(value)
}

fn parse_body_pool<'a, T, F>(
    bytes: &'a [u8],
    context: &'static str,
    pool: &ConstantPool<'a>,
    f: F,
) -> Result<T>
where
    F: FnOnce(&mut Reader<'a>, &ConstantPool<'a>) -> Result<T>,
{
    let mut reader = Reader::new(bytes);
    let value = f(&mut reader, pool)?;
    reader.finish(context)?;
    Ok(value)
}

/// Parses `count` consecutive attributes.
pub fn parse_attributes<'a>(
    reader: &mut Reader<'a>,
    pool: &ConstantPool<'a>,
    count: u16,
) -> Result<Vec<Attribute<'a>>> {
    let mut attributes = Vec::with_capacity(count as usize);
    for _ in 0..count {
        attributes.push(Attribute::parse(reader, pool)?);
    }
    Ok(attributes)
}

/// Writes `count` followed by every attribute.
pub fn write_attributes(writer: &mut Writer, attributes: &[Attribute<'_>]) -> Result<()> {
    writer.write_u2_len(attributes.len())?;
    for attribute in attributes {
        attribute.write_to(writer)?;
    }
    Ok(())
}
