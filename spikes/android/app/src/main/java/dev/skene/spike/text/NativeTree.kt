package dev.skene.spike.text

import java.nio.ByteBuffer

/** The Rust taffy tree whose text leaves measure through [engine]. */
class NativeTree(val engine: TextEngine, cards: Int) : AutoCloseable {
  val leaves = cards * Corpus.LEAVES_PER_CARD
  private val handle = create(engine, engine.buffer, cards)

  /**
   * Runs one pass at [width] and returns `[nanos, crossings, intrinsics calls, layout calls,
   * most crossings of one leaf, leaves crossing more than once, measure callbacks, leaves
   * crossing at all]`.
   */
  fun layout(width: Float, flags: Int = 0): LongArray {
    val stats = LongArray(8)
    layout(handle, width, flags, stats)
    return stats
  }

  /** Each leaf's absolute x, y, width and height, then the root's width and height. */
  fun rects(): FloatArray {
    val out = FloatArray(leaves * 4 + 2)
    rects(handle, out)
    return out
  }

  override fun close() = destroy(handle)

  companion object {
    const val DIRTY = 1
    const val CLEAR_LAID = 2
    const val CLEAR_INTRINSICS = 4

    init {
      System.loadLibrary("skene_text_spike")
    }

    @JvmStatic private external fun create(engine: TextEngine, buffer: ByteBuffer, cards: Int): Long

    @JvmStatic private external fun layout(handle: Long, width: Float, flags: Int, stats: LongArray)

    @JvmStatic private external fun rects(handle: Long, out: FloatArray)

    @JvmStatic private external fun destroy(handle: Long)
  }
}
