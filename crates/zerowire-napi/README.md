# @zerowire/napi

Native Node-API Rust bindings for the zerowire Ahead-of-Time TypeScript-to-wire compiler.

## Overview

`@zerowire/napi` exposes the core Rust compiler engine (`zerowire-compiler`) directly to Node.js, Bun, and build tools. It runs `oxc_parser`, `oxc_allocator`, and struct alignment calculations in native compiled code to lower TypeScript interfaces into branchless JavaScript validators and binary codecs in sub-millisecond execution times.

## Installation

```bash
pnpm add -D @zerowire/napi
# or
npm install -D @zerowire/napi
# or
bun add -d @zerowire/napi
```

## Programmatic API

```typescript
import { compileWireCodec } from "@zerowire/napi";

const tsCode = `
  type u32 = number & { __brand: "u32" };

  export interface MarketTick {
    price: number;
    volume: u32;
    symbol: string;
  }
`;

// Compile TypeScript schema directly to wire codec
const result = compileWireCodec(tsCode, "MarketTick");

console.log("Fixed Header Size:", result.fixedHeaderSize, "bytes");
console.log("Struct Alignment:", result.alignment, "bytes");
console.log("Layout Diagram:\n", result.layoutDiagram);
console.log("Generated JavaScript Code:\n", result.jsCode);
```

## Supported Platforms

Pre-compiled native bindings are provided for:
- Linux x64 (glibc and musl)
- Linux arm64 (glibc and musl)
- macOS arm64 (Apple Silicon)
- macOS x64 (Intel)
- Windows x64 (MSVC)

## License

MIT OR Apache-2.0
