//! Zero-allocation buffer and structural tape pool.
//!
//! Provides ring-buffered Uint32Array tapes and Uint8Array buffers
//! to guarantee zero GC pause overhead on hot network request paths.

export class StaticTapePool {
  private readonly _tapes: Uint32Array[];
  private _cursor: number = 0;

  constructor(poolSize: number = 8, tapeCapacity: number = 65536) {
    this._tapes = Array.from({ length: poolSize }, () => new Uint32Array(tapeCapacity));
  }

  /**
   * Acquire a pre-allocated static tape from the ring buffer.
   */
  acquire(): Uint32Array {
    const tape = this._tapes[this._cursor];
    this._cursor = (this._cursor + 1) % this._tapes.length;
    return tape;
  }
}

export class StaticBufferPool {
  private readonly _buffers: Uint8Array[];
  private _cursor: number = 0;

  constructor(poolSize: number = 8, bufferCapacity: number = 65536) {
    this._buffers = Array.from({ length: poolSize }, () => new Uint8Array(bufferCapacity));
  }

  /**
   * Acquire a pre-allocated static byte buffer from the ring buffer.
   */
  acquire(): Uint8Array {
    const buf = this._buffers[this._cursor];
    this._cursor = (this._cursor + 1) % this._buffers.length;
    return buf;
  }
}

export const defaultTapePool = new StaticTapePool();
export const defaultBufferPool = new StaticBufferPool();
