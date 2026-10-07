//! Byte-for-byte round-trip over every checked-in Java 25 fixture.

use std::fs;
use std::path::{Path, PathBuf};

use java_bytecode::ClassFile;

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

#[test]
fn every_fixture_round_trips_byte_for_byte() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/classes");
    let mut files = Vec::new();
    collect_class_files(&root, &mut files);
    files.sort();

    assert!(
        files.len() >= 30,
        "expected a substantial fixture corpus, found {}",
        files.len()
    );

    for path in &files {
        let bytes = fs::read(path).expect("read class file");
        let class = ClassFile::parse(&bytes)
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()));
        let rewritten = class
            .to_bytes()
            .unwrap_or_else(|err| panic!("failed to write {}: {err}", path.display()));
        assert_eq!(
            bytes,
            rewritten,
            "round-trip mismatch for {}",
            path.display()
        );
    }

    eprintln!("round-tripped {} fixture(s)", files.len());
}
