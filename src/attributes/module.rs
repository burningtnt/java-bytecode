//! `Module` attribute (JVMS §4.7.25).

use crate::access_flags::{ModuleAccessFlags, ModuleExportsFlags, ModuleRequiresFlags};
use crate::constant_pool::ConstantRef;
use crate::error::Result;
use crate::io::{Reader, Writer};

/// A `Module.requires` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleRequires<'a> {
    /// Index of the required module.
    pub requires_index: ConstantRef<'a>,
    /// Requires flags.
    pub requires_flags: ModuleRequiresFlags,
    /// Index of the required module version, or 0.
    pub requires_version_index: ConstantRef<'a>,
}

/// A `Module.exports` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleExports<'a> {
    /// Index of the exported package.
    pub exports_index: ConstantRef<'a>,
    /// Exports flags.
    pub exports_flags: ModuleExportsFlags,
    /// Modules the package is exported to (empty means unqualified).
    pub exports_to_index: Vec<ConstantRef<'a>>,
}

/// A `Module.opens` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleOpens<'a> {
    /// Index of the opened package.
    pub opens_index: ConstantRef<'a>,
    /// Opens flags.
    pub opens_flags: ModuleExportsFlags,
    /// Modules the package is opened to (empty means unqualified).
    pub opens_to_index: Vec<ConstantRef<'a>>,
}

/// A `Module.provides` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleProvides<'a> {
    /// Index of the provided service interface.
    pub provides_index: ConstantRef<'a>,
    /// Indices of the implementation classes.
    pub provides_with_index: Vec<ConstantRef<'a>>,
}

/// `Module` attribute (JVMS §4.7.25).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleAttribute<'a> {
    /// Index of the module name.
    pub module_name_index: ConstantRef<'a>,
    /// Module flags.
    pub module_flags: ModuleAccessFlags,
    /// Index of the module version, or 0.
    pub module_version_index: ConstantRef<'a>,
    /// Dependencies.
    pub requires: Vec<ModuleRequires<'a>>,
    /// Exported packages.
    pub exports: Vec<ModuleExports<'a>>,
    /// Opened packages.
    pub opens: Vec<ModuleOpens<'a>>,
    /// Service interfaces used.
    pub uses: Vec<ConstantRef<'a>>,
    /// Services provided.
    pub provides: Vec<ModuleProvides<'a>>,
}

impl<'a> ModuleAttribute<'a> {
    /// Parses the attribute body.
    pub fn parse(reader: &mut Reader<'a>) -> Result<Self> {
        let module_name_index = ConstantRef::new(reader.read_u2()?);
        let module_flags = ModuleAccessFlags::from_bits_retain(reader.read_u2()?);
        let module_version_index = ConstantRef::new(reader.read_u2()?);

        let requires_count = reader.read_u2()?;
        let mut requires = Vec::with_capacity(requires_count as usize);
        for _ in 0..requires_count {
            requires.push(ModuleRequires {
                requires_index: ConstantRef::new(reader.read_u2()?),
                requires_flags: ModuleRequiresFlags::from_bits_retain(reader.read_u2()?),
                requires_version_index: ConstantRef::new(reader.read_u2()?),
            });
        }

        let exports_count = reader.read_u2()?;
        let mut exports = Vec::with_capacity(exports_count as usize);
        for _ in 0..exports_count {
            let exports_index = ConstantRef::new(reader.read_u2()?);
            let exports_flags = ModuleExportsFlags::from_bits_retain(reader.read_u2()?);
            exports.push(ModuleExports {
                exports_index,
                exports_flags,
                exports_to_index: read_ref_list(reader)?,
            });
        }

        let opens_count = reader.read_u2()?;
        let mut opens = Vec::with_capacity(opens_count as usize);
        for _ in 0..opens_count {
            let opens_index = ConstantRef::new(reader.read_u2()?);
            let opens_flags = ModuleExportsFlags::from_bits_retain(reader.read_u2()?);
            opens.push(ModuleOpens {
                opens_index,
                opens_flags,
                opens_to_index: read_ref_list(reader)?,
            });
        }

        let uses = read_ref_list(reader)?;

        let provides_count = reader.read_u2()?;
        let mut provides = Vec::with_capacity(provides_count as usize);
        for _ in 0..provides_count {
            let provides_index = ConstantRef::new(reader.read_u2()?);
            provides.push(ModuleProvides {
                provides_index,
                provides_with_index: read_ref_list(reader)?,
            });
        }

        Ok(Self {
            module_name_index,
            module_flags,
            module_version_index,
            requires,
            exports,
            opens,
            uses,
            provides,
        })
    }

    /// Writes the attribute body.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_cp_u2(self.module_name_index.get());
        writer.write_u2(self.module_flags.bits());
        writer.write_cp_u2(self.module_version_index.get());

        writer.write_u2_len(self.requires.len())?;
        for entry in &self.requires {
            writer.write_cp_u2(entry.requires_index.get());
            writer.write_u2(entry.requires_flags.bits());
            writer.write_cp_u2(entry.requires_version_index.get());
        }

        writer.write_u2_len(self.exports.len())?;
        for entry in &self.exports {
            writer.write_cp_u2(entry.exports_index.get());
            writer.write_u2(entry.exports_flags.bits());
            write_ref_list(writer, &entry.exports_to_index)?;
        }

        writer.write_u2_len(self.opens.len())?;
        for entry in &self.opens {
            writer.write_cp_u2(entry.opens_index.get());
            writer.write_u2(entry.opens_flags.bits());
            write_ref_list(writer, &entry.opens_to_index)?;
        }

        write_ref_list(writer, &self.uses)?;

        writer.write_u2_len(self.provides.len())?;
        for entry in &self.provides {
            writer.write_cp_u2(entry.provides_index.get());
            write_ref_list(writer, &entry.provides_with_index)?;
        }

        Ok(())
    }
}

fn read_ref_list<'a>(reader: &mut Reader<'a>) -> Result<Vec<ConstantRef<'a>>> {
    let count = reader.read_u2()?;
    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(ConstantRef::new(reader.read_u2()?));
    }
    Ok(values)
}

fn write_ref_list(writer: &mut Writer, values: &[ConstantRef<'_>]) -> Result<()> {
    writer.write_u2_len(values.len())?;
    for value in values {
        writer.write_cp_u2(value.get());
    }
    Ok(())
}
