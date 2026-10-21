# AGENTS.md — Engineering Standards, Codegen Invariants & Workflow Guide

This file defines the strict architectural constraints, coding standards, commenting rules, verification workflows, and Git procedures for autonomous AI coding agents and human contributors working on this repository.

---

## 1. Project Context & Core Mission

We are building an Ahead-of-Time (AOT) TypeScript type-to-wire compiler and zero-copy binary codec powered by Rust (`oxc_parser`, `oxc_resolver`, `oxc_semantic`, `napi-rs`) and WebAssembly SIMD (`v128`) / JavaScript `DataView` SWAR (SIMD Within A Register).

### Non-Negotiable Performance Invariants
1. **Zero `JSON.parse()` on Hot Paths:** Generated validators and flyweight views must operate directly over raw `Uint8Array`, `ArrayBuffer`, or `SharedArrayBuffer` slices. Never emit code that falls back to `JSON.parse()` or `JSON.stringify()`.
2. **Zero Eager Heap Allocation:** Validation routines must return a primitive bitmask or populate a pre-allocated static `Uint32Array` offset tape. Strings and nested sub-views must only be decoded lazily inside Flyweight getter accessors.
3. **Straight-Line Branchless Execution:** For fixed-size struct headers, emit bitwise error accumulation (`|`, `&`, `^`) rather than chained short-circuiting `if / else` statements. Branch once at the end of a header block.
4. **No `tsc` Dependency:** Never introduce `typescript` Compiler API dependencies at build time. All AST parsing, module resolution, and semantic analysis must run through `oxc` crates in Rust.

---

## 2. Repository Structure & Commands

```text
├── crates/
│   ├── ox-compiler/       # Core Rust crate: oxc AST -> TypeIR -> MPHF & Layout -> JS/WASM Codegen
│   ├── ox-wasm-simd/      # Rust wasm32-unknown-unknown crate for v128 SIMD structural scanning
│   └── ox-napi/           # napi-rs bindings exposing the compiler to Node/Bun/Deno
├── packages/
│   ├── runtime/           # Zero-dependency TS runtime (Flyweight base views, static tape pools)
│   ├── unplugin/          # Vite, Rolldown, esbuild, Rollup, Webpack, and Bun build plugins
│   └── benchmarks/        # Mitata / Tinybench throughput & allocation benchmarks vs Zod/Typia
└── tests/
    ├── fixtures/          # Input TS interfaces and expected byte layouts / generated code snapshots
    └── property/          # Fuzzing & roundtrip property tests (proptest in Rust, fast-check in TS)
```

### Standard Development Commands
* **Build Rust Workspace:** `cargo build --workspace`
* **Run Rust Unit & Snapshot Tests:** `cargo test --workspace`
* **Lint Rust (Strict):** `cargo clippy --workspace --all-targets -- -D warnings`
* **Format Rust:** `cargo fmt --all -- --check`
* **Build NAPI Binding:** `pnpm --filter @ox-wire/napi build`
* **Run JS/TS Runtime & Integration Tests:** `pnpm test`
* **Run Benchmarks:** `pnpm --filter @ox-wire/benchmarks bench`

---

## 3. Code Writing Standards

### 3.1 Rust Compiler (`crates/*`)
* **Arena Allocation:** `oxc_parser` uses `oxc_allocator::Allocator` (bump allocation). Keep AST lifetimes scoped to the arena and lower needed metadata into compact, owned `TypeIR` structures or interned strings (`oxc_span::Atom`) before dropping the AST arena.
* **No Panics in Library Code:** Never use `.unwrap()` or `.expect()` in AST traversal, layout calculation, or codegen paths. Always return structured diagnostics using `oxc_diagnostics::OxcDiagnostic` with accurate source `Span` pointers so users get clear compiler errors pointing to the exact TypeScript line.
* **Deterministic Output:** Never iterate over standard `HashMap` or `HashSet` when emitting struct layouts, Perfect Hash tables, or JavaScript code. Use `FxHashMap` paired with explicit sorting, or `IndexMap`, so that identical TypeScript inputs produce byte-for-byte identical codegen across platforms.
* **Struct Packing & Alignment:** Every `TypeIR` struct must sort fixed-width fields in descending order of alignment ($8	ext{B} 
ightarrow 4	ext{B} 
ightarrow 2	ext{B} 
ightarrow 1	ext{B}$). Dynamic fields (`string`, `T[]`) must be represented in the fixed header as an 8-byte descriptor (`u32 offset`, `u32 length`).

### 3.2 Generated JavaScript & WebAssembly (`packages/runtime` & Codegen)
* **V8 TurboFan / Maglev Friendliness:**
  * Keep generated functions monomorphic. Always pass `(view: DataView, baseOffset: number)` with consistent argument types.
  * Use `Math.imul(a, b)` and unsigned right shifts (`>>>`) for 32-bit integer arithmetic and Minimal Perfect Hash Function (MPHF) index calculations so V8 keeps values in 32-bit SMI/unboxed registers.
* **Endianness:** Explicitly pass `true` (little-endian) as the final argument to all `DataView` multi-byte calls (`view.getFloat64(offset, true)`, `view.getUint32(offset, true)`, `view.getBigUint64(offset, true)`).
* **UTF-8 String Decoding Threshold:** Calling `new TextDecoder().decode(subarray)` has significant C++ binding overhead in V8 for short strings. In generated Flyweight string getters:
  * For ASCII strings $< 48$ bytes, emit an inline char-code loop (`String.fromCharCode`).
  * Fall back to a shared singleton `TextDecoder` instance only when length $\ge 48$ bytes or when SWAR high-bit detection (`(word & 0x80808080) !== 0`) indicates multi-byte UTF-8 characters.

---

## 4. Commenting Guidelines

Comments must explain **why** a decision was made, **what** memory layout or bit-level invariant holds, or **what** code is being emitted—never restate obvious syntax.

### 4.1 What to Comment
1. **Bitwise & SWAR Constants (Mandatory):** Every magic hex literal, SWAR mask, or MPHF multiplier must have an inline comment showing the exact ASCII bytes, endianness, or bit layout it represents.
   ```rust
   // GOOD: Explains exact byte layout in little-endian order
   // Matches JSON key `"price":` (8 bytes) -> ASCII [0x22, 0x70, 0x72, 0x69, 0x63, 0x65, 0x22, 0x3a]
   const KEY_PRICE_SWAR: u64 = 0x3a22_6563_6972_7022;
   ```
   ```rust
   // BAD: Restates the code without explaining the constant
   // Set the price constant to 0x3a22656369727022
   const KEY_PRICE_SWAR: u64 = 0x3a22656369727022;
   ```
2. **Memory Layout Diagrams:** Whenever modifying struct packing, header descriptors, or tape layouts, include or update an ASCII byte-offset table in the module doc-comment:
   ```rust
   //! Fixed Header Layout (32 bytes, 8-byte aligned):
   //! [0x00..0x07] timestamp: f64
   //! [0x08..0x0B] tradeId:   u32
   //! [0x0C..0x13] symbol:    (u32 offset, u32 len)
   ```
3. **Codegen Templates:** Inside Rust functions that emit JS or WASM instructions, place a block comment above the generator showing a concrete example of the target JS/WASM output.
4. **Safety Blocks (`unsafe`):** Any `unsafe` block in Rust (e.g., inside `ox-wasm-simd` `v128` intrinsics or unaligned pointer reads) **must** be preceded by a `// SAFETY:` comment proving why bounds and alignment preconditions are guaranteed prior to entry.

### 4.2 What NOT to Comment
* Do not leave AI narration comments (e.g., `// Here we updated the function as requested`, `// Step 1: loop over items`).
* Do not leave commented-out dead code. Delete it; Git history preserves it.
* Avoid vague `// TODO` comments. Every `TODO` must follow `// TODO(component): specific actionable description`.

---

## 5. Code Review & Pre-Submission Verification Process

Before marking a task complete or opening a Pull Request, agents and contributors must execute the following self-review checklist in order:

### Step 1: Static Analysis & Formatting Gate
* Run `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings`. Zero warnings are permitted.
* Run `pnpm typecheck` and `pnpm lint` across all TypeScript packages.

### Step 2: Codegen Snapshot & Byte-Layout Verification
* If your change alters `TypeIR`, struct packing, SWAR constants, or JS/WASM emission, run `cargo test` and inspect the snapshot diffs (`insta` snapshots in `tests/fixtures/`).
* Verify that:
  1. All 8-byte reads sit at offsets divisible by 8, and 4-byte reads at offsets divisible by 4 (or use `DataView` safely).
  2. Every fixed-header validator starts with a single bounds check (`if (baseOffset + HEADER_SIZE > view.byteLength) return 0;`).
  3. Every dynamic slice descriptor (`offset`, `length`) is validated against `view.byteLength` using overflow-safe arithmetic before any slice read.

### Step 3: Adversarial & Fuzz Testing
* Any change to byte validators or SIMD scanners must pass adversarial boundary tests:
  * Empty buffers (`new Uint8Array(0)`).
  * Truncated buffers (1 byte short of header size $H - 1$).
  * Malformed UTF-8 sequences and unescaped quotes inside JSON strings (`"`).
  * Corrupted dynamic length descriptors (`0xFFFFFFFF` offset/length overflow attacks).
  * Out-of-set discriminator strings that collide in the Minimal Perfect Hash table (verifying the full-word confirmation check rejects them).

### Step 4: Benchmark Regression Check
* For changes touching codegen, runtime views, or SIMD scanning, run the benchmark suite (`pnpm bench`) and verify no regressions in throughput (ops/sec) or heap allocations (must remain 0 heap allocations during validation).

---

## 6. Git & Commit Conventions

### 6.1 Atomic Commits
* Each commit must represent a single cohesive, buildable, and tested logical unit of work.
* Separate **refactoring / formatting** commits from **feature / bugfix** commits. Never mix workspace-wide formatting changes with subtle compiler logic changes.
* Never commit build artifacts (`target/`, `dist/`, `*.node` binaries, `node_modules/`) or temporary benchmark logs.

### 6.2 Conventional Commit Format
All commit messages must follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```text
<type>(<scope>): <concise summary in imperative mood, <= 72 chars>

<body: explain the 'why', architectural trade-offs, and byte-level impact>

<footer: benchmark deltas, breaking changes, or issue references>
```

#### Allowed `<type>` Values:
* `feat`: New compiler capability, IR lowering pass, or runtime API.
* `perf`: Optimization to generated JS/WASM code, SWAR matching, or compile speed.
* `fix`: Bug fix in parser extraction, layout alignment, or runtime validation.
* `refactor`: Internal code restructuring without changing external behavior or emitted code.
* `test`: Adding or updating unit, snapshot, or property fuzz tests.
* `docs`: Updates to `plan.md`, `AGENTS.md`, or inline architecture docs.
* `chore`: Cargo/pnpm dependency updates, CI configuration, or build scripts.

#### Allowed `<scope>` Values:
* `parser` (`oxc` AST extraction & module resolution)
* `ir` (`TypeIR` canonicalization & utility type expansion)
* `layout` (Struct packing, alignment, & byte offset calculation)
* `mphf` (Compile-time Minimal Perfect Hash generation)
* `codegen-js` (JS `DataView`, SWAR, & Flyweight class emission)
* `codegen-wasm` (WebAssembly `v128` SIMD structural scanner)
* `runtime` (`@ox-wire/runtime` TypeScript package)
* `unplugin` (Vite/Rolldown/esbuild/Bun bundler plugins)

#### Example Commit Message:
```text
perf(codegen-js): lower 4-to-12 byte JSON key checks to 64-bit SWAR reads

Previously, JSON object keys in the fallback scanner were matched using
byte-by-byte Uint8Array indexing, causing up to 12 loop branches per key.

Emit precomputed little-endian BigUint64 and Uint32 hex constants via
DataView.prototype.getBigUint64(pos, true). Keys up to 12 bytes now
validate in two straight-line integer comparisons with zero branches.

Benchmarks (MarketTick 250B payload, Node v22):
- Throughput: 14.2M ops/s -> 23.8M ops/s (+67.6%)
- Allocations: 0 B/op
```
