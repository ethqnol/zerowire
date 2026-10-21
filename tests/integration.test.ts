import { describe, it, expect } from "vitest";
import { compileWireCodec } from "@zerowire/napi";

describe("End-to-End AOT Codec Integration", () => {
  it("compiles, validates, and lazy-decodes TradeExecution flyweight view", async () => {
    const tsCode = `
      type u32 = number & { __brand: "u32" };

      export interface TradeExecution {
        timestamp: number;
        tradeId: u32;
        side: "BUY" | "SELL";
        isMaker: boolean;
        symbol: string;
        fills: number[];
      }
    `;

    const res = compileWireCodec(tsCode, "TradeExecution");
    expect(res.fixedHeaderSize).toBe(32);
    expect(res.alignment).toBe(8);

    // Evaluate generated JS codec bundle
    const codecModule = await import(
      `data:text/javascript;base64,${Buffer.from(res.jsCode).toString("base64")}`
    );
    const { TradeExecutionCodec } = codecModule;

    const sample = {
      timestamp: 1718900000.123,
      tradeId: 42000,
      side: "BUY",
      isMaker: true,
      symbol: "ETH-USDT",
      fills: [3500.5, 3501.0, 3500.75],
    };

    // 1. Encode to packed binary buffer
    const binary = TradeExecutionCodec.encodeBinary(sample);
    expect(binary.byteLength).toBe(32 + "ETH-USDT".length + 3 * 8); // 32 header + 8 str + 24 f64[] = 64 bytes

    // 2. Validate binary buffer
    const view = new DataView(binary.buffer, binary.byteOffset, binary.byteLength);
    expect(TradeExecutionCodec.validateBinary(view)).toBe(1);

    // 3. Lazy Flyweight decoding
    const flyweight = TradeExecutionCodec.validateAndView(binary);
    expect(flyweight).not.toBeNull();
    expect(flyweight.timestamp).toBeCloseTo(1718900000.123);
    expect(flyweight.tradeId).toBe(42000);
    expect(flyweight.side).toBe("BUY");
    expect(flyweight.isMaker).toBe(true);
    expect(flyweight.symbol).toBe("ETH-USDT");

    const fills = flyweight.fills;
    expect(fills.length).toBe(3);
    expect(fills[0]).toBe(3500.5);
    expect(fills[1]).toBe(3501.0);
    expect(fills[2]).toBe(3500.75);

    // 4. Truncated buffer should fail validation
    const truncated = binary.subarray(0, 31); // 1 byte short of header
    const truncView = new DataView(truncated.buffer, truncated.byteOffset, truncated.byteLength);
    expect(TradeExecutionCodec.validateBinary(truncView)).toBe(0);
    expect(TradeExecutionCodec.validateAndView(truncated)).toBeNull();
  });
});
