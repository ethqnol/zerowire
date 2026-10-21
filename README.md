# zerowire

Ahead-of-time TypeScript type-to-wire compiler and zero-copy binary codec.

zerowire eliminates `JSON.parse()` overhead and V8 heap allocation on high-throughput paths by compiling TypeScript interfaces directly into branchless DataView validators, Minimal Perfect Hash jump tables, and lazy flyweight proxy views. AST parsing and type analysis run entirely in Rust via Oxc with zero runtime or build-time dependency on `tsc`.

## Overview

Traditional runtime validation libraries like Zod, Valibot, or ArkType parse wire JSON into dynamic JavaScript objects before inspecting fields. In high-throughput environments (API gateways, WebSocket relays, and inter-thread messaging), this introduces three bottlenecks:

1. Eager heap allocation for fields the application may never read.
2. Garbage collection latency spikes from short-lived objects.
3. Branch mispredictions in recursive validation functions.

zerowire analyzes static TypeScript types ahead-of-time and generates:

1. A branchless binary struct validator that accumulates errors using bitwise operations and checks bounds in a single entry pass.
2. A lazy Flyweight proxy class that reads numbers directly from fixed DataView offsets and unrolls short string decoding on demand.
3. A single-pass binary encoder that writes JavaScript objects directly into flat, aligned buffers.
4. SWAR key matchers and Minimal Perfect Hash Functions for O(1) variant dispatch without string allocations.

## Benchmarks

Measured on Node.js v22 (x64-linux, AMD Ryzen AI 5) using Mitata on a market tick payload (price, volume, exchange enum, and dynamic symbol string):

| Benchmark | Throughput | Average Latency | Heap Allocation |
| --- | --- | --- | --- |
| `JSON.parse() + Zod Validation` | ~460k ops/s | 2.17 µs/iter | ~2.01 KB/op |
| `JSON.parse() Alone` | ~1.60M ops/s | 624.59 ns/iter | ~73 B/op |
| `zerowire validateAndView + Lazy Access` | ~6.88M ops/s | 145.16 ns/iter | ~229 B/op |
| `zerowire Binary Validation` | ~169.0M ops/s | 5.92 ns/iter | 0 B/op |

Binary validation in zerowire runs in under 6 nanoseconds with zero heap allocations on the hot path, outperforming Zod by approximately 366x.

## Core Features

- Zero tsc dependency: AST extraction and module resolution run via `oxc_parser` and `oxc_allocator` in Rust.
- Branchless execution: Struct headers accumulate validation errors via bitwise OR, AND, and XOR, branching once at the end of the header.
- Automatic struct packing: Sorts fixed fields by alignment (8B -> 4B -> 2B -> 1B) to guarantee zero interior padding holes.
- Lazy decoding: Unread strings and dynamic arrays remain raw bytes until accessed on the flyweight view.
- Unrolled short string fast path: ASCII strings up to 4 bytes decode directly via unrolled char codes, bypassing TextDecoder overhead.
- Minimal Perfect Hash Functions: Variant literals in tagged unions map to jump table indices with O(1) integer arithmetic.

## Installation

```bash
# Runtime package
pnpm add @zerowire/runtime

# Build-time plugin
pnpm add -D @zerowire/unplugin @zerowire/napi
```

## Quick Start

### Define Types and Compile Codec

```typescript
import { compileWireCodec, type u32, type f64 } from "@zerowire/runtime";

export interface TradeExecution {
  timestamp: f64;
  tradeId: u32;
  side: "BUY" | "SELL";
  isMaker: boolean;
  symbol: string;
  fills: f64[];
}

export const TradeExecutionCodec = compileWireCodec<TradeExecution>();
```

### Validate and Access Slices

```typescript
// Incoming raw Uint8Array from network or worker
const view = TradeExecutionCodec.validateAndView(rawBytes);

if (view) {
  // Numeric and boolean fields are read at fixed offsets from DataView
  console.log(view.timestamp, view.tradeId, view.side);

  // Strings and dynamic arrays decode lazily on first property access
  console.log(view.symbol);
}
```

### Encode to Binary

```typescript
const binaryBuffer = TradeExecutionCodec.encodeBinary({
  timestamp: Date.now(),
  tradeId: 10420,
  side: "BUY",
  isMaker: true,
  symbol: "BTC-USDT",
  fills: [64250.0, 64255.5],
});

// Fast binary validation
const isValid = TradeExecutionCodec.validateBinary(
  new DataView(binaryBuffer.buffer, binaryBuffer.byteOffset, binaryBuffer.byteLength)
);
```

## Memory Layout

The compiler partitions types into a fixed header and a variable-length tail heap:

```text
Fixed Header Layout (32 bytes, 8-byte aligned)
[0x00..0x07] timestamp     f64 (IEEE-754 little-endian) 8B
[0x08..0x0B] tradeId       u32                          4B
[0x0C..0x13] symbol        (u32 offset, u32 length)     8B
[0x14..0x1B] fills         (u32 offset, u32 length)     8B
[0x1C]       side          u8  (0 = BUY, 1 = SELL)      1B
[0x1D]       isMaker       u8  (0 = false, 1 = true)    1B
[0x1E..0x1F] <padding>     Zero-filled alignment pad    2B
------------------------------------------------------
Total Header Size = 32 Bytes

Tail Heap (Dynamic Payload)
[0x20..0x20+L1]  symbol UTF-8 bytes
[0x20+L1..End]   fills f64[] elements
```

## Bundler Integration

### Vite

```typescript
// vite.config.ts
import { defineConfig } from "vite";
import { zerowireVite } from "@zerowire/unplugin";

export default defineConfig({
  plugins: [zerowireVite()],
});
```

### Rollup and Rolldown

```typescript
// rollup.config.js
import { zerowireRollup } from "@zerowire/unplugin";

export default {
  plugins: [zerowireRollup()],
};
```

### esbuild

```typescript
import esbuild from "esbuild";
import { zerowireEsbuild } from "@zerowire/unplugin";

await esbuild.build({
  plugins: [zerowireEsbuild()],
});
```

## Repository Structure

```text
crates/
  zerowire-compiler/     Rust AST extraction, TypeIR, layout packer, and JS codegen
  zerowire-wasm-simd/    WASM v128 SIMD structural token scanner
  zerowire-napi/         Node-API bindings for compiler invocation
packages/
  runtime/               Zero-dependency TypeScript runtime views and tape pools
  unplugin/              Vite, Rollup, Rolldown, and esbuild integration
  benchmarks/            Performance benchmarks vs Zod and JSON.parse
tests/
  fixtures/              Sample TypeScript schemas and layouts
  integration.test.ts    End-to-end codec compilation and roundtrip tests
```

## Verification

```bash
# Rust test suite and linter
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# TypeScript packages and integration tests
pnpm test
pnpm typecheck

# Run benchmark suite
pnpm --filter @zerowire/benchmarks bench
```

## License

MIT OR Apache-2.0
