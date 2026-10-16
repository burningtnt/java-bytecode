//! Port of `ClassWriter` (class header/declaration, fields, methods, `Code`).

use java_bytecode::{
    Attribute, AttributeBody, ClassFile, CodeAttribute, FieldType, Instruction, MethodDescriptor,
    MethodInfo,
};

use super::raw;
use super::signature::{
    MethodSig, Sig, SigPrinter, parse_class_signature, parse_method_signature,
    parse_type_signature, sig_from_field_type,
};
use super::{Header, Javap, Loc, class_modifiers, get_java_name, known_flag_names, modifier_names};

/// Renders `class` as javap `-p -c -v` would, given the file metadata javap
/// prints in its header.
pub fn disassemble(class: &ClassFile<'_>, header: &Header) -> String {
    let mut javap = Javap::new(class, header);
    javap.write_class();
    javap.w.finish()
}

impl<'a, 'b> Javap<'a, 'b> {
    fn write_class(&mut self) {
        self.write_header();

        if let Some(source) = self.source_file() {
            self.w.println_str(&format!("Compiled from \"{source}\""));
        }

        self.w.indent_by(-1);

        let flags = self.class.access_flags.bits();
        let major = self.major_version();

        let class_mask = self.report_flags(Loc::Class, flags);
        self.write_modifiers(class_modifiers(class_mask, major));

        if flags & 0x8000 != 0 {
            match self.module_info() {
                Some((open, name, version)) => {
                    if open {
                        self.w.print("open ");
                    }
                    self.w.print("module ");
                    self.w.print(&name);
                    if let Some(version) = version {
                        self.w.print("@");
                        self.w.print(&version);
                    }
                }
                None => {
                    self.w.print("class ");
                    let name = get_java_name(&self.this_class_name());
                    self.w.print(&name);
                }
            }
        } else {
            if flags & 0x0200 == 0 {
                self.w.print("class ");
            } else {
                self.w.print("interface ");
            }
            let name = get_java_name(&self.this_class_name());
            self.w.print(&name);
        }

        let printer = SigPrinter::new(true);
        if let Some(signature) = self.signature_string(&self.class.attributes) {
            let class_sig = parse_class_signature(&signature);
            let text = printer.print_class(&class_sig, flags & 0x0200 != 0);
            self.w.print(&text);
        } else {
            let is_interface = flags & 0x0200 != 0;
            if !is_interface && raw(self.class.super_class) != 0 {
                let super_name = get_java_name(&self.class_name(raw(self.class.super_class)));
                if super_name != "java.lang.Object" {
                    self.w.print(" extends ");
                    self.w.print(&super_name);
                }
            }
            for (index, interface) in self.class.interfaces.iter().enumerate() {
                if index == 0 {
                    self.w.print(if is_interface {
                        " extends "
                    } else {
                        " implements "
                    });
                } else {
                    self.w.print(",");
                }
                let name = get_java_name(&self.class_name(raw(*interface)));
                self.w.print(&name);
            }
        }

        self.w.println();
        self.w.indent_by(1);
        self.w
            .println_str(&format!("minor version: {}", self.class.minor_version));
        self.w
            .println_str(&format!("major version: {}", self.class.major_version));

        let _ = self.report_flags(Loc::Class, flags);
        let (names, leftover) = known_flag_names(Loc::Class, flags, major);
        let mut items: Vec<String> = names.iter().map(|n| format!("ACC_{n}")).collect();
        let mut remaining = leftover;
        while remaining != 0 {
            let bit = 1u16 << (15 - remaining.leading_zeros());
            items.push(format!("0x{bit:x}"));
            remaining &= !bit;
        }
        self.write_list(&format!("flags: (0x{flags:04x}) "), &items, "\n");

        self.w.print("this_class: #");
        self.w.print(&self.this_class_index.to_string());
        self.w.tab();
        self.w.print(&format!("// {}", self.this_class_name()));
        self.w.println();

        self.w.print("super_class: #");
        let super_index = raw(self.class.super_class);
        self.w.print(&super_index.to_string());
        if super_index != 0 {
            self.w.tab();
            self.w
                .print(&format!("// {}", self.class_name(super_index)));
        }
        self.w.println();

        self.w.print("interfaces: ");
        self.w.print(&self.class.interfaces.len().to_string());
        self.w
            .print(&format!(", fields: {}", self.class.fields.len()));
        self.w
            .print(&format!(", methods: {}", self.class.methods.len()));
        self.w
            .println_str(&format!(", attributes: {}", self.class.attributes.len()));
        self.w.indent_by(-1);
        self.write_constant_pool();

        self.w.println_str("{");
        self.w.indent_by(1);
        self.write_fields();
        self.write_methods();
        self.w.indent_by(-1);
        self.w.println_str("}");

        let attributes = self.class.attributes.clone();
        self.write_attributes(&attributes, None, None);
    }

    fn write_header(&mut self) {
        self.w
            .println_str(&format!("Classfile {}", self.header.path));
        self.w.indent_by(1);
        let date = format_date_utc(self.header.last_modified_millis);
        self.w.println_str(&format!(
            "Last modified {date}; size {} bytes",
            self.header.size
        ));
        let digest: String = self
            .header
            .digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        self.w.println_str(&format!("SHA-256 checksum {digest}"));
    }

    fn write_fields(&mut self) {
        let major = self.major_version();
        for index in 0..self.class.fields.len() {
            let mask = self.class.fields[index].access_flags.bits();
            let name = self.utf8(raw(self.class.fields[index].name_index));
            let descriptor = self.utf8(raw(self.class.fields[index].descriptor_index));
            let attributes = self.class.fields[index].attributes.clone();
            let ty = self.field_signature(&attributes, &descriptor);

            let field_mask = self.report_flags(Loc::Field, mask);
            self.write_modifiers(modifier_names(Loc::Field, field_mask, major));
            self.w.print(&ty);
            self.w.print(" ");
            self.w.print(&name);
            self.w.print(";");
            self.w.println();

            self.w.indent_by(1);
            self.w.print("descriptor: ");
            self.w.println_str(&descriptor);

            let _ = self.report_flags(Loc::Field, mask);
            let (names, _) = known_flag_names(Loc::Field, mask, major);
            let items: Vec<String> = names.iter().map(|n| format!("ACC_{n}")).collect();
            self.write_list(&format!("flags: (0x{mask:04x}) "), &items, "\n");

            self.write_attributes(&attributes, None, None);
            self.w.indent_by(-1);
            self.w.println();
        }
    }

    fn write_methods(&mut self) {
        for index in 0..self.class.methods.len() {
            self.write_method(index);
        }
        self.w.set_pending_newline(false);
    }

    fn write_method(&mut self, index: usize) {
        let major = self.major_version();
        let mask = self.class.methods[index].access_flags.bits();
        let name = self.utf8(raw(self.class.methods[index].name_index));
        let descriptor = self.utf8(raw(self.class.methods[index].descriptor_index));
        let attributes = self.class.methods[index].attributes.clone();
        let method = self.class.methods[index].clone();

        let method_mask = self.report_flags(Loc::Method, mask);
        let mut items = modifier_names(Loc::Method, method_mask, major);
        if self.class.access_flags.bits() & 0x0200 != 0
            && mask & 0x0400 == 0
            && name != "<clinit>"
            && mask & (0x0008 | 0x0002) == 0
        {
            items.push("default".to_owned());
        }
        self.write_modifiers(items);

        let method_sig = self.method_signature(&attributes, &descriptor);
        let printer = SigPrinter::new(true);
        if !method_sig.params.is_empty() {
            let text = format!("{} ", printer.print_type_params(&method_sig.params));
            self.w.print(&text);
        }
        let param_types = get_java_name(&adjust_varargs(
            mask,
            printer.print_list("(", &method_sig.args, ")"),
        ));
        match name.as_str() {
            "<init>" => {
                let this_name = get_java_name(&self.this_class_name());
                self.w.print(&this_name);
                self.w.print(&param_types);
            }
            "<clinit>" => self.w.print("{}"),
            _ => {
                let result = get_java_name(&printer.print(&method_sig.result));
                self.w.print(&result);
                self.w.print(" ");
                self.w.print(&name);
                self.w.print(&param_types);
            }
        }

        let exceptions = match self.find_body(&attributes, "Exceptions") {
            Some(AttributeBody::Exceptions(exceptions)) => Some(exceptions.clone()),
            _ => None,
        };
        if let Some(exceptions) = exceptions {
            self.w.print(" throws ");
            if !method_sig.throws.is_empty() {
                let text = printer.print_list("", &method_sig.throws, "");
                self.w.print(&text);
            } else {
                for (i, exception) in exceptions.exception_index_table.iter().enumerate() {
                    if i > 0 {
                        self.w.print(", ");
                    }
                    let name = get_java_name(&self.class_name(raw(*exception)));
                    self.w.print(&name);
                }
            }
        }

        self.w.println_str(";");
        self.w.indent_by(1);

        self.w.print("descriptor: ");
        self.w.println_str(&descriptor);

        let _ = self.report_flags(Loc::Method, mask);
        let (names, _) = known_flag_names(Loc::Method, mask, major);
        let flag_items: Vec<String> = names.iter().map(|n| format!("ACC_{n}")).collect();
        let mut line = format!("flags: (0x{mask:04x}) ");
        let mut sep = "";
        for item in &flag_items {
            line.push_str(sep);
            line.push_str(item);
            sep = ", ";
        }
        self.w.println_str(&line);

        self.write_attributes(&attributes, None, Some(&method));

        self.w.indent_by(-1);
        self.w.set_pending_newline(true);
    }

    pub(crate) fn write_code(&mut self, code: &CodeAttribute<'b>, method: Option<&MethodInfo<'b>>) {
        self.w.println_str("Code:");
        self.w.indent_by(1);

        let descriptor = method.map(|m| self.utf8(raw(m.descriptor_index)));
        let param_count = descriptor
            .as_deref()
            .and_then(|d| MethodDescriptor::parse(d.as_bytes()).ok())
            .map_or(0, |d| d.parameters.len());
        let is_static = method.is_some_and(|m| m.access_flags.bits() & 0x0008 != 0);
        let args_size = param_count + usize::from(!is_static);
        self.w.println_str(&format!(
            "stack={}, locals={}, args_size={}",
            code.max_stack, code.max_locals, args_size
        ));

        self.write_instructions(code);
        self.write_exception_table(code);

        let attributes = code.attributes.clone();
        self.write_attributes(&attributes, Some(code), method);

        self.w.indent_by(-1);
    }

    fn write_instructions(&mut self, code: &CodeAttribute<'b>) {
        let instructions = code.instructions().expect("decodable bytecode");
        let mut pc = 0usize;
        for instruction in &instructions {
            self.write_instruction(pc, instruction);
            pc += instruction.encoded_len(pc);
        }
    }

    fn write_instruction(&mut self, pc: usize, instruction: &Instruction<'b>) {
        let mnemonic = match instruction {
            Instruction::WideLocal { opcode, .. } => format!("{}_w", opcode.mnemonic()),
            Instruction::IincWide { .. } => "iinc_w".to_owned(),
            _ => instruction.opcode().mnemonic().to_owned(),
        };
        let mnemonic = mnemonic.to_ascii_lowercase();
        self.w.print(&format!("{pc:>4}: {mnemonic:<13} "));

        match instruction {
            Instruction::None(_) => {}
            Instruction::Byte(_, value) => self.w.print(&value.to_string()),
            Instruction::Short(_, value) => self.w.print(&value.to_string()),
            Instruction::Local(_, index) => {
                if instruction.encoded_len(pc) > 1 {
                    self.w.print(&index.to_string());
                }
            }
            Instruction::Constant(_, reference) => self.print_constant_pool_ref(raw(*reference)),
            Instruction::ConstantWide(_, reference) => {
                self.print_constant_pool_ref(raw(*reference));
            }
            Instruction::Branch(_, offset) => {
                self.w.print(&(pc as i64 + i64::from(*offset)).to_string());
            }
            Instruction::BranchWide(_, offset) => {
                self.w.print(&(pc as i64 + i64::from(*offset)).to_string());
            }
            Instruction::Iinc { index, amount } => {
                self.w.print(&format!("{index}, {amount}"));
            }
            Instruction::IincWide { index, amount } => {
                self.w.print(&format!("{index}, {amount}"));
            }
            Instruction::WideLocal { opcode, index } => {
                if *opcode != java_bytecode::Opcode::Ret {
                    self.w.print(&index.to_string());
                }
            }
            Instruction::InvokeInterface { index, count } => {
                self.print_constant_pool_ref_and_value(raw(*index), i64::from(*count));
            }
            Instruction::InvokeDynamic { index, .. } => {
                self.print_constant_pool_ref_and_value(raw(*index), 0);
            }
            Instruction::NewArray { atype } => {
                self.w.print(" ");
                self.w.print(newarray_name(*atype));
            }
            Instruction::MultiANewArray { index, dimensions } => {
                self.print_constant_pool_ref_and_value(raw(*index), i64::from(*dimensions));
            }
            Instruction::TableSwitch {
                default,
                low,
                high,
                offsets,
                ..
            } => {
                self.w.print(&format!("{{ // {low} to {high}"));
                self.w.indent_by(3);
                for value in *low..=*high {
                    let slot = (value - *low) as usize;
                    let target = pc as i64 + i64::from(offsets[slot]);
                    self.w.print(&format!("\n{value:>12}: {target}"));
                }
                let default_target = pc as i64 + i64::from(*default);
                self.w
                    .print(&format!("\n     default: {default_target}\n}}"));
                self.w.indent_by(-3);
            }
            Instruction::LookupSwitch { default, pairs, .. } => {
                self.w.print(&format!("{{ // {}", pairs.len()));
                self.w.indent_by(3);
                for (key, offset) in pairs {
                    let target = pc as i64 + i64::from(*offset);
                    self.w.print(&format!("\n{key:>12}: {target}"));
                }
                let default_target = pc as i64 + i64::from(*default);
                self.w
                    .print(&format!("\n     default: {default_target}\n}}"));
                self.w.indent_by(-3);
            }
            _ => {}
        }
        self.w.println();
    }

    fn print_constant_pool_ref(&mut self, index: u16) {
        self.w.print(&format!("#{index}"));
        self.w.tab();
        self.w.print("// ");
        self.write_constant_ref(index);
    }

    fn print_constant_pool_ref_and_value(&mut self, index: u16, value: i64) {
        self.w.print(&format!("#{index},  {value}"));
        self.w.tab();
        self.w.print("// ");
        self.write_constant_ref(index);
    }

    fn write_exception_table(&mut self, code: &CodeAttribute<'b>) {
        if code.exception_table.is_empty() {
            return;
        }
        self.w.println_str("Exception table:");
        self.w.indent_by(1);
        self.w.println_str(" from    to  target type");
        for handler in &code.exception_table {
            self.w.print(&format!(
                " {:>5} {:>5} {:>5}",
                handler.start_pc, handler.end_pc, handler.handler_pc
            ));
            self.w.print("   ");
            let catch_index = raw(handler.catch_type);
            if catch_index == 0 {
                self.w.println_str("any");
            } else {
                self.w.print("Class ");
                let name = super::constants::check_name(&self.class_name(catch_index));
                self.w.println_str(&name);
            }
        }
        self.w.indent_by(-1);
    }

    fn write_modifiers(&mut self, items: Vec<String>) {
        for item in items {
            self.w.print(&item);
            self.w.print(" ");
        }
    }

    pub(crate) fn write_list(&mut self, prefix: &str, items: &[String], suffix: &str) {
        self.w.print(prefix);
        let mut sep = "";
        for item in items {
            self.w.print(sep);
            self.w.print(item);
            sep = ", ";
        }
        self.w.print(suffix);
    }

    fn source_file(&self) -> Option<String> {
        match self.find_body(&self.class.attributes, "SourceFile")? {
            AttributeBody::SourceFile(source) => Some(self.utf8(raw(source.sourcefile_index))),
            _ => None,
        }
    }

    /// `(is_open, name, version)` for a module class.
    fn module_info(&self) -> Option<(bool, String, Option<String>)> {
        match self.find_body(&self.class.attributes, "Module")? {
            AttributeBody::Module(module) => {
                let name = match self.cp(raw(module.module_name_index)) {
                    Some(java_bytecode::Constant::Module { name_index }) => {
                        self.utf8(raw(*name_index))
                    }
                    _ => String::new(),
                };
                let version = (raw(module.module_version_index) != 0)
                    .then(|| self.utf8(raw(module.module_version_index)));
                Some((module.module_flags.bits() & 0x0020 != 0, name, version))
            }
            _ => None,
        }
    }

    pub(crate) fn find_body<'c>(
        &self,
        attrs: &'c [Attribute<'b>],
        name: &str,
    ) -> Option<&'c AttributeBody<'b>> {
        let pool = &self.class.constant_pool;
        attrs.iter().find_map(|attr| {
            (attr.resolved_name(pool).ok().as_deref() == Some(name)).then_some(&attr.body)
        })
    }

    fn signature_string(&self, attrs: &[Attribute<'b>]) -> Option<String> {
        match self.find_body(attrs, "Signature")? {
            AttributeBody::Signature(signature) => Some(self.utf8(raw(signature.signature_index))),
            _ => None,
        }
    }

    fn field_signature(&self, attrs: &[Attribute<'b>], descriptor: &str) -> String {
        let printer = SigPrinter::new(true);
        if let Some(signature) = self.signature_string(attrs) {
            return printer.print(&parse_type_signature(&signature));
        }
        let (ty, _) = FieldType::parse(descriptor.as_bytes()).expect("valid field descriptor");
        printer.print(&sig_from_field_type(&ty))
    }

    fn method_signature(&self, attrs: &[Attribute<'b>], descriptor: &str) -> MethodSig {
        if let Some(signature) = self.signature_string(attrs) {
            return parse_method_signature(&signature);
        }
        method_sig_from_descriptor(descriptor)
    }
}

fn method_sig_from_descriptor(descriptor: &str) -> MethodSig {
    let parsed = MethodDescriptor::parse(descriptor.as_bytes()).expect("valid method descriptor");
    let args = parsed.parameters.iter().map(sig_from_field_type).collect();
    let result = match &parsed.return_type {
        Some(ty) => sig_from_field_type(ty),
        None => Sig::Base('V'),
    };
    MethodSig {
        params: Vec::new(),
        args,
        result,
        throws: Vec::new(),
    }
}

pub(crate) fn adjust_varargs(mask: u16, params: String) -> String {
    if mask & 0x0080 != 0 {
        if let Some(index) = params.rfind("[]") {
            if index > 0 {
                let mut out = String::with_capacity(params.len());
                out.push_str(&params[..index]);
                out.push_str("...");
                out.push_str(&params[index + 2..]);
                return out;
            }
        }
    }
    params
}

fn newarray_name(atype: u8) -> &'static str {
    match atype {
        4 => "boolean",
        5 => "char",
        6 => "float",
        7 => "double",
        8 => "byte",
        9 => "short",
        10 => "int",
        11 => "long",
        _ => "?",
    }
}

/// Formats an epoch-milliseconds timestamp as `DateFormat.getDateInstance()` in
/// the `en_US` locale under UTC (`MMM d, yyyy`), matching the test harness which
/// pins javap's timezone and locale.
fn format_date_utc(millis: i64) -> String {
    let days = millis.div_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!("{} {}, {}", MONTHS[(month - 1) as usize], day, year)
}

/// Howard Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}
