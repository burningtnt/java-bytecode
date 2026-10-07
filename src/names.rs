//! Class, interface, method, field, module and package name helpers
//! (JVMS §4.2).

use crate::mutf8::Mutf8;

/// Converts a binary class name (dot-separated, e.g. `java.lang.Thread`) to
/// the internal form used in class files (`java/lang/Thread`).
#[must_use]
pub fn binary_to_internal(name: &str) -> String {
    name.replace('.', "/")
}

/// Converts an internal class name (`java/lang/Thread`) to binary form
/// (`java.lang.Thread`).
#[must_use]
pub fn internal_to_binary(name: &str) -> String {
    name.replace('/', ".")
}

fn unqualified_bytes(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && !bytes
            .iter()
            .any(|&b| matches!(b, b'.' | b';' | b'[' | b'/'))
}

/// Returns `true` if `name` is a valid unqualified name: non-empty and free of
/// `.`, `;`, `[` and `/`.
#[must_use]
pub fn is_unqualified_name(name: Mutf8<'_>) -> bool {
    unqualified_bytes(name.as_bytes())
}

/// Returns `true` if `name` is a valid field name (an unqualified name).
#[must_use]
pub fn is_valid_field_name(name: Mutf8<'_>) -> bool {
    is_unqualified_name(name)
}

/// Returns `true` if `name` is a valid method name.
///
/// In addition to the unqualified-name rules, method names may not contain
/// `<` or `>` unless they are exactly `<init>` or `<clinit>`.
#[must_use]
pub fn is_valid_method_name(name: Mutf8<'_>) -> bool {
    let bytes = name.as_bytes();
    if !unqualified_bytes(bytes) {
        return false;
    }
    if bytes == b"<init>" || bytes == b"<clinit>" {
        return true;
    }
    !bytes.iter().any(|&b| matches!(b, b'<' | b'>'))
}

/// Returns `true` if `name` looks like a valid internal class name: non-empty
/// and every identifier separated by `/` is an unqualified name.
#[must_use]
pub fn is_valid_internal_name(name: Mutf8<'_>) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty() && bytes.split(|&b| b == b'/').all(unqualified_bytes)
}

/// Returns `true` if `name` is a valid module name (JVMS §4.2.3).
///
/// A module name is non-empty, contains no control characters or `\`, `:`, `@`,
/// and every dot-separated identifier is non-empty and unqualified.
#[must_use]
pub fn is_valid_module_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name
        .chars()
        .any(|c| c.is_control() || matches!(c, '\\' | ':' | '@'))
    {
        return false;
    }
    name.split('.')
        .all(|part| !part.is_empty() && part != "module")
}

/// Returns `true` if `name` is a valid package name (JVMS §4.2.3).
#[must_use]
pub fn is_valid_package_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name
        .chars()
        .any(|c| c.is_control() || matches!(c, '\\' | ':' | '@'))
    {
        return false;
    }
    name.split('.').all(|part| !part.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(bytes: &'static [u8]) -> Mutf8<'static> {
        Mutf8::new(bytes)
    }

    #[test]
    fn converts_binary_and_internal_names() {
        assert_eq!(binary_to_internal("java.lang.Thread"), "java/lang/Thread");
        assert_eq!(internal_to_binary("java/lang/Thread"), "java.lang.Thread");
    }

    #[test]
    fn checks_unqualified_names() {
        assert!(is_unqualified_name(m(b"foo")));
        assert!(!is_unqualified_name(m(b"")));
        assert!(!is_unqualified_name(m(b"a.b")));
        assert!(!is_unqualified_name(m(b"a;b")));
        assert!(!is_unqualified_name(m(b"a/b")));
        assert!(!is_unqualified_name(m(b"a[b")));
    }

    #[test]
    fn checks_method_names() {
        assert!(is_valid_method_name(m(b"main")));
        assert!(is_valid_method_name(m(b"<init>")));
        assert!(is_valid_method_name(m(b"<clinit>")));
        assert!(!is_valid_method_name(m(b"<foo>")));
        assert!(!is_valid_method_name(m(b"")));
    }

    #[test]
    fn checks_internal_names() {
        assert!(is_valid_internal_name(m(b"java/lang/Thread")));
        assert!(!is_valid_internal_name(m(b"java//Thread")));
        assert!(!is_valid_internal_name(m(b"")));
    }
}
