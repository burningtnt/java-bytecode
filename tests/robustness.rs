//! Robustness: the parser must never panic on arbitrary, mutated or truncated
//! input.

use std::fs;
use std::path::Path;

use java_bytecode::ClassFile;
use proptest::prelude::*;

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/classes/main/example")
        .join(name);
    fs::read(path).expect("read fixture")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn parse_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        // A malformed input may fail, but it must never panic.
        let _ = ClassFile::parse(&bytes);
    }

    #[test]
    fn parse_of_mutated_fixture_never_panics(
        mutations in proptest::collection::vec(
            (any::<prop::sample::Index>(), any::<u8>()),
            0..32,
        ),
    ) {
        let mut data = fixture_bytes("ControlFlow.class");
        if data.is_empty() {
            return Ok(());
        }
        for (index, value) in mutations {
            let pos = index.index(data.len());
            data[pos] = value;
        }
        let _ = ClassFile::parse(&data);
    }
}

#[test]
fn truncated_input_never_panics() {
    let bytes = fixture_bytes("Simple.class");
    for len in 0..bytes.len() {
        let result = ClassFile::parse(&bytes[..len]);
        assert!(
            result.is_err(),
            "a truncated class file ({len} bytes) unexpectedly parsed"
        );
    }
    assert!(ClassFile::parse(&bytes).is_ok());
}
