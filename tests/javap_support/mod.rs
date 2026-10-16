//! A javap `-p -c -v`-compatible renderer for `java-bytecode`.
//!
//! This lives under `tests/` (not in the library) and is only used by the
//! `tests/javap.rs` comparison test. It is a direct port of the relevant parts
//! of `com.sun.tools.javap` (JDK 27), so that its output can be compared
//! byte-for-byte with `javap -p -c -v`.

mod attributes;
mod class;
mod constants;
mod java_float;
mod signature;
mod writer;

pub use class::disassemble;

use java_bytecode::{ClassFile, Constant, ConstantRef};

use writer::Writer;

/// File metadata reproduced in javap's `Classfile` header.
#[derive(Debug, Clone)]
pub struct Header {
    /// The canonical path javap prints after `Classfile `.
    pub path: String,
    /// File size in bytes.
    pub size: u32,
    /// Last-modified time, in milliseconds since the Unix epoch.
    pub last_modified_millis: i64,
    /// SHA-256 digest of the raw class bytes.
    pub digest: [u8; 32],
}

pub(crate) struct Javap<'a, 'b> {
    pub(crate) w: Writer,
    pub(crate) class: &'a ClassFile<'b>,
    pub(crate) header: &'a Header,
    pub(crate) this_class_index: u16,
}

impl<'a, 'b> Javap<'a, 'b> {
    pub(crate) fn new(class: &'a ClassFile<'b>, header: &'a Header) -> Self {
        Self {
            w: Writer::new(),
            class,
            header,
            this_class_index: raw(class.this_class),
        }
    }

    pub(crate) fn major_version(&self) -> u16 {
        self.class.major_version
    }

    /// The constant at `index`, or `None` for a placeholder/out-of-range slot.
    pub(crate) fn cp(&self, index: u16) -> Option<&Constant<'b>> {
        self.class
            .constant_pool
            .get(unsafe { ConstantRef::from_raw(index) })
    }

    /// The decoded (non-escaped) `Utf8` value at `index`.
    pub(crate) fn utf8(&self, index: u16) -> String {
        match self.cp(index) {
            Some(Constant::Utf8(value)) => value.to_string_lossy(),
            _ => String::new(),
        }
    }

    /// The internal class name referenced by a `CONSTANT_Class` index.
    pub(crate) fn class_name(&self, index: u16) -> String {
        match self.cp(index) {
            Some(Constant::Class { name_index }) => self.utf8(raw(*name_index)),
            _ => String::new(),
        }
    }

    /// The internal name of this class.
    pub(crate) fn this_class_name(&self) -> String {
        self.class_name(self.this_class_index)
    }
}

/// Extracts the raw constant-pool index from a reference.
pub(crate) fn raw(reference: ConstantRef<'_>) -> u16 {
    unsafe { reference.into_raw() }
}

pub(crate) fn get_java_name(name: &str) -> String {
    name.replace('/', ".")
}

/// A flag location, matching `java.lang.reflect.AccessFlag.Location`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Loc {
    Class,
    Field,
    Method,
    InnerClass,
}

/// `(mask, ACC_NAME, is_source_modifier)` in `AccessFlag` declaration order.
fn flag_table(loc: Loc) -> &'static [(u16, &'static str, bool)] {
    const PUBLIC: (u16, &str, bool) = (0x0001, "PUBLIC", true);
    const PRIVATE: (u16, &str, bool) = (0x0002, "PRIVATE", true);
    const PROTECTED: (u16, &str, bool) = (0x0004, "PROTECTED", true);
    const STATIC: (u16, &str, bool) = (0x0008, "STATIC", true);
    const FINAL: (u16, &str, bool) = (0x0010, "FINAL", true);
    const SYNCHRONIZED: (u16, &str, bool) = (0x0020, "SYNCHRONIZED", true);
    const VOLATILE: (u16, &str, bool) = (0x0040, "VOLATILE", true);
    const BRIDGE: (u16, &str, bool) = (0x0040, "BRIDGE", false);
    const TRANSIENT: (u16, &str, bool) = (0x0080, "TRANSIENT", true);
    const VARARGS: (u16, &str, bool) = (0x0080, "VARARGS", false);
    const NATIVE: (u16, &str, bool) = (0x0100, "NATIVE", true);
    const INTERFACE: (u16, &str, bool) = (0x0200, "INTERFACE", false);
    const ABSTRACT: (u16, &str, bool) = (0x0400, "ABSTRACT", true);
    const STRICT: (u16, &str, bool) = (0x0800, "STRICT", true);
    const SYNTHETIC: (u16, &str, bool) = (0x1000, "SYNTHETIC", false);
    const ANNOTATION: (u16, &str, bool) = (0x2000, "ANNOTATION", false);
    const ENUM: (u16, &str, bool) = (0x4000, "ENUM", false);

    match loc {
        Loc::Class => &[
            PUBLIC,
            FINAL,
            (0x0020, "SUPER", false),
            INTERFACE,
            ABSTRACT,
            SYNTHETIC,
            ANNOTATION,
            ENUM,
            (0x8000, "MODULE", false),
        ],
        Loc::Field => &[
            PUBLIC, PRIVATE, PROTECTED, STATIC, FINAL, VOLATILE, TRANSIENT, SYNTHETIC, ENUM,
        ],
        Loc::Method => &[
            PUBLIC,
            PRIVATE,
            PROTECTED,
            STATIC,
            FINAL,
            SYNCHRONIZED,
            BRIDGE,
            VARARGS,
            NATIVE,
            ABSTRACT,
            STRICT,
            SYNTHETIC,
        ],
        Loc::InnerClass => &[
            PUBLIC, PRIVATE, PROTECTED, STATIC, FINAL, INTERFACE, ABSTRACT, SYNTHETIC, ANNOTATION,
            ENUM,
        ],
    }
}

/// Returns the bit positions valid for `loc` at `major` (matching
/// `AccessFlag.Location.flagsMask`).
pub(crate) fn valid_mask(loc: Loc, major: u16) -> u16 {
    let mut mask = 0u16;
    for &(bit, name, _) in flag_table(loc) {
        if name == "STRICT" && major > 60 {
            continue;
        }
        mask |= bit;
    }
    mask
}

/// The flag names present in `mask` (already restricted to valid bits).
pub(crate) fn known_flag_names(loc: Loc, mask: u16, major: u16) -> (Vec<&'static str>, u16) {
    let valid = valid_mask(loc, major);
    let mut names = Vec::new();
    let mut used = 0u16;
    for &(bit, name, _) in flag_table(loc) {
        if name == "STRICT" && major > 60 {
            continue;
        }
        if mask & valid & bit != 0 {
            names.push(name);
            used |= bit;
        }
    }
    (names, mask & !used)
}

/// The source modifiers for a location, in declaration order.
pub(crate) fn modifier_names(loc: Loc, mask: u16, major: u16) -> Vec<String> {
    let mut out = Vec::new();
    for &(bit, name, source) in flag_table(loc) {
        if name == "STRICT" && major > 60 {
            continue;
        }
        if mask & bit != 0 && source {
            out.push(if name == "STRICT" {
                "strictfp".to_owned()
            } else {
                name.to_ascii_lowercase()
            });
        }
    }
    out
}

/// Class modifiers: `ACC_ABSTRACT` is suppressed for interfaces.
pub(crate) fn class_modifiers(mask: u16, major: u16) -> Vec<String> {
    let mut items = modifier_names(Loc::Class, mask, major);
    if mask & 0x0200 != 0 {
        items.retain(|m| m != "abstract");
    }
    items
}

/// Inner-class modifiers: `ACC_ABSTRACT` is suppressed for interfaces.
pub(crate) fn inner_class_modifiers(mask: u16, major: u16) -> Vec<String> {
    let mut items = modifier_names(Loc::InnerClass, mask, major);
    if mask & 0x0200 != 0 {
        items.retain(|m| m != "abstract");
    }
    items
}

fn location_name(loc: Loc) -> &'static str {
    match loc {
        Loc::Class => "CLASS",
        Loc::Field => "FIELD",
        Loc::Method => "METHOD",
        Loc::InnerClass => "INNER_CLASS",
    }
}

impl Javap<'_, '_> {
    fn cffv_name(&self) -> String {
        let major = self.class.major_version;
        let minor = self.class.minor_version;
        let effective = if !(45..=71).contains(&major) || (major >= 56 && minor != 0) {
            71
        } else {
            major
        };
        format!("RELEASE_{}", effective.saturating_sub(44))
    }

    /// Reports (and masks off) flag bits invalid for `loc`, mirroring
    /// `BasicWriter.maskToAccessFlagsReportUnknown`. Returns the valid bits.
    pub(crate) fn report_flags(&mut self, loc: Loc, mask: u16) -> u16 {
        let valid = valid_mask(loc, self.class.major_version);
        let invalid = mask & !valid;
        if invalid != 0 {
            let message = format!(
                "Access Flags: Unmatched bit position 0x{invalid:x} for location {} for class file format {}",
                location_name(loc),
                self.cffv_name()
            );
            self.w.report(&message);
        }
        mask & valid
    }
}
