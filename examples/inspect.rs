//! Inspect a class file: prints its version, constant-pool size, fields,
//! methods and their attributes.
//!
//! Run with: `cargo run --example inspect -- path/to/Foo.class`

#![allow(unsafe_code)]

use std::process::ExitCode;

use java_bytecode::{AttributeBody, ClassFile};

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: inspect <class-file>");
        return ExitCode::FAILURE;
    };
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("failed to read {path}");
        return ExitCode::FAILURE;
    };
    let class = match ClassFile::parse(&bytes) {
        Ok(class) => class,
        Err(err) => {
            eprintln!("failed to parse {path}: {err}");
            return ExitCode::FAILURE;
        }
    };

    println!("version {}.{}", class.major_version, class.minor_version);
    println!(
        "class {} (super: {})",
        class
            .this_class_name()
            .unwrap_or_else(|_| "<unknown>".to_owned()),
        class
            .super_class_name()
            .ok()
            .flatten()
            .unwrap_or_else(|| "<none>".to_owned())
    );
    println!("constant pool: {} slots", class.constant_pool.len());
    println!("fields: {}", class.fields.len());
    for field in &class.fields {
        println!(
            "  {} : {} [{} attrs]",
            field.name(&class.constant_pool).unwrap_or_default(),
            field.descriptor(&class.constant_pool).unwrap_or_default(),
            field.attributes.len()
        );
    }
    println!("methods: {}", class.methods.len());
    for method in &class.methods {
        let code_len = method.code().map_or(0, |code| code.code().len());
        let instructions = method
            .code()
            .and_then(|code| unsafe { java_bytecode::bytecode::decode(code.code()) }.ok())
            .map_or(0, |list| list.len());
        println!(
            "  {} : {} [{} code bytes, {} instructions, {} attrs]",
            method.name(&class.constant_pool).unwrap_or_default(),
            method.descriptor(&class.constant_pool).unwrap_or_default(),
            code_len,
            instructions,
            method.attributes.len()
        );
    }
    if let Some(code) = class.methods.iter().find_map(|method| {
        method.attributes.iter().find_map(|a| match &a.body {
            AttributeBody::Code(code) => Some(code),
            _ => None,
        })
    }) {
        println!(
            "first Code attribute: max_stack={} max_locals={}",
            code.max_stack, code.max_locals
        );
    }

    ExitCode::SUCCESS
}
