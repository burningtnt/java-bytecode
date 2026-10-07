//! Run the full parser/serializer over a directory tree of class files.
//!
//! For every `.class` discovered the harness checks, in order:
//!
//! 1. `ClassFile::parse` succeeds;
//! 2. `to_bytes` reproduces the input byte for byte;
//! 3. every `Code` attribute's bytecode decodes and re-encodes exactly;
//! 4. `to_bytes_rebuilt` compacts the pool and the result re-parses.
//!
//! Usage: `cargo run --release --example corpus -- <dir>`

// `bytecode::decode`/`encode` are `unsafe` (they cannot manage constant-pool
// indices); here the instructions stay within their originating class.
#![allow(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use java_bytecode::{AttributeBody, ClassFile};

struct Stats {
    files: usize,
    bytes: u64,
    pool_slots: u64,
    code_attributes: usize,
    instructions: u64,
    attributes: BTreeMap<&'static str, u64>,
    raw_names: BTreeMap<String, u64>,
    failures: Vec<String>,
}

impl Stats {
    fn new() -> Self {
        Self {
            files: 0,
            bytes: 0,
            pool_slots: 0,
            code_attributes: 0,
            instructions: 0,
            attributes: BTreeMap::new(),
            raw_names: BTreeMap::new(),
            failures: Vec::new(),
        }
    }

    fn fail(&mut self, path: &Path, message: String) {
        if self.failures.len() < 100 {
            self.failures.push(format!("{}: {message}", path.display()));
        }
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "class") {
                out.push(path);
            }
        }
    }
}

fn attribute_name(body: &AttributeBody<'_>) -> &'static str {
    match body {
        AttributeBody::Raw(_) => "Raw",
        AttributeBody::ConstantValue(_) => "ConstantValue",
        AttributeBody::Code(_) => "Code",
        AttributeBody::Exceptions(_) => "Exceptions",
        AttributeBody::InnerClasses(_) => "InnerClasses",
        AttributeBody::EnclosingMethod(_) => "EnclosingMethod",
        AttributeBody::Synthetic => "Synthetic",
        AttributeBody::Deprecated => "Deprecated",
        AttributeBody::Signature(_) => "Signature",
        AttributeBody::SourceFile(_) => "SourceFile",
        AttributeBody::SourceDebugExtension(_) => "SourceDebugExtension",
        AttributeBody::LineNumberTable(_) => "LineNumberTable",
        AttributeBody::LocalVariableTable(_) => "LocalVariableTable",
        AttributeBody::LocalVariableTypeTable(_) => "LocalVariableTypeTable",
        AttributeBody::BootstrapMethods(_) => "BootstrapMethods",
        AttributeBody::MethodParameters(_) => "MethodParameters",
        AttributeBody::Module(_) => "Module",
        AttributeBody::ModulePackages(_) => "ModulePackages",
        AttributeBody::ModuleMainClass(_) => "ModuleMainClass",
        AttributeBody::NestHost(_) => "NestHost",
        AttributeBody::NestMembers(_) => "NestMembers",
        AttributeBody::Record(_) => "Record",
        AttributeBody::PermittedSubclasses(_) => "PermittedSubclasses",
        AttributeBody::Annotations(_) => "Annotations",
        AttributeBody::ParameterAnnotations(_) => "ParameterAnnotations",
        AttributeBody::TypeAnnotations(_) => "TypeAnnotations",
        AttributeBody::AnnotationDefault(_) => "AnnotationDefault",
        AttributeBody::StackMapTable(_) => "StackMapTable",
        _ => "Other",
    }
}

fn check(path: &Path, stats: &mut Stats) {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            stats.fail(path, format!("read: {err}"));
            return;
        }
    };
    stats.files += 1;
    stats.bytes += bytes.len() as u64;

    let class = match ClassFile::parse(&bytes) {
        Ok(class) => class,
        Err(err) => {
            stats.fail(path, format!("parse: {err}"));
            return;
        }
    };
    stats.pool_slots += class.constant_pool.len() as u64;

    match class.to_bytes() {
        Ok(rewritten) if rewritten == bytes => {}
        Ok(_) => stats.fail(path, "round-trip mismatch".to_owned()),
        Err(err) => stats.fail(path, format!("write: {err}")),
    }

    let mut method_attributes = Vec::new();
    method_attributes.extend(class.attributes.iter());
    for field in &class.fields {
        method_attributes.extend(field.attributes.iter());
    }
    for method in &class.methods {
        method_attributes.extend(method.attributes.iter());
    }
    for attribute in method_attributes {
        check_attribute(attribute, &class.constant_pool, stats, path);
    }

    match class.to_bytes_rebuilt() {
        Ok(rebuilt) => {
            if let Err(err) = ClassFile::parse(&rebuilt) {
                stats.fail(path, format!("reparse rebuilt: {err}"));
            }
        }
        Err(err) => stats.fail(path, format!("rebuild: {err}")),
    }
}

fn check_attribute<'a>(
    attribute: &java_bytecode::Attribute<'a>,
    pool: &java_bytecode::ConstantPool<'a>,
    stats: &mut Stats,
    path: &Path,
) {
    *stats
        .attributes
        .entry(attribute_name(&attribute.body))
        .or_default() += 1;
    if matches!(attribute.body, AttributeBody::Raw(_)) {
        let name = attribute
            .resolved_name(pool)
            .unwrap_or_else(|_| "<unknown>".to_owned());
        *stats.raw_names.entry(name).or_default() += 1;
    }
    match &attribute.body {
        AttributeBody::Code(code) => {
            stats.code_attributes += 1;
            let decoded = unsafe { java_bytecode::bytecode::decode(code.code()) };
            match decoded.and_then(|instructions| {
                stats.instructions += instructions.len() as u64;
                unsafe { java_bytecode::bytecode::encode(&instructions) }
            }) {
                Ok(reencoded) if reencoded == code.code() => {}
                Ok(_) => stats.fail(path, "bytecode round-trip mismatch".to_owned()),
                Err(err) => stats.fail(path, format!("bytecode: {err}")),
            }
            for nested in &code.attributes {
                check_attribute(nested, pool, stats, path);
            }
        }
        AttributeBody::Record(record) => {
            for component in &record.components {
                for nested in &component.attributes {
                    check_attribute(nested, pool, stats, path);
                }
            }
        }
        _ => {}
    }
}

fn main() -> ExitCode {
    let Some(root) = std::env::args().nth(1) else {
        eprintln!("usage: corpus <dir>");
        return ExitCode::FAILURE;
    };

    let mut files = Vec::new();
    collect(Path::new(&root), &mut files);
    files.sort();

    let mut stats = Stats::new();
    for (i, path) in files.iter().enumerate() {
        check(path, &mut stats);
        if (i + 1) % 5000 == 0 {
            eprintln!("checked {}/{}", i + 1, files.len());
        }
    }

    println!("files checked:      {}", stats.files);
    println!("total bytes:        {}", stats.bytes);
    println!("constant pool slots: {}", stats.pool_slots);
    println!("Code attributes:    {}", stats.code_attributes);
    println!("instructions:       {}", stats.instructions);
    println!("attribute histogram:");
    for (name, count) in &stats.attributes {
        println!("  {name:<28} {count}");
    }
    println!("uninterpreted (Raw) attribute names:");
    for (name, count) in &stats.raw_names {
        println!("  {name:<28} {count}");
    }
    println!("failures:           {}", stats.failures.len());
    for failure in &stats.failures {
        println!("  {failure}");
    }

    if stats.failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
