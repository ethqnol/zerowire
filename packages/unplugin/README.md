# @zerowire/unplugin

Universal Ahead-of-Time build plugin for zerowire TypeScript binary codec and validator.

## Overview

`@zerowire/unplugin` intercepts `compileWireCodec<T>()` macro calls in TypeScript code at build time and transforms them into compiled, branchless `DataView` validators, lazy Flyweight classes, and binary encoders. AST extraction and layout packing run via native Rust (`oxc_parser`), operating at sub-millisecond speeds without calling `tsc`.

Supported bundlers include Vite, Rolldown, Rollup, and esbuild.

## Installation

```bash
pnpm add -D @zerowire/unplugin @zerowire/runtime
# or
npm install -D @zerowire/unplugin @zerowire/runtime
# or
bun add -d @zerowire/unplugin @zerowire/runtime
```

## Bundler Configuration

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

```javascript
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
  entryPoints: ["src/index.ts"],
  bundle: true,
  outfile: "dist/bundle.js",
  plugins: [zerowireEsbuild()],
});
```

## How It Works

1. During bundling, `@zerowire/unplugin` scans imported source files for `compileWireCodec<T>()` markers.
2. The Rust compiler extracts the target interface `T` using `oxc_parser` and `oxc_allocator`.
3. The layout packer sorts fixed fields by alignment (8B -> 4B -> 2B -> 1B) and emits a self-contained JavaScript codec module.
4. The call site is replaced with the compiled codec class, providing zero-overhead execution with no runtime compiler dependency.

## License

MIT OR Apache-2.0
