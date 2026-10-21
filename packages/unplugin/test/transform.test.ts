import { describe, it, expect } from "vitest";
import { unplugin } from "../src/index.js";

describe("@zerowire/unplugin", () => {
  it("transforms compileWireCodec macro call into compiled JS code", () => {
    const plugin: any = unplugin.raw({}, {} as any);
    const sourceCode = `
      export interface MarketTick {
        price: number;
        volume: number;
        symbol: string;
      }

      export const MarketTickCodec = compileWireCodec<MarketTick>();
    `;

    expect(plugin.transformInclude("test.ts")).toBe(true);
    expect(plugin.transformInclude("test.js")).toBe(false);

    const transformed = plugin.transform(sourceCode, "test.ts");
    expect(transformed).not.toBeNull();
    expect(transformed.code).toContain("validateMarketTickBinary");
    expect(transformed.code).toContain("class MarketTickView");
    expect(transformed.code).toContain("export const MarketTickCodec = MarketTickCodec;");
  });
});
