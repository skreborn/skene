package dev.skene.spike.text

import android.content.Context
import android.database.ContentObserver
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Typeface
import android.graphics.fonts.Font
import android.graphics.fonts.FontFamily
import android.graphics.fonts.SystemFonts
import android.graphics.text.LineBreakConfig
import android.graphics.text.TextRunShaper
import android.icu.util.ULocale
import android.os.Build
import android.os.LocaleList
import android.provider.Settings
import android.text.Layout
import android.text.StaticLayout
import android.text.TextPaint
import android.util.Log
import android.util.TypedValue
import android.view.ViewGroup
import android.view.accessibility.AccessibilityManager
import android.widget.TextView
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.io.FileInputStream
import java.util.Locale
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.roundToInt
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The text-parity run on Android: option A (`StaticLayout`), option B (Parley and vello through
 * the shared harness) and option C (Parley's glyph runs drawn by `Canvas`), written under the
 * app's external files directory (`parity/`) for the host to compare, and the probes that need
 * a device: fonts, fallback, breaking, hyphenation, accessibility settings and timings. Each test
 * runs in a process of its own; results go to logcat under [TAG] as `key=value` lines.
 */
@RunWith(AndroidJUnit4::class)
class ParityTest {
  private val instrumentation = InstrumentationRegistry.getInstrumentation()
  private val context: Context = instrumentation.targetContext
  private val arguments = InstrumentationRegistry.getArguments()
  private val runId = arguments.getString("runId") ?: "local"
  private val density = context.resources.displayMetrics.density
  private val sdk = Build.VERSION.SDK_INT
  // The `dir` argument puts a run's files in a directory of their own (a density's sweep).
  private val out = File(File(context.getExternalFilesDir(null), "parity"), arguments.getString("dir") ?: "")
    .apply { mkdirs() }
  private val device = "${Build.MODEL} (${Build.PRODUCT}), density ${density}"

  private fun report(key: String, value: Any?) {
    Log.i(TAG, "RID:$runId $key=${value.toString().replace(' ', '_')}")
  }

  private fun msOf(nanos: Long) = "%.3f".format(nanos / 1e6)

  private fun <T> onMain(block: () -> T): T {
    var result: Result<T>? = null
    instrumentation.runOnMainSync { result = runCatching(block) }
    return result!!.getOrThrow()
  }

  private fun rssKb(): Long = File("/proc/self/status").readLines()
    .firstOrNull { it.startsWith("VmRSS:") }?.split(Regex("\\s+"))?.getOrNull(1)?.toLongOrNull() ?: 0

  private fun write(name: String, json: Any) {
    File(out, name).writeText(if (json is JSONObject) json.toString(1) else json.toString())
  }

  private fun header() {
    report("device.sdk", sdk)
    report("device.density", density)
    report("device.model", Build.MODEL)
  }

  /** The system's fonts four ways: the NDK's iterator, `SystemFonts`, fontique, and the fed source. */
  @Test
  fun fonts() {
    header()
    write("corpus.json", JSONObject(Parity.corpus()))
    val system = JSONObject(Parity.systemFonts())
    write("fonts-ndk.json", system)
    report("fonts.ndk.count", system.getInt("count"))
    report("fonts.ndk.ms", "%.3f".format(system.getDouble("ms")))
    report("fonts.ndk.directories", system.getJSONObject("directories").toString())

    var started = System.nanoTime()
    val available = SystemFonts.getAvailableFonts()
    report("fonts.systemFonts.ms", msOf(System.nanoTime() - started))
    report("fonts.systemFonts.count", available.size)
    report(
      "fonts.systemFonts.directories",
      available.groupingBy { it.file?.parent ?: "buffer" }.eachCount().toSortedMap().toString(),
    )

    val before = rssKb()
    val stock = JSONObject(Parity.stockCollection())
    write("fonts-fontique.json", stock)
    report("fonts.fontique.ms", "%.3f".format(stock.getDouble("ms")))
    report("fonts.fontique.families", stock.getInt("families"))
    report("fonts.fontique.rssDeltaKb", stock.getLong("rss_after_kb") - stock.getLong("rss_before_kb"))
    report("fonts.fontique.rssDeltaKbOuter", rssKb() - before)
    report("fonts.fontique.generics", stock.getJSONObject("generics").toString())
    report("fonts.fontique.namedFonts", stock.getJSONObject("named_fonts").toString())

    started = System.nanoTime()
    val fed = JSONObject(Parity.fedSource())
    report("fonts.fed.outerMs", msOf(System.nanoTime() - started))
    write("fonts-fed.json", fed)
    val source = fed.getJSONObject("source")
    report("fonts.fed.ms", "%.3f".format(fed.getDouble("ms")))
    report("fonts.fed.files", source.getJSONArray("files").length())
    report("fonts.fed.generics", source.getJSONArray("generics").toString())
    report("fonts.fed.fallbacks", source.getJSONArray("fallbacks").length())

    for ((name, text, locales) in listOf(
      Triple("latin", "a", "en-US"),
      Triple("han", "今", "ja-JP"),
      Triple("emoji", "😀", "en-US"),
      Triple("thai", "ภาษาไทย", "th-TH"),
    )) {
      Parity.matchBench(text, locales, 200)
      report("fonts.matcher.$name.nsPerCall", "%.0f".format(Parity.matchBench(text, locales, 5000)))
    }

    val corpus = ParityCorpus.variants.distinctBy { it.case }
    val matched = JSONObject()

    for (variant in corpus) {
      matched.put(
        variant.case,
        JSONArray(Parity.matchRuns("sans-serif", variant.text, variant.locale, variant.weight, variant.italic)),
      )
    }

    write("matcher-runs.json", matched)

    val han = corpus.first { it.case == "han-ja" }.text
    val lists = JSONObject()

    for (locales in listOf(
      "ja-JP,en-US", "en-US,ja-JP", "en-US", "ja-JP", "zh-Hans-CN", "zh-Hant-TW", "zh-Hant-HK", "ko-KR",
      "zh-CN,ja-JP",
    )) {
      val runs = JSONArray(Parity.matchRuns("sans-serif", han, locales, 400, false))
      val first = runs.getJSONObject(0).getJSONObject("font")
      val entry = JSONObject().put("matcher", "${File(first.getString("path")).name}#${first.getInt("index")}")

      if (sdk >= 31) {
        val paint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
          textSize = 17f * density
          textLocales = LocaleList.forLanguageTags(locales)
        }
        val font = TextRunShaper.shapeTextRun(han, 0, han.length, 0, han.length, 0f, 0f, false, paint).getFont(0)
        entry.put("staticlayout", "${font.file?.name}#${font.ttcIndex}")
      }

      lists.put(locales, entry)
      report("fonts.hanLocales.$locales", entry.toString())
    }

    write("han-locale-lists.json", lists)
  }

  private fun nativeSet(quality: Boolean, name: String, subpixel: Boolean = false) {
    header()
    val root = File(out, name).apply { mkdirs() }
    val started = System.nanoTime()
    val set = NativeRecords(quality, density, subpixel).run(root, device)
    report("$name.ms", msOf(System.nanoTime() - started))
    report("$name.records", set.getJSONArray("records").length())
    report("$name.firstLayoutMs", set.getJSONObject("timings").getDouble("first_layout_ms"))
    report("$name.labels", set.getJSONObject("timings").getJSONObject("labels").toString())
  }

  /** Option A, `BREAK_STRATEGY_SIMPLE` without hyphenation. */
  @Test
  fun nativeSimple() = nativeSet(false, "a-simple")

  /** Option A, `BREAK_STRATEGY_HIGH_QUALITY` with `HYPHENATION_FREQUENCY_NORMAL`. */
  @Test
  fun nativeQuality() = nativeSet(true, "a-quality")

  /** Option A simple with `SUBPIXEL_TEXT_FLAG`: Minikin's layout with fractional advances. */
  @Test
  fun nativeSubpixel() = nativeSet(false, "a-subpixel", subpixel = true)

  private fun parleyRun(name: String, fonts: JSONObject, glyphs: Boolean, features: String = "", images: Boolean = true) {
    val config = JSONObject().apply {
      put("out", File(out, name).absolutePath)
      put("scales", JSONArray(listOf(density.toDouble())))
      put("fonts", fonts)
      put("glyphs", glyphs)
      put("features", features)
      put("images", images)
      put("labels", 1000)
      put("repeats", 5)
      put("os_version", "${Build.VERSION.RELEASE} (API $sdk)")
      put("device", device)
    }
    val before = rssKb()
    val started = System.nanoTime()
    val result = Parity.parley(config.toString())
    report("$name.ms", msOf(System.nanoTime() - started))
    report("$name.rssDeltaKb", rssKb() - before)
    report("$name.result", result)
    check(result.startsWith("ok")) { result }
    val set = JSONObject(File(out, "$name/records.json").readText())
    val timings = set.getJSONObject("timings")
    report("$name.fontCollectionMs", timings.optDouble("font_collection_ms"))
    report("$name.firstLayoutMs", timings.optDouble("first_layout_ms"))
    report("$name.labels", timings.optJSONObject("labels")?.toString())
  }

  /** Option B with fonts fed from `AFontMatcher` (the matcher's picks per generic and script). */
  @Test
  fun parleyFed() {
    header()
    val started = System.nanoTime()
    val fed = JSONObject(Parity.fedSource())
    report("b-fed.sourceMs", msOf(System.nanoTime() - started))
    parleyRun("b-fed", fed.getJSONObject("source"), true)
    val plan = Parity.plan(File(out, "b-fed/records.json").absolutePath, File(out, "plan.json").absolutePath)
    report("b-fed.plan", plan)
    check(plan.startsWith("ok")) { plan }
  }

  /** Option B with fontique's own Android backend (`/system/fonts` scanned, `fonts.xml` read). */
  @Test
  fun parleyStock() {
    header()
    parleyRun("b-stock", JSONObject().put("system", true), false)
  }

  /**
   * Option C: Parley's glyph runs drawn by `drawGlyphs` (API 31) and the `drawTextRun` floor,
   * from `plan.json`, or with the `set` argument from `plan-<set>.json` into `c-<set>` and
   * `cfloor-<set>`.
   */
  @Test
  fun raster() {
    header()
    val set = arguments.getString("set")
    val suffix = set?.let { "-$it" } ?: ""
    val raster = Raster(File(out, if (set == null) "plan.json" else "plan-$set.json"))

    if (sdk >= 31) {
      val (images, ms) = raster.drawGlyphs(File(out, "c$suffix"))
      report("c$suffix.images", images)
      report("c$suffix.drawMs", "%.3f".format(ms))
    }

    val (images, ms) = raster.drawTextRun(File(out, "cfloor$suffix"))
    report("cfloor$suffix.images", images)
    report("cfloor$suffix.drawMs", "%.3f".format(ms))

    val shaping = raster.shaping(density)
    write("shaping$suffix.json", shaping)

    for (key in listOf("runs", "runs_compared", "runs_same_glyphs", "max_glyph_drift", "max_run_advance_delta")) {
      report("shaping$suffix.$key", shaping.get(key))
    }
  }

  /**
   * Per-glyph advances as Minikin takes them, for [Parity.advanceAnalysis] to find the rule
   * they follow: each probe font (the matcher's pick, as a one-font typeface) at every size of
   * the probe, a lone character's run advance (`getRunAdvance`, which `measureText` rounds up;
   * a script whose lone character is its cmap glyph) and each glyph by `Font.getGlyphBounds`
   * (API 31), both with and without `LINEAR_TEXT_FLAG`; kerning pairs against their parts; and
   * the CJK font's punctuation pairs with `chws` on and off. Writes `advances.json`: the
   * measurements and the analysis.
   */
  @Test
  fun advances() {
    header()
    val probe = JSONObject(Parity.advanceProbe())
    val sizes = probe.getJSONArray("sizes_dp")
    val fonts = probe.getJSONArray("fonts")
    val measuredFonts = JSONArray()
    val bounds = RectF()

    for (f in 0 until fonts.length()) {
      val spec = fonts.getJSONObject(f)
      val font = Font.Builder(File(spec.getString("path"))).setTtcIndex(spec.getInt("index")).apply {
        val axes = spec.getString("axes")
        if (axes.isNotEmpty()) setFontVariationSettings(axes)
      }.build()
      val typeface = Typeface.CustomFallbackBuilder(FontFamily.Builder(font).build()).build()
      val locale = spec.getString("locale").split(',')[0]

      fun paint(px: Float, linear: Boolean, features: String? = null) = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        this.typeface = typeface
        textSize = px
        isLinearText = linear
        textLocales = LocaleList.forLanguageTags(locale)
        if (features != null) fontFeatureSettings = features
      }

      fun advance(paint: TextPaint, text: String) =
        paint.getRunAdvance(text, 0, text.length, 0, text.length, false, text.length).toDouble()

      fun perSize(measure: (Float) -> Double?) =
        JSONArray(List(sizes.length()) { measure(sizes.getDouble(it).toFloat() * density) ?: JSONObject.NULL })

      val simple = spec.getBoolean("simple")
      val glyphs = spec.getJSONArray("glyphs")
      val glyphsOut = JSONArray()

      for (g in 0 until glyphs.length()) {
        val glyph = glyphs.getJSONObject(g)
        val text = glyph.getString("char")
        val id = glyph.getInt("glyph")
        // A lone character's run is its cmap glyph alone where the font says so, or where Minikin
        // shapes it to that glyph (API 31).
        val alone = simple || (sdk >= 31 && TextRunShaper.shapeTextRun(text, 0, text.length, 0, text.length, 0f, 0f, false, paint(17f * density, false))
          .let { shaped -> shaped.glyphCount() == 1 && shaped.getGlyphId(0) == id })
        glyphsOut.put(JSONObject().apply {
          put("char", text)
          put("glyph", id)
          put("alone", alone)
          put("hinted", perSize { px -> if (alone) advance(paint(px, false), text) else null })
          put("linear", perSize { px -> if (alone) advance(paint(px, true), text) else null })

          if (sdk >= 31) {
            put("bounds", perSize { px -> font.getGlyphBounds(id, paint(px, false), bounds).toDouble() })
            put("bounds_linear", perSize { px -> font.getGlyphBounds(id, paint(px, true), bounds).toDouble() })
          }
        })
      }

      fun kern(text: String, linear: Boolean, px: Float): Double {
        val paint = paint(px, linear)
        return advance(paint, text) - advance(paint, text.substring(0, 1)) - advance(paint, text.substring(1))
      }

      val pairs = spec.getJSONArray("pairs")
      val pairsOut = JSONArray()

      for (p in 0 until pairs.length()) {
        val text = pairs.getString(p)
        pairsOut.put(JSONObject().apply {
          put("text", text)
          put("pair_hinted", perSize { px -> advance(paint(px, false), text) })
          put("kern_hinted", perSize { px -> kern(text, false, px) })
          put("kern_linear", perSize { px -> kern(text, true, px) })
        })
      }

      val chws = spec.optJSONArray("chws") ?: JSONArray()
      val chwsOut = JSONArray()

      for (p in 0 until chws.length()) {
        val text = chws.getString(p)

        fun width(px: Float, linear: Boolean, features: String?) = advance(paint(px, linear, features), text)

        chwsOut.put(JSONObject().apply {
          put("text", text)
          put("hinted_default", perSize { px -> width(px, false, null) })
          put("hinted_off", perSize { px -> width(px, false, "'chws' 0") })
          put("hinted_on", perSize { px -> width(px, false, "'chws' 1") })
          put("linear_default", perSize { px -> width(px, true, null) })
          put("linear_off", perSize { px -> width(px, true, "'chws' 0") })
          put("linear_on", perSize { px -> width(px, true, "'chws' 1") })
        })
      }

      measuredFonts.put(JSONObject(spec.toString()).put("glyphs", glyphsOut).put("pairs", pairsOut).put("chws", chwsOut))
    }

    val measured = JSONObject().put("density", density.toDouble()).put("sdk", sdk).put("sizes_dp", sizes).put("fonts", measuredFonts)
    val analysis = JSONObject(Parity.advanceAnalysis(measured.toString()))
    write("advances.json", JSONObject().put("measured", measured).put("analysis", analysis))
    report("advances.best", analysis.optString("best"))
    report("advances.totals", analysis.optJSONObject("totals")?.toString())
    val analysed = analysis.optJSONArray("fonts") ?: JSONArray()

    for (f in 0 until analysed.length()) {
      val font = analysed.getJSONObject(f)
      val name = font.getString("name")
      val rules = font.getJSONObject("rules")
      report("advances.$name.font", "${File(font.getString("path")).name}#${font.getInt("index")} ${font.optString("axes")} instructions=${font.optBoolean("has_instructions")}")
      report("advances.$name.best", font.optString("best"))
      report("advances.$name.linearAgrees", font.optJSONObject("linear_agrees")?.toString())
      report("advances.$name.kerningLinear", font.optJSONObject("kerning_linear")?.toString())
      report("advances.$name.rules", rules.keys().asSequence().joinToString(",") {
        "$it=${rules.getJSONObject(it).getInt("matched")}/${rules.getJSONObject(it).getInt("of")}"
      })
    }
  }

  /**
   * The rule the density's advance probe found best for glyphs and kerning pairs, `hinted` where
   * it has not run.
   */
  private fun bestRule(): String = try {
    val analysis = JSONObject(File(out, "advances.json").readText()).getJSONObject("analysis")
    analysis.optString("best_with_kerning").ifEmpty { analysis.optString("best") }.ifEmpty { "hinted" }
  } catch (error: Exception) {
    "hinted"
  }

  /**
   * Option B under an advance rule, fed from `AFontMatcher`: the `set` argument names it
   * (`linear`: Parley's own advances, as `parleyFed`; `hinted`: the `rule` argument, or the
   * density's probe's best, with `chws` on from API 34 as Minikin sets it; `rule`: the same rule
   * without `chws`, records only), written to `b-<set>` with `plan-<set>.json` for [raster].
   */
  @Test
  fun parleyRule() {
    header()
    val set = arguments.getString("set") ?: "hinted"
    val rule = arguments.getString("rule") ?: if (set == "linear") "linear" else bestRule()
    val installed = Parity.setAdvanceRule(rule, density)
    report("b-$set.rule", installed)
    check(!installed.startsWith("error")) { installed }
    val chws = set == "hinted" && sdk >= 34
    report("b-$set.chws", chws)
    val fed = JSONObject(Parity.fedSource())
    parleyRun("b-$set", fed.getJSONObject("source"), true, if (chws) "\"chws\" 1" else "", images = set != "rule")
    val plan = Parity.plan(File(out, "b-$set/records.json").absolutePath, File(out, "plan-$set.json").absolutePath)
    report("b-$set.plan", plan)
    check(plan.startsWith("ok")) { plan }
  }

  private fun lineStarts(layout: Layout) = JSONArray((0 until layout.lineCount).map { layout.getLineStart(it) })

  /** The probes: hyphenation data and positions, complex-script and phrase breaking. */
  @Test
  fun probes() {
    header()
    val probes = JSONObject()
    val hyphenData = JSONObject()
    val directory = File("/system/usr/hyphen-data")
    hyphenData.put("listable", directory.list()?.size ?: -1)

    for (name in listOf("hyph-en-us.hyb", "hyph-de-1996.hyb")) {
      val file = File(directory, name)
      val entry = JSONObject().put("exists", file.exists()).put("canRead", file.canRead()).put("size", file.length())

      try {
        val bytes = ByteArray(16)
        val read = FileInputStream(file).use { it.read(bytes) }
        entry.put("first", bytes.take(read).joinToString("") { "%02x".format(it) })
      } catch (error: Exception) {
        entry.put("error", error.toString())
      }

      hyphenData.put(name, entry)
      report("probes.hyphenData.$name", entry.toString())
    }

    probes.put("hyphen_data", hyphenData)

    val corpus = ParityCorpus.variants
    val records = NativeRecords(false, density)
    val hyphenation = JSONObject()

    for (case in listOf("hyphenation-en", "hyphenation-de")) {
      val variant = corpus.first { it.case == case }
      val modes = JSONObject()

      for ((mode, strategy, frequency) in listOf(
        Triple("simple_none", Layout.BREAK_STRATEGY_SIMPLE, Layout.HYPHENATION_FREQUENCY_NONE),
        Triple("simple_normal", Layout.BREAK_STRATEGY_SIMPLE, Layout.HYPHENATION_FREQUENCY_NORMAL),
        Triple("hq_normal", Layout.BREAK_STRATEGY_HIGH_QUALITY, Layout.HYPHENATION_FREQUENCY_NORMAL),
        Triple("hq_full", Layout.BREAK_STRATEGY_HIGH_QUALITY, Layout.HYPHENATION_FREQUENCY_FULL),
      )) {
        val widths = JSONObject()

        for (width in listOf(80, 100, 110, 130, 160, 180)) {
          val paint = records.paint(variant, 17f * density)
          val layout = StaticLayout.Builder.obtain(variant.text, 0, variant.text.length, paint, (width * density).roundToInt())
            .setIncludePad(false)
            .setUseLineSpacingFromFallbacks(true)
            .setBreakStrategy(strategy)
            .setHyphenationFrequency(frequency)
            .build()
          widths.put(width.toString(), JSONArray((0 until layout.lineCount).map {
            variant.text.substring(layout.getLineStart(it), layout.getLineEnd(it)).trimEnd() +
              if (drewHyphen(layout, it, variant.text, paint)) "-" else ""
          }))
        }

        modes.put(mode, widths)
      }

      hyphenation.put(case, modes)
    }

    probes.put("hyphenation", hyphenation)
    report("probes.hyphenation.en.hq_normal.100", hyphenation.getJSONObject("hyphenation-en").getJSONObject("hq_normal").get("100"))

    val breaks = JSONObject()
    report("probes.plainParley.complex", PlainParley.complex())

    for (case in listOf("thai", "khmer", "lao", "burmese", "zh-hans-paragraph", "ja-kinsoku", "ja-phrase", "ko-keep-all")) {
      val variant = corpus.first { it.case == case }
      val text = variant.text
      val entry = JSONObject()
      val icu = android.icu.text.BreakIterator.getLineInstance(ULocale.forLanguageTag(variant.locale))
      icu.setText(text)
      entry.put("icu", JSONArray(generateSequence(icu.first()) { icu.next().takeIf { it != android.icu.text.BreakIterator.DONE } }.toList()))

      if (case == "ja-phrase") {
        val phrase = android.icu.text.BreakIterator.getLineInstance(ULocale("ja@lw=phrase"))
        phrase.setText(text)
        entry.put("icu_phrase", JSONArray(generateSequence(phrase.first()) { phrase.next().takeIf { it != android.icu.text.BreakIterator.DONE } }.toList()))
      }

      val plain = JSONObject()
      val native = JSONObject()

      for (width in corpus.filter { it.case == case }.mapNotNull { it.width }) {
        plain.put(width.roundToInt().toString(), PlainParley.lines(text, variant.locale, 17f, width)?.let { JSONArray(it.toList()) } ?: JSONObject.NULL)

        if (case == "ja-phrase" && sdk >= 33) {
          for ((style, word) in listOf("none" to LineBreakConfig.LINE_BREAK_WORD_STYLE_NONE, "phrase" to LineBreakConfig.LINE_BREAK_WORD_STYLE_PHRASE)) {
            val paint = records.paint(variant, 17f * density)
            val layout = StaticLayout.Builder.obtain(text, 0, text.length, paint, (width * density).roundToInt())
              .setIncludePad(false)
              .setUseLineSpacingFromFallbacks(true)
              .setLineBreakConfig(LineBreakConfig.Builder().setLineBreakWordStyle(word).build())
              .build()
            native.put("${width.roundToInt()}_$style", lineStarts(layout))
          }
        }
      }

      entry.put("plain_parley", plain)
      if (native.length() > 0) entry.put("staticlayout_word_style", native)
      breaks.put(case, entry)
      report("probes.breaks.$case", entry.toString())
    }

    probes.put("breaks", breaks)

    // One line's advance as TextView measures it (whole-pixel advances), with SUBPIXEL_TEXT_FLAG,
    // and with the contextual half-width spacing feature turned off.
    val advances = JSONObject()

    for (id in listOf(
      "latin-kerning/17/max", "latin-paragraph/17/max", "sizes-latin/11/max", "sizes-latin/17/max",
      "sizes-latin/64/max", "han-zh-hans/17/max", "han-ja/17/max", "sizes-cjk/17/max", "thai/17/160", "arabic/17/max",
    )) {
      val variant = corpus.first { it.id == id }
      val text = variant.text
      fun measure(configure: TextPaint.() -> Unit) =
        records.paint(variant, variant.size * density).apply(configure).measureText(text) / density
      val entry = JSONObject().apply {
        put("default", measure {}.toDouble())
        put("subpixel", measure { isSubpixelText = true }.toDouble())
        put("chws_off", measure { fontFeatureSettings = "'chws' 0" }.toDouble())
        put("subpixel_chws_off", measure { isSubpixelText = true; fontFeatureSettings = "'chws' 0" }.toDouble())
        put("linear", measure { isLinearText = true }.toDouble())
      }
      advances.put(id, entry)
      report("probes.advances.$id", entry.toString())
    }

    probes.put("advances", advances)
    write("probes.json", probes)
  }

  /** The font scale in force: the platform's sp to dp against Skene's port of the curve. */
  @Test
  fun fontScale() {
    header()
    val metrics = context.resources.displayMetrics
    val scale = context.resources.configuration.fontScale
    report("fontScale.configuration", scale)
    val sizes = listOf(8f, 10f, 11f, 12f, 13f, 14f, 15f, 16f, 17f, 18f, 20f, 22f, 24f, 28f, 30f, 34f, 40f, 48f, 64f, 100f)
    val table = JSONArray()
    var worstSkene = 0f
    var worstLinear = 0f

    for (sp in sizes) {
      val platform = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, sp, metrics) / metrics.density
      val skene = Parity.spToDp(sp, scale, sdk)
      val linear = sp * scale
      worstSkene = max(worstSkene, abs(platform - skene))
      worstLinear = max(worstLinear, abs(platform - linear))
      table.put(JSONObject().put("sp", sp.toDouble()).put("platform_dp", platform.toDouble()).put("skene_dp", skene.toDouble()).put("linear_dp", linear.toDouble()))
    }

    report("fontScale.table", table.toString())
    report("fontScale.maxSkeneVsPlatformDp", worstSkene)
    report("fontScale.maxLinearVsPlatformDp", worstLinear)
    write("font-scale-${(scale * 100).roundToInt()}.json", JSONObject().put("scale", scale.toDouble()).put("sdk", sdk).put("table", table))
  }

  /** Bold text: the configuration's weight adjustment, and who applies it. */
  @Test
  fun fontWeight() {
    header()
    val resolver = context.contentResolver
    report("fontWeight.secure", try { Settings.Secure.getInt(resolver, "font_weight_adjustment", Int.MIN_VALUE) } catch (error: Exception) { error.toString() })

    if (sdk >= 31) report("fontWeight.configuration", context.resources.configuration.fontWeightAdjustment)

    onMain {
      val view = TextView(context)
      report("fontWeight.textViewTypefaceWeight", view.typeface?.weight)
      report("fontWeight.textViewPaintWeight", view.paint.typeface?.weight)
      report("fontWeight.plainPaintWeight", TextPaint().typeface?.weight ?: android.graphics.Typeface.DEFAULT.weight)
    }
  }

  /**
   * The band's pixels against its own ground (the commonest colour): `dark` below half the
   * ground's luminance (text forced to black), `light` brighter than the ground (an outline),
   * `ink` any pixel off the ground. A dimmed screen scales every pixel alike, so the classes hold.
   */
  private fun counts(bitmap: Bitmap, left: Int, top: Int, width: Int, height: Int): JSONObject {
    val pixels = ArrayList<Int>()

    for (y in top until minOf(top + height, bitmap.height)) {
      for (x in left until minOf(left + width, bitmap.width)) {
        val pixel = bitmap.getPixel(x, y)
        pixels.add((Color.red(pixel) * 299 + Color.green(pixel) * 587 + Color.blue(pixel) * 114) / 1000)
      }
    }

    val ground = pixels.groupingBy { it }.eachCount().maxByOrNull { it.value }?.key ?: 0
    var dark = 0
    var light = 0
    var ink = 0
    var darkest = 255

    for (luma in pixels) {
      if (abs(luma - ground) > 8) ink++
      if (luma * 2 < ground) dark++
      if (luma > ground + 12) light++
      darkest = minOf(darkest, luma)
    }

    return JSONObject().put("ground", ground).put("ink", ink).put("dark", dark).put("light", light)
      .put("darkest", darkest).put("pixels", pixels.size)
  }

  /** High-contrast text: whether the setting reads, and which drawing path the renderer alters. */
  @Test
  fun contrast() {
    header()
    val state = arguments.getString("state") ?: "unknown"
    val resolver = context.contentResolver
    report("contrast.state", state)
    report("contrast.secure", try { Settings.Secure.getInt(resolver, "high_text_contrast_enabled", -1).toString() } catch (error: Exception) { error.toString() })
    report("contrast.observer", try {
      resolver.registerContentObserver(Settings.Secure.getUriFor("high_text_contrast_enabled"), false, object : ContentObserver(null) {})
      "registered"
    } catch (error: Exception) {
      error.toString()
    })
    val manager = context.getSystemService(AccessibilityManager::class.java)
    report("contrast.hiddenMethod", try {
      AccessibilityManager::class.java.getMethod("isHighTextContrastEnabled").invoke(manager)!!.toString()
    } catch (error: Throwable) {
      error.javaClass.simpleName
    })

    val scenario = ActivityScenario.launch(SpikeActivity::class.java)
    var view: ContrastView? = null
    scenario.onActivity { activity ->
      view = ContrastView(activity)
      activity.setContentView(view, ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ContrastView.height()))
    }
    instrumentation.waitForIdleSync()
    Thread.sleep(2000)
    val location = IntArray(2)
    val (width, height) = onMain {
      view!!.getLocationOnScreen(location)
      view.width to view.height
    }
    report("contrast.hardwareAccelerated", onMain { view!!.isHardwareAccelerated })
    // The screenshot may be a hardware bitmap, whose pixels cannot be read until copied.
    val shot = instrumentation.uiAutomation.takeScreenshot()?.copy(Bitmap.Config.ARGB_8888, false)
    val result = JSONObject().put("state", state).put("sdk", sdk)

    if (shot != null) {
      for (band in 0 until 3) {
        val numbers = counts(shot, location[0], location[1] + band * ContrastView.BAND, width, ContrastView.BAND)
        result.put("screen_band$band", numbers)
        report("contrast.screen.band$band", numbers.toString())
      }

      val crop = Bitmap.createBitmap(shot, location[0], location[1], minOf(width, shot.width - location[0]), minOf(height, shot.height - location[1]))
      writePng(crop, File(out, "contrast-$state.png"))
    } else {
      report("contrast.screen", "no screenshot")
    }

    val software = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
    onMain { view!!.draw(Canvas(software)) }

    for (band in 0 until 3) {
      val numbers = counts(software, 0, band * ContrastView.BAND, width, ContrastView.BAND)
      result.put("software_band$band", numbers)
      report("contrast.software.band$band", numbers.toString())
    }

    write("contrast-$state.json", result)
    scenario.close()
  }

  private fun passes(prefix: String, name: String, runs: List<LongArray>) {
    val nanos = runs.map { it[0] }.sorted()
    report("$prefix.$name.ms", msOf(nanos[nanos.size / 2]))
    report("$prefix.$name.runs", runs.size)
    report("$prefix.$name.measureCallbacks", runs.maxOf { it[6] })
    report("$prefix.$name.engineCalls", runs.maxOf { it[1] })
  }

  /**
   * The spike's 1000-leaf taffy tree with Parley measuring in process (fonts fed from the
   * matcher), timed as the JNI path is, and a 60-frame font-size animation of one leaf in Parley
   * and in `StaticLayout`.
   */
  @Test
  fun taffyParley() {
    header()
    val corpus = Corpus(CARDS)
    val texts = Array(corpus.leaves) { corpus.text(it) }
    val sizes = FloatArray(corpus.leaves) { Corpus.SIZES[corpus.style(it)] }
    var started = System.nanoTime()
    val fed = JSONObject(Parity.fedSource())
    report("taffyParley.fedSourceMs", msOf(System.nanoTime() - started))
    val fonts = fed.getJSONObject("source").toString()
    val prefix = "taffyParley"

    onMain {
      started = System.nanoTime()
      val tree = ParleyTree(texts, sizes, fonts, CARDS)
      report("$prefix.create.ms", msOf(System.nanoTime() - started))
      report("$prefix.fonts.ms", "%.3f".format(tree.fontsMs))

      fun passes(name: String, count: Int, width: (Int) -> Float, flags: Int = 0) =
        passes(prefix, name, List(count) { tree.layout(width(it), flags) })

      passes("cold", 1, { W1 })
      passes("warmSameWidth", 5, { W1 }, NativeTree.DIRTY)
      passes("widthChange", 1, { W2 })
      passes("backToFirstWidth", 1, { W1 })
      passes("alternate", 10, { if (it % 2 == 0) W2 else W1 })
      passes("newWidth", 1, { W3 })
      passes("relayoutNoCache", 5, { W1 }, NativeTree.DIRTY or NativeTree.CLEAR_LAID or NativeTree.CLEAR_INTRINSICS)
      passes("reshape", 3, { W1 }, NativeTree.DIRTY or NativeTree.CLEAR_LAID or NativeTree.CLEAR_INTRINSICS or ParleyTree.RESHAPE)
      tree.layout(W1)
      val rects = tree.rects()
      report("$prefix.rootHeightAtW1", rects[rects.size - 1])

      val width = W1 - 32f
      val (frames, height) = tree.animate(0, 34f, 68f, width, FRAMES)
      val parley = frames.sorted()
      report("$prefix.animate.parley.medianMs", msOf(parley[FRAMES / 2]))
      report("$prefix.animate.parley.maxMs", msOf(parley.last()))
      report("$prefix.animate.parley.totalMs", msOf(parley.sum()))
      report("$prefix.animate.parley.lastHeight", height)
      tree.close()

      val text = corpus.text(0)
      val native = List(FRAMES) { frame ->
        val begin = System.nanoTime()
        val paint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply { textSize = 34f + 34f * frame / (FRAMES - 1) }
        StaticLayout.Builder.obtain(text, 0, text.length, paint, width.toInt())
          .setIncludePad(false)
          .setUseLineSpacingFromFallbacks(true)
          .setBreakStrategy(Layout.BREAK_STRATEGY_SIMPLE)
          .setHyphenationFrequency(Layout.HYPHENATION_FREQUENCY_NONE)
          .build()
        System.nanoTime() - begin
      }.sorted()
      report("$prefix.animate.staticLayout.medianMs", msOf(native[FRAMES / 2]))
      report("$prefix.animate.staticLayout.maxMs", msOf(native.last()))
      report("$prefix.animate.staticLayout.totalMs", msOf(native.sum()))
    }
  }

  companion object {
    const val TAG = "SKENE_SPIKE"
    const val CARDS = 100
    const val W1 = 1080f
    const val W2 = 720f
    const val W3 = 900f
    const val FRAMES = 60
  }
}
