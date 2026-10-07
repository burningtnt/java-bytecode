//! Access and property flag sets (JVMS §4.1, §4.5, §4.6, §4.7.6, §4.7.25).
//!
//! Every set uses [`bitflags::Flags::from_bits_retain`] so unknown bits survive
//! a parse/write round-trip untouched.

bitflags::bitflags! {
    /// Class and interface access flags (Table 4.1-B).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ClassAccessFlags: u16 {
        /// `ACC_PUBLIC`.
        const PUBLIC = 0x0001;
        /// `ACC_FINAL`.
        const FINAL = 0x0010;
        /// `ACC_SUPER`.
        const SUPER = 0x0020;
        /// `ACC_INTERFACE`.
        const INTERFACE = 0x0200;
        /// `ACC_ABSTRACT`.
        const ABSTRACT = 0x0400;
        /// `ACC_SYNTHETIC`.
        const SYNTHETIC = 0x1000;
        /// `ACC_ANNOTATION`.
        const ANNOTATION = 0x2000;
        /// `ACC_ENUM`.
        const ENUM = 0x4000;
        /// `ACC_MODULE`.
        const MODULE = 0x8000;
    }

    /// Field access flags (Table 4.5-A).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct FieldAccessFlags: u16 {
        /// `ACC_PUBLIC`.
        const PUBLIC = 0x0001;
        /// `ACC_PRIVATE`.
        const PRIVATE = 0x0002;
        /// `ACC_PROTECTED`.
        const PROTECTED = 0x0004;
        /// `ACC_STATIC`.
        const STATIC = 0x0008;
        /// `ACC_FINAL`.
        const FINAL = 0x0010;
        /// `ACC_VOLATILE`.
        const VOLATILE = 0x0040;
        /// `ACC_TRANSIENT`.
        const TRANSIENT = 0x0080;
        /// `ACC_SYNTHETIC`.
        const SYNTHETIC = 0x1000;
        /// `ACC_ENUM`.
        const ENUM = 0x4000;
    }

    /// Method access flags (Table 4.6-A).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MethodAccessFlags: u16 {
        /// `ACC_PUBLIC`.
        const PUBLIC = 0x0001;
        /// `ACC_PRIVATE`.
        const PRIVATE = 0x0002;
        /// `ACC_PROTECTED`.
        const PROTECTED = 0x0004;
        /// `ACC_STATIC`.
        const STATIC = 0x0008;
        /// `ACC_FINAL`.
        const FINAL = 0x0010;
        /// `ACC_SYNCHRONIZED`.
        const SYNCHRONIZED = 0x0020;
        /// `ACC_BRIDGE`.
        const BRIDGE = 0x0040;
        /// `ACC_VARARGS`.
        const VARARGS = 0x0080;
        /// `ACC_NATIVE`.
        const NATIVE = 0x0100;
        /// `ACC_ABSTRACT`.
        const ABSTRACT = 0x0400;
        /// `ACC_STRICT`.
        const STRICT = 0x0800;
        /// `ACC_SYNTHETIC`.
        const SYNTHETIC = 0x1000;
    }

    /// `InnerClasses` class access flags (Table 4.7.6-A).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct InnerClassAccessFlags: u16 {
        /// `ACC_PUBLIC`.
        const PUBLIC = 0x0001;
        /// `ACC_PRIVATE`.
        const PRIVATE = 0x0002;
        /// `ACC_PROTECTED`.
        const PROTECTED = 0x0004;
        /// `ACC_STATIC`.
        const STATIC = 0x0008;
        /// `ACC_FINAL`.
        const FINAL = 0x0010;
        /// `ACC_INTERFACE`.
        const INTERFACE = 0x0200;
        /// `ACC_ABSTRACT`.
        const ABSTRACT = 0x0400;
        /// `ACC_SYNTHETIC`.
        const SYNTHETIC = 0x1000;
        /// `ACC_ANNOTATION`.
        const ANNOTATION = 0x2000;
        /// `ACC_ENUM`.
        const ENUM = 0x4000;
    }

    /// `Module` attribute access flags (Table 4.7.25-A).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ModuleAccessFlags: u16 {
        /// `ACC_OPEN`.
        const OPEN = 0x0020;
        /// `ACC_SYNTHETIC`.
        const SYNTHETIC = 0x1000;
        /// `ACC_MANDATED`.
        const MANDATED = 0x8000;
    }

    /// `MethodParameters` parameter access flags (Table 4.7.24-A).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MethodParameterAccessFlags: u16 {
        /// `ACC_FINAL`.
        const FINAL = 0x0010;
        /// `ACC_SYNTHETIC`.
        const SYNTHETIC = 0x1000;
        /// `ACC_MANDATED`.
        const MANDATED = 0x8000;
    }

    /// `Module.requires` flags (Table 4.7.25-B).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ModuleRequiresFlags: u16 {
        /// `ACC_TRANSITIVE`.
        const TRANSITIVE = 0x0020;
        /// `ACC_STATIC_PHASE`.
        const STATIC_PHASE = 0x0040;
        /// `ACC_SYNTHETIC`.
        const SYNTHETIC = 0x1000;
        /// `ACC_MANDATED`.
        const MANDATED = 0x8000;
    }

    /// `Module.exports` / `Module.opens` flags (Table 4.7.25-C).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ModuleExportsFlags: u16 {
        /// `ACC_SYNTHETIC`.
        const SYNTHETIC = 0x1000;
        /// `ACC_MANDATED`.
        const MANDATED = 0x8000;
    }
}
