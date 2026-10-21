//! Branded primitives, macro signatures, and WireCodec interfaces.

export type Brand<K extends string> = { readonly __brand: K };

export type u8 = number & Brand<"u8">;
export type u16 = number & Brand<"u16">;
export type u32 = number & Brand<"u32">;
export type u64 = bigint & Brand<"u64">;
export type i8 = number & Brand<"i8">;
export type i16 = number & Brand<"i16">;
export type i32 = number & Brand<"i32">;
export type i64 = bigint & Brand<"i64">;
export type f32 = number & Brand<"f32">;
export type f64 = number & Brand<"f64">;

/**
 * Interface describing an AOT-compiled Zero-Parse Wire Codec.
 */
export interface WireCodec<T> {
  /** Target TypeScript interface / type name */
  readonly name: string;
  /** Size in bytes of the fixed struct header (including alignment padding) */
  readonly fixedHeaderSize: number;
  /** Alignment requirement in bytes */
  readonly alignment: number;

  /**
   * Validates raw binary struct bytes with straight-line bitwise error accumulation.
   * Returns 1 if valid, 0 if invalid.
   */
  validateBinary(view: DataView, baseOffset?: number): 0 | 1;

  /**
   * Validates raw binary bytes and returns a zero-copy Flyweight proxy view,
   * or null if buffer fails validation.
   */
  validateAndView(buffer: ArrayBufferView | ArrayBuffer, baseOffset?: number): T | null;

  /**
   * Encodes a JavaScript object into the packed binary struct format.
   * If `targetBuffer` is provided, writes in-place without heap allocation.
   */
  encodeBinary(obj: any, targetBuffer?: Uint8Array | null, baseOffset?: number): Uint8Array;

  /** Flyweight class constructor */
  readonly Flyweight: new (view: DataView, offset?: number) => T;
}

/**
 * Compile-time macro marking a TypeScript type for AOT validator and binary codec lowering.
 * Transformed at build time by `@ox-wire/unplugin`.
 */
export function compileWireCodec<T>(): WireCodec<T> {
  throw new Error(
    "[@ox-wire] Macro `compileWireCodec<T>()` was not transformed at build time. " +
      "Ensure `@ox-wire/unplugin` is registered in your Vite, Rolldown, esbuild, or Rollup configuration."
  );
}

/** Alias for `compileWireCodec<T>()` */
export function compileCodec<T>(): WireCodec<T> {
  return compileWireCodec<T>();
}

/**
 * Creates a zero-copy Flyweight view over a binary buffer without full object instantiation.
 */
export function createFlyweight<T>(_buffer: ArrayBufferView | ArrayBuffer, _offset?: number): T {
  throw new Error(
    "[@ox-wire] Macro `createFlyweight<T>()` was not transformed at build time."
  );
}
