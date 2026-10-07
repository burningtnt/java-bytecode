# java-bytecode

> **该项目（甚至 README）是 100% AI 生成的，因此如果你觉得这是 AI Slop，那你的直觉是正确的。**

一个用 Rust 编写的 Java class 文件（JVMS class file format）读取、编辑与写入库。覆盖 JVMS 第 4 章的类文件结构（常量池、字段、方法、全部 30 个预定义属性，包括注解、类型注解、`StackMapTable`）以及第 6 章的完整指令集。

## 特性

- **零拷贝读取**：`ClassFile<'a>` 借用输入字节，所有变长数据（`Utf8`、未知属性体、字节码等）都以 `&'a` 形式借用，解析不复制。
- **不透明索引**：常量池下标是 `ConstantRef<'a>`（`#[repr(transparent)]` 包裹 `u16`）。它带生命周期标记且对 `'a` 不变，因此**无法把一个类的下标用到另一个类上**。仅提供 `unsafe` 的 `from_raw` / `into_raw` 逃逸口。
- **Modified UTF-8 封装**：`Mutf8<'a>` 替代裸 `&[u8]`，可直接与 `str` / `String` / `&[u8]` 比较并互转；通过 `TryFrom` 校验构造，另有 `unsafe from_bytes_unchecked`。
- **可编辑字节码**：`CodeAttribute::instructions_mut()` 返回作用域绑定的 `Instructions<'a, '_>` 视图（`Deref` 到 `Vec<Instruction<'a>>`），`apply()` 自动重新编码并写回。
- **常量池重建**：`to_bytes()` 保序（索引稳定）；`to_bytes_rebuilt()` 压缩并去重常量池，重写包括字节码在内的全部索引。
- **不做语义校验**：只做结构解析，不实现 §4.8/§4.9/§4.10 的校验；畸形输入返回 `Error` 而非 panic。

## 设计要点

- **纯借用模型**：所有可变长数据由调用方持有，`ClassFile<'a>` 只借用。新增的字符串/字节必须以 `&'a` 传入。
- **`ConstantRef<'a>`**：由 `ConstantPool::push` / `intern` / `intern_utf8` 返回；`get` / `utf8` / `utf8_str` 等接收该类型。
- **`Mutf8<'a>`**：`Constant::Utf8`、`ConstantPool::utf8`、`SourceDebugExtension`、`FieldType::Object`、名称校验等均使用它。
- **`Instructions` + `apply`**：编辑字节码的正道；`bytecode::decode` / `encode` 因无法管理常量池下标而为 `unsafe`，仅作底层/高级用途。

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

插桩（在 `voidMethod()V` 前插入对 `example/Hook.entry()V` 的调用）：

```rust
use java_bytecode::{ClassFile, Constant, Instruction, Mutf8, Opcode};

fn instrument(bytes: &[u8]) -> Result<Vec<u8>, java_bytecode::Error> {
    let mut class = ClassFile::parse(bytes)?;

    // 1. 添加所需常量。
    let class_name = class.constant_pool.intern_utf8(Mutf8::try_from("example/Hook")?)?;
    let class_ref = class
        .constant_pool
        .intern(Constant::Class { name_index: class_name })?;
    let name = class.constant_pool.intern_utf8(Mutf8::try_from("entry")?)?;
    let descriptor = class.constant_pool.intern_utf8(Mutf8::try_from("()V")?)?;
    let name_and_type = class.constant_pool.intern(Constant::NameAndType {
        name_index: name,
        descriptor_index: descriptor,
    })?;
    let method_ref = class.constant_pool.intern(Constant::Methodref {
        class_index: class_ref,
        name_and_type_index: name_and_type,
    })?;

    // 2. 通过作用域绑定的视图编辑并写回。
    {
        let code = class
            .find_method_mut("voidMethod", "()V")
            .expect("method not found")
            .code_mut()
            .expect("method has no Code");
        let mut instructions = code.instructions_mut()?;
        instructions.insert(0, Instruction::ConstantWide(Opcode::Invokestatic, method_ref));
        instructions.apply()?;
    }

    // 3. 序列化（索引稳定；如需压缩常量池可用 to_bytes_rebuilt）。
    class.to_bytes()
}
```

> 注意：`apply()` 只负责重新编码字节码。改变代码长度后，分支/switch 偏移、异常表、调试表与 `StackMapTable` 需由调用方自行处理。

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
