//! A big-endian writer that appends to an owned buffer.

use crate::error::{Error, Result};

/// An append-only, big-endian writer.
///
/// The resulting bytes are owned by the writer and handed back via
/// [`Writer::into_bytes`]. Data borrowed by a parsed class file is never
/// mutated; writes always copy into this buffer.
///
/// A writer may additionally carry a constant-pool index remapping. When one
/// is present, [`Writer::write_cp_u2`] and [`Writer::write_cp_u1`] translate
/// indices through it; this is how a rebuilt class file renumbers its constant
/// pool without changing any serialization code.
#[derive(Debug, Default)]
pub struct Writer {
    buf: Vec<u8>,
    remap: Option<Vec<u16>>,
}

impl Writer {
    /// Creates an empty writer.
    #[inline]
    pub const fn new() -> Self {
        Self {
            buf: Vec::new(),
            remap: None,
        }
    }

    /// Creates an empty writer with the given capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
            remap: None,
        }
    }

    /// Creates an empty writer that remaps constant-pool indices through
    /// `remap` (`remap[old] == new`).
    #[inline]
    pub fn with_remap(remap: Vec<u16>) -> Self {
        Self {
            buf: Vec::new(),
            remap: Some(remap),
        }
    }

    /// Creates a new, empty writer sharing this writer's remapping.
    #[inline]
    #[must_use]
    pub fn fork(&self) -> Self {
        Self {
            buf: Vec::new(),
            remap: self.remap.clone(),
        }
    }

    /// Returns `true` if a constant-pool remapping is active.
    #[inline]
    pub fn has_remap(&self) -> bool {
        self.remap.is_some()
    }

    /// Maps a constant-pool index through the active remapping.
    #[inline]
    pub fn remap_index(&self, index: u16) -> u16 {
        match &self.remap {
            Some(remap) => remap.get(index as usize).copied().unwrap_or(index),
            None => index,
        }
    }

    /// Writes a constant-pool index as a big-endian `u2`, remapping it first.
    #[inline]
    pub fn write_cp_u2(&mut self, index: u16) {
        let mapped = self.remap_index(index);
        self.write_u2(mapped);
    }

    /// Writes a constant-pool index as a single byte, remapping it first.
    #[inline]
    pub fn write_cp_u1(&mut self, index: u8) -> Result<()> {
        let mapped = self.remap_index(u16::from(index));
        let value = u8::try_from(mapped).map_err(|_| Error::ValueTooLarge {
            value: u64::from(mapped),
            target: "u8 (constant pool index)",
        })?;
        self.write_u1(value);
        Ok(())
    }

    /// Number of bytes written so far.
    #[inline]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Returns `true` if nothing has been written yet.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Returns the written bytes.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    /// Consumes the writer and returns the written bytes.
    #[inline]
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// Writes a single byte.
    #[inline]
    pub fn write_u1(&mut self, value: u8) {
        self.buf.push(value);
    }

    /// Writes a big-endian `u16`.
    #[inline]
    pub fn write_u2(&mut self, value: u16) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    /// Writes a big-endian `u32`.
    #[inline]
    pub fn write_u4(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    /// Writes a big-endian signed `i32`.
    #[inline]
    pub fn write_i4(&mut self, value: i32) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    /// Writes a big-endian `u64`.
    #[inline]
    pub fn write_u8(&mut self, value: u64) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    /// Writes a big-endian signed `i64`.
    #[inline]
    pub fn write_i8(&mut self, value: i64) {
        self.buf.extend_from_slice(&value.to_be_bytes());
    }

    /// Writes raw bytes verbatim.
    #[inline]
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Writes a length as a big-endian `u16`, failing if it does not fit.
    #[inline]
    pub fn write_u2_len(&mut self, len: usize) -> Result<()> {
        let value = u16::try_from(len).map_err(|_| Error::ValueTooLarge {
            value: len as u64,
            target: "u16",
        })?;
        self.write_u2(value);
        Ok(())
    }

    /// Writes a length as a big-endian `u32`, failing if it does not fit.
    #[inline]
    pub fn write_u4_len(&mut self, len: usize) -> Result<()> {
        let value = u32::try_from(len).map_err(|_| Error::ValueTooLarge {
            value: len as u64,
            target: "u32",
        })?;
        self.write_u4(value);
        Ok(())
    }
}

impl From<Writer> for Vec<u8> {
    #[inline]
    fn from(writer: Writer) -> Self {
        writer.into_bytes()
    }
}
