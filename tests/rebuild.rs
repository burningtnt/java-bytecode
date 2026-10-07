//! Rebuilt (compacted) class files must re-parse, be idempotent, and preserve
//! the public structure. When `javap` is available, its declaration output is
//! used as an external oracle.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

fn fixtures() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/classes");
    let mut files = Vec::new();
    collect_class_files(&root, &mut files);
    files.sort();
    files
}

#[test]
fn rebuild_reparses_and_is_idempotent() {
    for path in fixtures() {
        let bytes = fs::read(&path).expect("read class file");
        let class = ClassFile::parse(&bytes).expect("parse");
        let rebuilt = class
            .to_bytes_rebuilt()
            .unwrap_or_else(|e| panic!("rebuild {}: {e}", path.display()));

        let reparsed = ClassFile::parse(&rebuilt)
            .unwrap_or_else(|e| panic!("reparse {}: {e}", path.display()));

        // No data loss: the rebuilt pool cannot be larger.
        assert!(
            reparsed.constant_pool.len() <= class.constant_pool.len(),
            "rebuilt pool grew for {}",
            path.display()
        );
        assert_eq!(reparsed.fields.len(), class.fields.len());
        assert_eq!(reparsed.methods.len(), class.methods.len());

        // Idempotent: rebuilding the rebuilt file is a fixed point.
        let rebuilt_again = reparsed
            .to_bytes_rebuilt()
            .unwrap_or_else(|e| panic!("second rebuild {}: {e}", path.display()));
        assert_eq!(
            rebuilt,
            rebuilt_again,
            "rebuild is not idempotent for {}",
            path.display()
        );
    }
}

#[test]
fn rebuild_matches_javap_declarations() {
    if Command::new("javap").arg("-version").output().is_err() {
        eprintln!("skipping: javap not available");
        return;
    }

    let dir = std::env::temp_dir().join(format!("java-bytecode-rebuild-{}", std::process::id()));
    let orig_dir = dir.join("orig");
    let rebuilt_dir = dir.join("rebuilt");
    fs::create_dir_all(&orig_dir).expect("create temp dir");
    fs::create_dir_all(&rebuilt_dir).expect("create temp dir");

    for path in fixtures() {
        let bytes = fs::read(&path).expect("read class file");
        let class = ClassFile::parse(&bytes).expect("parse");
        let rebuilt = class.to_bytes_rebuilt().expect("rebuild");

        let file_name = path.file_name().expect("file name");
        let original_path = orig_dir.join(file_name);
        let rebuilt_path = rebuilt_dir.join(file_name);
        fs::write(&original_path, &bytes).expect("write original");
        fs::write(&rebuilt_path, &rebuilt).expect("write rebuilt");

        let original = javap(&original_path, &["-p"]);
        let rebuilt_out = javap(&rebuilt_path, &["-p"]);
        assert_eq!(
            original,
            rebuilt_out,
            "javap declarations differ for {}",
            path.display()
        );

        // Bytecode disassembly resolves constant-pool references to names, so
        // after dropping the raw `#index` tokens the two must agree.
        let original_code = normalize_indices(&javap(&original_path, &["-p", "-c"]));
        let rebuilt_code = normalize_indices(&javap(&rebuilt_path, &["-p", "-c"]));
        assert_eq!(
            original_code,
            rebuilt_code,
            "javap bytecode differs for {}",
            path.display()
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

/// Removes raw constant-pool index tokens (`#123`) from javap output.
fn normalize_indices(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '#' {
            while chars.peek().is_some_and(char::is_ascii_digit) {
                chars.next();
            }
            out.push('#');
        } else {
            out.push(ch);
        }
    }
    out
}

fn javap(path: &Path, flags: &[&str]) -> String {
    let output = Command::new("javap")
        .args(flags)
        .arg(path)
        .output()
        .expect("run javap");
    assert!(
        output.status.success(),
        "javap failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}
