//! Bytecode encode/decode checks over a large OpenJDK corpus.
//!
//! The corpus is found at `JAVA_BYTECODE_CORPUS`, or else at
//! `tests/fixtures/jdk-corpus` (created by `scripts/fetch_jdk_corpus.sh`). When
//! neither exists the test prints a warning and skips.
//!
//! Each `Code` attribute is checked with [`common::check_code`] (round-trip,
//! `encoded_len` consistency and branch-target boundaries). The run also
//! reports opcode coverage and `switch` alignment statistics, and feeds
//! truncated/mutated bytecode to `decode` to confirm it never panics.
//!
//! `bytecode::decode` is `unsafe` because it cannot manage constant-pool
//! indices; here the instructions stay within their originating class.
#![allow(unsafe_code)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use java_bytecode::ClassFile;
use java_bytecode::bytecode::{Instruction, Opcode, decode};

#[derive(Default)]
struct Stats {
    files: usize,
    codes: usize,
    instructions: usize,
    opcodes: BTreeSet<u8>,
    switch_alignment: [usize; 4],
    failures: Vec<String>,
}

impl Stats {
    fn fail(&mut self, path: &Path, reason: impl std::fmt::Display) {
        if self.failures.len() < 100 {
            self.failures.push(format!("{}: {reason}", path.display()));
        }
    }
}

fn corpus_dir() -> Option<PathBuf> {
    if let Ok(value) = std::env::var("JAVA_BYTECODE_CORPUS") {
        let dir = PathBuf::from(value);
        if dir.is_dir() {
            return Some(dir);
        }
        eprintln!(
            "warning: JAVA_BYTECODE_CORPUS={} is not a directory",
            dir.display()
        );
    }
    let default = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jdk-corpus");
    default.is_dir().then_some(default)
}

fn check_file(path: &Path, stats: &mut Stats) {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            stats.fail(path, format!("read: {err}"));
            return;
        }
    };
    stats.files += 1;

    let class = match ClassFile::parse(&bytes) {
        Ok(class) => class,
        Err(err) => {
            stats.fail(path, format!("parse: {err}"));
            return;
        }
    };

    let mut codes = Vec::new();
    common::collect_code(&class, &mut codes);
    for code in codes {
        stats.codes += 1;
        if let Err(reason) = common::check_code(code.code()) {
            stats.fail(path, reason);
            continue;
        }
        collect_stats(path, code.code(), stats);
    }
}

fn collect_stats(path: &Path, code: &[u8], stats: &mut Stats) {
    let instructions = match unsafe { decode(code) } {
        Ok(instructions) => instructions,
        Err(err) => {
            stats.fail(path, format!("decode: {err}"));
            return;
        }
    };
    let mut pc = 0usize;
    for instruction in &instructions {
        stats.instructions += 1;
        stats.opcodes.insert(instruction.opcode().as_u8());
        if matches!(
            instruction,
            Instruction::TableSwitch { .. } | Instruction::LookupSwitch { .. }
        ) {
            stats.switch_alignment[pc % 4] += 1;
        }
        pc += instruction.encoded_len(pc);
    }

    // Truncations and a byte mutation must never panic.
    for len in [code.len() / 3, code.len() / 2, code.len().saturating_sub(1)] {
        let _ = unsafe { decode(&code[..len]) };
    }
    let mut mutated = code.to_vec();
    if let Some(first) = mutated.first_mut() {
        *first ^= 0xFF;
        let _ = unsafe { decode(&mutated) };
    }
}

#[test]
fn jdk_corpus_bytecode() {
    let Some(root) = corpus_dir() else {
        eprintln!(
            "warning: skipping bytecode corpus test; run scripts/fetch_jdk_corpus.sh \
             or set JAVA_BYTECODE_CORPUS"
        );
        return;
    };

    let mut files = Vec::new();
    common::collect_class_files(&root, &mut files);
    if let Ok(max) = std::env::var("JAVA_BYTECODE_CORPUS_MAX") {
        if let Ok(max) = max.parse::<usize>() {
            files.truncate(max);
        }
    }
    assert!(!files.is_empty(), "no class files under {}", root.display());

    let mut stats = Stats::default();
    for path in &files {
        check_file(path, &mut stats);
    }

    let total_opcodes = (0u16..=0xFF)
        .filter(|byte| Opcode::from_u8(*byte as u8).is_some())
        .count();
    eprintln!(
        "checked {} file(s), {} Code attribute(s), {} instruction(s)",
        stats.files, stats.codes, stats.instructions
    );
    eprintln!(
        "opcode coverage: {}/{} distinct opcodes",
        stats.opcodes.len(),
        total_opcodes
    );
    let uncovered: Vec<&str> = (0u16..=0xFF)
        .filter_map(|byte| Opcode::from_u8(byte as u8))
        .filter(|op| !stats.opcodes.contains(&op.as_u8()))
        .map(Opcode::mnemonic)
        .collect();
    if !uncovered.is_empty() {
        eprintln!("uncovered opcodes: {}", uncovered.join(", "));
    }
    eprintln!("switch alignment (pc%4): {:?}", stats.switch_alignment);

    assert!(
        stats.failures.is_empty(),
        "{} failure(s) over {} file(s):\n{}",
        stats.failures.len(),
        files.len(),
        stats.failures.join("\n")
    );
}
