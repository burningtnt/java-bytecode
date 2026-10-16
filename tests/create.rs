//! Building a class file from scratch, without a template.
//!
//! The crate has no dedicated builder: fields are public, so a `ClassFile` is
//! assembled directly. The one subtlety is `CodeAttribute`, whose bytecode field
//! is private: build it from an instruction list with
//! [`CodeAttribute::from_instructions`] and attach it with
//! [`CodeAttribute::into_attribute`].

use java_bytecode::{
    ClassAccessFlags, ClassFile, CodeAttribute, Constant, ConstantPool, Instruction,
    MethodAccessFlags, MethodInfo, Opcode,
};

const SEND_DESCRIPTOR: &str = "(Ljava/nio/ByteBuffer;Ljava/net/SocketAddress;)I";

fn build_sender() -> Result<Vec<u8>, java_bytecode::Error> {
    let mut pool = ConstantPool::new();

    let this_name = pool.intern_utf8("example/Sender")?;
    let this_class = pool.intern(Constant::Class {
        name_index: this_name,
    })?;
    let super_name = pool.intern_utf8("java/lang/Object")?;
    let super_class = pool.intern(Constant::Class {
        name_index: super_name,
    })?;

    let send_name = pool.intern_utf8("send")?;
    let send_descriptor = pool.intern_utf8(SEND_DESCRIPTOR)?;
    let code_name = pool.intern_utf8("Code")?;

    // Assemble the `Code` body as instructions: `iconst_0; ireturn`, with
    // `max_stack = 1` and `max_locals = 3` (`this` + two arguments).
    let code = CodeAttribute::from_instructions(
        1,
        3,
        [
            Instruction::None(Opcode::Iconst0),
            Instruction::None(Opcode::Ireturn),
        ],
    )?;
    let code_attribute = code.into_attribute(code_name);

    let send = MethodInfo {
        access_flags: MethodAccessFlags::PUBLIC,
        name_index: send_name,
        descriptor_index: send_descriptor,
        attributes: vec![code_attribute],
    };

    let class = ClassFile {
        minor_version: 0,
        major_version: 52,
        constant_pool: pool,
        access_flags: ClassAccessFlags::PUBLIC | ClassAccessFlags::SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![send],
        attributes: Vec::new(),
    };

    class.to_bytes()
}

#[test]
fn creates_a_class_with_a_send_method() {
    let bytes = build_sender().expect("build class");

    let class = ClassFile::parse(&bytes).expect("parse freshly built class");
    assert_eq!(class.this_class_name().unwrap(), "example/Sender");
    assert_eq!(
        class.super_class_name().unwrap().as_deref(),
        Some("java/lang/Object")
    );
    assert!(!class.constant_pool.is_empty());

    let method = class
        .find_method("send", SEND_DESCRIPTOR)
        .expect("send method present");
    assert_eq!(method.access_flags, MethodAccessFlags::PUBLIC);
    assert_eq!(method.name(&class.constant_pool).unwrap(), "send");
    assert_eq!(
        method.descriptor(&class.constant_pool).unwrap(),
        SEND_DESCRIPTOR
    );

    let code = method.code().expect("send has a Code attribute");
    assert_eq!(code.max_locals, 3);

    let instructions = code.instructions().expect("decode code");
    let opcodes: Vec<_> = instructions.iter().map(|i| i.opcode()).collect();
    assert_eq!(opcodes, [Opcode::Iconst0, Opcode::Ireturn]);
}

#[test]
fn created_class_round_trips_and_rebuilds() {
    let bytes = build_sender().expect("build class");

    let class = ClassFile::parse(&bytes).expect("parse");
    assert_eq!(class.to_bytes().unwrap(), bytes, "index-stable round-trip");

    let rebuilt = class.to_bytes_rebuilt().expect("rebuild");
    let reparsed = ClassFile::parse(&rebuilt).expect("parse rebuilt");
    assert!(
        reparsed.find_method("send", SEND_DESCRIPTOR).is_some(),
        "rebuilt class still declares send"
    );
}
