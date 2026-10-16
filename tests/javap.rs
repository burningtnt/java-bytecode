//! Character-for-character comparison of the crate's javap-compatible
//! disassembler against `javap -p -c -v`.
//!
//! The fixture classes run by default. The OpenJDK corpus is opt-in: set
//! `JAVA_BYTECODE_CORPUS` (or place it at `tests/fixtures/jdk-corpus`).
//!
//! The reference `javap` is located through `JAVA_BYTECODE_JAVAP`, then
//! `JAVA_HOME/bin/javap`, then `/tmp/opencode/jdk-27/bin/javap`. It must be a
//! Java 27 launcher; otherwise the comparison is skipped (the corpus targets
//! class-file major version 71 = Java 27).
//!
//! Performance: many class files are handed to a single `javap` process per
//! batch (JVM startup dominates a per-file invocation), and a shared atomic
//! work queue keeps every worker busy instead of busy-waiting. Tune with
//! `JAVA_BYTECODE_JOBS` (workers) and `JAVA_BYTECODE_BATCH` (files per javap).

// `bytecode::decode`/`encode` are `unsafe` and used by `common::check_code`;
// `ConstantRef::from_raw` is used by the renderer.
#![allow(unsafe_code)]

mod common;
#[path = "javap_support/mod.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use java_bytecode::ClassFile;
use sha2::{Digest, Sha256};

use support::Header;

static T_JAVAP_NANOS: AtomicU64 = AtomicU64::new(0);
static T_RENDER_NANOS: AtomicU64 = AtomicU64::new(0);
static N_JAVAP: AtomicU64 = AtomicU64::new(0);
static N_RENDER: AtomicU64 = AtomicU64::new(0);

const FIXTURES: &str = "tests/fixtures/classes";

#[test]
fn library_parses_and_roundtrips_corpus() {
    let mut files = Vec::new();
    common::collect_class_files(&fixtures_dir(), &mut files);
    if let Some(corpus) = corpus_dir() {
        common::collect_class_files(&corpus, &mut files);
    }

    let mut failures = Vec::new();
    let mut classes = 0usize;
    let mut codes = 0usize;
    for path in &files {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) => {
                failures.push(format!("{}: read: {err}", path.display()));
                continue;
            }
        };
        let class = match ClassFile::parse(&bytes) {
            Ok(class) => class,
            Err(err) => {
                failures.push(format!("{}: parse: {err}", path.display()));
                continue;
            }
        };
        classes += 1;

        let mut code_attrs = Vec::new();
        common::collect_code(&class, &mut code_attrs);
        for code in &code_attrs {
            codes += 1;
            if let Err(reason) = common::check_code(code.code()) {
                failures.push(format!("{}: {reason}", path.display()));
            }
        }

        match class.to_bytes() {
            Ok(out) if out == bytes => {}
            Ok(_) => failures.push(format!("{}: to_bytes() != input", path.display())),
            Err(err) => failures.push(format!("{}: to_bytes: {err}", path.display())),
        }
    }

    eprintln!("P0: parsed {classes} class(es), {codes} Code attribute(s)");

    let fixture_prefix = fixtures_dir();
    let fixture_failures: Vec<&String> = failures
        .iter()
        .filter(|fail| fail.starts_with(&fixture_prefix.display().to_string()))
        .collect();

    assert!(
        fixture_failures.is_empty(),
        "fixture parse/roundtrip failures (library bug):\n{}",
        fixture_failures
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    if !failures.is_empty() {
        eprintln!(
            "warning: {} corpus parse/roundtrip failure(s):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

#[test]
fn javap_matches_fixtures() {
    let Some(javap) = find_javap() else {
        eprintln!("warning: no Java 27 javap found; skipping javap comparison");
        return;
    };

    let mut files = Vec::new();
    common::collect_class_files(&fixtures_dir(), &mut files);
    run_comparison(&javap, &files, "fixtures");
}

#[test]
fn javap_matches_corpus() {
    let t0 = Instant::now();
    let Some(corpus) = corpus_dir() else {
        eprintln!("warning: skipping javap corpus comparison; no corpus found");
        return;
    };
    let t1 = Instant::now();
    let Some(javap) = find_javap() else {
        eprintln!("warning: no Java 27 javap found; skipping javap comparison");
        return;
    };
    let t2 = Instant::now();

    let mut files = Vec::new();
    common::collect_class_files(&corpus, &mut files);
    let t3 = Instant::now();
    if let Ok(max) = std::env::var("JAVA_BYTECODE_CORPUS_MAX") {
        if let Ok(max) = max.parse::<usize>() {
            files.truncate(max);
        }
    }
    if std::env::var("JAVA_BYTECODE_TIMING").is_ok() {
        eprintln!(
            "setup: corpus_dir {} ms, find_javap {} ms, collect {} ms",
            (t1 - t0).as_millis(),
            (t2 - t1).as_millis(),
            (t3 - t2).as_millis()
        );
    }
    run_comparison(&javap, &files, "corpus");
}

struct Tally {
    matched: usize,
    failed: usize,
    failures: Vec<String>,
}

impl Tally {
    fn new() -> Self {
        Self {
            matched: 0,
            failed: 0,
            failures: Vec::new(),
        }
    }

    fn record(&mut self, result: Result<(), String>, path: &Path) {
        match result {
            Ok(()) => self.matched += 1,
            Err(reason) => {
                self.failed += 1;
                if self.failures.len() < 25 {
                    self.failures.push(format!("{}: {reason}", path.display()));
                }
            }
        }
    }
}

fn run_comparison(javap: &Path, files: &[PathBuf], label: &str) {
    let jobs = std::env::var("JAVA_BYTECODE_JOBS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(std::num::NonZero::get)
                .unwrap_or(1)
        });
    let batch_size = std::env::var("JAVA_BYTECODE_BATCH")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(256);

    let batches: Vec<&[PathBuf]> = files.chunks(batch_size).collect();
    let next = AtomicUsize::new(0);
    let wall_start = Instant::now();

    let tallies: Vec<Tally> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..jobs.max(1))
            .map(|_| {
                scope.spawn(|| {
                    let mut tally = Tally::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(batch) = batches.get(index) else {
                            break;
                        };
                        compare_batch(javap, batch, &mut tally);
                    }
                    tally
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("worker thread panicked"))
            .collect()
    });

    let mut matched = 0usize;
    let mut failed = 0usize;
    let mut failures = Vec::new();
    for tally in tallies {
        matched += tally.matched;
        failed += tally.failed;
        failures.extend(tally.failures);
    }
    failures.truncate(25);
    if std::env::var("JAVA_BYTECODE_TIMING").is_ok() {
        let jn = T_JAVAP_NANOS.load(Ordering::Relaxed);
        let rn = T_RENDER_NANOS.load(Ordering::Relaxed);
        let jc = N_JAVAP.load(Ordering::Relaxed).max(1);
        let rc = N_RENDER.load(Ordering::Relaxed).max(1);
        eprintln!(
            "timing[{label}]: wall {} ms; jobs {jobs} batches {}; javap {jc} call(s) {} ms ({} ms/call); render {rc} file(s) {} ms ({} us/file)",
            wall_start.elapsed().as_millis(),
            batches.len(),
            jn / 1_000_000,
            jn / 1_000_000 / jc,
            rn / 1_000_000,
            rn / 1_000 / rc
        );
    }
    eprintln!(
        "javap {label}: {matched} matched, {failed} failed (diff files under target/javap-diff)"
    );
    assert!(
        failed == 0,
        "javap comparison failed for {failed} {label} file(s):\n{}",
        failures.join("\n")
    );
}

fn compare_batch(javap: &Path, batch: &[PathBuf], tally: &mut Tally) {
    let paths: Vec<&Path> = batch.iter().map(PathBuf::as_path).collect();
    if let Ok(expected) = javap_output(javap, &paths) {
        if expected.len() == batch.len() {
            for (path, expected) in batch.iter().zip(&expected) {
                tally.record(render_and_compare(path, expected), path);
            }
            return;
        }
    }
    // Batch output could not be split reliably; fall back to one javap per file.
    for path in batch {
        tally.record(compare_one(javap, path), path);
    }
}

fn compare_one(javap: &Path, path: &Path) -> Result<(), String> {
    let expected = javap_output(javap, &[path])?
        .into_iter()
        .next()
        .ok_or_else(|| "javap produced no output".to_owned())?;
    render_and_compare(path, &expected)
}

/// Runs javap once over `paths` and splits its stdout back into one fragment
/// per input (each begins with the `Classfile ` header).
fn javap_output(javap: &Path, paths: &[&Path]) -> Result<Vec<String>, String> {
    let started = Instant::now();
    let output = Command::new(javap)
        .args([
            "-J-Duser.language=en",
            "-J-Duser.country=US",
            "-J-Duser.variant=",
            "-J-Dstdout.encoding=UTF-8",
            "-J-Dstderr.encoding=UTF-8",
            "-p",
            "-c",
            "-v",
        ])
        .args(paths)
        .env("TZ", "UTC")
        .env("LANG", "C.utf8")
        .env("LC_ALL", "C.utf8")
        .output()
        .map_err(|e| format!("spawn javap: {e}"))?;
    T_JAVAP_NANOS.fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
    N_JAVAP.fetch_add(1, Ordering::Relaxed);
    Ok(split_javap_output(&str::from_utf8(&output.stdout).map_err(|e| format!("javap utf8: {e}"))?))
}

fn split_javap_output(stdout: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut start: Option<usize> = None;
    let mut offset = 0usize;
    for line in stdout.split_inclusive('\n') {
        if line.starts_with("Classfile ") {
            if let Some(previous) = start {
                segments.push(stdout[previous..offset].to_owned());
            }
            start = Some(offset);
        }
        offset += line.len();
    }
    if let Some(previous) = start {
        segments.push(stdout[previous..].to_owned());
    }
    segments
}

fn render_and_compare(path: &Path, expected: &str) -> Result<(), String> {
    let started = Instant::now();
    let result = render_and_compare_inner(path, expected);
    T_RENDER_NANOS.fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
    N_RENDER.fetch_add(1, Ordering::Relaxed);
    result
}

fn render_and_compare_inner(path: &Path, expected: &str) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("read: {e}"))?;
    let class = ClassFile::parse(&bytes).map_err(|e| format!("parse: {e}"))?;

    let printed_path = expected
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("Classfile "))
        .unwrap_or_else(|| path.to_str().unwrap_or_default())
        .to_owned();

    let metadata = fs::metadata(path).map_err(|e| format!("metadata: {e}"))?;
    let size = metadata.len() as u32;
    let last_modified_millis = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64);
    let digest: [u8; 32] = Sha256::digest(&bytes).into();

    let header = Header {
        path: printed_path,
        size,
        last_modified_millis,
        digest,
    };
    let actual = support::disassemble(&class, &header);

    match first_difference(&actual, expected) {
        None => Ok(()),
        Some(difference) => {
            write_debug_files(path, &actual, expected);
            Err(difference)
        }
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

/// Returns a description of the first difference, or `None` if the two texts
/// are identical. Streams the lines (and consumes the tail with `count`) so it
/// never allocates a `Vec` of the (potentially very large) outputs.
fn first_difference(actual: &str, expected: &str) -> Option<String> {
    let mut actual_lines = actual.lines();
    let mut expected_lines = expected.lines();
    let mut line = 0usize;
    loop {
        match (actual_lines.next(), expected_lines.next()) {
            (Some(ours), Some(theirs)) => {
                if ours != theirs {
                    return Some(format!(
                        "first difference at line {}:\n  ours:   {:?}\n  javap:  {:?}",
                        line + 1,
                        truncate(ours, 160),
                        truncate(theirs, 160)
                    ));
                }
                line += 1;
            }
            (None, None) => return None,
            (ours, theirs) => {
                let ours_total = line + usize::from(ours.is_some()) + actual_lines.count();
                let theirs_total = line + usize::from(theirs.is_some()) + expected_lines.count();
                return Some(format!(
                    "line count differs: ours {ours_total} lines, javap {theirs_total} lines (diff files under target/javap-diff)"
                ));
            }
        }
    }
}

fn write_debug_files(path: &Path, actual: &str, expected: &str) {
    let name = path
        .to_string_lossy()
        .replace(|c: char| !c.is_ascii_alphanumeric(), "_");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/javap-diff");
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join(format!("{name}.ours.txt")), actual);
    let _ = fs::write(dir.join(format!("{name}.javap.txt")), expected);
}

fn find_javap() -> Option<PathBuf> {
    let mut bases: Vec<PathBuf> = Vec::new();
    if let Ok(value) = std::env::var("JAVA_BYTECODE_JAVAP") {
        bases.push(PathBuf::from(value));
    }
    if let Ok(home) = std::env::var("JAVA_HOME") {
        bases.push(PathBuf::from(home).join("bin").join("javap"));
    }
    bases.push(PathBuf::from("/tmp/opencode/jdk-27/bin/javap"));

    // On Windows the launcher is `javap.exe`; try both spellings.
    let mut candidates: Vec<PathBuf> = Vec::with_capacity(bases.len() * 2);
    for base in bases {
        candidates.push(base.with_extension(""));
        candidates.push(base.with_extension("exe"));
    }

    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        let Ok(output) = Command::new(&candidate).arg("-version").output() else {
            continue;
        };
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if text.lines().any(|line| line.starts_with("27")) {
            return Some(candidate);
        }
    }
    None
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES)
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
