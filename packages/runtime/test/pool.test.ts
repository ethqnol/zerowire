import { describe, it, expect } from "vitest";
import { defaultTapePool, defaultBufferPool, isAsciiWord32, isAsciiWord64 } from "../src/index.js";

describe("@ox-wire/runtime", () => {
  it("acquires tapes from StaticTapePool without throwing", () => {
    const t1 = defaultTapePool.acquire();
    const t2 = defaultTapePool.acquire();
    expect(t1).toBeInstanceOf(Uint32Array);
    expect(t2).toBeInstanceOf(Uint32Array);
    expect(t1.length).toBe(65536);
  });

  it("acquires buffers from StaticBufferPool", () => {
    const b1 = defaultBufferPool.acquire();
    expect(b1).toBeInstanceOf(Uint8Array);
    expect(b1.length).toBe(65536);
  });

  it("detects ASCII words with SWAR", () => {
    const buf = new Uint8Array([0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48]); // "ABCDEFGH"
    const view = new DataView(buf.buffer);
    expect(isAsciiWord32(view, 0)).toBe(true);
    expect(isAsciiWord64(view, 0)).toBe(true);

    // Non-ASCII byte
    buf[1] = 0x80;
    expect(isAsciiWord32(view, 0)).toBe(false);
    expect(isAsciiWord64(view, 0)).toBe(false);
  });
});
