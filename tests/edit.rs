//! Demonstrates the borrowed editing workflow: new data is supplied by the
//! caller as `&'a` and the class file borrows it.

// The `ConstantRef::into_raw` escape hatch is exercised below.
#![allow(unsafe_code)]

use std::fs;
use std::path::Path;

use java_bytecode::{ClassFile, Constant, ConstantRef, Instruction, Mutf8, Opcode};

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/classes/main/example")
        .join(name);
    fs::read(path).expect("read fixture")
}

/// Interns `example/Hook.entry()V` and returns a `Methodref` to it.
fn intern_hook<'a>(class: &mut ClassFile<'a>) -> ConstantRef<'a> {
    class
        .constant_pool
        .intern_method("example/Hook", "entry", "()V", false)
        .unwrap()
}

#[test]
fn edits_instructions_through_the_scoped_view() {
    let bytes = fixture("Simple.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");

    let method_ref = intern_hook(&mut class);

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
        instructions
            .insert(
                0,
                Instruction::ConstantWide(Opcode::Invokestatic, method_ref),
            )
            .unwrap();
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

/// Reads a method body and returns `(opcode, relative_offset)` for each branch.
fn branch_offsets(class: &ClassFile<'_>, name: &str, descriptor: &str) -> Vec<(Opcode, i32)> {
    class
        .find_method(name, descriptor)
        .unwrap()
        .code()
        .unwrap()
        .instructions()
        .unwrap()
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::Branch(opcode, offset) => Some((*opcode, i32::from(*offset))),
            Instruction::BranchWide(opcode, offset) => Some((*opcode, *offset)),
            _ => None,
        })
        .collect()
}

/// Returns the opcode at every `tableswitch`/`lookupswitch` target (default
/// first, then cases) of the switch at `switch_index`, in wire order.
fn switch_target_opcodes(instructions: &[Instruction<'_>], switch_index: usize) -> Vec<Opcode> {
    let mut pcs = Vec::with_capacity(instructions.len());
    let mut pc = 0usize;
    for instruction in instructions {
        pcs.push(pc);
        pc += instruction.encoded_len(pc);
    }
    let base = pcs[switch_index] as i64;
    let mut targets = Vec::new();
    match &instructions[switch_index] {
        Instruction::TableSwitch {
            default, offsets, ..
        } => {
            targets.push(base + i64::from(*default));
            targets.extend(offsets.iter().map(|offset| base + i64::from(*offset)));
        }
        Instruction::LookupSwitch { default, pairs, .. } => {
            targets.push(base + i64::from(*default));
            targets.extend(pairs.iter().map(|(_, offset)| base + i64::from(*offset)));
        }
        other => panic!("expected a switch, got {:?}", other.opcode()),
    }
    targets
        .into_iter()
        .map(|target| {
            let index = pcs
                .iter()
                .position(|&pc| pc as i64 == target)
                .expect("switch target must land on an instruction boundary");
            instructions[index].opcode()
        })
        .collect()
}

#[test]
fn inserts_invokestatic_at_method_head_and_fixes_offsets() {
    let bytes = fixture("ControlFlow.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");
    let method_ref = intern_hook(&mut class);

    // Record the switch default offset and the opcode each case body starts with.
    let (default_before, cases_before) = {
        let instructions = class
            .find_method("dense", "(I)I")
            .unwrap()
            .code()
            .unwrap()
            .instructions()
            .unwrap();
        let default = match &instructions[1] {
            Instruction::TableSwitch { default, .. } => *default,
            other => panic!("expected a tableswitch, got {:?}", other.opcode()),
        };
        (default, switch_target_opcodes(&instructions, 1))
    };

    // Insert the hook at the head. `iload_1; tableswitch` means the switch moved
    // and its alignment padding grew, so the offsets must be rebased.
    {
        let code = class
            .find_method_mut("dense", "(I)I")
            .unwrap()
            .code_mut()
            .unwrap();
        let mut instructions = code.instructions_mut().unwrap();
        instructions
            .insert(
                0,
                Instruction::ConstantWide(Opcode::Invokestatic, method_ref),
            )
            .unwrap();
        instructions.apply().unwrap();
    }

    let rewritten = class.to_bytes().expect("write");
    let reparsed = ClassFile::parse(&rewritten).expect("reparse");
    let loaded = reparsed
        .find_method("dense", "(I)I")
        .unwrap()
        .code()
        .unwrap()
        .instructions()
        .unwrap();

    assert_eq!(loaded[0].opcode(), Opcode::Invokestatic);
    assert_eq!(loaded[1].opcode(), Opcode::Iload1);
    let default_after = match &loaded[2] {
        Instruction::TableSwitch { default, .. } => *default,
        other => panic!("expected a tableswitch, got {:?}", other.opcode()),
    };
    assert_eq!(
        default_after,
        default_before + 1,
        "the switch default must be rebased for the new padding"
    );
    assert_eq!(
        switch_target_opcodes(&loaded, 2),
        cases_before,
        "each case must still reach the same body opcode"
    );
}

#[test]
fn head_insertion_keeps_plain_branch_offsets() {
    let bytes = fixture("ControlFlow.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");
    let method_ref = intern_hook(&mut class);

    let before = branch_offsets(&class, "loops", "(I)I");

    {
        let code = class
            .find_method_mut("loops", "(I)I")
            .unwrap()
            .code_mut()
            .unwrap();
        let mut instructions = code.instructions_mut().unwrap();
        instructions
            .insert(
                0,
                Instruction::ConstantWide(Opcode::Invokestatic, method_ref),
            )
            .unwrap();
        instructions.apply().unwrap();
    }

    let rewritten = class.to_bytes().expect("write");
    let reparsed = ClassFile::parse(&rewritten).expect("reparse");
    // A uniform shift leaves every relative branch offset unchanged.
    assert_eq!(branch_offsets(&reparsed, "loops", "(I)I"), before);
}

#[test]
fn renames_a_method_by_interning_a_new_utf8() {
    let bytes = fixture("Simple.class");
    let mut class = ClassFile::parse(&bytes).expect("parse");

    let renamed = class.constant_pool.intern_utf8("addTwo").expect("intern");

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
    let index = class
        .constant_pool
        .intern_utf8("unused-but-valid")
        .expect("intern");
    // `into_raw` is `unsafe`: we promise the index belongs to `class`.
    let raw = unsafe { index.into_raw() };
    assert_eq!(raw as usize, before); // first free slot

    let rewritten = class.to_bytes().expect("write");
    let reparsed = ClassFile::parse(&rewritten).expect("reparse");
    // A reference from the parsed file is scoped to *it*, so fetch it there.
    let found = reparsed
        .constant_pool
        .find(&Constant::Utf8(
            Mutf8::try_from("unused-but-valid").unwrap(),
        ))
        .expect("interned constant present");
    assert_eq!(
        reparsed.constant_pool.utf8(found).unwrap(),
        "unused-but-valid"
    );
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
        .intern_utf8("completely-unused")
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

    let first = class.constant_pool.intern_utf8("dup").expect("intern");
    let second = class.constant_pool.intern_utf8("dup").expect("intern");
    assert_eq!(first, second);
    assert_eq!(
        class.constant_pool.get(first),
        Some(&Constant::Utf8(Mutf8::try_from("dup").unwrap()))
    );
}
