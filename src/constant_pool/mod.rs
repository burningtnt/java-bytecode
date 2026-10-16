//! The constant pool (JVMS §4.4).

mod tags;

pub use tags::ConstantTag;

use core::marker::PhantomData;

use crate::error::{Error, Result};
use crate::io::{Reader, Writer};
use crate::mutf8::{self, Mutf8};

/// An opaque reference to an entry in a specific constant pool.
///
/// The `'a` parameter ties the reference to the [`ClassFile`](crate::ClassFile)
/// / [`ConstantPool`] that produced it. The `PhantomData<fn(&'a ()) -> &'a ()>`
/// field makes `ConstantRef` *invariant* in `'a`, so the compiler will not let
/// you use an index from one class file with another.
///
/// The value `0` is a valid reference and denotes "no entry" where the JVMS
/// uses a zero index (for example `super_class` of `java/lang/Object` or
/// `catch_type` of a `finally` handler).
///
/// A `ConstantRef` is obtained from [`ConstantPool::push`],
/// [`ConstantPool::intern`] or [`ConstantPool::intern_utf8`]. The only way to
/// turn it into or from a raw `u16` is the `unsafe` [`from_raw`](Self::from_raw)
/// / [`into_raw`](Self::into_raw) pair.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstantRef<'a> {
    index: u16,
    _marker: PhantomData<fn(&'a ()) -> &'a ()>,
}

impl<'a> ConstantRef<'a> {
    /// Creates a reference from a raw constant-pool index.
    ///
    /// # Safety
    ///
    /// The caller must ensure `index` is a valid entry in the *same* constant
    /// pool the returned reference is used with, and that the chosen `'a`
    /// matches that pool. Passing an index from a different class file or an
    /// out-of-range value is not checked by the type system and will produce an
    /// incorrect class file.
    #[allow(unsafe_code)]
    pub const unsafe fn from_raw(index: u16) -> Self {
        Self {
            index,
            _marker: PhantomData,
        }
    }

    /// Returns the underlying raw constant-pool index.
    ///
    /// # Safety
    ///
    /// This erases the type-level scope check. The returned index must still be
    /// interpreted as belonging to the constant pool this reference came from.
    #[allow(unsafe_code)]
    pub const unsafe fn into_raw(self) -> u16 {
        self.index
    }

    /// Crate-internal safe constructor.
    #[inline]
    pub(crate) const fn new(index: u16) -> Self {
        Self {
            index,
            _marker: PhantomData,
        }
    }

    /// Crate-internal safe accessor.
    #[inline]
    pub(crate) const fn get(self) -> u16 {
        self.index
    }
}

/// A single `cp_info` entry (JVMS §4.4).
///
/// `Utf8` keeps the raw modified-UTF-8 bytes borrowed from the class file; use
/// [`ConstantPool::utf8_str`] or [`crate::mutf8::decode`] to obtain a Rust
/// string. `Float` and `Double` store the raw IEEE-754 bit pattern so that a
/// parse/write round-trip is byte-exact even for `NaN` payloads.
#[derive(Debug, Clone, PartialEq)]
pub enum Constant<'a> {
    /// `CONSTANT_Utf8` (tag 1).
    Utf8(Mutf8<'a>),
    /// `CONSTANT_Integer` (tag 3).
    Integer(i32),
    /// `CONSTANT_Float` (tag 4); raw `f32` bits.
    Float(u32),
    /// `CONSTANT_Long` (tag 5); occupies two constant-pool slots.
    Long(i64),
    /// `CONSTANT_Double` (tag 6); raw `f64` bits; occupies two slots.
    Double(u64),
    /// `CONSTANT_Class` (tag 7).
    Class {
        /// Index of the `Utf8` internal class name.
        name_index: ConstantRef<'a>,
    },
    /// `CONSTANT_String` (tag 8).
    String {
        /// Index of the `Utf8` value.
        string_index: ConstantRef<'a>,
    },
    /// `CONSTANT_Fieldref` (tag 9).
    Fieldref {
        /// Index of the declaring class.
        class_index: ConstantRef<'a>,
        /// Index of the name-and-type.
        name_and_type_index: ConstantRef<'a>,
    },
    /// `CONSTANT_Methodref` (tag 10).
    Methodref {
        /// Index of the declaring class.
        class_index: ConstantRef<'a>,
        /// Index of the name-and-type.
        name_and_type_index: ConstantRef<'a>,
    },
    /// `CONSTANT_InterfaceMethodref` (tag 11).
    InterfaceMethodref {
        /// Index of the declaring interface.
        class_index: ConstantRef<'a>,
        /// Index of the name-and-type.
        name_and_type_index: ConstantRef<'a>,
    },
    /// `CONSTANT_NameAndType` (tag 12).
    NameAndType {
        /// Index of the `Utf8` name.
        name_index: ConstantRef<'a>,
        /// Index of the `Utf8` descriptor.
        descriptor_index: ConstantRef<'a>,
    },
    /// `CONSTANT_MethodHandle` (tag 15).
    MethodHandle {
        /// Reference kind (1..=9).
        reference_kind: u8,
        /// Index of the referenced constant.
        reference_index: ConstantRef<'a>,
    },
    /// `CONSTANT_MethodType` (tag 16).
    MethodType {
        /// Index of the `Utf8` method descriptor.
        descriptor_index: ConstantRef<'a>,
    },
    /// `CONSTANT_Dynamic` (tag 17).
    Dynamic {
        /// Index into the `BootstrapMethods` attribute (not a constant-pool
        /// index).
        bootstrap_method_attr_index: u16,
        /// Index of the name-and-type.
        name_and_type_index: ConstantRef<'a>,
    },
    /// `CONSTANT_InvokeDynamic` (tag 18).
    InvokeDynamic {
        /// Index into the `BootstrapMethods` attribute (not a constant-pool
        /// index).
        bootstrap_method_attr_index: u16,
        /// Index of the name-and-type.
        name_and_type_index: ConstantRef<'a>,
    },
    /// `CONSTANT_Module` (tag 19).
    Module {
        /// Index of the `Utf8` module name.
        name_index: ConstantRef<'a>,
    },
    /// `CONSTANT_Package` (tag 20).
    Package {
        /// Index of the `Utf8` package name.
        name_index: ConstantRef<'a>,
    },
    /// An unusable constant-pool slot.
    ///
    /// Occupies index 0 of every constant pool and the second slot of a
    /// `Long`/`Double`. It is never serialized and [`Constant::tag`] returns the
    /// sentinel `0`.
    Invalid,
}

fn read_ref<'a>(reader: &mut Reader<'a>) -> Result<ConstantRef<'a>> {
    Ok(ConstantRef::new(reader.read_u2()?))
}

impl Constant<'_> {
    /// Returns the wire tag byte of this constant.
    ///
    /// [`Constant::Invalid`] is not a real entry and returns the sentinel `0`.
    #[must_use]
    pub const fn tag(&self) -> u8 {
        match self {
            Self::Utf8(_) => 1,
            Self::Integer(_) => 3,
            Self::Float(_) => 4,
            Self::Long(_) => 5,
            Self::Double(_) => 6,
            Self::Class { .. } => 7,
            Self::String { .. } => 8,
            Self::Fieldref { .. } => 9,
            Self::Methodref { .. } => 10,
            Self::InterfaceMethodref { .. } => 11,
            Self::NameAndType { .. } => 12,
            Self::MethodHandle { .. } => 15,
            Self::MethodType { .. } => 16,
            Self::Dynamic { .. } => 17,
            Self::InvokeDynamic { .. } => 18,
            Self::Module { .. } => 19,
            Self::Package { .. } => 20,
            Self::Invalid => 0,
        }
    }

    /// Returns `true` for `Long` and `Double`, which take two pool slots.
    #[must_use]
    pub const fn takes_two_slots(&self) -> bool {
        matches!(self, Self::Long(_) | Self::Double(_))
    }

    /// Returns the value of a `Float` constant as an `f32`.
    #[must_use]
    pub const fn as_f32(&self) -> Option<f32> {
        match self {
            Self::Float(bits) => Some(f32::from_bits(*bits)),
            _ => None,
        }
    }

    /// Returns the value of a `Double` constant as an `f64`.
    #[must_use]
    pub const fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Double(bits) => Some(f64::from_bits(*bits)),
            _ => None,
        }
    }

    /// Builds a `Float` constant from an `f32`, preserving its bit pattern.
    #[must_use]
    pub const fn from_f32(value: f32) -> Self {
        Self::Float(value.to_bits())
    }

    /// Builds a `Double` constant from an `f64`, preserving its bit pattern.
    #[must_use]
    pub const fn from_f64(value: f64) -> Self {
        Self::Double(value.to_bits())
    }

    /// Parses one `cp_info` entry from `reader`.
    pub fn parse<'a>(reader: &mut Reader<'a>) -> Result<Constant<'a>> {
        let tag_offset = reader.position();
        let tag = reader.read_u1()?;
        let constant = match tag {
            1 => {
                let len = reader.read_u2()? as usize;
                Constant::Utf8(Mutf8::new(reader.read_bytes(len)?))
            }
            3 => Constant::Integer(reader.read_i4()?),
            4 => Constant::Float(reader.read_u4()?),
            5 => Constant::Long(reader.read_i8()?),
            6 => Constant::Double(reader.read_u8()?),
            7 => Constant::Class {
                name_index: read_ref(reader)?,
            },
            8 => Constant::String {
                string_index: read_ref(reader)?,
            },
            9 => Constant::Fieldref {
                class_index: read_ref(reader)?,
                name_and_type_index: read_ref(reader)?,
            },
            10 => Constant::Methodref {
                class_index: read_ref(reader)?,
                name_and_type_index: read_ref(reader)?,
            },
            11 => Constant::InterfaceMethodref {
                class_index: read_ref(reader)?,
                name_and_type_index: read_ref(reader)?,
            },
            12 => Constant::NameAndType {
                name_index: read_ref(reader)?,
                descriptor_index: read_ref(reader)?,
            },
            15 => Constant::MethodHandle {
                reference_kind: reader.read_u1()?,
                reference_index: read_ref(reader)?,
            },
            16 => Constant::MethodType {
                descriptor_index: read_ref(reader)?,
            },
            17 => Constant::Dynamic {
                bootstrap_method_attr_index: reader.read_u2()?,
                name_and_type_index: read_ref(reader)?,
            },
            18 => Constant::InvokeDynamic {
                bootstrap_method_attr_index: reader.read_u2()?,
                name_and_type_index: read_ref(reader)?,
            },
            19 => Constant::Module {
                name_index: read_ref(reader)?,
            },
            20 => Constant::Package {
                name_index: read_ref(reader)?,
            },
            _ => {
                return Err(Error::InvalidConstantTag {
                    tag,
                    offset: tag_offset,
                });
            }
        };
        Ok(constant)
    }

    /// Serializes this entry (tag byte included) into `writer`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidConstant`] for [`Constant::Invalid`], which is a
    /// bookkeeping placeholder and must not appear on the wire.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        if matches!(self, Self::Invalid) {
            return Err(Error::InvalidConstant);
        }
        writer.write_u1(self.tag());
        match self {
            Self::Utf8(value) => {
                writer.write_u2_len(value.as_bytes().len())?;
                writer.write_bytes(value.as_bytes());
            }
            Self::Integer(v) => writer.write_i4(*v),
            Self::Float(bits) => writer.write_u4(*bits),
            Self::Long(v) => writer.write_i8(*v),
            Self::Double(bits) => writer.write_u8(*bits),
            Self::Class { name_index } | Self::Module { name_index } => {
                writer.write_cp_u2(name_index.get());
            }
            Self::String { string_index } => writer.write_cp_u2(string_index.get()),
            Self::Fieldref {
                class_index,
                name_and_type_index,
            }
            | Self::Methodref {
                class_index,
                name_and_type_index,
            }
            | Self::InterfaceMethodref {
                class_index,
                name_and_type_index,
            } => {
                writer.write_cp_u2(class_index.get());
                writer.write_cp_u2(name_and_type_index.get());
            }
            Self::NameAndType {
                name_index,
                descriptor_index,
            } => {
                writer.write_cp_u2(name_index.get());
                writer.write_cp_u2(descriptor_index.get());
            }
            Self::MethodHandle {
                reference_kind,
                reference_index,
            } => {
                writer.write_u1(*reference_kind);
                writer.write_cp_u2(reference_index.get());
            }
            Self::MethodType { descriptor_index } => writer.write_cp_u2(descriptor_index.get()),
            Self::Dynamic {
                bootstrap_method_attr_index,
                name_and_type_index,
            }
            | Self::InvokeDynamic {
                bootstrap_method_attr_index,
                name_and_type_index,
            } => {
                // `bootstrap_method_attr_index` indexes the BootstrapMethods
                // attribute, not the constant pool, so it is not remapped.
                writer.write_u2(*bootstrap_method_attr_index);
                writer.write_cp_u2(name_and_type_index.get());
            }
            Self::Package { name_index } => writer.write_cp_u2(name_index.get()),
            // Rejected above; this arm only satisfies exhaustiveness.
            Self::Invalid => {}
        }
        Ok(())
    }
}

/// The constant pool table.
///
/// Index 0 and the second slot of every `Long`/`Double` are [`Constant::Invalid`]
/// placeholders, mirroring the on-disk slot layout so that indices remain stable
/// for byte-exact round-trips.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConstantPool<'a> {
    entries: Vec<Constant<'a>>,
}

impl<'a> ConstantPool<'a> {
    /// Creates an empty constant pool (only the unused index 0).
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: vec![Constant::Invalid],
        }
    }

    /// Creates a pool from raw entries (index 0 must be [`Constant::Invalid`]).
    #[must_use]
    pub(crate) fn from_entries(entries: Vec<Constant<'a>>) -> Self {
        Self { entries }
    }

    /// Parses `constant_pool_count - 1` entries from `reader`.
    pub fn parse(reader: &mut Reader<'a>, constant_pool_count: u16) -> Result<Self> {
        let count = constant_pool_count as usize;
        let mut entries = Vec::with_capacity(count.max(1));
        if count == 0 {
            return Ok(Self { entries });
        }
        entries.push(Constant::Invalid); // index 0 is unused
        while entries.len() < count {
            let constant = Constant::parse(reader)?;
            let two_slots = constant.takes_two_slots();
            entries.push(constant);
            if two_slots {
                entries.push(Constant::Invalid);
            }
        }
        Ok(Self { entries })
    }

    /// Writes `constant_pool_count` followed by every entry.
    pub fn write_to(&self, writer: &mut Writer) -> Result<()> {
        writer.write_u2_len(self.entries.len())?;
        for constant in self.entries.iter().skip(1) {
            match constant {
                Constant::Invalid => {}
                constant => constant.write_to(writer)?,
            }
        }
        Ok(())
    }

    /// Number of slots in the pool, i.e. the value of `constant_pool_count`.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the pool has no usable entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.len() <= 1
    }

    /// Returns the constant at `index`, or `None` if it is a placeholder
    /// ([`Constant::Invalid`]) or out of range.
    #[must_use]
    pub fn get(&self, index: ConstantRef<'a>) -> Option<&Constant<'a>> {
        match self.entries.get(index.get() as usize)? {
            Constant::Invalid => None,
            constant => Some(constant),
        }
    }

    /// Crate-internal lookup by raw index (used by the rebuild pass).
    #[must_use]
    pub(crate) fn entry(&self, index: u16) -> Option<&Constant<'a>> {
        match self.entries.get(index as usize)? {
            Constant::Invalid => None,
            constant => Some(constant),
        }
    }

    /// Returns a mutable reference to the constant at `index`.
    ///
    /// Returns `None` for placeholder ([`Constant::Invalid`]) or out-of-range
    /// slots.
    pub fn get_mut(&mut self, index: ConstantRef<'a>) -> Option<&mut Constant<'a>> {
        match self.entries.get_mut(index.get() as usize)? {
            Constant::Invalid => None,
            constant => Some(constant),
        }
    }

    /// Appends a constant and returns a reference to it.
    ///
    /// `Long` and `Double` values allocate two slots. The returned reference is
    /// to the first slot.
    pub fn push(&mut self, constant: Constant<'a>) -> Result<ConstantRef<'a>> {
        if self.entries.is_empty() {
            self.entries.push(Constant::Invalid);
        }
        let index = u16::try_from(self.entries.len()).map_err(|_| Error::ValueTooLarge {
            value: self.entries.len() as u64,
            target: "u16 (constant pool index)",
        })?;
        let two_slots = constant.takes_two_slots();
        self.entries.push(constant);
        if two_slots {
            self.entries.push(Constant::Invalid);
        }
        Ok(ConstantRef::new(index))
    }

    /// Returns a reference to the first entry equal to `constant`, if any.
    #[must_use]
    pub fn find(&self, constant: &Constant<'a>) -> Option<ConstantRef<'a>> {
        self.iter()
            .find(|(_, candidate)| *candidate == constant)
            .map(|(reference, _)| reference)
    }

    /// Appends `constant` unless an equal entry already exists, returning a
    /// reference to it. This is the default, index-stable way to add constants.
    pub fn intern(&mut self, constant: Constant<'a>) -> Result<ConstantRef<'a>> {
        if let Some(reference) = self.find(&constant) {
            return Ok(reference);
        }
        self.push(constant)
    }

    /// Interns a `CONSTANT_Utf8` value for `value`.
    ///
    /// `value` is validated with [`Mutf8::try_from`] and the resulting entry
    /// borrows it for the pool's lifetime `'a`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidModifiedUtf8`] if `value` contains U+0000 or a
    /// supplementary character, which standard UTF-8 represents but modified
    /// UTF-8 does not.
    pub fn intern_utf8(&mut self, value: &'a str) -> Result<ConstantRef<'a>> {
        self.intern(Constant::Utf8(Mutf8::try_from(value)?))
    }

    /// Interns a `CONSTANT_Methodref` for `class`.`name` with `descriptor`.
    ///
    /// `class` is an internal class name (for example `"example/Hook"`), `name`
    /// the method name and `descriptor` the method descriptor. `interface`
    /// selects a `CONSTANT_InterfaceMethodref` instead of a
    /// `CONSTANT_Methodref`. The `Utf8`, `Class` and `NameAndType` entries it
    /// depends on are interned (deduplicated) as needed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidModifiedUtf8`] for the same reason as
    /// [`ConstantPool::intern_utf8`], or [`Error::ValueTooLarge`] if the pool
    /// would exceed `u16::MAX` entries.
    pub fn intern_method(
        &mut self,
        class: &'a str,
        name: &'a str,
        descriptor: &'a str,
        interface: bool,
    ) -> Result<ConstantRef<'a>> {
        let class_name = self.intern_utf8(class)?;
        let class_index = self.intern(Constant::Class {
            name_index: class_name,
        })?;
        let name_index = self.intern_utf8(name)?;
        let descriptor_index = self.intern_utf8(descriptor)?;
        let name_and_type_index = self.intern(Constant::NameAndType {
            name_index,
            descriptor_index,
        })?;
        if interface {
            self.intern(Constant::InterfaceMethodref {
                class_index,
                name_and_type_index,
            })
        } else {
            self.intern(Constant::Methodref {
                class_index,
                name_and_type_index,
            })
        }
    }

    /// Iterates over `(reference, constant)` pairs, skipping placeholder
    /// ([`Constant::Invalid`]) slots.
    pub fn iter(&self) -> impl Iterator<Item = (ConstantRef<'a>, &Constant<'a>)> {
        self.entries.iter().enumerate().filter_map(|(i, entry)| {
            let index = u16::try_from(i).ok()?;
            match entry {
                Constant::Invalid => None,
                constant => Some((ConstantRef::new(index), constant)),
            }
        })
    }

    /// Resolves `index` to a `CONSTANT_Utf8` entry.
    pub fn utf8(&self, index: ConstantRef<'a>) -> Result<Mutf8<'a>> {
        match self.get(index) {
            Some(Constant::Utf8(value)) => Ok(*value),
            _ => Err(Error::ConstantIndexOutOfRange {
                index: index.get(),
                len: self.entries.len(),
            }),
        }
    }

    /// Resolves `index` to a `CONSTANT_Utf8` entry and decodes it.
    pub fn utf8_str(&self, index: ConstantRef<'a>) -> Result<String> {
        mutf8::decode(self.utf8(index)?)
    }

    /// Resolves `index` to a `CONSTANT_Class` name as a decoded string.
    pub fn class_name(&self, index: ConstantRef<'a>) -> Result<String> {
        match self.get(index) {
            Some(Constant::Class { name_index }) => self.utf8_str(*name_index),
            _ => Err(Error::ConstantIndexOutOfRange {
                index: index.get(),
                len: self.entries.len(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Constant, ConstantPool, ConstantRef};
    use crate::io::{Reader, Writer};
    use crate::mutf8::Mutf8;

    fn round_trip(pool: &ConstantPool<'_>) -> ConstantPool<'static> {
        let mut writer = Writer::new();
        pool.write_to(&mut writer).unwrap();
        let bytes = writer.into_bytes();
        let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let mut reader = Reader::new(leaked);
        let count = reader.read_u2().unwrap();
        ConstantPool::parse(&mut reader, count).unwrap()
    }

    #[test]
    fn long_and_double_take_two_slots() {
        let mut pool = ConstantPool::new();
        let i1 = pool.push(Constant::Utf8(Mutf8::new(b"x"))).unwrap();
        let i2 = pool.push(Constant::Long(7)).unwrap();
        let i3 = pool.push(Constant::Double(1.5f64.to_bits())).unwrap();
        let i4 = pool.push(Constant::Utf8(Mutf8::new(b"y"))).unwrap();
        assert_eq!((i1.get(), i2.get(), i3.get(), i4.get()), (1, 2, 4, 6));
        assert_eq!(pool.len(), 7); // [0] + utf8 + long(2) + double(2) + utf8
        assert_eq!(pool.get(ConstantRef::new(3)), None); // second slot of long
        assert_eq!(pool.get(ConstantRef::new(5)), None); // second slot of double
    }

    #[test]
    fn parses_and_writes_all_tag_kinds() {
        let mut pool = ConstantPool::new();
        let one = pool
            .push(Constant::Utf8(Mutf8::new(b"java/lang/Object")))
            .unwrap();
        pool.push(Constant::Class { name_index: one }).unwrap();
        pool.push(Constant::Integer(-1)).unwrap();
        pool.push(Constant::Float(1.0f32.to_bits())).unwrap();
        pool.push(Constant::MethodHandle {
            reference_kind: 6,
            reference_index: one,
        })
        .unwrap();
        pool.push(Constant::Module { name_index: one }).unwrap();
        pool.push(Constant::Package { name_index: one }).unwrap();

        let reparsed = round_trip(&pool);
        let a: Vec<_> = pool.iter().collect();
        let b: Vec<_> = reparsed.iter().collect();
        assert_eq!(a, b);
    }

    #[test]
    fn invalid_slots_are_not_serialized() {
        let mut pool = ConstantPool::new();
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.get(ConstantRef::new(0)), None);
        pool.push(Constant::Long(7)).unwrap();

        let mut writer = Writer::new();
        pool.write_to(&mut writer).unwrap();
        // `constant_pool_count = 3`, then the tag-5 `Long` and its 8 payload
        // bytes. The index-0 placeholder and the `Long`'s second slot are
        // `Constant::Invalid` and are not emitted.
        assert_eq!(writer.into_bytes(), [0, 3, 5, 0, 0, 0, 0, 0, 0, 0, 7]);
    }

    #[test]
    fn invalid_constant_is_a_sentinel() {
        assert_eq!(Constant::Invalid.tag(), 0);
        let mut writer = Writer::new();
        assert!(matches!(
            Constant::Invalid.write_to(&mut writer),
            Err(crate::error::Error::InvalidConstant)
        ));
        assert!(writer.into_bytes().is_empty());
    }

    #[test]
    fn intern_utf8_accepts_str_and_validates() {
        let mut pool = ConstantPool::new();
        let first = pool.intern_utf8("hello").unwrap();
        let second = pool.intern_utf8("hello").unwrap();
        assert_eq!(first, second);
        assert_eq!(pool.utf8(first).unwrap(), "hello");
        // Modified UTF-8 rejects NUL and supplementary characters.
        assert!(pool.intern_utf8("a\0b").is_err());
        assert!(pool.intern_utf8("emoji 😀").is_err());
    }

    #[test]
    fn intern_method_builds_a_deduplicated_methodref() {
        let mut pool = ConstantPool::new();
        let first = pool
            .intern_method("example/Hook", "entry", "()V", false)
            .unwrap();
        let second = pool
            .intern_method("example/Hook", "entry", "()V", false)
            .unwrap();
        assert_eq!(first, second);

        let Some(Constant::Methodref {
            class_index,
            name_and_type_index,
        }) = pool.get(first)
        else {
            panic!("expected a Methodref");
        };
        assert_eq!(pool.class_name(*class_index).unwrap(), "example/Hook");
        let Some(Constant::NameAndType {
            name_index,
            descriptor_index,
        }) = pool.get(*name_and_type_index)
        else {
            panic!("expected a NameAndType");
        };
        assert_eq!(pool.utf8_str(*name_index).unwrap(), "entry");
        assert_eq!(pool.utf8_str(*descriptor_index).unwrap(), "()V");
    }
}
