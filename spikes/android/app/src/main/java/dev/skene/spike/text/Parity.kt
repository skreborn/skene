package dev.skene.spike.text

/** The Rust side of the text-parity run: the corpus, the NDK's fonts, Parley through the harness. */
object Parity {
  init {
    System.loadLibrary("skene_text_spike")
  }

  /** The corpus with every variant spelled out (`text-parity corpus --expanded`). */
  @JvmStatic external fun corpus(): String

  /** Every font `ASystemFontIterator` lists, as JSON with the time taken. */
  @JvmStatic external fun systemFonts(): String

  /** The runs `AFontMatcher` makes of [text] in [family] under the comma-separated [locales]. */
  @JvmStatic external fun matchRuns(family: String, text: String, locales: String, weight: Int, italic: Boolean): String

  /** Nanoseconds per `AFontMatcher_match` call for [text], over [count] calls. */
  @JvmStatic external fun matchBench(text: String, locales: String, count: Int): Double

  /** The family and PostScript names of face [index] of the file at [path], a newline between. */
  @JvmStatic external fun fontNames(path: String, index: Int): String

  /** A font source fed from `AFontMatcher`, with each question's answer and the time taken. */
  @JvmStatic external fun fedSource(): String

  /** fontique's own Android collection: time, memory, families, generics and fallbacks. */
  @JvmStatic external fun stockCollection(): String

  /** One harness run of Parley, configured as `run::parley_json` takes it. */
  @JvmStatic external fun parley(config: String): String

  /** Writes the option C plan (glyph runs, fonts' axes in user space) from Parley's records. */
  @JvmStatic external fun plan(records: String, out: String): String

  /**
   * Sets Parley's glyph advances to the advance rule named [rule] at [density] device pixels per
   * dp (`linear` restores Parley's own): the rule's name, or `error: ` and why.
   */
  @JvmStatic external fun setAdvanceRule(rule: String, density: Float): String

  /** The fonts, glyphs and pairs the advance probe measures, as JSON. */
  @JvmStatic external fun advanceProbe(): String

  /** Which advance rule [measured] (the probe's fonts measured at each size) follows, as JSON. */
  @JvmStatic external fun advanceAnalysis(measured: String): String

  /** Skene's port of the platform's font scale: dp for [sp] at [scale] on API level [sdk]. */
  @JvmStatic external fun spToDp(sp: Float, scale: Float, sdk: Int): Float

  private val names = HashMap<String, Pair<String, String>>()

  /** The family and PostScript names of a font file's face, cached. */
  fun names(path: String, index: Int): Pair<String, String> = names.getOrPut("$path#$index") {
    val both = fontNames(path, index).split('\n')
    if (both.size == 2) both[0] to both[1] else "" to ""
  }
}

/** Parley without its complex-script dictionaries, as an app would ship it. */
object PlainParley {
  init {
    System.loadLibrary("skene_plain_parley")
  }

  /** Whether the dictionaries are compiled in. */
  @JvmStatic external fun complex(): Boolean

  /** The UTF-16 start of every line of [text] in [locale] at [size] broken at [width]. */
  @JvmStatic external fun lines(text: String, locale: String, size: Float, width: Float): IntArray?
}

/** The spike's taffy tree with Parley measuring its leaves in process. */
class ParleyTree(texts: Array<String>, sizes: FloatArray, fonts: String, cards: Int) : AutoCloseable {
  val leaves = cards * Corpus.LEAVES_PER_CARD
  private val handle = create(texts, sizes, fonts, cards)

  init {
    check(handle != 0L) { "Parley's fonts failed" }
  }

  /** How long the font collection took to make. */
  val fontsMs: Double get() = fontsMs(handle)

  /** One pass at [width], answering the same numbers as [NativeTree.layout]. */
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

  /** Each frame's nanoseconds of leaf [text] animating its size from [from] to [to] at [width]. */
  fun animate(text: Int, from: Float, to: Float, width: Float, frames: Int): Pair<LongArray, Float> {
    val out = LongArray(frames)
    val height = animate(handle, text, from, to, width, out)
    return out to height
  }

  override fun close() = destroy(handle)

  companion object {
    const val RESHAPE = 8

    init {
      System.loadLibrary("skene_text_spike")
    }

    @JvmStatic private external fun create(texts: Array<String>, sizes: FloatArray, fonts: String, cards: Int): Long

    @JvmStatic private external fun fontsMs(handle: Long): Double

    @JvmStatic private external fun layout(handle: Long, width: Float, flags: Int, stats: LongArray)

    @JvmStatic private external fun rects(handle: Long, out: FloatArray)

    @JvmStatic private external fun animate(
      handle: Long,
      text: Int,
      from: Float,
      to: Float,
      width: Float,
      out: LongArray,
    ): Float

    @JvmStatic private external fun destroy(handle: Long)
  }
}
