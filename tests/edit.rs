//! Demonstrates the borrowed editing workflow: new data is supplied by the
//! caller as `&'a` and the class file borrows it.

// The `ConstantRef::into_raw` escape hatch is exercised below.
#![allow(unsafe_code)]

use std::fs;
use std::path::Path;

use java_bytecode::{ClassFile, Constant, Instruction, Mutf8, Opcode};

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/classes/main/example")
        .join(name);
    fs::read(path).expect("read fixture")
}

#[test]
fn edits_instructions_through_the_scoped_view() {
    let bytes = fixture("Simple.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");

    // Intern `example/Hook.entry()V` and build a Methodref to it.
    let class_name = class
        .constant_pool
        .intern_utf8(Mutf8::try_from("example/Hook").unwrap())
        .unwrap();
    let class_ref = class
        .constant_pool
        .intern(Constant::Class {
            name_index: class_name,
        })
        .unwrap();
    let name = class
        .constant_pool
        .intern_utf8(Mutf8::try_from("entry").unwrap())
        .unwrap();
    let descriptor = class
        .constant_pool
        .intern_utf8(Mutf8::try_from("()V").unwrap())
        .unwrap();
    let name_and_type = class
        .constant_pool
        .intern(Constant::NameAndType {
            name_index: name,
            descriptor_index: descriptor,
        })
        .unwrap();
    let method_ref = class
        .constant_pool
        .intern(Constant::Methodref {
            class_index: class_ref,
            name_and_type_index: name_and_type,
        })
        .unwrap();

    // Edit through the scoped view, then write back.
    {
        let code = class
            .find_method_mut("voidMethod", "()V")
            .unwrap()
            .code_mut()
            .unwrap();
        // Read-only counterpart.
        assert_eq!(code.instructions().unwrap().len(), 1);
        let mut instructions = code.instructions_mut().unwrap();
        instructions.insert(
            0,
            Instruction::ConstantWide(Opcode::Invokestatic, method_ref),
        );
        instructions.apply().unwrap();
    }

    let rewritten = class.to_bytes().expect("write");
    let mut reparsed = ClassFile::parse(&rewritten).expect("reparse");
    let loaded = reparsed
        .find_method_mut("voidMethod", "()V")
        .unwrap()
        .code_mut()
        .unwrap()
        .instructions_mut()
        .unwrap()
        .into_vec();
    // `voidMethod` is just `return`; the hook was prepended.
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].opcode(), Opcode::Invokestatic);
    assert_eq!(loaded[1].opcode(), Opcode::Return);
}

#[test]
fn renames_a_method_by_interning_a_new_utf8() {
    let bytes = fixture("Simple.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");

    let renamed = class
        .constant_pool
        .intern_utf8(Mutf8::try_from("addTwo").unwrap())
        .expect("intern");

    let method = class
        .find_method_mut("add", "(IJ)J")
        .expect("method add(IJ)J");
    method.name_index = renamed;

    let rewritten = class.to_bytes().expect("write");
    let reparsed = ClassFile::parse(&rewritten).expect("reparse");
    assert!(reparsed.find_method("addTwo", "(IJ)J").is_some());
    assert!(reparsed.find_method("add", "(IJ)J").is_none());
}

#[test]
fn append_only_write_keeps_existing_indices() {
    let bytes = fixture("Simple.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");

    let before = class.constant_pool.len();
    let appended = Mutf8::try_from("unused-but-valid").unwrap();
    let index = class.constant_pool.intern_utf8(appended).expect("intern");
    // `into_raw` is `unsafe`: we promise the index belongs to `class`.
    let raw = unsafe { index.into_raw() };
    assert_eq!(raw as usize, before); // first free slot

    let rewritten = class.to_bytes().expect("write");
    let reparsed = ClassFile::parse(&rewritten).expect("reparse");
    // A reference from the parsed file is scoped to *it*, so fetch it there.
    let found = reparsed
        .constant_pool
        .find(&Constant::Utf8(appended))
        .expect("interned constant present");
    assert_eq!(reparsed.constant_pool.utf8(found).unwrap(), appended);
    assert_eq!(unsafe { found.into_raw() }, raw);
    // Appending one constant adds exactly one pool slot.
    assert_eq!(reparsed.constant_pool.len(), before + 1);
}

#[test]
fn rebuild_drops_unused_constants() {
    let bytes = fixture("Simple.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");

    let original_len = class.constant_pool.len();
    let _ = class
        .constant_pool
        .intern_utf8(Mutf8::try_from("completely-unused").unwrap())
        .expect("intern");

    let rebuilt = class.to_bytes_rebuilt().expect("rebuild");
    let reparsed = ClassFile::parse(&rebuilt).expect("reparse");
    assert!(
        reparsed.constant_pool.len() <= original_len,
        "rebuild should drop the added unused constant"
    );
}

#[test]
fn set_code_variants() {
    let bytes = fixture("Simple.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");

    // Borrowed replacement with a static bytecode stream (`return`).
    {
        let code = class
            .find_method_mut("voidMethod", "()V")
            .unwrap()
            .code_mut()
            .unwrap();
        unsafe { code.set_code(&[0xb1]) };
    }

    // Owned replacement: reinstall the current bytes.
    {
        let code = class
            .find_method_mut("voidMethod", "()V")
            .unwrap()
            .code_mut()
            .unwrap();
        let owned = code.code().to_vec().into_boxed_slice();
        unsafe { code.set_code_owned(owned) };
    }

    let rewritten = class.to_bytes().expect("write");
    let mut reparsed = ClassFile::parse(&rewritten).expect("reparse");
    let loaded = reparsed
        .find_method_mut("voidMethod", "()V")
        .unwrap()
        .code_mut()
        .unwrap()
        .instructions_mut()
        .unwrap()
        .into_vec();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].opcode(), Opcode::Return);
}

#[test]
fn interns_are_deduplicated() {
    let bytes = fixture("Simple.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");

    let first = class
        .constant_pool
        .intern_utf8(Mutf8::try_from("dup").unwrap())
        .expect("intern");
    let second = class
        .constant_pool
        .intern_utf8(Mutf8::try_from("dup").unwrap())
        .expect("intern");
    assert_eq!(first, second);
    assert_eq!(
        class.constant_pool.get(first),
        Some(&Constant::Utf8(Mutf8::try_from("dup").unwrap()))
    );
}
