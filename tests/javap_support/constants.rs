//! Constant-pool rendering: `ConstantWriter` and its `stringValue` helpers.

use java_bytecode::{Constant, Mutf8};

use super::{Javap, java_float, raw};

impl<'a, 'b> Javap<'a, 'b> {
    pub(crate) fn write_constant_pool(&mut self) {
        let count = self.class.constant_pool.len();
        self.w.println_str("Constant pool:");
        self.w.indent_by(1);
        let width = count.to_string().len() + 1;
        let mut cpx: u16 = 1;
        while (cpx as usize) < count {
            self.w
                .print(&format!("{:>width$}", format!("#{cpx}"), width = width));
            let Some(constant) = self.cp(cpx).cloned() else {
                cpx += 1;
                continue;
            };
            self.w
                .print(&format!(" = {:<18} ", cp_tag_name(constant.tag())));

            match &constant {
                Constant::Utf8(_)
                | Constant::Integer(_)
                | Constant::Float(_)
                | Constant::Long(_)
                | Constant::Double(_) => {
                    let value = self.string_value(&constant);
                    self.w.println_str(&value);
                }
                Constant::Class { name_index } => {
                    self.w.print(&format!("#{}", raw(*name_index)));
                    self.w.tab();
                    let value = self.string_value(&constant);
                    self.w.println_str(&format!("// {value}"));
                }
                Constant::Fieldref {
                    class_index,
                    name_and_type_index,
                }
                | Constant::Methodref {
                    class_index,
                    name_and_type_index,
                }
                | Constant::InterfaceMethodref {
                    class_index,
                    name_and_type_index,
                } => {
                    self.w.print(&format!(
                        "#{}.#{}",
                        raw(*class_index),
                        raw(*name_and_type_index)
                    ));
                    self.w.tab();
                    let value = self.string_value(&constant);
                    self.w.println_str(&format!("// {value}"));
                }
                Constant::Dynamic {
                    bootstrap_method_attr_index,
                    name_and_type_index,
                }
                | Constant::InvokeDynamic {
                    bootstrap_method_attr_index,
                    name_and_type_index,
                } => {
                    self.w.print(&format!(
                        "#{}:#{}",
                        bootstrap_method_attr_index,
                        raw(*name_and_type_index)
                    ));
                    self.w.tab();
                    let value = self.string_value(&constant);
                    self.w.println_str(&format!("// {value}"));
                }
                Constant::MethodHandle {
                    reference_kind,
                    reference_index,
                } => {
                    self.w
                        .print(&format!("{}:#{}", reference_kind, raw(*reference_index)));
                    self.w.tab();
                    let value = self.string_value(&constant);
                    self.w.println_str(&format!("// {value}"));
                }
                Constant::MethodType { descriptor_index } => {
                    self.w.print(&format!("#{}", raw(*descriptor_index)));
                    self.w.tab();
                    let value = self.string_value(&constant);
                    self.w.println_str(&format!("//  {value}"));
                }
                Constant::Module { name_index } | Constant::Package { name_index } => {
                    self.w.print(&format!("#{}", raw(*name_index)));
                    self.w.tab();
                    let value = self.string_value(&constant);
                    self.w.println_str(&format!("// {value}"));
                }
                Constant::NameAndType {
                    name_index,
                    descriptor_index,
                } => {
                    self.w.print(&format!(
                        "#{}:#{}",
                        raw(*name_index),
                        raw(*descriptor_index)
                    ));
                    self.w.tab();
                    let value = self.string_value(&constant);
                    self.w.println_str(&format!("// {value}"));
                }
                Constant::String { string_index } => {
                    self.w.print(&format!("#{}", raw(*string_index)));
                    self.w.tab();
                    let value = self.string_value(&constant);
                    self.w.println_str(&format!("// {value}"));
                }
                Constant::Invalid => unreachable!("invalid slots are not visited"),
            }

            let width_in_slots = if constant.takes_two_slots() { 2 } else { 1 };
            cpx += width_in_slots;
        }
        self.w.indent_by(-1);
    }

    /// `ConstantWriter.write`, used inside bytecode comments and attributes.
    pub(crate) fn write_constant_ref(&mut self, index: u16) {
        if index == 0 {
            self.w.print("#0");
            return;
        }
        let original = self.cp(index).cloned();
        let Some(original) = original else {
            self.w.print(&format!("#{index}"));
            return;
        };
        let tag = original.tag();
        let mut value = &original;
        let simplified;
        if let Constant::Fieldref {
            class_index,
            name_and_type_index,
        }
        | Constant::Methodref {
            class_index,
            name_and_type_index,
        }
        | Constant::InterfaceMethodref {
            class_index,
            name_and_type_index,
        } = &original
        {
            if raw(*class_index) == self.this_class_index {
                simplified = self.cp(raw(*name_and_type_index)).cloned();
                if let Some(simplified) = simplified.as_ref() {
                    value = simplified;
                }
            }
        }
        let text = self.string_value(value);
        self.w.print(&format!("{} {}", tag_name(tag), text));
    }

    pub(crate) fn string_value_at(&self, index: u16) -> String {
        match self.cp(index) {
            Some(constant) => self.string_value(constant),
            None => format!("#{index}"),
        }
    }

    pub(crate) fn string_value(&self, constant: &Constant<'b>) -> String {
        match &constant {
            Constant::Utf8(value) => escape_utf8(*value),
            Constant::Integer(value) => value.to_string(),
            Constant::Float(bits) => {
                format!("{}f", java_float::f32_to_string(f32::from_bits(*bits)))
            }
            Constant::Long(value) => format!("{value}l"),
            Constant::Double(bits) => {
                format!("{}d", java_float::f64_to_string(f64::from_bits(*bits)))
            }
            Constant::Class { name_index } => check_name(&self.utf8(raw(*name_index))),
            Constant::String { string_index } => self.string_value_at(raw(*string_index)),
            Constant::Fieldref {
                class_index,
                name_and_type_index,
            }
            | Constant::Methodref {
                class_index,
                name_and_type_index,
            }
            | Constant::InterfaceMethodref {
                class_index,
                name_and_type_index,
            } => {
                let owner = check_name(&self.class_name(raw(*class_index)));
                let nat = self.string_value_at(raw(*name_and_type_index));
                format!("{owner}.{nat}")
            }
            Constant::NameAndType {
                name_index,
                descriptor_index,
            } => {
                let name = check_name(&self.utf8(raw(*name_index)));
                let descriptor = self.utf8(raw(*descriptor_index));
                format!("{name}:{descriptor}")
            }
            Constant::MethodHandle {
                reference_kind,
                reference_index,
            } => {
                let kind = ref_kind_name(*reference_kind);
                let reference = self.string_value_at(raw(*reference_index));
                format!("{kind} {reference}")
            }
            Constant::MethodType { descriptor_index } => self.utf8(raw(*descriptor_index)),
            Constant::Dynamic {
                bootstrap_method_attr_index,
                name_and_type_index,
            }
            | Constant::InvokeDynamic {
                bootstrap_method_attr_index,
                name_and_type_index,
            } => {
                let nat = self.string_value_at(raw(*name_and_type_index));
                format!("#{bootstrap_method_attr_index}:{nat}")
            }
            Constant::Module { name_index } | Constant::Package { name_index } => {
                check_name(&self.utf8(raw(*name_index)))
            }
            Constant::Invalid => String::new(),
        }
    }
}

pub(crate) fn cp_tag_name(tag: u8) -> &'static str {
    match tag {
        1 => "Utf8",
        3 => "Integer",
        4 => "Float",
        5 => "Long",
        6 => "Double",
        7 => "Class",
        8 => "String",
        9 => "Fieldref",
        15 => "MethodHandle",
        16 => "MethodType",
        10 => "Methodref",
        11 => "InterfaceMethodref",
        18 => "InvokeDynamic",
        17 => "Dynamic",
        12 => "NameAndType",
        _ => "Unknown",
    }
}

pub(crate) fn tag_name(tag: u8) -> &'static str {
    match tag {
        1 => "Utf8",
        3 => "int",
        4 => "float",
        5 => "long",
        6 => "double",
        7 => "class",
        8 => "String",
        9 => "Field",
        15 => "MethodHandle",
        16 => "MethodType",
        10 => "Method",
        11 => "InterfaceMethod",
        18 => "InvokeDynamic",
        17 => "Dynamic",
        12 => "NameAndType",
        _ => "(unknown tag)",
    }
}

fn ref_kind_name(kind: u8) -> &'static str {
    match kind {
        1 => "REF_getField",
        2 => "REF_getStatic",
        3 => "REF_putField",
        4 => "REF_putStatic",
        5 => "REF_invokeVirtual",
        6 => "REF_invokeStatic",
        7 => "REF_invokeSpecial",
        8 => "REF_newInvokeSpecial",
        9 => "REF_invokeInterface",
        _ => "REF_unknown",
    }
}

/// `ConstantWriter.stringValue(Utf8Entry)` escaping.
///
/// This decodes the modified UTF-8 itself rather than going through
/// [`Mutf8::decode`], because that rejects lone surrogates. Java represents a
/// lone surrogate as-is, and its UTF-8 output encoder then emits a `?` for it;
/// javac's charset tables rely on this, so we reproduce it.
pub(crate) fn escape_utf8(value: Mutf8<'_>) -> String {
    let bytes = value.as_ref();
    let mut decoded = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        if b0 < 0x80 {
            decoded.push(char::from(b0));
            i += 1;
            continue;
        }
        if b0 & 0xE0 == 0xC0 {
            if let Some(&b1) = bytes.get(i + 1).filter(|b| *b & 0xC0 == 0x80) {
                let cp = (u32::from(b0 & 0x1F) << 6) | u32::from(b1 & 0x3F);
                decoded.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                i += 2;
            } else {
                decoded.push('\u{FFFD}');
                i += 1;
            }
            continue;
        }
        if b0 & 0xF0 == 0xE0 {
            if i + 2 < bytes.len() && bytes[i + 1] & 0xC0 == 0x80 && bytes[i + 2] & 0xC0 == 0x80 {
                let cp = (u32::from(b0 & 0x0F) << 12)
                    | (u32::from(bytes[i + 1] & 0x3F) << 6)
                    | u32::from(bytes[i + 2] & 0x3F);
                if (0xD800..=0xDBFF).contains(&cp) {
                    // A high surrogate: combine with a following low half into a
                    // supplementary code point, otherwise emit the `?` that
                    // Java's UTF-8 output encoder produces for a lone surrogate.
                    let j = i + 3;
                    let low = if j + 2 < bytes.len()
                        && bytes[j] & 0xF0 == 0xE0
                        && bytes[j + 1] & 0xC0 == 0x80
                        && bytes[j + 2] & 0xC0 == 0x80
                    {
                        Some(
                            (u32::from(bytes[j] & 0x0F) << 12)
                                | (u32::from(bytes[j + 1] & 0x3F) << 6)
                                | u32::from(bytes[j + 2] & 0x3F),
                        )
                    } else {
                        None
                    };
                    match low.filter(|lo| (0xDC00..=0xDFFF).contains(lo)) {
                        Some(lo) => {
                            let combined = 0x1_0000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                            decoded.push(char::from_u32(combined).unwrap_or('\u{FFFD}'));
                            i += 6;
                        }
                        None => {
                            decoded.push('?');
                            i += 3;
                        }
                    }
                } else if (0xDC00..=0xDFFF).contains(&cp) {
                    decoded.push('?');
                    i += 3;
                } else {
                    decoded.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                    i += 3;
                }
            } else {
                decoded.push('\u{FFFD}');
                i += 1;
            }
            continue;
        }
        decoded.push('\u{FFFD}');
        i += 1;
    }

    let mut out = String::with_capacity(decoded.len());
    for c in decoded.chars() {
        match c {
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '"' => out.push_str("\\\""),
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            _ if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(c),
        }
    }
    out
}

fn is_java_identifier_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$' || (c as u32) > 0x7f
}

fn is_java_identifier_part(c: char) -> bool {
    is_java_identifier_start(c) || c.is_ascii_digit()
}

/// `ConstantWriter.checkName`: quote a name that is not a valid binary name.
pub(crate) fn check_name(name: &str) -> String {
    if name.is_empty() {
        return "\"\"".to_owned();
    }
    let mut previous = '/';
    for c in name.chars() {
        let bad = (previous == '/' && !is_java_identifier_start(c))
            || (c != '/' && !is_java_identifier_part(c));
        if bad {
            return format!("\"{}\"", add_escapes(name));
        }
        previous = c;
    }
    name.to_owned()
}

fn add_escapes(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}
