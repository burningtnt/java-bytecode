//! Shared helpers for the integration tests.
//!
//! This module is included by several test crates; unused items are expected.

#![allow(dead_code)]
// `bytecode::decode`/`encode` are `unsafe`: they cannot check constant-pool
// indices, but here the instructions come from, and go back to, the same class.
#![allow(unsafe_code)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use java_bytecode::bytecode::{Instruction, decode, encode};
use java_bytecode::{Attribute, AttributeBody, ClassFile, CodeAttribute};

/// Recursively collects `.class` files under `dir`.
pub fn collect_class_files(dir: &Path, out: &mut Vec<PathBuf>) {
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
    out.sort();
}

/// Collects every `Code` attribute reachable from `class`.
pub fn collect_code<'a>(class: &'a ClassFile<'a>, out: &mut Vec<&'a CodeAttribute<'a>>) {
    collect_from_attributes(&class.attributes, out);
    for field in &class.fields {
        collect_from_attributes(&field.attributes, out);
    }
    for method in &class.methods {
        collect_from_attributes(&method.attributes, out);
    }
}

fn collect_from_attributes<'a>(
    attributes: &'a [Attribute<'a>],
    out: &mut Vec<&'a CodeAttribute<'a>>,
) {
    for attribute in attributes {
        match &attribute.body {
            AttributeBody::Code(code) => {
                out.push(code);
                collect_from_attributes(&code.attributes, out);
            }
            AttributeBody::Record(record) => {
                for component in &record.components {
                    collect_from_attributes(&component.attributes, out);
                }
            }
            _ => {}
        }
    }
}

/// Runs every encode/decode consistency check on one `Code` byte array.
///
/// Verifies, in order:
///
/// 1. `decode` succeeds;
/// 2. `encoded_len` accumulation reproduces `code.len()`;
/// 3. `encode(decode(code)) == code`;
/// 4. re-decoding the re-encoded bytes yields the same instructions;
/// 5. every branch/switch target is an instruction boundary.
pub fn check_code(code: &[u8]) -> Result<(), String> {
    let instructions = unsafe { decode(code) }.map_err(|err| format!("decode failed: {err}"))?;

    // Instruction start offsets, derived from the encoded lengths.
    let mut starts = Vec::with_capacity(instructions.len() + 1);
    let mut pc = 0usize;
    for instruction in &instructions {
        starts.push(pc);
        pc += instruction.encoded_len(pc);
    }
    if pc != code.len() {
        return Err(format!(
            "encoded_len sum {pc} != code length {}",
            code.len()
        ));
    }
    starts.push(pc);

    let reencoded =
        unsafe { encode(&instructions) }.map_err(|err| format!("encode failed: {err}"))?;
    if reencoded != code {
        return Err("encode(decode(code)) != code".to_owned());
    }

    // Re-decode the re-encoded bytes and confirm the codec is a fixed point.
    // (Instruction values carry their constant-pool scope and cannot be compared
    // across decodes, so compare the re-encoded bytes instead.)
    let again = unsafe { decode(&reencoded) }.map_err(|err| format!("re-decode failed: {err}"))?;
    let reencoded_again =
        unsafe { encode(&again) }.map_err(|err| format!("re-encode failed: {err}"))?;
    if reencoded_again != reencoded {
        return Err("decode(encode(decode(code))) re-encodes differently".to_owned());
    }

    let boundaries: BTreeSet<usize> = starts.iter().copied().collect();
    for (index, instruction) in instructions.iter().enumerate() {
        let at = starts[index] as i64;
        let targets: Vec<i64> = match instruction {
            Instruction::Branch(_, offset) => vec![at + i64::from(*offset)],
            Instruction::BranchWide(_, offset) => vec![at + i64::from(*offset)],
            Instruction::TableSwitch {
                default, offsets, ..
            } => {
                let mut targets = vec![at + i64::from(*default)];
                targets.extend(offsets.iter().map(|offset| at + i64::from(*offset)));
                targets
            }
            Instruction::LookupSwitch { default, pairs, .. } => {
                let mut targets = vec![at + i64::from(*default)];
                targets.extend(pairs.iter().map(|(_, offset)| at + i64::from(*offset)));
                targets
            }
            _ => Vec::new(),
        };
        for target in targets {
            if target < 0 || !boundaries.contains(&(target as usize)) {
                return Err(format!(
                    "branch target {target} at pc {at} is not an instruction boundary"
                ));
            }
        }
    }

    Ok(())
}
