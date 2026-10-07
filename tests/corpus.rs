//! Optional large-corpus test.
//!
//! Set `JAVA_BYTECODE_CORPUS` to a directory of `.class` files (for example the
//! classes extracted from a JDK with `jimage extract`) to run the parser,
//! serializer, bytecode codec and rebuild pass over every one of them:
//!
//! ```text
//! jimage extract --dir /tmp/jdk-classes /path/to/jdk/lib/modules
//! JAVA_BYTECODE_CORPUS=/tmp/jdk-classes cargo test --release --test corpus -- --nocapture
//! ```
//!
//! When the variable is unset the test is skipped, so the normal suite stays
//! self-contained.

#![allow(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use java_bytecode::{AttributeBody, ClassFile};

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

fn check(path: &Path, failures: &mut Vec<String>) -> bool {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            failures.push(format!("{}: read: {err}", path.display()));
            return false;
        }
    };
    let class = match ClassFile::parse(&bytes) {
        Ok(class) => class,
        Err(err) => {
            failures.push(format!("{}: parse: {err}", path.display()));
            return false;
        }
    };
    match class.to_bytes() {
        Ok(rewritten) if rewritten == bytes => {}
        Ok(_) => failures.push(format!("{}: round-trip mismatch", path.display())),
        Err(err) => failures.push(format!("{}: write: {err}", path.display())),
    }
    check_attributes(&class.attributes, path, failures);
    for field in &class.fields {
        check_attributes(&field.attributes, path, failures);
    }
    for method in &class.methods {
        check_attributes(&method.attributes, path, failures);
    }
    match class.to_bytes_rebuilt() {
        Ok(rebuilt) => {
            if let Err(err) = ClassFile::parse(&rebuilt) {
                failures.push(format!("{}: reparse rebuilt: {err}", path.display()));
            }
        }
        Err(err) => failures.push(format!("{}: rebuild: {err}", path.display())),
    }
    true
}

fn check_attributes(
    attributes: &[java_bytecode::Attribute<'_>],
    path: &Path,
    failures: &mut Vec<String>,
) {
    for attribute in attributes {
        match &attribute.body {
            AttributeBody::Code(code) => {
                let decoded = unsafe { java_bytecode::bytecode::decode(code.code()) };
                match decoded.and_then(|instructions| unsafe {
                    java_bytecode::bytecode::encode(&instructions)
                }) {
                    Ok(reencoded) if reencoded == code.code() => {}
                    Ok(_) => failures.push(format!("{}: bytecode mismatch", path.display())),
                    Err(err) => failures.push(format!("{}: bytecode: {err}", path.display())),
                }
                check_attributes(&code.attributes, path, failures);
            }
            AttributeBody::Record(record) => {
                for component in &record.components {
                    check_attributes(&component.attributes, path, failures);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn jdk_corpus() {
    let Ok(root) = std::env::var("JAVA_BYTECODE_CORPUS") else {
        eprintln!("skipping corpus test: set JAVA_BYTECODE_CORPUS to a class directory");
        return;
    };

    let mut files = Vec::new();
    collect(Path::new(&root), &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .class files found under {root}");

    let mut failures = Vec::new();
    let mut checked = 0usize;
    for path in &files {
        if check(path, &mut failures) {
            checked += 1;
        }
    }
    eprintln!("checked {checked}/{} class file(s)", files.len());

    assert!(
        failures.is_empty(),
        "{} failure(s) over {} file(s):\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}
