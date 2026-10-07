//! Decode/encode checks over the checked-in fixture corpus.

mod common;

use std::fs;
use std::path::Path;

use java_bytecode::ClassFile;

#[test]
fn every_code_attribute_round_trips() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/classes");
    let mut files = Vec::new();
    common::collect_class_files(&root, &mut files);
    assert!(
        files.len() >= 30,
        "expected a substantial fixture corpus, found {}",
        files.len()
    );

    let mut checked = 0usize;
    let mut failures = Vec::new();
    for path in &files {
        let bytes = fs::read(path).expect("read class file");
        let class = ClassFile::parse(&bytes)
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()));

        let mut codes = Vec::new();
        common::collect_code(&class, &mut codes);
        for code in codes {
            checked += 1;
            if let Err(reason) = common::check_code(code.code()) {
                failures.push(format!("{}: {reason}", path.display()));
            }
        }
    }

    assert!(
        checked >= 20,
        "expected many Code attributes, found {checked}"
    );
    assert!(
        failures.is_empty(),
        "{} code check(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("checked {checked} Code attribute(s)");
}
