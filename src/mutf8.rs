//! Java *modified UTF-8* (MUTF-8) codec and the opaque [`Mutf8`] wrapper.
//!
//! The `CONSTANT_Utf8` constant pool entry (JVMS §4.4.7) uses a variant of
//! UTF-8 with two differences from the standard encoding:
//!
//! * the code point U+0000 is encoded as the two bytes `0xC0 0x80` rather than
//!   a single NUL byte;
//! * characters outside the Basic Multilingual Plane are encoded as a UTF-16
//!   surrogate pair, each half encoded as three bytes (CESU-8).
//!
//! [`Mutf8`] wraps the raw bytes of such a value. It is a `#[repr(transparent)]`
//! wrapper over `&[u8]` (so no copy is made), compares equal to `str`/`String`
//! after decoding, and converts to and from those types.

use core::fmt;

use crate::error::{Error, Result};

const fn invalid(offset: usize, reason: &'static str) -> Error {
    Error::InvalidModifiedUtf8 { offset, reason }
}

/// An opaque, borrowed Java modified UTF-8 value.
///
/// This is a `#[repr(transparent)]` wrapper over `&'a [u8]`, so it has the same
/// layout as the slice it borrows. It can be compared directly with `str`,
/// `&str`, `String`, and `&[u8]`, and converted to and from those types.
///
/// Construction is validated through [`TryFrom`]; use
/// [`Mutf8::from_bytes_unchecked`] (unsafe) to skip validation.
///
/// ```
/// use java_bytecode::Mutf8;
///
/// // From bytes; the input is validated.
/// let value = Mutf8::try_from(b"java/lang/Object".as_slice()).unwrap();
/// assert_eq!(value, "java/lang/Object");
/// assert_eq!(String::try_from(value).unwrap(), "java/lang/Object");
///
/// // A `&str` with no NUL and no supplementary characters is valid MUTF-8.
/// let value = Mutf8::try_from("hello").unwrap();
/// let bytes: &[u8] = value.as_ref();
/// assert_eq!(bytes, b"hello");
///
/// // Unsafe: skip validation.
/// let value = unsafe { Mutf8::from_bytes_unchecked(b"unchecked") };
/// assert_eq!(value, "unchecked");
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mutf8<'a> {
    bytes: &'a [u8],
}

impl<'a> Mutf8<'a> {
    /// Crate-internal unchecked constructor.
    #[inline]
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// Wraps raw bytes as modified UTF-8 **without validating them**.
    ///
    /// Prefer [`Mutf8::try_from`], which validates the bytes. Use this only
    /// when the bytes are already known to be valid modified UTF-8 (for
    /// example, when copying a constant from a parsed class file).
    ///
    /// # Safety
    ///
    /// The caller must ensure `bytes` is well-formed Java modified UTF-8. If it
    /// is not, decoding may fail or produce unexpected output, and writing the
    /// value back out may yield an invalid class file.
    #[must_use]
    #[allow(unsafe_code)]
    pub const unsafe fn from_bytes_unchecked(bytes: &'a [u8]) -> Self {
        Self::new(bytes)
    }

    /// Returns the raw modified UTF-8 bytes.
    #[must_use]
    pub(crate) const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Decodes this value into a Rust [`String`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidModifiedUtf8`] if the bytes are malformed or
    /// contain an unpaired surrogate.
    pub fn decode(self) -> Result<String> {
        decode(self)
    }

    /// Decodes this value, replacing anything invalid with `U+FFFD`.
    #[must_use]
    pub fn to_string_lossy(self) -> String {
        decode(self).unwrap_or_else(|_| String::from_utf8_lossy(self.bytes).into_owned())
    }
}

impl fmt::Display for Mutf8<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_lossy())
    }
}

impl<'a> TryFrom<&'a [u8]> for Mutf8<'a> {
    type Error = Error;

    /// Validates `bytes` as Java modified UTF-8 and borrows them.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidModifiedUtf8`] if the bytes are malformed or
    /// contain an unpaired surrogate. Use [`Mutf8::from_bytes_unchecked`] to
    /// skip validation.
    fn try_from(bytes: &'a [u8]) -> Result<Self> {
        let value = Self::new(bytes);
        validate(value)?;
        Ok(value)
    }
}

impl<'a> From<Mutf8<'a>> for &'a [u8] {
    fn from(value: Mutf8<'a>) -> Self {
        value.as_bytes()
    }
}

impl AsRef<[u8]> for Mutf8<'_> {
    fn as_ref(&self) -> &[u8] {
        self.bytes
    }
}

impl TryFrom<Mutf8<'_>> for String {
    type Error = Error;

    fn try_from(value: Mutf8<'_>) -> Result<Self> {
        value.decode()
    }
}

impl<'a> TryFrom<&'a str> for Mutf8<'a> {
    type Error = Error;

    /// Borrows `value` as modified UTF-8.
    ///
    /// # Errors
    ///
    /// Standard UTF-8 and modified UTF-8 agree for all code points in
    /// `U+0001..=U+FFFF`, but not for U+0000 or supplementary characters. A
    /// string containing either is rejected; use [`encode`] to obtain a buffer
    /// and [`Mutf8::from_bytes_unchecked`] to borrow it.
    fn try_from(value: &'a str) -> Result<Self> {
        if value.as_bytes().contains(&0) {
            return Err(invalid(0, "NUL must be encoded as 0xC0 0x80"));
        }
        if value.chars().any(|c| c as u32 > 0xFFFF) {
            return Err(invalid(
                0,
                "supplementary characters need CESU-8 surrogate pairs",
            ));
        }
        Ok(Self::new(value.as_bytes()))
    }
}

/// A destination for the code points produced while scanning modified UTF-8.
///
/// A [`String`] decodes into an owned buffer; the zero-sized `Validate` discards
/// every code point, so the compiler monomorphizes a validation-only pass that
/// allocates nothing. This is how [`Mutf8::try_from`] validates without the cost
/// of building an intermediate `String`.
trait CharSink {
    /// Appends one decoded code point.
    fn push_char(&mut self, c: char);
    /// Appends one decoded ASCII code point.
    fn push_ascii(&mut self, b: u8);
}

impl CharSink for String {
    #[inline]
    fn push_char(&mut self, c: char) {
        self.push(c);
    }

    #[inline]
    fn push_ascii(&mut self, b: u8) {
        self.push(char::from(b));
    }
}

/// The validation-only [`CharSink`]: it drops every code point.
struct Validate;

impl CharSink for Validate {
    #[inline]
    fn push_char(&mut self, _c: char) {}

    #[inline]
    fn push_ascii(&mut self, _b: u8) {}
}

/// Scans `value`, feeding every code point to `sink` and validating as it goes.
///
/// `sink` is statically dispatched, so the `String` instantiation is the
/// decoding path and the `Validate` instantiation is a no-allocation
/// validation pass.
///
/// # Errors
///
/// Returns [`Error::InvalidModifiedUtf8`] if the input is malformed, contains
/// an encoded NUL or otherwise overlong sequence, or contains an unpaired
/// UTF-16 surrogate.
fn decode_into<S: CharSink>(value: Mutf8<'_>, sink: &mut S) -> Result<()> {
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        if b0 < 0x80 {
            if b0 == 0 {
                return Err(invalid(
                    i,
                    "unescaped NUL byte (must be encoded as 0xC0 0x80)",
                ));
            }
            sink.push_ascii(b0);
            i += 1;
            continue;
        }

        if b0 & 0xE0 == 0xC0 {
            let b1 = *bytes
                .get(i + 1)
                .ok_or_else(|| invalid(i, "truncated 2-byte sequence"))?;
            if b1 & 0xC0 != 0x80 {
                return Err(invalid(i + 1, "bad continuation byte"));
            }
            let cp = ((u32::from(b0) & 0x1F) << 6) | (u32::from(b1) & 0x3F);
            if cp == 0 {
                sink.push_char('\0');
            } else if cp < 0x80 {
                return Err(invalid(i, "overlong 2-byte sequence"));
            } else {
                sink.push_char(char::from_u32(cp).ok_or_else(|| invalid(i, "invalid code point"))?);
            }
            i += 2;
            continue;
        }

        if b0 & 0xF0 == 0xE0 {
            let cp = decode_three(bytes, i)?;
            if (0xD800..=0xDBFF).contains(&cp) {
                // High surrogate: the low half must follow as another 3-byte
                // sequence (CESU-8 surrogate pair).
                let next = i + 3;
                if next >= bytes.len() || bytes[next] & 0xF0 != 0xE0 {
                    return Err(invalid(i, "unpaired high surrogate"));
                }
                let low = decode_three(bytes, next)?;
                if !(0xDC00..=0xDFFF).contains(&low) {
                    return Err(invalid(next, "expected low surrogate"));
                }
                let combined = 0x1_0000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                sink.push_char(
                    char::from_u32(combined).ok_or_else(|| invalid(i, "invalid code point"))?,
                );
                i += 6;
            } else if (0xDC00..=0xDFFF).contains(&cp) {
                return Err(invalid(i, "unpaired low surrogate"));
            } else {
                sink.push_char(char::from_u32(cp).ok_or_else(|| invalid(i, "invalid code point"))?);
                i += 3;
            }
            continue;
        }

        return Err(invalid(i, "invalid lead byte"));
    }
    Ok(())
}

/// Decodes a Java modified UTF-8 value into an owned [`String`].
///
/// # Errors
///
/// Returns [`Error::InvalidModifiedUtf8`] if the input is malformed, contains
/// an encoded NUL or otherwise overlong sequence, or contains an unpaired
/// UTF-16 surrogate.
pub fn decode(value: Mutf8<'_>) -> Result<String> {
    let mut out = String::with_capacity(value.as_bytes().len());
    decode_into(value, &mut out)?;
    Ok(out)
}

/// Validates `value` as modified UTF-8 without allocating.
fn validate(value: Mutf8<'_>) -> Result<()> {
    decode_into(value, &mut Validate)
}

fn decode_three(bytes: &[u8], i: usize) -> Result<u32> {
    let b1 = *bytes
        .get(i + 1)
        .ok_or_else(|| invalid(i, "truncated 3-byte sequence"))?;
    let b2 = *bytes
        .get(i + 2)
        .ok_or_else(|| invalid(i, "truncated 3-byte sequence"))?;
    if b1 & 0xC0 != 0x80 {
        return Err(invalid(i + 1, "bad continuation byte"));
    }
    if b2 & 0xC0 != 0x80 {
        return Err(invalid(i + 2, "bad continuation byte"));
    }
    let cp = ((u32::from(bytes[i]) & 0x0F) << 12)
        | ((u32::from(b1) & 0x3F) << 6)
        | (u32::from(b2) & 0x3F);
    if cp < 0x800 {
        return Err(invalid(i, "overlong 3-byte sequence"));
    }
    Ok(cp)
}

/// Encodes a [`str`] into a Java modified UTF-8 byte sequence.
#[must_use]
pub fn encode(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    encode_into(s, &mut out);
    out
}

/// Appends the modified UTF-8 encoding of `s` to `out`.
pub fn encode_into(s: &str, out: &mut Vec<u8>) {
    for ch in s.chars() {
        let c = ch as u32;
        match c {
            0 => out.extend_from_slice(&[0xC0, 0x80]),
            0x01..=0x7F => out.push(c as u8),
            0x80..=0x7FF => {
                out.push(0xC0 | (c >> 6) as u8);
                out.push(0x80 | (c & 0x3F) as u8);
            }
            0x800..=0xFFFF => encode_three(c, out),
            _ => {
                let v = c - 0x1_0000;
                encode_three(0xD800 + (v >> 10), out);
                encode_three(0xDC00 + (v & 0x3FF), out);
            }
        }
    }
}

#[inline]
fn encode_three(c: u32, out: &mut Vec<u8>) {
    out.push(0xE0 | (c >> 12) as u8);
    out.push(0x80 | ((c >> 6) & 0x3F) as u8);
    out.push(0x80 | (c & 0x3F) as u8);
}

// --- Comparisons with the common string and byte types --------------------

impl PartialEq<str> for Mutf8<'_> {
    fn eq(&self, other: &str) -> bool {
        matches!(self.decode(), Ok(value) if value == other)
    }
}

impl PartialEq<Mutf8<'_>> for str {
    fn eq(&self, other: &Mutf8<'_>) -> bool {
        other == self
    }
}

impl PartialEq<&str> for Mutf8<'_> {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl PartialEq<Mutf8<'_>> for &str {
    fn eq(&self, other: &Mutf8<'_>) -> bool {
        other == *self
    }
}

impl PartialEq<String> for Mutf8<'_> {
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<Mutf8<'_>> for String {
    fn eq(&self, other: &Mutf8<'_>) -> bool {
        other == self.as_str()
    }
}

impl PartialEq<&String> for Mutf8<'_> {
    fn eq(&self, other: &&String) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<Mutf8<'_>> for &String {
    fn eq(&self, other: &Mutf8<'_>) -> bool {
        other == self.as_str()
    }
}

impl PartialEq<[u8]> for Mutf8<'_> {
    fn eq(&self, other: &[u8]) -> bool {
        self.bytes == other
    }
}

impl PartialEq<Mutf8<'_>> for [u8] {
    fn eq(&self, other: &Mutf8<'_>) -> bool {
        other.bytes == self
    }
}

impl PartialEq<&[u8]> for Mutf8<'_> {
    fn eq(&self, other: &&[u8]) -> bool {
        self.bytes == *other
    }
}

impl PartialEq<Mutf8<'_>> for &[u8] {
    fn eq(&self, other: &Mutf8<'_>) -> bool {
        other.bytes == *self
    }
}

#[cfg(test)]
mod tests {
    use super::{Mutf8, decode, encode};

    #[test]
    fn round_trips_ascii() {
        let s = "hello world";
        let encoded = encode(s);
        assert_eq!(decode(Mutf8::new(&encoded)).unwrap(), s);
    }

    #[test]
    fn encodes_nul_as_c0_80() {
        assert_eq!(encode("\0"), vec![0xC0, 0x80]);
        assert_eq!(decode(Mutf8::new(&[0xC0, 0x80])).unwrap(), "\0");
    }

    #[test]
    fn rejects_unescaped_nul() {
        assert!(decode(Mutf8::new(&[0x00])).is_err());
    }

    #[test]
    fn round_trips_multibyte() {
        for s in ["héllo", "日本語", "emoji 😀🚀", "\u{10FFFF}", "\u{1F600}"] {
            let encoded = encode(s);
            assert_eq!(decode(Mutf8::new(&encoded)).unwrap(), s, "round trip {s:?}");
        }
    }

    #[test]
    fn supplementary_uses_surrogates() {
        // U+1F600 encodes as six bytes (CESU-8 surrogate pair).
        assert_eq!(encode("\u{1F600}").len(), 6);
    }

    #[test]
    fn rejects_lone_surrogate() {
        // ED A0 80 is the UTF-8/CESU-8 encoding of the lone high surrogate
        // U+D800, which has no Rust `char`.
        assert!(decode(Mutf8::new(&[0xED, 0xA0, 0x80])).is_err());
    }

    #[test]
    fn rejects_truncated() {
        assert!(decode(Mutf8::new(&[0xC3])).is_err());
        assert!(decode(Mutf8::new(&[0xE3, 0x81])).is_err());
    }

    #[test]
    fn compares_with_strings_and_bytes() {
        let value = Mutf8::new(b"java/lang/Object");
        assert_eq!(value, "java/lang/Object");
        assert_eq!(value, String::from("java/lang/Object"));
        assert_eq!(value, b"java/lang/Object".as_slice());
        assert_eq!("java/lang/Object", value);
        assert_ne!(value, "other");
    }

    #[test]
    fn converts_to_and_from_str() {
        let value = Mutf8::try_from("hello").unwrap();
        assert_eq!(String::try_from(value).unwrap(), "hello");
        assert!(Mutf8::try_from("a\0b").is_err());
        assert!(Mutf8::try_from("emoji 😀").is_err());
    }

    #[test]
    fn validation_matches_decode() {
        // The allocation-free `try_from` path must accept exactly the inputs the
        // decoding path accepts.
        let cases: &[&[u8]] = &[
            b"",
            b"ascii",
            b"h\xC3\xA9llo",
            &[0xC0, 0x80], // encoded NUL
            &[0x00],       // unescaped NUL
            &[0xED, 0xA0, 0x80],
            &[0xC3],
            &[0xE3, 0x81],
            &[0xE2, 0x82, 0xAC],                   // U+20AC
            &[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80], // U+1F600 surrogate pair
            &[0xF0, 0x9F, 0x98, 0x80],             // UTF-8 U+1F600
        ];
        for bytes in cases {
            let decoded = decode(Mutf8::new(bytes)).is_ok();
            let validated = Mutf8::try_from(*bytes).is_ok();
            assert_eq!(decoded, validated, "mismatch for {bytes:02X?}");
        }
    }

    #[test]
    fn try_from_bytes_validates() {
        let value = Mutf8::try_from(b"java/lang/Object".as_slice()).unwrap();
        assert_eq!(value, "java/lang/Object");
        // An unescaped NUL is not valid modified UTF-8.
        assert!(Mutf8::try_from(&[0x00][..]).is_err());
        assert!(Mutf8::try_from(&[0xED, 0xA0, 0x80][..]).is_err());
    }

    #[test]
    #[allow(unsafe_code)]
    fn from_bytes_unchecked_skips_validation() {
        // Unsafe construction accepts bytes that would fail `TryFrom`.
        let value = unsafe { Mutf8::from_bytes_unchecked(&[0x00]) };
        assert!(value.decode().is_err());
    }

    proptest::proptest! {
        #[test]
        fn round_trips_arbitrary_strings(s in ".*") {
            let encoded = encode(&s);
            let decoded = decode(Mutf8::new(&encoded)).expect("decode");
            proptest::prop_assert_eq!(decoded, s);
        }
    }
}
