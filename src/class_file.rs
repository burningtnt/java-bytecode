//! The top-level `ClassFile` structure (JVMS §4.1).

use crate::access_flags::ClassAccessFlags;
use crate::attributes::{Attribute, parse_attributes, write_attributes};
use crate::constant_pool::{ConstantPool, ConstantRef};
use crate::error::{Error, Result};
use crate::field::FieldInfo;
use crate::io::{Reader, Writer};
use crate::method::MethodInfo;

/// The class file magic number, `0xCAFEBABE`.
pub const MAGIC: u32 = 0xCAFE_BABE;

/// A parsed class file.
///
/// Every variable-length payload is borrowed from the input slice passed to
/// [`ClassFile::parse`]; ownership of those bytes remains with the caller for
/// the lifetime `'a`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassFile<'a> {
    /// Class file minor version.
    pub minor_version: u16,
    /// Class file major version.
    pub major_version: u16,
    /// The constant pool table.
    pub constant_pool: ConstantPool<'a>,
    /// Class access and property flags.
    pub access_flags: ClassAccessFlags,
    /// Index of this class.
    pub this_class: ConstantRef<'a>,
    /// Index of the super class (0 for `java/lang/Object` and `module-info`).
    pub super_class: ConstantRef<'a>,
    /// Implemented interfaces.
    pub interfaces: Vec<ConstantRef<'a>>,
    /// Declared fields.
    pub fields: Vec<FieldInfo<'a>>,
    /// Declared methods.
    pub methods: Vec<MethodInfo<'a>>,
    /// Class attributes.
    pub attributes: Vec<Attribute<'a>>,
}

impl<'a> ClassFile<'a> {
    /// Parses a complete class file from `bytes`.
    ///
    /// No semantic verification is performed; structural problems (bad magic,
    /// truncation, unknown tags) are still reported as errors. Trailing bytes
    /// after the structure are rejected so that a successful parse can always
    /// be written back byte for byte.
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        let mut reader = Reader::new(bytes);

        let magic = reader.read_u4()?;
        if magic != MAGIC {
            return Err(Error::InvalidMagic { found: magic });
        }

        let minor_version = reader.read_u2()?;
        let major_version = reader.read_u2()?;

        let constant_pool_count = reader.read_u2()?;
        let constant_pool = ConstantPool::parse(&mut reader, constant_pool_count)?;

        let access_flags = ClassAccessFlags::from_bits_retain(reader.read_u2()?);
        let this_class = ConstantRef::new(reader.read_u2()?);
        let super_class = ConstantRef::new(reader.read_u2()?);

        let interfaces_count = reader.read_u2()?;
        let mut interfaces = Vec::with_capacity(interfaces_count as usize);
        for _ in 0..interfaces_count {
            interfaces.push(ConstantRef::new(reader.read_u2()?));
        }

        let fields_count = reader.read_u2()?;
        let mut fields = Vec::with_capacity(fields_count as usize);
        for _ in 0..fields_count {
            fields.push(FieldInfo::parse(&mut reader, &constant_pool)?);
        }

        let methods_count = reader.read_u2()?;
        let mut methods = Vec::with_capacity(methods_count as usize);
        for _ in 0..methods_count {
            methods.push(MethodInfo::parse(&mut reader, &constant_pool)?);
        }

        let attributes_count = reader.read_u2()?;
        let attributes = parse_attributes(&mut reader, &constant_pool, attributes_count)?;

        reader.finish("class file")?;

        Ok(Self {
            minor_version,
            major_version,
            constant_pool,
            access_flags,
            this_class,
            super_class,
            interfaces,
            fields,
            methods,
            attributes,
        })
    }

    /// Serializes this class file into `writer`.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u4(MAGIC);
        writer.write_u2(self.minor_version);
        writer.write_u2(self.major_version);
        self.constant_pool.write_to(writer)?;
        writer.write_u2(self.access_flags.bits());
        writer.write_cp_u2(self.this_class.get());
        writer.write_cp_u2(self.super_class.get());

        writer.write_u2_len(self.interfaces.len())?;
        for interface in &self.interfaces {
            writer.write_cp_u2(interface.get());
        }

        writer.write_u2_len(self.fields.len())?;
        for field in &self.fields {
            field.write_to(writer)?;
        }

        writer.write_u2_len(self.methods.len())?;
        for method in &self.methods {
            method.write_to(writer)?;
        }

        write_attributes(writer, &self.attributes)?;
        Ok(())
    }

    /// Serializes this class file and returns the bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new();
        self.write_to(&mut writer)?;
        Ok(writer.into_bytes())
    }

    /// Resolves this class's binary name (dot-separated).
    pub fn this_class_name(&self) -> Result<String> {
        self.constant_pool.class_name(self.this_class)
    }

    /// Resolves the super class name, or `None` for `java/lang/Object` and
    /// `module-info`.
    pub fn super_class_name(&self) -> Result<Option<String>> {
        if self.super_class.get() == 0 {
            Ok(None)
        } else {
            Ok(Some(self.constant_pool.class_name(self.super_class)?))
        }
    }

    fn method_index(&self, name: &str, descriptor: &str) -> Option<usize> {
        self.methods.iter().position(|method| {
            self.constant_pool
                .utf8_str(method.name_index)
                .ok()
                .as_deref()
                == Some(name)
                && self
                    .constant_pool
                    .utf8_str(method.descriptor_index)
                    .ok()
                    .as_deref()
                    == Some(descriptor)
        })
    }

    /// Finds a method by decoded name and descriptor.
    #[must_use]
    pub fn find_method(&self, name: &str, descriptor: &str) -> Option<&MethodInfo<'a>> {
        self.methods.get(self.method_index(name, descriptor)?)
    }

    /// Finds a method by decoded name and descriptor.
    #[must_use]
    pub fn find_method_mut(&mut self, name: &str, descriptor: &str) -> Option<&mut MethodInfo<'a>> {
        let index = self.method_index(name, descriptor)?;
        self.methods.get_mut(index)
    }

    fn field_index(&self, name: &str, descriptor: &str) -> Option<usize> {
        self.fields.iter().position(|field| {
            self.constant_pool
                .utf8_str(field.name_index)
                .ok()
                .as_deref()
                == Some(name)
                && self
                    .constant_pool
                    .utf8_str(field.descriptor_index)
                    .ok()
                    .as_deref()
                    == Some(descriptor)
        })
    }

    /// Finds a field by decoded name and descriptor.
    #[must_use]
    pub fn find_field(&self, name: &str, descriptor: &str) -> Option<&FieldInfo<'a>> {
        self.fields.get(self.field_index(name, descriptor)?)
    }

    /// Finds a field by decoded name and descriptor.
    #[must_use]
    pub fn find_field_mut(&mut self, name: &str, descriptor: &str) -> Option<&mut FieldInfo<'a>> {
        let index = self.field_index(name, descriptor)?;
        self.fields.get_mut(index)
    }
}
