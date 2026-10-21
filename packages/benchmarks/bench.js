import { run, bench, group } from "mitata";
import { z } from "zod";
import { compileWireCodec } from "@ox-wire/napi";

// 1. Compile MarketTick with ox-wire
const tsDefinition = `
  type u32 = number & { __brand: "u32" };
  export interface MarketTick {
    price: number;
    volume: u32;
    exchange: "NYSE" | "NASDAQ" | "CME";
    symbol: string;
  }
`;

const res = compileWireCodec(tsDefinition, "MarketTick");

// Dynamically import or evaluate the generated codec
const codecModule = await import(
  `data:text/javascript;base64,${Buffer.from(res.jsCode).toString("base64")}`
);
const { MarketTickCodec } = codecModule;

// 2. Define Zod Schema for identical type
const ZodMarketTick = z.object({
  price: z.number(),
  volume: z.number().int().nonnegative(),
  exchange: z.enum(["NYSE", "NASDAQ", "CME"]),
  symbol: z.string(),
});

// 3. Prepare Payloads
const sampleObj = {
  price: 68450.25,
  volume: 1500,
  exchange: "CME",
  symbol: "BTC-USDT",
};

const jsonString = JSON.stringify(sampleObj);
const utf8Bytes = new TextEncoder().encode(jsonString);

// Encode into binary format using ox-wire
const binaryBuffer = MarketTickCodec.encodeBinary(sampleObj);
const binaryView = new DataView(binaryBuffer.buffer, binaryBuffer.byteOffset, binaryBuffer.byteLength);

console.log("=== BENCHMARK ENVIRONMENT ===");
console.log("Node version:", process.version);
console.log("JSON Payload Size:", utf8Bytes.byteLength, "bytes");
console.log("Packed Binary Struct Size:", MarketTickCodec.fixedHeaderSize, "bytes");
console.log("Total Binary Buffer Size:", binaryBuffer.byteLength, "bytes");
console.log(res.layoutDiagram);

group("Validation & Deserialization Throughput", () => {
  bench("JSON.parse() + Zod Validation", () => {
    const parsed = JSON.parse(jsonString);
    return ZodMarketTick.parse(parsed);
  });

  bench("JSON.parse() Alone (Eager Object Allocation)", () => {
    return JSON.parse(jsonString);
  });

  bench("ox-wire Binary Validation (Zero Heap Allocations)", () => {
    return MarketTickCodec.validateBinary(binaryView);
  });

  bench("ox-wire validateAndView + Lazy Access (price, exchange)", () => {
    const view = MarketTickCodec.validateAndView(binaryBuffer);
    return view.price + (view.exchange === "CME" ? 1 : 0);
  });
});

await run();
