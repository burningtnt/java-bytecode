//! Constant-pool compaction and index remapping.
//!
//! The default way to serialize a [`ClassFile`] keeps the original constant
//! pool order (and therefore preserves indices). [`ClassFile::to_bytes_rebuilt`]
//! instead drops unused constants and merges exact duplicates, renumbering
//! every reference — including references embedded in bytecode — as it writes.
//!
//! Because the rebuilt bytes are owned by the caller, this works under the
//! crate's borrowed model: no new string or byte data is created, only existing
//! constants are reordered.
//!
//! If the class file contains an attribute whose body is not interpreted
//! ([`AttributeBody::Raw`]), its constant references cannot be discovered, so
//! rebuilding falls back to a plain, index-preserving write.

use std::collections::BTreeSet;

use crate::attributes::{
    Annotation, Attribute, AttributeBody, ElementValue, StackMapFrame, VerificationType,
};
use crate::bytecode::{self, Instruction};
use crate::class_file::{ClassFile, MAGIC};
use crate::constant_pool::{Constant, ConstantPool};
use crate::error::Result;
use crate::io::Writer;

impl ClassFile<'_> {
    /// Serializes this class file with a rebuilt constant pool.
    ///
    /// Unused constants are removed and exact duplicates merged. All index
    /// references are rewritten, including those inside `Code` bytecode. Falls
    /// back to [`ClassFile::to_bytes`] when a raw (uninterpreted) attribute is
    /// present, since its references cannot be discovered.
    pub fn to_bytes_rebuilt(&self) -> Result<Vec<u8>> {
        match build_plan(self) {
            Some((remap, pool)) => {
                let mut writer = Writer::with_remap(remap);
                write_rebuilt(self, &pool, &mut writer)?;
                Ok(writer.into_bytes())
            }
            None => self.to_bytes(),
        }
    }
}

#[derive(Default)]
struct Collect {
    used: BTreeSet<u16>,
    saw_raw: bool,
}

impl Collect {
    fn insert(&mut self, index: u16) {
        if index != 0 {
            self.used.insert(index);
        }
    }
}

fn build_plan<'a>(class: &ClassFile<'a>) -> Option<(Vec<u16>, ConstantPool<'a>)> {
    let mut collect = Collect::default();
    collect_class(class, &mut collect).ok()?;
    if collect.saw_raw {
        return None;
    }

    // Close over constants referenced by other constants.
    let mut worklist: Vec<u16> = collect.used.iter().copied().collect();
    while let Some(index) = worklist.pop() {
        if let Some(constant) = class.constant_pool.entry(index) {
            for referenced in constant_refs(constant) {
                if referenced != 0 && collect.used.insert(referenced) {
                    worklist.push(referenced);
                }
            }
        }
    }

    let old_len = class.constant_pool.len().max(1);
    let mut remap: Vec<u16> = (0..old_len).map(|i| i as u16).collect();

    let mut entries: Vec<Option<Constant<'a>>> = vec![None];
    let mut assigned: Vec<(Constant<'a>, u16)> = Vec::new();
    let mut next: u16 = 1;

    for &old in &collect.used {
        let Some(constant) = class.constant_pool.entry(old) else {
            continue;
        };
        if let Some((_, new)) = assigned.iter().find(|(existing, _)| existing == constant) {
            remap[old as usize] = *new;
            continue;
        }
        let new = next;
        let two_slots = constant.takes_two_slots();
        entries.push(Some(constant.clone()));
        if two_slots {
            entries.push(None);
            next = next.checked_add(2)?;
        } else {
            next = next.checked_add(1)?;
        }
        remap[old as usize] = new;
        assigned.push((constant.clone(), new));
    }

    Some((remap, ConstantPool::from_entries(entries)))
}

fn write_rebuilt(
    class: &ClassFile<'_>,
    pool: &ConstantPool<'_>,
    writer: &mut Writer,
) -> Result<()> {
    writer.write_u4(MAGIC);
    writer.write_u2(class.minor_version);
    writer.write_u2(class.major_version);
    pool.write_to(writer)?;
    writer.write_u2(class.access_flags.bits());
    writer.write_cp_u2(class.this_class.get());
    writer.write_cp_u2(class.super_class.get());

    writer.write_u2_len(class.interfaces.len())?;
    for interface in &class.interfaces {
        writer.write_cp_u2(interface.get());
    }

    writer.write_u2_len(class.fields.len())?;
    for field in &class.fields {
        field.write_to(writer)?;
    }

    writer.write_u2_len(class.methods.len())?;
    for method in &class.methods {
        method.write_to(writer)?;
    }

    crate::attributes::write_attributes(writer, &class.attributes)
}

fn constant_refs(constant: &Constant<'_>) -> Vec<u16> {
    match constant {
        Constant::Utf8(_)
        | Constant::Integer(_)
        | Constant::Float(_)
        | Constant::Long(_)
        | Constant::Double(_) => Vec::new(),
        Constant::Class { name_index } | Constant::Module { name_index } => {
            vec![name_index.get()]
        }
        Constant::String { string_index } => vec![string_index.get()],
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
        } => vec![class_index.get(), name_and_type_index.get()],
        Constant::NameAndType {
            name_index,
            descriptor_index,
        } => vec![name_index.get(), descriptor_index.get()],
        Constant::MethodHandle {
            reference_index, ..
        } => vec![reference_index.get()],
        Constant::MethodType { descriptor_index } => vec![descriptor_index.get()],
        Constant::Dynamic {
            name_and_type_index,
            ..
        }
        | Constant::InvokeDynamic {
            name_and_type_index,
            ..
        } => vec![name_and_type_index.get()],
        Constant::Package { name_index } => vec![name_index.get()],
    }
}

fn collect_class(class: &ClassFile<'_>, collect: &mut Collect) -> Result<()> {
    collect.insert(class.this_class.get());
    collect.insert(class.super_class.get());
    for interface in &class.interfaces {
        collect.insert(interface.get());
    }
    for field in &class.fields {
        collect.insert(field.name_index.get());
        collect.insert(field.descriptor_index.get());
        collect_attributes(&field.attributes, collect)?;
    }
    for method in &class.methods {
        collect.insert(method.name_index.get());
        collect.insert(method.descriptor_index.get());
        collect_attributes(&method.attributes, collect)?;
    }
    collect_attributes(&class.attributes, collect)
}

fn collect_attributes(attributes: &[Attribute<'_>], collect: &mut Collect) -> Result<()> {
    for attribute in attributes {
        collect.insert(attribute.name_index.get());
        collect_body(&attribute.body, collect)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn collect_body(body: &AttributeBody<'_>, collect: &mut Collect) -> Result<()> {
    match body {
        AttributeBody::Raw(_) => collect.saw_raw = true,
        AttributeBody::ConstantValue(a) => collect.insert(a.constantvalue_index.get()),
        AttributeBody::Code(a) => {
            for entry in &a.exception_table {
                collect.insert(entry.catch_type.get());
            }
            let instructions = bytecode::decode_impl(a.code())?;
            for instruction in &instructions {
                collect_instruction(instruction, collect);
            }
            collect_attributes(&a.attributes, collect)?;
        }
        AttributeBody::Exceptions(a) => {
            for index in &a.exception_index_table {
                collect.insert(index.get());
            }
        }
        AttributeBody::InnerClasses(a) => {
            for entry in &a.classes {
                collect.insert(entry.inner_class_info_index.get());
                collect.insert(entry.outer_class_info_index.get());
                collect.insert(entry.inner_name_index.get());
            }
        }
        AttributeBody::EnclosingMethod(a) => {
            collect.insert(a.class_index.get());
            collect.insert(a.method_index.get());
        }
        AttributeBody::Synthetic | AttributeBody::Deprecated => {}
        AttributeBody::Signature(a) => collect.insert(a.signature_index.get()),
        AttributeBody::SourceFile(a) => collect.insert(a.sourcefile_index.get()),
        AttributeBody::SourceDebugExtension(_) => {}
        AttributeBody::LineNumberTable(_) => {}
        AttributeBody::LocalVariableTable(a) => {
            for entry in &a.local_variable_table {
                collect.insert(entry.name_index.get());
                collect.insert(entry.descriptor_index.get());
            }
        }
        AttributeBody::LocalVariableTypeTable(a) => {
            for entry in &a.local_variable_type_table {
                collect.insert(entry.name_index.get());
                collect.insert(entry.signature_index.get());
            }
        }
        AttributeBody::BootstrapMethods(a) => {
            for method in &a.bootstrap_methods {
                collect.insert(method.bootstrap_method_ref.get());
                for argument in &method.bootstrap_arguments {
                    collect.insert(argument.get());
                }
            }
        }
        AttributeBody::MethodParameters(a) => {
            for parameter in &a.parameters {
                collect.insert(parameter.name_index.get());
            }
        }
        AttributeBody::Module(a) => {
            collect.insert(a.module_name_index.get());
            collect.insert(a.module_version_index.get());
            for entry in &a.requires {
                collect.insert(entry.requires_index.get());
                collect.insert(entry.requires_version_index.get());
            }
            for entry in &a.exports {
                collect.insert(entry.exports_index.get());
                for index in &entry.exports_to_index {
                    collect.insert(index.get());
                }
            }
            for entry in &a.opens {
                collect.insert(entry.opens_index.get());
                for index in &entry.opens_to_index {
                    collect.insert(index.get());
                }
            }
            for index in &a.uses {
                collect.insert(index.get());
            }
            for entry in &a.provides {
                collect.insert(entry.provides_index.get());
                for index in &entry.provides_with_index {
                    collect.insert(index.get());
                }
            }
        }
        AttributeBody::ModulePackages(a) => {
            for index in &a.package_index {
                collect.insert(index.get());
            }
        }
        AttributeBody::ModuleMainClass(a) => collect.insert(a.main_class_index.get()),
        AttributeBody::NestHost(a) => collect.insert(a.host_class_index.get()),
        AttributeBody::NestMembers(a) => {
            for index in &a.classes {
                collect.insert(index.get());
            }
        }
        AttributeBody::Record(a) => {
            for component in &a.components {
                collect.insert(component.name_index.get());
                collect.insert(component.descriptor_index.get());
                collect_attributes(&component.attributes, collect)?;
            }
        }
        AttributeBody::PermittedSubclasses(a) => {
            for index in &a.classes {
                collect.insert(index.get());
            }
        }
        AttributeBody::Annotations(a) => {
            for annotation in &a.annotations {
                collect_annotation(annotation, collect);
            }
        }
        AttributeBody::ParameterAnnotations(a) => {
            for annotations in &a.parameters {
                for annotation in annotations {
                    collect_annotation(annotation, collect);
                }
            }
        }
        AttributeBody::TypeAnnotations(a) => {
            for annotation in &a.annotations {
                collect.insert(annotation.type_index.get());
                for pair in &annotation.element_value_pairs {
                    collect.insert(pair.element_name_index.get());
                    collect_element_value(&pair.value, collect);
                }
            }
        }
        AttributeBody::AnnotationDefault(a) => collect_element_value(&a.default_value, collect),
        AttributeBody::StackMapTable(a) => {
            for frame in &a.entries {
                collect_frame(frame, collect);
            }
        }
    }
    Ok(())
}

fn collect_annotation(annotation: &Annotation<'_>, collect: &mut Collect) {
    collect.insert(annotation.type_index.get());
    for pair in &annotation.element_value_pairs {
        collect.insert(pair.element_name_index.get());
        collect_element_value(&pair.value, collect);
    }
}

fn collect_element_value(value: &ElementValue<'_>, collect: &mut Collect) {
    match value {
        ElementValue::Const {
            const_value_index, ..
        } => collect.insert(const_value_index.get()),
        ElementValue::Enum {
            type_name_index,
            const_name_index,
        } => {
            collect.insert(type_name_index.get());
            collect.insert(const_name_index.get());
        }
        ElementValue::Class { class_info_index } => collect.insert(class_info_index.get()),
        ElementValue::Annotation(annotation) => collect_annotation(annotation, collect),
        ElementValue::Array(values) => {
            for value in values {
                collect_element_value(value, collect);
            }
        }
    }
}

fn collect_verification(verification: &VerificationType<'_>, collect: &mut Collect) {
    if let VerificationType::Object { cpool_index } = verification {
        collect.insert(cpool_index.get());
    }
}

fn collect_frame(frame: &StackMapFrame<'_>, collect: &mut Collect) {
    match frame {
        StackMapFrame::Same { .. } | StackMapFrame::SameExtended { .. } => {}
        StackMapFrame::SameLocals1StackItem { stack, .. }
        | StackMapFrame::SameLocals1StackItemExtended { stack, .. } => {
            collect_verification(stack, collect);
        }
        StackMapFrame::Chop { .. } => {}
        StackMapFrame::Append { locals, .. } => {
            for local in locals {
                collect_verification(local, collect);
            }
        }
        StackMapFrame::Full { locals, stack, .. } => {
            for local in locals {
                collect_verification(local, collect);
            }
            for item in stack {
                collect_verification(item, collect);
            }
        }
    }
}

fn collect_instruction(instruction: &Instruction<'_>, collect: &mut Collect) {
    match instruction {
        Instruction::Constant(_, index) => collect.insert(index.get()),
        Instruction::ConstantWide(_, index) => collect.insert(index.get()),
        Instruction::InvokeInterface { index, .. }
        | Instruction::InvokeDynamic { index, .. }
        | Instruction::MultiANewArray { index, .. } => collect.insert(index.get()),
        _ => {}
    }
}
