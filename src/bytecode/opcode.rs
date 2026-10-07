//! JVM instruction opcodes (JVMS §6.5).
//!
//! The complete opcode table (including the reserved `breakpoint`, `impdep1`
//! and `impdep2` opcodes), together with each opcode's operand shape
//! ([`OperandKind`]).

/// The shape of an instruction's operands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperandKind {
    /// No operands.
    None,
    /// A signed 8-bit immediate (`bipush`).
    Byte,
    /// A signed 16-bit immediate (`sipush`).
    Short,
    /// A one-byte local variable index.
    Local,
    /// A one-byte constant pool index (`ldc`).
    Ldc,
    /// A two-byte constant pool index.
    ConstantPool,
    /// A signed 16-bit relative branch offset.
    Branch,
    /// A signed 32-bit relative branch offset (`goto_w`, `jsr_w`).
    BranchWide,
    /// `iinc`: one-byte index and signed 8-bit amount.
    Iinc,
    /// `invokeinterface`: two-byte index plus a count byte.
    InvokeInterface,
    /// `invokedynamic`: two-byte index plus two reserved zero bytes.
    InvokeDynamic,
    /// `newarray`: the array type byte.
    NewArray,
    /// `multianewarray`: two-byte index plus a dimensions byte.
    MultiANewArray,
    /// `tableswitch`.
    TableSwitch,
    /// `lookupswitch`.
    LookupSwitch,
    /// The `wide` prefix.
    Wide,
}

/// A JVM opcode.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Opcode {
    Nop = 0x00,
    AconstNull = 0x01,
    IconstM1 = 0x02,
    Iconst0 = 0x03,
    Iconst1 = 0x04,
    Iconst2 = 0x05,
    Iconst3 = 0x06,
    Iconst4 = 0x07,
    Iconst5 = 0x08,
    Lconst0 = 0x09,
    Lconst1 = 0x0A,
    Fconst0 = 0x0B,
    Fconst1 = 0x0C,
    Fconst2 = 0x0D,
    Dconst0 = 0x0E,
    Dconst1 = 0x0F,
    Bipush = 0x10,
    Sipush = 0x11,
    Ldc = 0x12,
    LdcW = 0x13,
    Ldc2W = 0x14,
    Iload = 0x15,
    Lload = 0x16,
    Fload = 0x17,
    Dload = 0x18,
    Aload = 0x19,
    Iload0 = 0x1A,
    Iload1 = 0x1B,
    Iload2 = 0x1C,
    Iload3 = 0x1D,
    Lload0 = 0x1E,
    Lload1 = 0x1F,
    Lload2 = 0x20,
    Lload3 = 0x21,
    Fload0 = 0x22,
    Fload1 = 0x23,
    Fload2 = 0x24,
    Fload3 = 0x25,
    Dload0 = 0x26,
    Dload1 = 0x27,
    Dload2 = 0x28,
    Dload3 = 0x29,
    Aload0 = 0x2A,
    Aload1 = 0x2B,
    Aload2 = 0x2C,
    Aload3 = 0x2D,
    Iaload = 0x2E,
    Laload = 0x2F,
    Faload = 0x30,
    Daload = 0x31,
    Aaload = 0x32,
    Baload = 0x33,
    Caload = 0x34,
    Saload = 0x35,
    Istore = 0x36,
    Lstore = 0x37,
    Fstore = 0x38,
    Dstore = 0x39,
    Astore = 0x3A,
    Istore0 = 0x3B,
    Istore1 = 0x3C,
    Istore2 = 0x3D,
    Istore3 = 0x3E,
    Lstore0 = 0x3F,
    Lstore1 = 0x40,
    Lstore2 = 0x41,
    Lstore3 = 0x42,
    Fstore0 = 0x43,
    Fstore1 = 0x44,
    Fstore2 = 0x45,
    Fstore3 = 0x46,
    Dstore0 = 0x47,
    Dstore1 = 0x48,
    Dstore2 = 0x49,
    Dstore3 = 0x4A,
    Astore0 = 0x4B,
    Astore1 = 0x4C,
    Astore2 = 0x4D,
    Astore3 = 0x4E,
    Iastore = 0x4F,
    Lastore = 0x50,
    Fastore = 0x51,
    Dastore = 0x52,
    Aastore = 0x53,
    Bastore = 0x54,
    Castore = 0x55,
    Sastore = 0x56,
    Pop = 0x57,
    Pop2 = 0x58,
    Dup = 0x59,
    DupX1 = 0x5A,
    DupX2 = 0x5B,
    Dup2 = 0x5C,
    Dup2X1 = 0x5D,
    Dup2X2 = 0x5E,
    Swap = 0x5F,
    Iadd = 0x60,
    Ladd = 0x61,
    Fadd = 0x62,
    Dadd = 0x63,
    Isub = 0x64,
    Lsub = 0x65,
    Fsub = 0x66,
    Dsub = 0x67,
    Imul = 0x68,
    Lmul = 0x69,
    Fmul = 0x6A,
    Dmul = 0x6B,
    Idiv = 0x6C,
    Ldiv = 0x6D,
    Fdiv = 0x6E,
    Ddiv = 0x6F,
    Irem = 0x70,
    Lrem = 0x71,
    Frem = 0x72,
    Drem = 0x73,
    Ineg = 0x74,
    Lneg = 0x75,
    Fneg = 0x76,
    Dneg = 0x77,
    Ishl = 0x78,
    Lshl = 0x79,
    Ishr = 0x7A,
    Lshr = 0x7B,
    Iushr = 0x7C,
    Lushr = 0x7D,
    Iand = 0x7E,
    Land = 0x7F,
    Ior = 0x80,
    Lor = 0x81,
    Ixor = 0x82,
    Lxor = 0x83,
    Iinc = 0x84,
    I2l = 0x85,
    I2f = 0x86,
    I2d = 0x87,
    L2i = 0x88,
    L2f = 0x89,
    L2d = 0x8A,
    F2i = 0x8B,
    F2l = 0x8C,
    F2d = 0x8D,
    D2i = 0x8E,
    D2l = 0x8F,
    D2f = 0x90,
    I2b = 0x91,
    I2c = 0x92,
    I2s = 0x93,
    Lcmp = 0x94,
    Fcmpl = 0x95,
    Fcmpg = 0x96,
    Dcmpl = 0x97,
    Dcmpg = 0x98,
    Ifeq = 0x99,
    Ifne = 0x9A,
    Iflt = 0x9B,
    Ifge = 0x9C,
    Ifgt = 0x9D,
    Ifle = 0x9E,
    IfIcmpeq = 0x9F,
    IfIcmpne = 0xA0,
    IfIcmplt = 0xA1,
    IfIcmpge = 0xA2,
    IfIcmpgt = 0xA3,
    IfIcmple = 0xA4,
    IfAcmpeq = 0xA5,
    IfAcmpne = 0xA6,
    Goto = 0xA7,
    Jsr = 0xA8,
    Ret = 0xA9,
    TableSwitch = 0xAA,
    LookupSwitch = 0xAB,
    Ireturn = 0xAC,
    Lreturn = 0xAD,
    Freturn = 0xAE,
    Dreturn = 0xAF,
    Areturn = 0xB0,
    Return = 0xB1,
    Getstatic = 0xB2,
    Putstatic = 0xB3,
    Getfield = 0xB4,
    Putfield = 0xB5,
    Invokevirtual = 0xB6,
    Invokespecial = 0xB7,
    Invokestatic = 0xB8,
    Invokeinterface = 0xB9,
    Invokedynamic = 0xBA,
    New = 0xBB,
    Newarray = 0xBC,
    Anewarray = 0xBD,
    Arraylength = 0xBE,
    Athrow = 0xBF,
    Checkcast = 0xC0,
    Instanceof = 0xC1,
    Monitorenter = 0xC2,
    Monitorexit = 0xC3,
    Wide = 0xC4,
    Multianewarray = 0xC5,
    Ifnull = 0xC6,
    Ifnonnull = 0xC7,
    GotoW = 0xC8,
    JsrW = 0xC9,
    Breakpoint = 0xCA,
    Impdep1 = 0xFE,
    Impdep2 = 0xFF,
}

impl Opcode {
    /// Returns the opcode byte.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Maps a byte to its opcode, or `None` if undefined/reserved.
    #[must_use]
    pub const fn from_u8(byte: u8) -> Option<Self> {
        Some(match byte {
            0x00 => Self::Nop,
            0x01 => Self::AconstNull,
            0x02 => Self::IconstM1,
            0x03 => Self::Iconst0,
            0x04 => Self::Iconst1,
            0x05 => Self::Iconst2,
            0x06 => Self::Iconst3,
            0x07 => Self::Iconst4,
            0x08 => Self::Iconst5,
            0x09 => Self::Lconst0,
            0x0A => Self::Lconst1,
            0x0B => Self::Fconst0,
            0x0C => Self::Fconst1,
            0x0D => Self::Fconst2,
            0x0E => Self::Dconst0,
            0x0F => Self::Dconst1,
            0x10 => Self::Bipush,
            0x11 => Self::Sipush,
            0x12 => Self::Ldc,
            0x13 => Self::LdcW,
            0x14 => Self::Ldc2W,
            0x15 => Self::Iload,
            0x16 => Self::Lload,
            0x17 => Self::Fload,
            0x18 => Self::Dload,
            0x19 => Self::Aload,
            0x1A => Self::Iload0,
            0x1B => Self::Iload1,
            0x1C => Self::Iload2,
            0x1D => Self::Iload3,
            0x1E => Self::Lload0,
            0x1F => Self::Lload1,
            0x20 => Self::Lload2,
            0x21 => Self::Lload3,
            0x22 => Self::Fload0,
            0x23 => Self::Fload1,
            0x24 => Self::Fload2,
            0x25 => Self::Fload3,
            0x26 => Self::Dload0,
            0x27 => Self::Dload1,
            0x28 => Self::Dload2,
            0x29 => Self::Dload3,
            0x2A => Self::Aload0,
            0x2B => Self::Aload1,
            0x2C => Self::Aload2,
            0x2D => Self::Aload3,
            0x2E => Self::Iaload,
            0x2F => Self::Laload,
            0x30 => Self::Faload,
            0x31 => Self::Daload,
            0x32 => Self::Aaload,
            0x33 => Self::Baload,
            0x34 => Self::Caload,
            0x35 => Self::Saload,
            0x36 => Self::Istore,
            0x37 => Self::Lstore,
            0x38 => Self::Fstore,
            0x39 => Self::Dstore,
            0x3A => Self::Astore,
            0x3B => Self::Istore0,
            0x3C => Self::Istore1,
            0x3D => Self::Istore2,
            0x3E => Self::Istore3,
            0x3F => Self::Lstore0,
            0x40 => Self::Lstore1,
            0x41 => Self::Lstore2,
            0x42 => Self::Lstore3,
            0x43 => Self::Fstore0,
            0x44 => Self::Fstore1,
            0x45 => Self::Fstore2,
            0x46 => Self::Fstore3,
            0x47 => Self::Dstore0,
            0x48 => Self::Dstore1,
            0x49 => Self::Dstore2,
            0x4A => Self::Dstore3,
            0x4B => Self::Astore0,
            0x4C => Self::Astore1,
            0x4D => Self::Astore2,
            0x4E => Self::Astore3,
            0x4F => Self::Iastore,
            0x50 => Self::Lastore,
            0x51 => Self::Fastore,
            0x52 => Self::Dastore,
            0x53 => Self::Aastore,
            0x54 => Self::Bastore,
            0x55 => Self::Castore,
            0x56 => Self::Sastore,
            0x57 => Self::Pop,
            0x58 => Self::Pop2,
            0x59 => Self::Dup,
            0x5A => Self::DupX1,
            0x5B => Self::DupX2,
            0x5C => Self::Dup2,
            0x5D => Self::Dup2X1,
            0x5E => Self::Dup2X2,
            0x5F => Self::Swap,
            0x60 => Self::Iadd,
            0x61 => Self::Ladd,
            0x62 => Self::Fadd,
            0x63 => Self::Dadd,
            0x64 => Self::Isub,
            0x65 => Self::Lsub,
            0x66 => Self::Fsub,
            0x67 => Self::Dsub,
            0x68 => Self::Imul,
            0x69 => Self::Lmul,
            0x6A => Self::Fmul,
            0x6B => Self::Dmul,
            0x6C => Self::Idiv,
            0x6D => Self::Ldiv,
            0x6E => Self::Fdiv,
            0x6F => Self::Ddiv,
            0x70 => Self::Irem,
            0x71 => Self::Lrem,
            0x72 => Self::Frem,
            0x73 => Self::Drem,
            0x74 => Self::Ineg,
            0x75 => Self::Lneg,
            0x76 => Self::Fneg,
            0x77 => Self::Dneg,
            0x78 => Self::Ishl,
            0x79 => Self::Lshl,
            0x7A => Self::Ishr,
            0x7B => Self::Lshr,
            0x7C => Self::Iushr,
            0x7D => Self::Lushr,
            0x7E => Self::Iand,
            0x7F => Self::Land,
            0x80 => Self::Ior,
            0x81 => Self::Lor,
            0x82 => Self::Ixor,
            0x83 => Self::Lxor,
            0x84 => Self::Iinc,
            0x85 => Self::I2l,
            0x86 => Self::I2f,
            0x87 => Self::I2d,
            0x88 => Self::L2i,
            0x89 => Self::L2f,
            0x8A => Self::L2d,
            0x8B => Self::F2i,
            0x8C => Self::F2l,
            0x8D => Self::F2d,
            0x8E => Self::D2i,
            0x8F => Self::D2l,
            0x90 => Self::D2f,
            0x91 => Self::I2b,
            0x92 => Self::I2c,
            0x93 => Self::I2s,
            0x94 => Self::Lcmp,
            0x95 => Self::Fcmpl,
            0x96 => Self::Fcmpg,
            0x97 => Self::Dcmpl,
            0x98 => Self::Dcmpg,
            0x99 => Self::Ifeq,
            0x9A => Self::Ifne,
            0x9B => Self::Iflt,
            0x9C => Self::Ifge,
            0x9D => Self::Ifgt,
            0x9E => Self::Ifle,
            0x9F => Self::IfIcmpeq,
            0xA0 => Self::IfIcmpne,
            0xA1 => Self::IfIcmplt,
            0xA2 => Self::IfIcmpge,
            0xA3 => Self::IfIcmpgt,
            0xA4 => Self::IfIcmple,
            0xA5 => Self::IfAcmpeq,
            0xA6 => Self::IfAcmpne,
            0xA7 => Self::Goto,
            0xA8 => Self::Jsr,
            0xA9 => Self::Ret,
            0xAA => Self::TableSwitch,
            0xAB => Self::LookupSwitch,
            0xAC => Self::Ireturn,
            0xAD => Self::Lreturn,
            0xAE => Self::Freturn,
            0xAF => Self::Dreturn,
            0xB0 => Self::Areturn,
            0xB1 => Self::Return,
            0xB2 => Self::Getstatic,
            0xB3 => Self::Putstatic,
            0xB4 => Self::Getfield,
            0xB5 => Self::Putfield,
            0xB6 => Self::Invokevirtual,
            0xB7 => Self::Invokespecial,
            0xB8 => Self::Invokestatic,
            0xB9 => Self::Invokeinterface,
            0xBA => Self::Invokedynamic,
            0xBB => Self::New,
            0xBC => Self::Newarray,
            0xBD => Self::Anewarray,
            0xBE => Self::Arraylength,
            0xBF => Self::Athrow,
            0xC0 => Self::Checkcast,
            0xC1 => Self::Instanceof,
            0xC2 => Self::Monitorenter,
            0xC3 => Self::Monitorexit,
            0xC4 => Self::Wide,
            0xC5 => Self::Multianewarray,
            0xC6 => Self::Ifnull,
            0xC7 => Self::Ifnonnull,
            0xC8 => Self::GotoW,
            0xC9 => Self::JsrW,
            0xCA => Self::Breakpoint,
            0xFE => Self::Impdep1,
            0xFF => Self::Impdep2,
            _ => return None,
        })
    }

    /// Returns the JVMS mnemonic.
    #[must_use]
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Nop => "nop",
            Self::AconstNull => "aconst_null",
            Self::IconstM1 => "iconst_m1",
            Self::Iconst0 => "iconst_0",
            Self::Iconst1 => "iconst_1",
            Self::Iconst2 => "iconst_2",
            Self::Iconst3 => "iconst_3",
            Self::Iconst4 => "iconst_4",
            Self::Iconst5 => "iconst_5",
            Self::Lconst0 => "lconst_0",
            Self::Lconst1 => "lconst_1",
            Self::Fconst0 => "fconst_0",
            Self::Fconst1 => "fconst_1",
            Self::Fconst2 => "fconst_2",
            Self::Dconst0 => "dconst_0",
            Self::Dconst1 => "dconst_1",
            Self::Bipush => "bipush",
            Self::Sipush => "sipush",
            Self::Ldc => "ldc",
            Self::LdcW => "ldc_w",
            Self::Ldc2W => "ldc2_w",
            Self::Iload => "iload",
            Self::Lload => "lload",
            Self::Fload => "fload",
            Self::Dload => "dload",
            Self::Aload => "aload",
            Self::Iload0 => "iload_0",
            Self::Iload1 => "iload_1",
            Self::Iload2 => "iload_2",
            Self::Iload3 => "iload_3",
            Self::Lload0 => "lload_0",
            Self::Lload1 => "lload_1",
            Self::Lload2 => "lload_2",
            Self::Lload3 => "lload_3",
            Self::Fload0 => "fload_0",
            Self::Fload1 => "fload_1",
            Self::Fload2 => "fload_2",
            Self::Fload3 => "fload_3",
            Self::Dload0 => "dload_0",
            Self::Dload1 => "dload_1",
            Self::Dload2 => "dload_2",
            Self::Dload3 => "dload_3",
            Self::Aload0 => "aload_0",
            Self::Aload1 => "aload_1",
            Self::Aload2 => "aload_2",
            Self::Aload3 => "aload_3",
            Self::Iaload => "iaload",
            Self::Laload => "laload",
            Self::Faload => "faload",
            Self::Daload => "daload",
            Self::Aaload => "aaload",
            Self::Baload => "baload",
            Self::Caload => "caload",
            Self::Saload => "saload",
            Self::Istore => "istore",
            Self::Lstore => "lstore",
            Self::Fstore => "fstore",
            Self::Dstore => "dstore",
            Self::Astore => "astore",
            Self::Istore0 => "istore_0",
            Self::Istore1 => "istore_1",
            Self::Istore2 => "istore_2",
            Self::Istore3 => "istore_3",
            Self::Lstore0 => "lstore_0",
            Self::Lstore1 => "lstore_1",
            Self::Lstore2 => "lstore_2",
            Self::Lstore3 => "lstore_3",
            Self::Fstore0 => "fstore_0",
            Self::Fstore1 => "fstore_1",
            Self::Fstore2 => "fstore_2",
            Self::Fstore3 => "fstore_3",
            Self::Dstore0 => "dstore_0",
            Self::Dstore1 => "dstore_1",
            Self::Dstore2 => "dstore_2",
            Self::Dstore3 => "dstore_3",
            Self::Astore0 => "astore_0",
            Self::Astore1 => "astore_1",
            Self::Astore2 => "astore_2",
            Self::Astore3 => "astore_3",
            Self::Iastore => "iastore",
            Self::Lastore => "lastore",
            Self::Fastore => "fastore",
            Self::Dastore => "dastore",
            Self::Aastore => "aastore",
            Self::Bastore => "bastore",
            Self::Castore => "castore",
            Self::Sastore => "sastore",
            Self::Pop => "pop",
            Self::Pop2 => "pop2",
            Self::Dup => "dup",
            Self::DupX1 => "dup_x1",
            Self::DupX2 => "dup_x2",
            Self::Dup2 => "dup2",
            Self::Dup2X1 => "dup2_x1",
            Self::Dup2X2 => "dup2_x2",
            Self::Swap => "swap",
            Self::Iadd => "iadd",
            Self::Ladd => "ladd",
            Self::Fadd => "fadd",
            Self::Dadd => "dadd",
            Self::Isub => "isub",
            Self::Lsub => "lsub",
            Self::Fsub => "fsub",
            Self::Dsub => "dsub",
            Self::Imul => "imul",
            Self::Lmul => "lmul",
            Self::Fmul => "fmul",
            Self::Dmul => "dmul",
            Self::Idiv => "idiv",
            Self::Ldiv => "ldiv",
            Self::Fdiv => "fdiv",
            Self::Ddiv => "ddiv",
            Self::Irem => "irem",
            Self::Lrem => "lrem",
            Self::Frem => "frem",
            Self::Drem => "drem",
            Self::Ineg => "ineg",
            Self::Lneg => "lneg",
            Self::Fneg => "fneg",
            Self::Dneg => "dneg",
            Self::Ishl => "ishl",
            Self::Lshl => "lshl",
            Self::Ishr => "ishr",
            Self::Lshr => "lshr",
            Self::Iushr => "iushr",
            Self::Lushr => "lushr",
            Self::Iand => "iand",
            Self::Land => "land",
            Self::Ior => "ior",
            Self::Lor => "lor",
            Self::Ixor => "ixor",
            Self::Lxor => "lxor",
            Self::Iinc => "iinc",
            Self::I2l => "i2l",
            Self::I2f => "i2f",
            Self::I2d => "i2d",
            Self::L2i => "l2i",
            Self::L2f => "l2f",
            Self::L2d => "l2d",
            Self::F2i => "f2i",
            Self::F2l => "f2l",
            Self::F2d => "f2d",
            Self::D2i => "d2i",
            Self::D2l => "d2l",
            Self::D2f => "d2f",
            Self::I2b => "i2b",
            Self::I2c => "i2c",
            Self::I2s => "i2s",
            Self::Lcmp => "lcmp",
            Self::Fcmpl => "fcmpl",
            Self::Fcmpg => "fcmpg",
            Self::Dcmpl => "dcmpl",
            Self::Dcmpg => "dcmpg",
            Self::Ifeq => "ifeq",
            Self::Ifne => "ifne",
            Self::Iflt => "iflt",
            Self::Ifge => "ifge",
            Self::Ifgt => "ifgt",
            Self::Ifle => "ifle",
            Self::IfIcmpeq => "if_icmpeq",
            Self::IfIcmpne => "if_icmpne",
            Self::IfIcmplt => "if_icmplt",
            Self::IfIcmpge => "if_icmpge",
            Self::IfIcmpgt => "if_icmpgt",
            Self::IfIcmple => "if_icmple",
            Self::IfAcmpeq => "if_acmpeq",
            Self::IfAcmpne => "if_acmpne",
            Self::Goto => "goto",
            Self::Jsr => "jsr",
            Self::Ret => "ret",
            Self::TableSwitch => "tableswitch",
            Self::LookupSwitch => "lookupswitch",
            Self::Ireturn => "ireturn",
            Self::Lreturn => "lreturn",
            Self::Freturn => "freturn",
            Self::Dreturn => "dreturn",
            Self::Areturn => "areturn",
            Self::Return => "return",
            Self::Getstatic => "getstatic",
            Self::Putstatic => "putstatic",
            Self::Getfield => "getfield",
            Self::Putfield => "putfield",
            Self::Invokevirtual => "invokevirtual",
            Self::Invokespecial => "invokespecial",
            Self::Invokestatic => "invokestatic",
            Self::Invokeinterface => "invokeinterface",
            Self::Invokedynamic => "invokedynamic",
            Self::New => "new",
            Self::Newarray => "newarray",
            Self::Anewarray => "anewarray",
            Self::Arraylength => "arraylength",
            Self::Athrow => "athrow",
            Self::Checkcast => "checkcast",
            Self::Instanceof => "instanceof",
            Self::Monitorenter => "monitorenter",
            Self::Monitorexit => "monitorexit",
            Self::Wide => "wide",
            Self::Multianewarray => "multianewarray",
            Self::Ifnull => "ifnull",
            Self::Ifnonnull => "ifnonnull",
            Self::GotoW => "goto_w",
            Self::JsrW => "jsr_w",
            Self::Breakpoint => "breakpoint",
            Self::Impdep1 => "impdep1",
            Self::Impdep2 => "impdep2",
        }
    }

    /// Returns the operand shape.
    #[must_use]
    pub const fn operand_kind(self) -> OperandKind {
        match self {
            Self::Bipush => OperandKind::Byte,
            Self::Sipush => OperandKind::Short,
            Self::Iload
            | Self::Lload
            | Self::Fload
            | Self::Dload
            | Self::Aload
            | Self::Istore
            | Self::Lstore
            | Self::Fstore
            | Self::Dstore
            | Self::Astore
            | Self::Ret => OperandKind::Local,
            Self::Ldc => OperandKind::Ldc,
            Self::LdcW
            | Self::Ldc2W
            | Self::Getstatic
            | Self::Putstatic
            | Self::Getfield
            | Self::Putfield
            | Self::Invokevirtual
            | Self::Invokespecial
            | Self::Invokestatic
            | Self::New
            | Self::Anewarray
            | Self::Checkcast
            | Self::Instanceof => OperandKind::ConstantPool,
            Self::Ifeq
            | Self::Ifne
            | Self::Iflt
            | Self::Ifge
            | Self::Ifgt
            | Self::Ifle
            | Self::IfIcmpeq
            | Self::IfIcmpne
            | Self::IfIcmplt
            | Self::IfIcmpge
            | Self::IfIcmpgt
            | Self::IfIcmple
            | Self::IfAcmpeq
            | Self::IfAcmpne
            | Self::Goto
            | Self::Jsr
            | Self::Ifnull
            | Self::Ifnonnull => OperandKind::Branch,
            Self::GotoW | Self::JsrW => OperandKind::BranchWide,
            Self::Newarray => OperandKind::NewArray,
            Self::Iinc => OperandKind::Iinc,
            Self::Invokeinterface => OperandKind::InvokeInterface,
            Self::Invokedynamic => OperandKind::InvokeDynamic,
            Self::Multianewarray => OperandKind::MultiANewArray,
            Self::TableSwitch => OperandKind::TableSwitch,
            Self::LookupSwitch => OperandKind::LookupSwitch,
            Self::Wide => OperandKind::Wide,
            Self::Nop
            | Self::AconstNull
            | Self::IconstM1
            | Self::Iconst0
            | Self::Iconst1
            | Self::Iconst2
            | Self::Iconst3
            | Self::Iconst4
            | Self::Iconst5
            | Self::Lconst0
            | Self::Lconst1
            | Self::Fconst0
            | Self::Fconst1
            | Self::Fconst2
            | Self::Dconst0
            | Self::Dconst1
            | Self::Iload0
            | Self::Iload1
            | Self::Iload2
            | Self::Iload3
            | Self::Lload0
            | Self::Lload1
            | Self::Lload2
            | Self::Lload3
            | Self::Fload0
            | Self::Fload1
            | Self::Fload2
            | Self::Fload3
            | Self::Dload0
            | Self::Dload1
            | Self::Dload2
            | Self::Dload3
            | Self::Aload0
            | Self::Aload1
            | Self::Aload2
            | Self::Aload3
            | Self::Iaload
            | Self::Laload
            | Self::Faload
            | Self::Daload
            | Self::Aaload
            | Self::Baload
            | Self::Caload
            | Self::Saload
            | Self::Istore0
            | Self::Istore1
            | Self::Istore2
            | Self::Istore3
            | Self::Lstore0
            | Self::Lstore1
            | Self::Lstore2
            | Self::Lstore3
            | Self::Fstore0
            | Self::Fstore1
            | Self::Fstore2
            | Self::Fstore3
            | Self::Dstore0
            | Self::Dstore1
            | Self::Dstore2
            | Self::Dstore3
            | Self::Astore0
            | Self::Astore1
            | Self::Astore2
            | Self::Astore3
            | Self::Iastore
            | Self::Lastore
            | Self::Fastore
            | Self::Dastore
            | Self::Aastore
            | Self::Bastore
            | Self::Castore
            | Self::Sastore
            | Self::Pop
            | Self::Pop2
            | Self::Dup
            | Self::DupX1
            | Self::DupX2
            | Self::Dup2
            | Self::Dup2X1
            | Self::Dup2X2
            | Self::Swap
            | Self::Iadd
            | Self::Ladd
            | Self::Fadd
            | Self::Dadd
            | Self::Isub
            | Self::Lsub
            | Self::Fsub
            | Self::Dsub
            | Self::Imul
            | Self::Lmul
            | Self::Fmul
            | Self::Dmul
            | Self::Idiv
            | Self::Ldiv
            | Self::Fdiv
            | Self::Ddiv
            | Self::Irem
            | Self::Lrem
            | Self::Frem
            | Self::Drem
            | Self::Ineg
            | Self::Lneg
            | Self::Fneg
            | Self::Dneg
            | Self::Ishl
            | Self::Lshl
            | Self::Ishr
            | Self::Lshr
            | Self::Iushr
            | Self::Lushr
            | Self::Iand
            | Self::Land
            | Self::Ior
            | Self::Lor
            | Self::Ixor
            | Self::Lxor
            | Self::I2l
            | Self::I2f
            | Self::I2d
            | Self::L2i
            | Self::L2f
            | Self::L2d
            | Self::F2i
            | Self::F2l
            | Self::F2d
            | Self::D2i
            | Self::D2l
            | Self::D2f
            | Self::I2b
            | Self::I2c
            | Self::I2s
            | Self::Lcmp
            | Self::Fcmpl
            | Self::Fcmpg
            | Self::Dcmpl
            | Self::Dcmpg
            | Self::Ireturn
            | Self::Lreturn
            | Self::Freturn
            | Self::Dreturn
            | Self::Areturn
            | Self::Return
            | Self::Arraylength
            | Self::Athrow
            | Self::Monitorenter
            | Self::Monitorexit
            | Self::Breakpoint
            | Self::Impdep1
            | Self::Impdep2 => OperandKind::None,
        }
    }
}
