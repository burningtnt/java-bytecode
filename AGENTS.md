# AGENTS.md

Operational guide for AI coding assistants working on `java-bytecode`. Imperative rules, not prose.

## 1. Project Overview

- Rust library crate: package `java-bytecode`, lib `java_bytecode`, edition 2024, MSRV 1.85.
- Domain: read/edit/write JVM `.class` files (JVMS chapter 4 structures + chapter 6 instruction set).
- Layout:
  - `src/class_file.rs`, `field.rs`, `method.rs` — top-level structures.
  - `src/constant_pool/{mod,tags}.rs` — 17 constant kinds, `ConstantRef`.
  - `src/mutf8.rs` — modified UTF-8 codec + `Mutf8` wrapper.
  - `src/attributes/*.rs` — all 30 attributes; dispatch in `attributes/mod.rs`.
  - `src/bytecode/{mod,opcode}.rs` — `Instruction`, 205-entry opcode table, codec.
  - `src/rebuild.rs` — constant-pool compaction/remap; `src/io/{reader,writer}.rs`.
- Key types: `ClassFile<'a>`, `ConstantRef<'a>`, `Mutf8<'a>`, `ConstantPool<'a>`, `Instructions<'a, 'code>`.
- Dependencies: `bitflags` 2, `thiserror` 2, `zerocopy` 0.8 (`derive`). Dev: `proptest`.
- Model: **pure borrowing**. `ClassFile<'a>` borrows input; variable-length data is `&'a`. No validation of §4.8/§4.9/§4.10.

## 2. Build & Test Commands

- Toolchain: stable Rust via `rustup`. If `cargo` is missing, run under `bash -l -c "cargo ..."` (env persisted in `/etc/sandbox-persistent.sh`).
- Build: `cargo build` / `cargo build --release`
- Test: `cargo test` (fast). Full incl. javap oracle: `cargo test --release`
- Doctests: `cargo test --doc`
- Lint (must be zero warnings): `cargo clippy --all-targets`
- Format: `cargo fmt` / verify `cargo fmt --check`
- Docs (watch broken intra-doc links): `cargo doc --no-deps`
- Example: `cargo run --example inspect -- tests/fixtures/classes/main/example/Simple.class`
- Regenerate javac-25 fixtures (needs `javac`): `./scripts/gen_fixtures.sh`
- Fetch OpenJDK 25 corpus (network): `./scripts/fetch_jdk_corpus.sh`
- Large corpus test: `JAVA_BYTECODE_CORPUS=tests/fixtures/jdk-corpus cargo test --release --test bytecode_corpus -- --nocapture`

## 3. Code Style & Conventions

### Error handling
- Return `crate::Result<T>`. Add variants to `src/error.rs` with `#[derive(thiserror::Error)]`.
- Library code must not `panic!`/`unwrap()`/`expect()`/index-unchecked. Tests may `unwrap()`.
- ✅ `let value = pool.utf8(index)?;`
- ❌ `let value = pool.utf8(index).unwrap();` (in `src/`)

### Constant-pool indices
- Represent every cp index as `ConstantRef<'a>`; return it from `push`/`intern`/`intern_utf8`.
- Inside the crate use `ConstantRef::new` / `.get()`; never expose raw `u16` cp indices publicly.
- ✅ `pub fn intern(&mut self, c: Constant<'a>) -> Result<ConstantRef<'a>>`
- ❌ `pub fn intern(&mut self, c: Constant<'a>) -> Result<u16>`

### Modified UTF-8
- Use `Mutf8<'a>`, never `&[u8]`, for `Utf8`/names/descriptors/`SourceDebugExtension`.
- Construct via `TryFrom` (`Mutf8::try_from(bytes)`, `try_from(&str)`); parsing uses `Mutf8::new`.
- ✅ `Constant::Utf8(Mutf8)`; `name == "java/lang/Object"`
- ❌ `Constant::Utf8(&'a [u8])`; `impl From<&[u8]> for Mutf8`

### Editing bytecode
- `CodeAttribute.code` is private. Edit only via `instructions_mut()` + `Instructions::apply()`; read via `instructions()`.
- Internally call `bytecode::decode_impl` / `encode_impl`; `bytecode::decode`/`encode`/`encode_into` are `unsafe` and public only as escape hatches.
- ✅ `let mut i = code.instructions_mut()?; i.insert(0, ins)?; i.apply()?;`
- ❌ `code.code = encoded;` (field private) or calling `unsafe { encode(..) }` in library code.

### unsafe
- Crate lint `unsafe_code = "deny"`. Only the documented escape hatches may use `#[allow(unsafe_code)]`: `ConstantRef::{from_raw,into_raw}`, `Mutf8::from_bytes_unchecked`, `bytecode::{decode,encode,encode_into}`, `CodeAttribute::{set_code,set_code_owned}`.
- Every `unsafe fn`/method needs a `# Safety` section stating the invariant.
- ✅ `#[allow(unsafe_code)] pub unsafe fn from_bytes_unchecked(..) { .. } // # Safety ...`
- ❌ adding `unsafe` anywhere else or without a safety doc.

### Attributes
- Add one module under `src/attributes/`; wire parsing in `AttributeBody::parse` and writing in `AttributeBody::encode`.
- Preserve unknown attributes as `AttributeBody::Raw` and unknown flag bits via `from_bits_retain` (byte-exact round-trip).
- ✅ `AttributeBody::Raw(bytes)` for unrecognized names.
- ❌ dropping/rewriting bytes you did not interpret.

### Naming & docs
- Types/traits `CamelCase`; functions/fields `snake_case`; index fields end in `_index` (typed `ConstantRef`).
- Public items need doc comments (`missing_docs` is warn); avoid gratuitous inline comments.
- Keep intra-doc links valid (`crate::` paths for cross-module links).

## 4. Boundaries & Constraints

- **Never hand-edit** `tests/fixtures/classes/**` or `tests/fixtures/src/**` binaries/derived classes; edit the `.java` sources and run `./scripts/gen_fixtures.sh`.
- **Never commit or depend on** `tests/fixtures/jdk-corpus/**` (generated download cache).
- **Never edit `Cargo.lock` by hand**; change `Cargo.toml` instead.
- **Never weaken lints** (`unsafe_code`, `missing_docs`, `missing_debug_implementations`).
- **Never expose raw `u16` cp indices**, and never change `ConstantRef`/`Mutf8`/`Instructions` lifetime invariants.
- **Never make `CodeAttribute.code` public** or reintroduce leaked/`Cow` code storage; `Instructions::apply` owns the bytes.
- **No network I/O in tests**; the only network script is `scripts/fetch_jdk_corpus.sh`.
- **Do not add `panic`/`unwrap`/`expect` in `src/`** for malformed input; return `Err`.
- **Breaking public API changes require** updating doctests, `README.md`, and any `tests/`/`examples/` call sites.
- Keep the `deny(unsafe_code)` discipline: prefer safe internal helpers over new `unsafe`.
