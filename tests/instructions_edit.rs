//! Exercises the [`Instructions`] editing API: read accessors and the
//! `insert` / `remove` / `replace` / `splice` methods.
//!
//! [`Instructions`]: java_bytecode::Instructions

use std::fs;
use std::path::Path;

use java_bytecode::{ClassFile, Error, Instruction, Opcode};

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/classes/main/example")
        .join(name);
    fs::read(path).expect("read fixture")
}

#[test]
fn read_accessors_describe_the_body() {
    let bytes = fixture("ControlFlow.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");
    let code = class
        .find_method_mut("loops", "(I)I")
        .unwrap()
        .code_mut()
        .unwrap();
    let instructions = code.instructions_mut().unwrap();

    assert_eq!(instructions.len(), instructions.as_slice().len());
    assert!(!instructions.is_empty());
    assert_eq!(
        instructions.get(0).map(Instruction::opcode),
        Some(Opcode::Iconst0)
    );
    assert_eq!(instructions.get(usize::MAX), None);
    let opcodes: Vec<_> = instructions.iter().map(Instruction::opcode).collect();
    assert_eq!(opcodes[0], Opcode::Iconst0);
}

#[test]
fn remove_and_replace_return_the_old_instruction() {
    let bytes = fixture("Simple.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");
    let code = class
        .find_method_mut("voidMethod", "()V")
        .unwrap()
        .code_mut()
        .unwrap();
    let mut instructions = code.instructions_mut().unwrap();
    assert_eq!(instructions.len(), 1);

    let replaced = instructions
        .replace(0, Instruction::None(Opcode::Nop))
        .unwrap();
    assert_eq!(replaced.opcode(), Opcode::Return);
    assert_eq!(
        instructions.get(0).map(Instruction::opcode),
        Some(Opcode::Nop)
    );

    let removed = instructions.remove(0).unwrap();
    assert_eq!(removed.opcode(), Opcode::Nop);
    assert!(instructions.is_empty());
}

#[test]
fn splice_replaces_a_range_and_returns_the_removed() {
    let bytes = fixture("ControlFlow.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");
    let code = class
        .find_method_mut("loops", "(I)I")
        .unwrap()
        .code_mut()
        .unwrap();
    let mut instructions = code.instructions_mut().unwrap();
    let len_before = instructions.len();

    let removed = instructions
        .splice(0..2, [Instruction::None(Opcode::Nop)])
        .unwrap();
    assert_eq!(removed.len(), 2);
    assert_eq!(instructions.len(), len_before - 1);
    assert_eq!(
        instructions.get(0).map(Instruction::opcode),
        Some(Opcode::Nop)
    );
}

#[test]
fn insert_all_adds_a_sequence_at_an_index() {
    let bytes = fixture("Simple.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");
    let code = class
        .find_method_mut("voidMethod", "()V")
        .unwrap()
        .code_mut()
        .unwrap();
    let mut instructions = code.instructions_mut().unwrap();

    instructions
        .insert_all(
            0,
            [
                Instruction::None(Opcode::Nop),
                Instruction::None(Opcode::Nop),
            ],
        )
        .unwrap();
    let opcodes: Vec<_> = instructions.iter().map(Instruction::opcode).collect();
    assert_eq!(opcodes, [Opcode::Nop, Opcode::Nop, Opcode::Return]);
}

#[test]
fn out_of_range_edit_is_an_error() {
    let bytes = fixture("Simple.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");
    let code = class
        .find_method_mut("voidMethod", "()V")
        .unwrap()
        .code_mut()
        .unwrap();
    let mut instructions = code.instructions_mut().unwrap();

    assert!(matches!(
        instructions.remove(999),
        Err(Error::InstructionIndexOutOfRange { .. })
    ));
    assert_eq!(instructions.len(), 1);
}
