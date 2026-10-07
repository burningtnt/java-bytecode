//! Verifies that every known attribute name is decoded to a typed body, not
//! left as `Raw`, across the whole fixture corpus (including nested
//! `Code`/`Record` attributes).

use std::fs;
use std::path::{Path, PathBuf};

use java_bytecode::{Attribute, AttributeBody, ClassFile, ConstantPool};

const KNOWN: &[&str] = &[
    "ConstantValue",
    "Code",
    "Exceptions",
    "InnerClasses",
    "EnclosingMethod",
    "Synthetic",
    "Deprecated",
    "Signature",
    "SourceFile",
    "SourceDebugExtension",
    "LineNumberTable",
    "LocalVariableTable",
    "LocalVariableTypeTable",
    "BootstrapMethods",
    "MethodParameters",
    "Module",
    "ModulePackages",
    "ModuleMainClass",
    "NestHost",
    "NestMembers",
    "Record",
    "PermittedSubclasses",
    "RuntimeVisibleAnnotations",
    "RuntimeInvisibleAnnotations",
    "RuntimeVisibleParameterAnnotations",
    "RuntimeInvisibleParameterAnnotations",
    "RuntimeVisibleTypeAnnotations",
    "RuntimeInvisibleTypeAnnotations",
    "AnnotationDefault",
    "StackMapTable",
];

fn collect_class_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read fixture dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_class_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "class") {
            out.push(path);
        }
    }
}

fn check_attributes<'a>(attributes: &[Attribute<'a>], pool: &ConstantPool<'a>, path: &Path) {
    for attribute in attributes {
        let name = attribute
            .resolved_name(pool)
            .unwrap_or_else(|err| panic!("bad attribute name in {}: {err}", path.display()));
        if KNOWN.contains(&name.as_str()) {
            assert!(
                !matches!(attribute.body, AttributeBody::Raw(_)),
                "known attribute {name} was left as Raw in {}",
                path.display()
            );
        }
        match &attribute.body {
            AttributeBody::Code(code) => check_attributes(&code.attributes, pool, path),
            AttributeBody::Record(record) => {
                for component in &record.components {
                    check_attributes(&component.attributes, pool, path);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn known_attributes_are_typed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/classes");
    let mut files = Vec::new();
    collect_class_files(&root, &mut files);
    files.sort();
    assert!(files.len() >= 30);

    for path in &files {
        let bytes = fs::read(path).expect("read class file");
        let class = ClassFile::parse(&bytes).expect("parse");
        check_attributes(&class.attributes, &class.constant_pool, path);
        for field in &class.fields {
            check_attributes(&field.attributes, &class.constant_pool, path);
        }
        for method in &class.methods {
            check_attributes(&method.attributes, &class.constant_pool, path);
        }
    }
}
