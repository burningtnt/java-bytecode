//! A bounds-checked, big-endian cursor over an immutable byte slice.

use zerocopy::FromBytes;
use zerocopy::byteorder::big_endian::{U16, U32, U64};

use crate::error::{Error, Result};

/// A cursor over a byte slice.
///
/// Every read is bounds checked and returns [`Error::UnexpectedEof`] rather
/// than panicking. Slices returned by [`Reader::read_bytes`] borrow directly
/// from the original input, so no copying occurs.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Creates a reader over `data`.
    #[inline]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Current position, in bytes, from the start of the input.
    #[inline]
    pub const fn position(&self) -> usize {
        self.pos
    }

    /// Number of bytes still unread.
    #[inline]
    pub const fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// Returns `true` once every byte has been consumed.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.pos == self.data.len()
    }

    #[inline]
    fn require(&self, n: usize) -> Result<()> {
        if self.remaining() < n {
            return Err(Error::UnexpectedEof {
                offset: self.pos,
                needed: n,
                remaining: self.remaining(),
            });
        }
        Ok(())
    }

    /// Reads `n` bytes and returns a slice borrowing from the input.
    #[inline]
    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        self.require(n)?;
        let start = self.pos;
        self.pos += n;
        Ok(&self.data[start..self.pos])
    }

    /// Reads the next byte.
    #[inline]
    pub fn read_u1(&mut self) -> Result<u8> {
        self.require(1)?;
        let byte = self.data[self.pos];
        self.pos += 1;
        Ok(byte)
    }

    /// Reads a big-endian `u16`.
    #[inline]
    pub fn read_u2(&mut self) -> Result<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(U16::read_from_bytes(bytes)
            .expect("read_bytes returned 2 bytes")
            .get())
    }

    /// Reads a big-endian `u32`.
    #[inline]
    pub fn read_u4(&mut self) -> Result<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(U32::read_from_bytes(bytes)
            .expect("read_bytes returned 4 bytes")
            .get())
    }

    /// Reads a big-endian `u64`.
    #[inline]
    pub fn read_u8(&mut self) -> Result<u64> {
        let bytes = self.read_bytes(8)?;
        Ok(U64::read_from_bytes(bytes)
            .expect("read_bytes returned 8 bytes")
            .get())
    }

    /// Reads a big-endian signed `i32`.
    #[inline]
    pub fn read_i4(&mut self) -> Result<i32> {
        Ok(self.read_u4()? as i32)
    }

    /// Reads a big-endian signed `i64`.
    #[inline]
    pub fn read_i8(&mut self) -> Result<i64> {
        Ok(self.read_u8()? as i64)
    }

    /// Skips `n` bytes.
    #[inline]
    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.require(n)?;
        self.pos += n;
        Ok(())
    }

    /// Reads a `u2` and returns a nested reader over the following that many
    /// bytes. The nested reader borrows from the same input.
    #[inline]
    pub fn read_sub_reader_u2(&mut self) -> Result<Reader<'a>> {
        let len = self.read_u2()? as usize;
        self.read_sub_reader(len)
    }

    /// Returns a nested reader over the next `len` bytes.
    #[inline]
    pub fn read_sub_reader(&mut self, len: usize) -> Result<Reader<'a>> {
        Ok(Reader::new(self.read_bytes(len)?))
    }

    /// Consumes the reader, ensuring no bytes are left unread.
    #[inline]
    pub fn finish(self, context: &'static str) -> Result<()> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(Error::TrailingBytes {
                remaining: self.remaining(),
                context,
            })
        }
    }
}
