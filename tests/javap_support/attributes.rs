//! Port of `AttributeWriter` and `AnnotationWriter`.

use java_bytecode::{
    Annotation, Attribute, AttributeBody, CodeAttribute, Constant, ElementValue, ElementValuePair,
    MethodInfo, StackMapFrame, TargetInfo, TypeAnnotation, VerificationType,
};

use super::raw;
use super::signature::{SigPrinter, parse_type_signature};
use super::{Javap, Loc, get_java_name, inner_class_modifiers};

impl<'a, 'b> Javap<'a, 'b> {
    pub(crate) fn write_attributes(
        &mut self,
        attrs: &[Attribute<'b>],
        lr: Option<&CodeAttribute<'b>>,
        method: Option<&MethodInfo<'b>>,
    ) {
        for attr in attrs {
            let name = attr
                .resolved_name(&self.class.constant_pool)
                .unwrap_or_default();
            self.write_attribute(&name, attr, lr, method);
        }
    }

    fn write_attribute(
        &mut self,
        name: &str,
        attr: &Attribute<'b>,
        lr: Option<&CodeAttribute<'b>>,
        method: Option<&MethodInfo<'b>>,
    ) {
        let major = self.major_version();
        match (name, &attr.body) {
            ("Code", AttributeBody::Code(code)) => self.write_code(code, method),
            ("ConstantValue", AttributeBody::ConstantValue(a)) => {
                self.w.print("ConstantValue: ");
                self.write_constant_ref(raw(a.constantvalue_index));
                self.w.println();
            }
            ("Deprecated", _) => self.w.println_str("Deprecated: true"),
            ("Synthetic", _) => self.w.println_str("Synthetic: true"),
            ("EnclosingMethod", AttributeBody::EnclosingMethod(a)) => {
                self.w.print(&format!(
                    "EnclosingMethod: #{}.#{}",
                    raw(a.class_index),
                    raw(a.method_index)
                ));
                self.w.tab();
                self.w.print(&format!(
                    "// {}",
                    get_java_name(&self.class_name(raw(a.class_index)))
                ));
                if raw(a.method_index) != 0 {
                    if let Some(Constant::NameAndType { name_index, .. }) =
                        self.cp(raw(a.method_index))
                    {
                        let method_name = self.utf8(raw(*name_index));
                        self.w.print(&format!(".{method_name}"));
                    }
                }
                self.w.println();
            }
            ("Exceptions", AttributeBody::Exceptions(a)) => {
                self.w.println_str("Exceptions:");
                self.w.indent_by(1);
                self.w.print("throws ");
                for (i, exception) in a.exception_index_table.iter().enumerate() {
                    if i > 0 {
                        self.w.print(", ");
                    }
                    let name = get_java_name(&self.class_name(raw(*exception)));
                    self.w.print(&name);
                }
                self.w.println();
                self.w.indent_by(-1);
            }
            ("InnerClasses", AttributeBody::InnerClasses(a)) => {
                let mut first = true;
                for info in &a.classes {
                    let access = info.inner_class_access_flags.bits();
                    if first {
                        self.w.println_str("InnerClasses:");
                        self.w.indent_by(1);
                        first = false;
                    }
                    let masked = self.report_flags(Loc::InnerClass, access);
                    for modifier in inner_class_modifiers(masked, major) {
                        self.w.print(&modifier);
                        self.w.print(" ");
                    }
                    let inner_name = raw(info.inner_name_index);
                    let inner_class = raw(info.inner_class_info_index);
                    let outer_class = raw(info.outer_class_info_index);
                    if inner_name != 0 {
                        self.w.print(&format!("#{inner_name}= "));
                    }
                    self.w.print(&format!("#{inner_class}"));
                    if outer_class != 0 {
                        self.w.print(&format!(" of #{outer_class}"));
                    }
                    self.w.print(";");
                    self.w.tab();
                    self.w.print("// ");
                    if inner_name != 0 {
                        let simple = self.utf8(inner_name);
                        self.w.print(&format!("{simple}="));
                    }
                    self.write_constant_ref(inner_class);
                    if outer_class != 0 {
                        self.w.print(" of ");
                        self.write_constant_ref(outer_class);
                    }
                    self.w.println();
                }
                if !first {
                    self.w.indent_by(-1);
                }
            }
            ("LineNumberTable", AttributeBody::LineNumberTable(a)) => {
                self.w.println_str("LineNumberTable:");
                self.w.indent_by(1);
                for entry in &a.line_number_table {
                    self.w
                        .println_str(&format!("line {}: {}", entry.line_number, entry.start_pc));
                }
                self.w.indent_by(-1);
            }
            ("LocalVariableTable", AttributeBody::LocalVariableTable(a)) => {
                self.w.println_str("LocalVariableTable:");
                self.w.indent_by(1);
                self.w.println_str("Start  Length  Slot  Name   Signature");
                for entry in &a.local_variable_table {
                    let name = self.string_value_at(raw(entry.name_index));
                    let descriptor = self.string_value_at(raw(entry.descriptor_index));
                    self.w.println_str(&format!(
                        "{:>5} {:>7} {:>5} {:>5}   {}",
                        entry.start_pc, entry.length, entry.index, name, descriptor
                    ));
                }
                self.w.indent_by(-1);
            }
            ("LocalVariableTypeTable", AttributeBody::LocalVariableTypeTable(a)) => {
                self.w.println_str("LocalVariableTypeTable:");
                self.w.indent_by(1);
                self.w.println_str("Start  Length  Slot  Name   Signature");
                for entry in &a.local_variable_type_table {
                    let name = self.string_value_at(raw(entry.name_index));
                    let signature = self.string_value_at(raw(entry.signature_index));
                    self.w.println_str(&format!(
                        "{:>5} {:>7} {:>5} {:>5}   {}",
                        entry.start_pc, entry.length, entry.index, name, signature
                    ));
                }
                self.w.indent_by(-1);
            }
            ("BootstrapMethods", AttributeBody::BootstrapMethods(a)) => {
                self.w.println_str("BootstrapMethods:");
                for (i, bsm) in a.bootstrap_methods.iter().enumerate() {
                    self.w.indent_by(1);
                    let method_ref = raw(bsm.bootstrap_method_ref);
                    self.w.print(&format!("{i}: #{method_ref} "));
                    let value = self.string_value_at(method_ref);
                    self.w.println_str(&value);
                    self.w.indent_by(1);
                    self.w.println_str("Method arguments:");
                    self.w.indent_by(1);
                    for argument in &bsm.bootstrap_arguments {
                        let index = raw(*argument);
                        self.w.print(&format!("#{index} "));
                        let value = self.string_value_at(index);
                        self.w.println_str(&value);
                    }
                    self.w.indent_by(-3);
                }
            }
            ("MethodParameters", AttributeBody::MethodParameters(a)) => {
                let header = format!("{:<31}{}", "Name", "Flags");
                self.w.println_str("MethodParameters:");
                self.w.indent_by(1);
                self.w.println_str(&header);
                for parameter in &a.parameters {
                    let name = if raw(parameter.name_index) != 0 {
                        self.string_value_at(raw(parameter.name_index))
                    } else {
                        "<no name>".to_owned()
                    };
                    let flags = parameter.access_flags.bits();
                    let flagstr = format!(
                        "{}{}{}",
                        if flags & 0x0010 != 0 { "final " } else { "" },
                        if flags & 0x8000 != 0 { "mandated " } else { "" },
                        if flags & 0x1000 != 0 { "synthetic" } else { "" }
                    );
                    self.w.println_str(&format!("{name:<31}{flagstr}"));
                }
                self.w.indent_by(-1);
            }
            ("Module", AttributeBody::Module(module)) => self.write_module(module),
            ("ModulePackages", AttributeBody::ModulePackages(a)) => {
                self.w.println_str("ModulePackages: ");
                self.w.indent_by(1);
                for package in &a.package_index {
                    let index = raw(*package);
                    self.w.print(&format!("#{index}"));
                    self.w.tab();
                    let name = self.package_name(index);
                    self.w.println_str(&format!("// {}", get_java_name(&name)));
                }
                self.w.indent_by(-1);
            }
            ("ModuleMainClass", AttributeBody::ModuleMainClass(a)) => {
                self.w
                    .print(&format!("ModuleMainClass: #{}", raw(a.main_class_index)));
                self.w.tab();
                let name = get_java_name(&self.class_name(raw(a.main_class_index)));
                self.w.println_str(&format!("// {name}"));
            }
            ("NestHost", AttributeBody::NestHost(a)) => {
                self.w.print("NestHost: ");
                self.write_constant_ref(raw(a.host_class_index));
                self.w.println();
            }
            ("NestMembers", AttributeBody::NestMembers(a)) => {
                self.w.println_str("NestMembers:");
                self.w.indent_by(1);
                for class in &a.classes {
                    let value = self.string_value_at(raw(*class));
                    self.w.println_str(&value);
                }
                self.w.indent_by(-1);
            }
            ("PermittedSubclasses", AttributeBody::PermittedSubclasses(a)) => {
                self.w.println_str("PermittedSubclasses:");
                self.w.indent_by(1);
                for class in &a.classes {
                    let value = self.string_value_at(raw(*class));
                    self.w.println_str(&value);
                }
                self.w.indent_by(-1);
            }
            ("Record", AttributeBody::Record(record)) => {
                self.w.println_str("Record:");
                self.w.indent_by(1);
                for component in &record.components {
                    let printer = SigPrinter::new(true);
                    let ty = match self.find_body(&component.attributes, "Signature") {
                        Some(AttributeBody::Signature(signature)) => {
                            let text = self.utf8(raw(signature.signature_index));
                            printer.print(&parse_type_signature(&text))
                        }
                        _ => {
                            let descriptor = self.utf8(raw(component.descriptor_index));
                            let (ty, _) = java_bytecode::FieldType::parse(descriptor.as_bytes())
                                .expect("valid field descriptor");
                            printer.print(&super::signature::sig_from_field_type(&ty))
                        }
                    };
                    let name = self.utf8(raw(component.name_index));
                    self.w.print(&get_java_name(&ty));
                    self.w.print(" ");
                    self.w.print(&name);
                    self.w.print(";");
                    self.w.println();
                    self.w.indent_by(1);
                    self.w.print("descriptor: ");
                    let descriptor = self.utf8(raw(component.descriptor_index));
                    self.w.println_str(&descriptor);
                    let attributes = component.attributes.clone();
                    self.write_attributes(&attributes, None, None);
                    self.w.println();
                    self.w.indent_by(-1);
                }
                self.w.indent_by(-1);
            }
            ("Signature", AttributeBody::Signature(a)) => {
                self.w
                    .print(&format!("Signature: #{}", raw(a.signature_index)));
                self.w.tab();
                let text = self.utf8(raw(a.signature_index));
                self.w.println_str(&format!("// {text}"));
            }
            ("SourceFile", AttributeBody::SourceFile(a)) => {
                let source = self.utf8(raw(a.sourcefile_index));
                self.w.println_str(&format!("SourceFile: \"{source}\""));
            }
            ("SourceDebugExtension", AttributeBody::SourceDebugExtension(a)) => {
                self.w.println_str("SourceDebugExtension:");
                self.w.indent_by(1);
                let text = String::from_utf8_lossy(a.debug_extension.as_ref());
                for part in split_lines(&text) {
                    self.w.println_str(&part);
                }
                self.w.indent_by(-1);
            }
            ("StackMapTable", AttributeBody::StackMapTable(a)) => {
                self.write_stack_map_table(&a.entries);
            }
            ("RuntimeVisibleAnnotations", AttributeBody::Annotations(a)) => {
                self.print_annotations("RuntimeVisibleAnnotations:", &a.annotations);
            }
            ("RuntimeInvisibleAnnotations", AttributeBody::Annotations(a)) => {
                self.print_annotations("RuntimeInvisibleAnnotations:", &a.annotations);
            }
            ("RuntimeVisibleTypeAnnotations", AttributeBody::TypeAnnotations(a)) => {
                self.print_type_annotations("RuntimeVisibleTypeAnnotations:", &a.annotations, lr);
            }
            ("RuntimeInvisibleTypeAnnotations", AttributeBody::TypeAnnotations(a)) => {
                self.print_type_annotations("RuntimeInvisibleTypeAnnotations:", &a.annotations, lr);
            }
            ("RuntimeVisibleParameterAnnotations", AttributeBody::ParameterAnnotations(a)) => {
                self.print_parameter_annotations(
                    "RuntimeVisibleParameterAnnotations:",
                    &a.parameters,
                );
            }
            ("RuntimeInvisibleParameterAnnotations", AttributeBody::ParameterAnnotations(a)) => {
                self.print_parameter_annotations(
                    "RuntimeInvisibleParameterAnnotations:",
                    &a.parameters,
                );
            }
            ("AnnotationDefault", AttributeBody::AnnotationDefault(a)) => {
                self.w.println_str("AnnotationDefault:");
                self.w.indent_by(1);
                self.w.print("default_value: ");
                self.write_element_value_lines(&a.default_value);
                self.w.indent_by(-1);
                self.w.println();
            }
            (_, AttributeBody::Raw(bytes)) => match name {
                "ModuleTarget" => self.print_module_target(bytes),
                "ModuleHashes" => self.print_module_hashes(bytes),
                "ModuleResolution" => self.print_module_resolution(bytes),
                _ => self.print_unknown(name, bytes),
            },
            _ => {}
        }
    }

    fn print_module_target(&mut self, bytes: &[u8]) {
        let mut pos = 0;
        let index = read_u2(bytes, &mut pos);
        self.w.println_str("ModuleTarget:");
        self.w.indent_by(1);
        self.w.print(&format!("target_platform: #{index}"));
        self.w.tab();
        let value = self.utf8(index);
        self.w.println_str(&format!("// {value}"));
        self.w.indent_by(-1);
    }

    fn print_module_resolution(&mut self, bytes: &[u8]) {
        let mut pos = 0;
        let flags = read_u2(bytes, &mut pos);
        self.w.println_str("ModuleResolution:");
        self.w.indent_by(1);
        self.w.print(&format!("{flags:x}"));
        self.w.tab();
        self.w.print("// ");
        if flags & 0x0001 != 0 {
            self.w.print(" DO_NOT_RESOLVE_BY_DEFAULT");
        }
        if flags & 0x0002 != 0 {
            self.w.print(" WARN_DEPRECATED");
        }
        if flags & 0x0004 != 0 {
            self.w.print(" WARN_DEPRECATED_FOR_REMOVAL");
        }
        if flags & 0x0008 != 0 {
            self.w.print(" WARN_INCUBATING");
        }
        self.w.println();
        self.w.indent_by(-1);
    }

    fn print_module_hashes(&mut self, bytes: &[u8]) {
        let mut pos = 0;
        let algorithm = read_u2(bytes, &mut pos);
        let count = read_u2(bytes, &mut pos);

        self.w.println_str("ModuleHashes:");
        self.w.indent_by(1);
        self.w.print(&format!("algorithm: #{algorithm}"));
        self.w.tab();
        let algorithm_name = self.utf8(algorithm);
        self.w.println_str(&format!("// {algorithm_name}"));
        self.w.print(&count.to_string());
        self.w.tab();
        self.w.println_str("// hashes");
        for _ in 0..count {
            let module_name = read_u2(bytes, &mut pos);
            let length = usize::from(read_u2(bytes, &mut pos));
            let hash = bytes.get(pos..pos + length).unwrap_or(&[]);
            pos += length;
            self.w.print(&format!("#{module_name}"));
            self.w.tab();
            let name = match self.cp(module_name) {
                Some(Constant::Module { name_index }) => self.utf8(raw(*name_index)),
                _ => self.utf8(module_name),
            };
            self.w.println_str(&format!("// {name}"));
            self.w.println_str(&format!("hash_length: {length}"));
            let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
            self.w.println_str(&format!("hash: [{hex}]"));
        }
        self.w.indent_by(-1);
    }

    fn package_name(&self, index: u16) -> String {
        match self.cp(index) {
            Some(Constant::Package { name_index }) => self.utf8(raw(*name_index)),
            _ => String::new(),
        }
    }

    fn write_module(&mut self, module: &java_bytecode::ModuleAttribute<'b>) {
        self.w.println_str("Module:");
        self.w.indent_by(1);

        let flags = module.module_flags.bits();
        self.w.print(&format!("#{}", raw(module.module_name_index)));
        self.w.print(",");
        self.w.print(&format!("{flags:x}"));
        self.w.tab();
        let name = self.string_value_at(raw(module.module_name_index));
        self.w.print(&format!("// {name}"));
        if flags & 0x0020 != 0 {
            self.w.print(" ACC_OPEN");
        }
        if flags & 0x8000 != 0 {
            self.w.print(" ACC_MANDATED");
        }
        if flags & 0x1000 != 0 {
            self.w.print(" ACC_SYNTHETIC");
        }
        self.w.println();

        let version = raw(module.module_version_index);
        self.w.print(&format!("#{version}"));
        if version != 0 {
            self.w.tab();
            let value = self.string_value_at(version);
            self.w.print(&format!("// {value}"));
        }
        self.w.println();

        self.w.print(&module.requires.len().to_string());
        self.w.tab();
        self.w.println_str("// requires");
        self.w.indent_by(1);
        for entry in &module.requires {
            let index = raw(entry.requires_index);
            let flags = entry.requires_flags.bits();
            self.w.print(&format!("#{index},{flags:x}"));
            self.w.tab();
            let value = self.string_value_at(index);
            self.w.print(&format!("// {value}"));
            if flags & 0x0020 != 0 {
                self.w.print(" ACC_TRANSITIVE");
            }
            if flags & 0x0040 != 0 {
                self.w.print(" ACC_STATIC_PHASE");
            }
            if flags & 0x1000 != 0 {
                self.w.print(" ACC_SYNTHETIC");
            }
            if flags & 0x8000 != 0 {
                self.w.print(" ACC_MANDATED");
            }
            self.w.println();
            let version = raw(entry.requires_version_index);
            self.w.print(&format!("#{version}"));
            if version != 0 {
                self.w.tab();
                let value = self.string_value_at(version);
                self.w.print(&format!("// {value}"));
            }
            self.w.println();
        }
        self.w.indent_by(-1);

        self.w.print(&module.exports.len().to_string());
        self.w.tab();
        self.w.println_str("// exports");
        self.w.indent_by(1);
        for entry in &module.exports {
            self.print_export_open(
                raw(entry.exports_index),
                entry.exports_flags.bits(),
                &entry.exports_to_index,
            );
        }
        self.w.indent_by(-1);

        self.w.print(&module.opens.len().to_string());
        self.w.tab();
        self.w.println_str("// opens");
        self.w.indent_by(1);
        for entry in &module.opens {
            self.print_export_open(
                raw(entry.opens_index),
                entry.opens_flags.bits(),
                &entry.opens_to_index,
            );
        }
        self.w.indent_by(-1);

        self.w.print(&module.uses.len().to_string());
        self.w.tab();
        self.w.println_str("// uses");
        self.w.indent_by(1);
        for entry in &module.uses {
            let index = raw(*entry);
            self.w.print(&format!("#{index}"));
            self.w.tab();
            let value = self.string_value_at(index);
            self.w.println_str(&format!("// {value}"));
        }
        self.w.indent_by(-1);

        self.w.print(&module.provides.len().to_string());
        self.w.tab();
        self.w.println_str("// provides");
        self.w.indent_by(1);
        for entry in &module.provides {
            let index = raw(entry.provides_index);
            self.w.print(&format!("#{index}"));
            self.w.tab();
            self.w.print("// ");
            let value = self.string_value_at(index);
            self.w.print(&value);
            self.w
                .println_str(&format!(" with ... {}", entry.provides_with_index.len()));
            self.w.indent_by(1);
            for with in &entry.provides_with_index {
                let index = raw(*with);
                self.w.print(&format!("#{index}"));
                self.w.tab();
                let value = self.string_value_at(index);
                self.w.println_str(&format!("// ... with {value}"));
            }
            self.w.indent_by(-1);
        }
        self.w.indent_by(-1);

        self.w.indent_by(-1);
    }

    fn print_export_open(&mut self, index: u16, flags: u16, to: &[java_bytecode::ConstantRef<'b>]) {
        self.w.print(&format!("#{index},{flags:x}"));
        self.w.tab();
        self.w.print("// ");
        let value = self.string_value_at(index);
        self.w.print(&value);
        if flags & 0x8000 != 0 {
            self.w.print(" ACC_MANDATED");
        }
        if flags & 0x1000 != 0 {
            self.w.print(" ACC_SYNTHETIC");
        }
        if to.is_empty() {
            self.w.println();
        } else {
            self.w.println_str(&format!(" to ... {}", to.len()));
            self.w.indent_by(1);
            for module in to {
                let index = raw(*module);
                self.w.print(&format!("#{index}"));
                self.w.tab();
                let value = self.string_value_at(index);
                self.w.println_str(&format!("// ... to {value}"));
            }
            self.w.indent_by(-1);
        }
    }

    fn write_stack_map_table(&mut self, entries: &[StackMapFrame<'b>]) {
        self.w.println_str(&format!(
            "StackMapTable: number_of_entries = {}",
            entries.len()
        ));
        self.w.indent_by(1);
        for frame in entries {
            let (frame_type, offset_delta) = frame_type_of(frame);
            if frame_type < 64 {
                self.print_frame_header(frame_type, "/* same */");
            } else if frame_type < 128 {
                self.print_frame_header(frame_type, "/* same_locals_1_stack_item */");
                self.w.indent_by(1);
                if let StackMapFrame::SameLocals1StackItem { stack, .. } = frame {
                    self.print_map("stack", std::slice::from_ref(stack));
                }
                self.w.indent_by(-1);
            } else {
                match frame_type {
                    247 => {
                        self.print_frame_header(
                            frame_type,
                            "/* same_locals_1_stack_item_frame_extended */",
                        );
                        self.w.indent_by(1);
                        self.w
                            .println_str(&format!("offset_delta = {offset_delta}"));
                        if let StackMapFrame::SameLocals1StackItemExtended { stack, .. } = frame {
                            self.print_map("stack", std::slice::from_ref(stack));
                        }
                        self.w.indent_by(-1);
                    }
                    248..=250 => {
                        self.print_frame_header(frame_type, "/* chop */");
                        self.w.indent_by(1);
                        self.w
                            .println_str(&format!("offset_delta = {offset_delta}"));
                        self.w.indent_by(-1);
                    }
                    251 => {
                        self.print_frame_header(frame_type, "/* same_frame_extended */");
                        self.w.indent_by(1);
                        self.w
                            .println_str(&format!("offset_delta = {offset_delta}"));
                        self.w.indent_by(-1);
                    }
                    252..=254 => {
                        self.print_frame_header(frame_type, "/* append */");
                        self.w.indent_by(1);
                        self.w
                            .println_str(&format!("offset_delta = {offset_delta}"));
                        if let StackMapFrame::Append { locals, .. } = frame {
                            self.print_map("locals", locals);
                        }
                        self.w.indent_by(-1);
                    }
                    255 => {
                        self.print_frame_header(frame_type, "/* full_frame */");
                        self.w.indent_by(1);
                        self.w
                            .println_str(&format!("offset_delta = {offset_delta}"));
                        if let StackMapFrame::Full { locals, stack, .. } = frame {
                            self.print_map("locals", locals);
                            self.print_map("stack", stack);
                        }
                        self.w.indent_by(-1);
                    }
                    _ => {}
                }
            }
        }
        self.w.indent_by(-1);
    }

    fn print_frame_header(&mut self, frame_type: u16, extra: &str) {
        self.w.print(&format!("frame_type = {frame_type} "));
        self.w.println_str(extra);
    }

    fn print_map(&mut self, name: &str, map: &[VerificationType<'b>]) {
        self.w.print(&format!("{name} = ["));
        let last = map.len().saturating_sub(1);
        for (i, info) in map.iter().enumerate() {
            match info {
                VerificationType::Object { cpool_index } => {
                    self.w.print(" ");
                    self.write_constant_ref(raw(*cpool_index));
                }
                VerificationType::Uninitialized { offset } => {
                    self.w.print(&format!(" uninitialized {offset}"));
                }
                other => {
                    self.w.print(" ");
                    self.w.print(map_type_name(other));
                }
            }
            self.w.print(if i == last { " " } else { "," });
        }
        self.w.println_str("]");
    }

    fn print_unknown(&mut self, name: &str, bytes: &[u8]) {
        self.w.print("  ");
        self.w.print(name);
        self.w.print(": ");
        self.w.print(&format!("length = 0x{:X}", bytes.len()));
        self.w.print(" (unknown attribute)");
        self.w.println();
        self.w.print("   ");
        let mut j = 0;
        for b in bytes {
            self.w.print(&format!("{b:02X}"));
            j += 1;
            if j == 16 {
                self.w.println();
                self.w.print("   ");
                j = 0;
            } else {
                self.w.print(" ");
            }
        }
        self.w.println();
    }

    // --- Annotations -------------------------------------------------------

    fn print_annotations(&mut self, header: &str, annotations: &[Annotation<'b>]) {
        self.w.println_str(header);
        self.w.indent_by(1);
        for (i, annotation) in annotations.iter().enumerate() {
            self.w.print(&format!("{i}: "));
            self.write_annotation_lines(
                raw(annotation.type_index),
                &annotation.element_value_pairs,
            );
            self.w.println();
        }
        self.w.indent_by(-1);
    }

    fn print_parameter_annotations(&mut self, header: &str, parameters: &[Vec<Annotation<'b>>]) {
        self.w.println_str(header);
        self.w.indent_by(1);
        for (param, annotations) in parameters.iter().enumerate() {
            self.w.println_str(&format!("parameter {param}: "));
            self.w.indent_by(1);
            for (i, annotation) in annotations.iter().enumerate() {
                self.w.print(&format!("{i}: "));
                self.write_annotation_lines(
                    raw(annotation.type_index),
                    &annotation.element_value_pairs,
                );
                self.w.println();
            }
            self.w.indent_by(-1);
        }
        self.w.indent_by(-1);
    }

    fn print_type_annotations(
        &mut self,
        header: &str,
        annotations: &[TypeAnnotation<'b>],
        lr: Option<&CodeAttribute<'b>>,
    ) {
        self.w.println_str(header);
        self.w.indent_by(1);
        for (i, annotation) in annotations.iter().enumerate() {
            self.w.print(&format!("{i}: "));
            self.write_type_annotation_lines(annotation, lr);
            self.w.println();
        }
        self.w.indent_by(-1);
    }

    fn write_annotation_lines(&mut self, type_index: u16, pairs: &[ElementValuePair<'b>]) {
        self.write_annotation_parts(type_index, pairs, false);
        self.w.println();
        self.w.indent_by(1);
        self.write_annotation_parts(type_index, pairs, true);
        self.w.indent_by(-1);
    }

    fn write_annotation_parts(
        &mut self,
        type_index: u16,
        pairs: &[ElementValuePair<'b>],
        resolve: bool,
    ) {
        self.write_annotation_descriptor(type_index, resolve);
        if resolve {
            let show_parens = !pairs.is_empty();
            if show_parens {
                self.w.println_str("(");
                self.w.indent_by(1);
            }
            for pair in pairs {
                self.write_element_pair(pair, true);
                self.w.println();
            }
            if show_parens {
                self.w.indent_by(-1);
                self.w.print(")");
            }
        } else {
            self.w.print("(");
            for (i, pair) in pairs.iter().enumerate() {
                if i > 0 {
                    self.w.print(",");
                }
                self.write_element_pair(pair, false);
            }
            self.w.print(")");
        }
    }

    fn write_annotation_descriptor(&mut self, index: u16, resolve: bool) {
        if resolve {
            let descriptor = self.utf8(index);
            let printer = SigPrinter::new(true);
            let text = printer.print(&parse_type_signature(&descriptor));
            self.w.print(&text);
        } else {
            self.w.print(&format!("#{index}"));
        }
    }

    fn write_element_pair(&mut self, pair: &ElementValuePair<'b>, resolve: bool) {
        self.write_index_ref(raw(pair.element_name_index), resolve);
        self.w.print("=");
        self.write_element_value(&pair.value, resolve);
    }

    fn write_element_value_lines(&mut self, value: &ElementValue<'b>) {
        self.write_element_value(value, false);
        self.w.println();
        self.w.indent_by(1);
        self.write_element_value(value, true);
        self.w.indent_by(-1);
    }

    fn write_element_value(&mut self, value: &ElementValue<'b>, resolve: bool) {
        match value {
            ElementValue::Const {
                tag,
                const_value_index,
            } => {
                let index = raw(*const_value_index);
                if resolve {
                    match *tag {
                        b'B' => {
                            self.w.print("(byte) ");
                            let text = self.string_value_at(index);
                            self.w.print(&text);
                        }
                        b'C' => {
                            self.w.print("'");
                            let text = self.char_value(index);
                            self.w.print(&text);
                            self.w.print("'");
                        }
                        b'D' | b'F' | b'I' | b'J' => {
                            let text = self.string_value_at(index);
                            self.w.print(&text);
                        }
                        b'S' => {
                            self.w.print("(short) ");
                            let text = self.string_value_at(index);
                            self.w.print(&text);
                        }
                        b'Z' => {
                            let text = self.boolean_value(index);
                            self.w.print(&text);
                        }
                        b's' => {
                            self.w.print("\"");
                            let text = self.string_value_at(index);
                            self.w.print(&text);
                            self.w.print("\"");
                        }
                        other => {
                            self.w.print(&format!("{}#{index}", char::from(other)));
                        }
                    }
                } else {
                    self.w.print(&format!("{}#{index}", char::from(*tag)));
                }
            }
            ElementValue::Enum {
                type_name_index,
                const_name_index,
            } => {
                if resolve {
                    self.write_index_ref(raw(*type_name_index), true);
                    self.w.print(".");
                    self.write_index_ref(raw(*const_name_index), true);
                } else {
                    self.w.print(&format!(
                        "e#{}.#{}",
                        raw(*type_name_index),
                        raw(*const_name_index)
                    ));
                }
            }
            ElementValue::Class { class_info_index } => {
                if resolve {
                    self.w.print("class ");
                    self.write_index_ref(raw(*class_info_index), true);
                } else {
                    self.w.print(&format!("c#{}", raw(*class_info_index)));
                }
            }
            ElementValue::Annotation(annotation) => {
                self.w.print("@");
                self.write_annotation_parts(
                    raw(annotation.type_index),
                    &annotation.element_value_pairs,
                    resolve,
                );
            }
            ElementValue::Array(values) => {
                self.w.print("[");
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        self.w.print(",");
                    }
                    self.write_element_value(value, resolve);
                }
                self.w.print("]");
            }
        }
    }

    fn write_index_ref(&mut self, index: u16, resolve: bool) {
        if resolve {
            let text = self.string_value_at(index);
            self.w.print(&text);
        } else {
            self.w.print(&format!("#{index}"));
        }
    }

    fn char_value(&self, index: u16) -> String {
        if let Some(Constant::Integer(value)) = self.cp(index) {
            char::from_u32((*value as u32) & 0xFFFF)
                .map(|c| c.to_string())
                .unwrap_or_else(|| format!("#{index}"))
        } else {
            format!("#{index}")
        }
    }

    fn boolean_value(&self, index: u16) -> String {
        if let Some(Constant::Integer(value)) = self.cp(index) {
            match value {
                0 => "false".to_owned(),
                1 => "true".to_owned(),
                _ => format!("#{index}"),
            }
        } else {
            format!("#{index}")
        }
    }

    // --- Type annotations --------------------------------------------------

    fn write_type_annotation_lines(
        &mut self,
        annotation: &TypeAnnotation<'b>,
        lr: Option<&CodeAttribute<'b>>,
    ) {
        self.write_type_annotation(annotation, true, false, lr);
        self.w.println();
        self.w.indent_by(1);
        self.write_annotation_parts(
            raw(annotation.type_index),
            &annotation.element_value_pairs,
            true,
        );
        self.w.indent_by(-1);
    }

    fn write_type_annotation(
        &mut self,
        annotation: &TypeAnnotation<'b>,
        show_offsets: bool,
        resolve: bool,
        lr: Option<&CodeAttribute<'b>>,
    ) {
        self.write_annotation_parts(
            raw(annotation.type_index),
            &annotation.element_value_pairs,
            resolve,
        );
        self.w.print(": ");
        self.write_target_info(annotation, show_offsets, lr);
    }

    fn write_target_info(
        &mut self,
        annotation: &TypeAnnotation<'b>,
        show_offsets: bool,
        _lr: Option<&CodeAttribute<'b>>,
    ) {
        self.w.print(target_type_name(annotation.target_type));
        match &annotation.target_info {
            TargetInfo::Offset { offset } => {
                if show_offsets {
                    self.w.print(&format!(", offset={offset}"));
                }
            }
            TargetInfo::LocalVar { table } => {
                if table.is_empty() {
                    self.w.print(", lvarOffset is Null!");
                } else {
                    self.w.print(", {");
                    for (i, entry) in table.iter().enumerate() {
                        if i != 0 {
                            self.w.print("; ");
                        }
                        if show_offsets {
                            self.w.print(&format!("start_pc={}", entry.start_pc));
                        }
                        self.w.print(&format!(", length={}", entry.length));
                        self.w.print(&format!(", index={}", entry.index));
                    }
                    self.w.print("}");
                }
            }
            TargetInfo::Catch {
                exception_table_index,
            } => {
                self.w
                    .print(&format!(", exception_index={exception_table_index}"));
            }
            TargetInfo::TypeParameter {
                type_parameter_index,
            } => {
                self.w
                    .print(&format!(", param_index={type_parameter_index}"));
            }
            TargetInfo::TypeParameterBound {
                type_parameter_index,
                bound_index,
            } => {
                self.w
                    .print(&format!(", param_index={type_parameter_index}"));
                self.w.print(&format!(", bound_index={bound_index}"));
            }
            TargetInfo::Supertype { supertype_index } => {
                self.w.print(&format!(", type_index={supertype_index}"));
            }
            TargetInfo::Throws { throws_type_index } => {
                self.w.print(&format!(", type_index={throws_type_index}"));
            }
            TargetInfo::FormalParameter {
                formal_parameter_index,
            } => {
                self.w
                    .print(&format!(", param_index={formal_parameter_index}"));
            }
            TargetInfo::TypeArgument {
                offset,
                type_argument_index,
            } => {
                if show_offsets {
                    self.w.print(&format!(", offset={offset}"));
                }
                self.w.print(&format!(", type_index={type_argument_index}"));
            }
            TargetInfo::Empty => {}
        }

        if !annotation.target_path.path.is_empty() {
            self.w.print(", location=");
            let mut text = String::from("[");
            for (i, entry) in annotation.target_path.path.iter().enumerate() {
                if i > 0 {
                    text.push_str(", ");
                }
                match entry.type_path_kind {
                    0 => text.push_str("ARRAY"),
                    1 => text.push_str("INNER_TYPE"),
                    2 => text.push_str("WILDCARD"),
                    3 => text.push_str(&format!("TYPE_ARGUMENT({})", entry.type_argument_index)),
                    _ => {}
                }
            }
            text.push(']');
            self.w.print(&text);
        }
    }
}

fn read_u2(bytes: &[u8], pos: &mut usize) -> u16 {
    let value = match (bytes.get(*pos), bytes.get(*pos + 1)) {
        (Some(&hi), Some(&lo)) => u16::from_be_bytes([hi, lo]),
        _ => 0,
    };
    *pos += 2;
    value
}

fn frame_type_of(frame: &StackMapFrame<'_>) -> (u16, u16) {
    match frame {
        StackMapFrame::Same { offset_delta } => (*offset_delta, *offset_delta),
        StackMapFrame::SameLocals1StackItem { offset_delta, .. } => {
            (64 + offset_delta, *offset_delta)
        }
        StackMapFrame::SameLocals1StackItemExtended { offset_delta, .. } => (247, *offset_delta),
        StackMapFrame::Chop { offset_delta, k } => (251 - u16::from(*k), *offset_delta),
        StackMapFrame::SameExtended { offset_delta } => (251, *offset_delta),
        StackMapFrame::Append {
            offset_delta,
            locals,
        } => (251 + locals.len() as u16, *offset_delta),
        StackMapFrame::Full { offset_delta, .. } => (255, *offset_delta),
        _ => (0, 0),
    }
}

fn target_type_name(value: u8) -> &'static str {
    match value {
        0x00 => "CLASS_TYPE_PARAMETER",
        0x01 => "METHOD_TYPE_PARAMETER",
        0x10 => "CLASS_EXTENDS",
        0x11 => "CLASS_TYPE_PARAMETER_BOUND",
        0x12 => "METHOD_TYPE_PARAMETER_BOUND",
        0x13 => "FIELD",
        0x14 => "METHOD_RETURN",
        0x15 => "METHOD_RECEIVER",
        0x16 => "METHOD_FORMAL_PARAMETER",
        0x17 => "THROWS",
        0x40 => "LOCAL_VARIABLE",
        0x41 => "RESOURCE_VARIABLE",
        0x42 => "EXCEPTION_PARAMETER",
        0x43 => "INSTANCEOF",
        0x44 => "NEW",
        0x45 => "CONSTRUCTOR_REFERENCE",
        0x46 => "METHOD_REFERENCE",
        0x47 => "CAST",
        0x48 => "CONSTRUCTOR_INVOCATION_TYPE_ARGUMENT",
        0x49 => "METHOD_INVOCATION_TYPE_ARGUMENT",
        0x4A => "CONSTRUCTOR_REFERENCE_TYPE_ARGUMENT",
        0x4B => "METHOD_REFERENCE_TYPE_ARGUMENT",
        _ => "UNKNOWN",
    }
}

fn map_type_name(info: &VerificationType<'_>) -> &'static str {
    match info {
        VerificationType::Top => "top",
        VerificationType::Integer => "int",
        VerificationType::Float => "float",
        VerificationType::Long => "long",
        VerificationType::Double => "double",
        VerificationType::Null => "null",
        VerificationType::UninitializedThis => "this",
        VerificationType::Object { .. } | VerificationType::Uninitialized { .. } => "?",
    }
}

fn split_lines(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut last_was_separator = false;
    for c in text.chars() {
        if c == '\r' || c == '\n' {
            if !last_was_separator {
                parts.push(std::mem::take(&mut current));
                last_was_separator = true;
            }
        } else {
            last_was_separator = false;
            current.push(c);
        }
    }
    if !last_was_separator {
        parts.push(current);
    }
    parts
}
