//! Field and method descriptors (JVMS §4.3).
//!
//! Descriptors are parsed in place: object type names are returned as slices
//! borrowing from the input descriptor bytes.

use core::fmt;

use crate::error::{Error, Result};
use crate::mutf8::Mutf8;

const fn invalid(offset: usize, reason: &'static str) -> Error {
    Error::InvalidDescriptor { offset, reason }
}

/// A primitive (base) type, including `void` for method return types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaseType {
    /// `B` — `byte`.
    Byte,
    /// `C` — `char`.
    Char,
    /// `D` — `double`.
    Double,
    /// `F` — `float`.
    Float,
    /// `I` — `int`.
    Int,
    /// `J` — `long`.
    Long,
    /// `S` — `short`.
    Short,
    /// `Z` — `boolean`.
    Boolean,
    /// `V` — `void` (return types only).
    Void,
}

impl BaseType {
    /// Parses a base-type character, or returns `None`.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        Some(match byte {
            b'B' => Self::Byte,
            b'C' => Self::Char,
            b'D' => Self::Double,
            b'F' => Self::Float,
            b'I' => Self::Int,
            b'J' => Self::Long,
            b'S' => Self::Short,
            b'Z' => Self::Boolean,
            b'V' => Self::Void,
            _ => return None,
        })
    }

    /// Returns the descriptor byte.
    #[must_use]
    pub const fn as_byte(self) -> u8 {
        match self {
            Self::Byte => b'B',
            Self::Char => b'C',
            Self::Double => b'D',
            Self::Float => b'F',
            Self::Int => b'I',
            Self::Long => b'J',
            Self::Short => b'S',
            Self::Boolean => b'Z',
            Self::Void => b'V',
        }
    }

    /// Number of local-variable/operand-stack slots used.
    #[must_use]
    pub const fn slot_count(self) -> u16 {
        match self {
            Self::Long | Self::Double => 2,
            _ => 1,
        }
    }
}

impl fmt::Display for BaseType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Byte => "byte",
            Self::Char => "char",
            Self::Double => "double",
            Self::Float => "float",
            Self::Int => "int",
            Self::Long => "long",
            Self::Short => "short",
            Self::Boolean => "boolean",
            Self::Void => "void",
        })
    }
}

/// A field type as it appears in a field or method descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType<'a> {
    /// A primitive type.
    Base(BaseType),
    /// An object type; holds the internal (slash-separated) class name.
    Object(Mutf8<'a>),
    /// An array type.
    Array(Box<FieldType<'a>>),
}

impl<'a> FieldType<'a> {
    /// Parses a field type from the front of `input`, returning it together
    /// with the unconsumed remainder.
    pub fn parse(input: &'a [u8]) -> Result<(Self, &'a [u8])> {
        let (&first, rest) = input
            .split_first()
            .ok_or_else(|| invalid(0, "empty field type"))?;
        match first {
            b'L' => {
                let end = rest
                    .iter()
                    .position(|&b| b == b';')
                    .ok_or_else(|| invalid(0, "unterminated object type"))?;
                let name = &rest[..end];
                if name.is_empty() {
                    return Err(invalid(1, "empty class name"));
                }
                Ok((Self::Object(Mutf8::new(name)), &rest[end + 1..]))
            }
            b'[' => {
                let (component, remainder) = Self::parse(rest)?;
                Ok((Self::Array(Box::new(component)), remainder))
            }
            other => {
                let base = BaseType::from_byte(other)
                    .filter(|b| *b != BaseType::Void)
                    .ok_or_else(|| invalid(0, "invalid field type character"))?;
                Ok((Self::Base(base), rest))
            }
        }
    }

    /// Number of local-variable/operand-stack slots used by this type.
    #[must_use]
    pub fn slot_count(&self) -> u16 {
        match self {
            Self::Base(base) => base.slot_count(),
            Self::Object(_) | Self::Array(_) => 1,
        }
    }

    /// Appends the descriptor encoding of this type to `out`.
    pub fn write_to(&self, out: &mut Vec<u8>) {
        match self {
            Self::Base(base) => out.push(base.as_byte()),
            Self::Object(name) => {
                out.push(b'L');
                out.extend_from_slice(name.as_bytes());
                out.push(b';');
            }
            Self::Array(component) => {
                out.push(b'[');
                component.write_to(out);
            }
        }
    }

    /// Returns the descriptor encoding of this type.
    #[must_use]
    pub fn to_descriptor(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_to(&mut out);
        out
    }
}

impl fmt::Display for FieldType<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Base(base) => write!(f, "{base}"),
            Self::Object(name) => write!(f, "{name}"),
            Self::Array(component) => write!(f, "{component}[]"),
        }
    }
}

/// A parsed method descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodDescriptor<'a> {
    /// Parameter types, in order.
    pub parameters: Vec<FieldType<'a>>,
    /// Return type, or `None` for `void`.
    pub return_type: Option<FieldType<'a>>,
}

impl<'a> MethodDescriptor<'a> {
    /// Parses a complete method descriptor.
    pub fn parse(input: &'a [u8]) -> Result<Self> {
        if input.first() != Some(&b'(') {
            return Err(invalid(0, "method descriptor must start with '('"));
        }
        let mut rest = &input[1..];
        let mut parameters = Vec::new();
        loop {
            match rest.first() {
                Some(b')') => {
                    rest = &rest[1..];
                    break;
                }
                Some(_) => {
                    let consumed = input.len() - rest.len();
                    let (param, remainder) = FieldType::parse(rest)
                        .map_err(|_| invalid(consumed, "invalid parameter type"))?;
                    parameters.push(param);
                    rest = remainder;
                }
                None => return Err(invalid(input.len(), "unterminated parameter list")),
            }
        }
        let return_offset = input.len() - rest.len();
        let return_type = match rest.first() {
            Some(b'V') => {
                rest = &rest[1..];
                None
            }
            Some(_) => {
                let (ty, remainder) = FieldType::parse(rest)
                    .map_err(|_| invalid(return_offset, "invalid return type"))?;
                rest = remainder;
                Some(ty)
            }
            None => return Err(invalid(return_offset, "missing return type")),
        };
        if !rest.is_empty() {
            return Err(invalid(
                input.len() - rest.len(),
                "trailing bytes after method descriptor",
            ));
        }
        Ok(Self {
            parameters,
            return_type,
        })
    }

    /// Sum of the parameter slot counts (for `max_locals` bookkeeping).
    #[must_use]
    pub fn parameter_slot_count(&self) -> u16 {
        self.parameters.iter().map(FieldType::slot_count).sum()
    }

    /// Appends the descriptor encoding to `out`.
    pub fn write_to(&self, out: &mut Vec<u8>) {
        out.push(b'(');
        for parameter in &self.parameters {
            parameter.write_to(out);
        }
        out.push(b')');
        match &self.return_type {
            Some(ty) => ty.write_to(out),
            None => out.push(b'V'),
        }
    }

    /// Returns the descriptor encoding.
    #[must_use]
    pub fn to_descriptor(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_to(&mut out);
        out
    }
}

impl fmt::Display for MethodDescriptor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(")?;
        for (i, parameter) in self.parameters.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{parameter}")?;
        }
        f.write_str(") -> ")?;
        match &self.return_type {
            Some(ty) => write!(f, "{ty}"),
            None => f.write_str("void"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BaseType, FieldType, MethodDescriptor};
    use crate::mutf8::Mutf8;

    #[test]
    fn parses_field_types() {
        assert_eq!(
            FieldType::parse(b"I").unwrap(),
            (FieldType::Base(BaseType::Int), &b""[..])
        );
        assert_eq!(
            FieldType::parse(b"[[J").unwrap(),
            (
                FieldType::Array(Box::new(FieldType::Array(Box::new(FieldType::Base(
                    BaseType::Long
                ))))),
                &b""[..]
            )
        );
        assert_eq!(
            FieldType::parse(b"Ljava/lang/String;x").unwrap(),
            (
                FieldType::Object(Mutf8::new(b"java/lang/String")),
                &b"x"[..]
            )
        );
    }

    #[test]
    fn rejects_void_field_type() {
        assert!(FieldType::parse(b"V").is_err());
    }

    #[test]
    fn parses_and_round_trips_method_descriptor() {
        let input = b"(ILjava/lang/String;[D)J";
        let descriptor = MethodDescriptor::parse(input).unwrap();
        assert_eq!(descriptor.parameters.len(), 3);
        assert_eq!(descriptor.parameter_slot_count(), 1 + 1 + 1);
        assert_eq!(
            descriptor.return_type,
            Some(FieldType::Base(BaseType::Long))
        );
        assert_eq!(descriptor.to_descriptor(), input);
    }

    #[test]
    fn parses_void_method() {
        let descriptor = MethodDescriptor::parse(b"()V").unwrap();
        assert!(descriptor.parameters.is_empty());
        assert_eq!(descriptor.return_type, None);
    }

    #[test]
    fn rejects_malformed_descriptors() {
        assert!(MethodDescriptor::parse(b"II)V").is_err());
        assert!(MethodDescriptor::parse(b"()").is_err());
        assert!(MethodDescriptor::parse(b"(I)Vx").is_err());
    }
}
