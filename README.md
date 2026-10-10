# java-bytecode

> **该项目（甚至 README）是 100% AI 生成的，因此如果你觉得这是 AI Slop，那你的直觉是正确的。**

一个用 Rust 编写的 Java class 文件（JVMS class file format）读取、编辑与写入库。覆盖 JVMS 第 4 章的类文件结构（常量池、字段、方法、全部 30 个预定义属性，包括注解、类型注解、`StackMapTable`）以及第 6 章的完整指令集。

## 特性

- **零拷贝读取**：`ClassFile<'a>` 借用输入字节，所有变长数据（`Utf8`、未知属性体、字节码等）都以 `&'a` 形式借用，解析不复制。
- **不透明索引**：常量池下标是 `ConstantRef<'a>`（`#[repr(transparent)]` 包裹 `u16`）。它带生命周期标记且对 `'a` 不变，因此**无法把一个类的下标用到另一个类上**。仅提供 `unsafe` 的 `from_raw` / `into_raw` 逃逸口。
- **Modified UTF-8 封装**：`Mutf8<'a>` 替代裸 `&[u8]`，可直接与 `str` / `String` / `&[u8]` 比较并互转；通过 `TryFrom` 校验构造，另有 `unsafe from_bytes_unchecked`。
- **可编辑字节码**：`CodeAttribute::instructions_mut()` 返回作用域绑定的 `Instructions<'a, '_>` 视图。用它的 `insert` / `remove` / `replace` / `splice` 修改方法体，分支与 switch 偏移会自动重定位；`apply()` 重新编码并写回。
- **常量池重建**：`to_bytes()` 保序（索引稳定）；`to_bytes_rebuilt()` 压缩并去重常量池，重写包括字节码在内的全部索引。
- **不做语义校验**：只做结构解析，不实现 §4.8/§4.9/§4.10 的校验；畸形输入返回 `Error` 而非 panic。

## 设计要点

- **纯借用模型**：所有可变长数据由调用方持有，`ClassFile<'a>` 只借用。新增的字符串/字节必须以 `&'a` 传入。
- **`ConstantRef<'a>`**：由 `ConstantPool::push` / `intern` / `intern_utf8` 返回；`get` / `utf8` / `utf8_str` 等接收该类型。`intern_utf8` 直接接受 `&'a str`，内部完成 `Mutf8::try_from` 校验。
- **常量池槽位**：`ConstantPool` 内部用 `Vec<Constant<'a>>` 表示，0 号槽以及 `Long`/`Double` 的第二槽是 `Constant::Invalid` 占位符（不会被序列化）。
- **`Mutf8<'a>`**：`Constant::Utf8`、`ConstantPool::utf8`、`SourceDebugExtension`、`FieldType::Object`、名称校验等均使用它。
- **`Instructions` + `apply`**：编辑字节码的正道，`insert` / `remove` / `replace` / `splice` 会自动重定位分支与 switch 偏移；`bytecode::decode` / `encode` 因无法管理常量池下标而为 `unsafe`，仅作底层/高级用途。

## 快速开始

解析并查看：

```rust
use java_bytecode::ClassFile;

fn main() -> Result<(), java_bytecode::Error> {
    let bytes = std::fs::read("Foo.class")?;
    let class = ClassFile::parse(&bytes)?;

    println!("{}", class.this_class_name()?);
    for method in &class.methods {
        println!(
            "{} {}",
            method.name(&class.constant_pool)?,
            method.descriptor(&class.constant_pool)?,
        );
    }
    Ok(())
}
```

插桩（在 `int dense(int)` 的方法头插入对 `example/Hook.entry()V` 的调用）：

```rust
use java_bytecode::{ClassFile, Constant, Instruction, Opcode};

fn instrument(bytes: &[u8]) -> Result<Vec<u8>, java_bytecode::Error> {
    let mut class = ClassFile::parse(bytes)?;

    // 1. 添加所需常量。
    let class_name = class.constant_pool.intern_utf8("example/Hook")?;
    let class_ref = class
        .constant_pool
        .intern(Constant::Class { name_index: class_name })?;
    let name = class.constant_pool.intern_utf8("entry")?;
    let descriptor = class.constant_pool.intern_utf8("()V")?;
    let name_and_type = class.constant_pool.intern(Constant::NameAndType {
        name_index: name,
        descriptor_index: descriptor,
    })?;
    let method_ref = class.constant_pool.intern(Constant::Methodref {
        class_index: class_ref,
        name_and_type_index: name_and_type,
    })?;

    // 2. 通过作用域绑定的视图编辑并写回；分支/switch 偏移自动重定位。
    {
        let code = class
            .find_method_mut("dense", "(I)I")
            .expect("method not found")
            .code_mut()
            .expect("method has no Code");
        let mut instructions = code.instructions_mut()?;
        instructions.insert(0, Instruction::ConstantWide(Opcode::Invokestatic, method_ref))?;
        instructions.apply()?;
    }

    // 3. 序列化（索引稳定；如需压缩常量池可用 to_bytes_rebuilt）。
    class.to_bytes()
}
```

> 注意：`insert` / `remove` / `replace` / `splice` 会重定位分支与 switch 偏移。异常表、调试表与 `StackMapTable` 仍由调用方负责；`max_stack` / `max_locals` 也需按需更新。

## 从零创建类

库没有专门的 builder：`ClassFile` 各字段都是 `pub`，直接拼装即可。先填常量池（`intern` / `intern_utf8` 返回后续结构要引用的 `ConstantRef`），再搭 `ClassFile` / `MethodInfo` / `Attribute`。`CodeAttribute` 的字节码字段私有，但可以用 `CodeAttribute::from_instructions(max_stack, max_locals, instructions)` 从指令列表构建，再用 `into_attribute(name_index)` 包成 `Attribute`（原类文件也可用 `instructions_mut()` 就地编辑）。所有借用数据必须比 `ClassFile` 活得久（`'static` 字面量天然满足）。

```rust
use java_bytecode::{
    ClassAccessFlags, ClassFile, CodeAttribute, Constant, ConstantPool, Instruction,
    MethodAccessFlags, MethodInfo, Opcode,
};

fn build() -> Result<Vec<u8>, java_bytecode::Error> {
    let mut pool = ConstantPool::new();

    let this_name = pool.intern_utf8("example/Sender")?;
    let this_class = pool.intern(Constant::Class { name_index: this_name })?;
    let super_name = pool.intern_utf8("java/lang/Object")?;
    let super_class = pool.intern(Constant::Class { name_index: super_name })?;
    let send_name = pool.intern_utf8("send")?;
    let send_descriptor = pool.intern_utf8("(Ljava/nio/ByteBuffer;Ljava/net/SocketAddress;)I")?;
    let code_name = pool.intern_utf8("Code")?;

    // 用指令列表构建方法体：iconst_0; ireturn。
    // max_stack = 1，max_locals = 3（this + 两个参数）。
    let code = CodeAttribute::from_instructions(
        1,
        3,
        [Instruction::None(Opcode::Iconst0), Instruction::None(Opcode::Ireturn)],
    )?;

    let send = MethodInfo {
        access_flags: MethodAccessFlags::PUBLIC,
        name_index: send_name,
        descriptor_index: send_descriptor,
        attributes: vec![code.into_attribute(code_name)],
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
```

要点：

- 非静态方法 `max_locals` 含 `this`（此处 3）；加 `ACC_STATIC` 后应改为 2。
- `abstract` / `native` 方法不能带 `Code` 属性，`attributes` 留空即可。
- 上面的方法体只返回 0；要写真正的逻辑，往指令列表里加 `Instruction` 即可，有分支/跳转时还要处理偏移与 `StackMapTable`。
- 更省事的替代方案：用 `javac` 生成带目标签名的骨架类，`ClassFile::parse` 后再改常量池/名字。
- 冒烟测试见 `tests/create.rs`。

## 运行示例

```bash
cargo run --example inspect -- tests/fixtures/classes/main/example/Simple.class
cargo run --release --example corpus -- /path/to/classes
```

## 测试[LICENSE](LICENSE)

```bash
cargo test
```

- `tests/fixtures` 由 Java 25 通过 `scripts/gen_fixtures.sh` 生成，覆盖 record/sealed/enum/注解/类型注解/泛型/lambda/switch/module-info/嵌套类等。
- 大语料测试（可选）：先用 `scripts/fetch_jdk_corpus.sh` 下载并提取 OpenJDK 25 的全部 class，然后：

```bash
JAVA_BYTECODE_CORPUS=tests/fixtures/jdk-corpus cargo test --release --test bytecode_corpus -- --nocapture
```

该测试会对每个 `Code` 做整段与指令级往返、`encoded_len` 一致性、分支目标边界校验。最近一次在 OpenJDK 25 上：27054 个类、214853 个 `Code`、约 1248 万条指令，0 失败。
