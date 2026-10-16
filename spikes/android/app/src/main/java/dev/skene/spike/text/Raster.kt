package dev.skene.spike.text

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface
import android.graphics.fonts.Font
import android.graphics.fonts.FontFamily
import android.graphics.text.TextRunShaper
import android.os.Build
import android.os.LocaleList
import java.io.File
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.tan
import org.json.JSONArray
import org.json.JSONObject

/** One glyph run of the option C plan: Parley's glyphs in one font, with the text they draw. */
class PlanRun(o: JSONObject) {
  val line = o.getInt("line")
  val path: String? = if (o.isNull("path")) null else o.getString("path")
  val index = o.getInt("index")
  val size = o.getDouble("size").toFloat()
  val axes: String = o.getJSONArray("axes").let { array ->
    (0 until array.length()).joinToString(",") {
      val axis = array.getJSONArray(it)
      "'${axis.getString(0)}' ${axis.getDouble(1)}"
    }
  }
  val embolden = o.getBoolean("embolden")
  val skew: Float? = if (o.isNull("skew")) null else o.getDouble("skew").toFloat()
  val rtl = o.getBoolean("rtl")
  val text: IntArray? = if (o.isNull("text")) null else o.getJSONArray("text").let { intArrayOf(it.getInt(0), it.getInt(1)) }
  val left = o.getDouble("left").toFloat()
  val advance = o.getDouble("advance").toFloat()
  val baseline = o.getDouble("baseline").toFloat()
  val glyphs: IntArray = o.getJSONArray("glyphs").let { array -> IntArray(array.length()) { array.getInt(it) } }
  val xs: FloatArray = o.getJSONArray("xs").let { array -> FloatArray(array.length()) { array.getDouble(it).toFloat() } }
  val ys: FloatArray = o.getJSONArray("ys").let { array -> FloatArray(array.length()) { array.getDouble(it).toFloat() } }
}

/**
 * Option C on Android: Parley's glyph runs drawn by the platform's rasterizer.
 *
 * - [drawGlyphs]: `Canvas.drawGlyphs` (API 31), a `Font` built from the same file, face and axes,
 *   synthetic bold and slant as `Paint` fakes them. Only the positions are Parley's.
 * - [drawTextRun]: the floor below API 31, each run's text drawn with `Canvas.drawTextRun`
 *   through a `Typeface` holding only the run's font, so Minikin shapes it again in that font.
 *
 * Both write images named as Parley's, so `text-parity restamp` puts Parley's records over them.
 * [shaping] compares, per run, the glyphs and advances Minikin makes in Parley's font with
 * Parley's own.
 */
class Raster(planFile: File) {
  private val plan = JSONObject(planFile.readText())
  private val fonts = HashMap<String, Font>()
  private val typefaces = HashMap<String, Typeface>()
  private val variants = ParityCorpus.variants.associateBy { it.id }

  private fun font(run: PlanRun): Font? {
    val path = run.path ?: return null
    return fonts.getOrPut("$path#${run.index}[${run.axes}]") {
      Font.Builder(File(path)).setTtcIndex(run.index).apply {
        if (run.axes.isNotEmpty()) setFontVariationSettings(run.axes)
      }.build()
    }
  }

  private fun typeface(run: PlanRun): Typeface? {
    val font = font(run) ?: return null
    return typefaces.getOrPut("${run.path}#${run.index}[${run.axes}]") {
      Typeface.CustomFallbackBuilder(FontFamily.Builder(font).build()).build()
    }
  }

  private fun paint(run: PlanRun, scale: Float): Paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
    color = Color.BLACK
    textSize = run.size * scale
    isFakeBoldText = run.embolden
    textSkewX = run.skew?.let { -tan(Math.toRadians(it.toDouble())).toFloat() } ?: 0f
  }

  private fun forEachVariant(out: File, draw: (Canvas, JSONObject, Float) -> Unit): Pair<Int, Double> {
    val list = plan.getJSONArray("variants")
    var drawn = 0
    var nanos = 0L

    for (at in 0 until list.length()) {
      val variant = list.getJSONObject(at)
      val images = variant.getJSONObject("images")

      for (key in images.keys()) {
        val scale = key.toFloat()
        val (wide, high) = imageSize(variant.getDouble("width").toFloat(), variant.getDouble("height").toFloat(), scale)
        val bitmap = Bitmap.createBitmap(wide, high, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(Color.WHITE)
        val canvas = Canvas(bitmap)
        val started = System.nanoTime()
        draw(canvas, variant, scale)
        nanos += System.nanoTime() - started
        writePng(bitmap, File(out, images.getString(key)))
        bitmap.recycle()
        drawn++
      }
    }

    return drawn to ms(nanos)
  }

  private fun runs(variant: JSONObject): List<PlanRun> =
    variant.getJSONArray("runs").let { array -> List(array.length()) { PlanRun(array.getJSONObject(it)) } }

  /** Draws every variant's runs with `Canvas.drawGlyphs` into [out]: images drawn and ms spent. */
  fun drawGlyphs(out: File): Pair<Int, Double> {
    check(Build.VERSION.SDK_INT >= 31) { "Canvas.drawGlyphs needs API 31" }

    return forEachVariant(out) { canvas, variant, scale ->
      for (run in runs(variant)) {
        val font = font(run) ?: continue
        val positions = FloatArray(run.glyphs.size * 2)

        for (i in run.glyphs.indices) {
          positions[2 * i] = (8f + run.xs[i]) * scale
          positions[2 * i + 1] = (8f + run.ys[i]) * scale
        }

        canvas.drawGlyphs(run.glyphs, 0, positions, 0, run.glyphs.size, font, paint(run, scale))
      }
    }
  }

  /**
   * Draws every variant's text runs with `Canvas.drawTextRun` in a single-font typeface into
   * [out]; a hyphen or an ellipsis Parley's caller added is drawn with `drawGlyphs` where there
   * is one (API 31) and left out below.
   */
  fun drawTextRun(out: File): Pair<Int, Double> = forEachVariant(out) { canvas, variant, scale ->
    val case = variants.getValue(variant.getString("variant"))

    for (run in runs(variant)) {
      val range = run.text

      if (range == null) {
        val font = font(run) ?: continue

        if (Build.VERSION.SDK_INT >= 31) {
          val positions = FloatArray(run.glyphs.size * 2) { if (it % 2 == 0) (8f + run.xs[it / 2]) * scale else (8f + run.ys[it / 2]) * scale }
          canvas.drawGlyphs(run.glyphs, 0, positions, 0, run.glyphs.size, font, paint(run, scale))
        }

        continue
      }

      val paint = paint(run, scale).apply {
        typeface = typeface(run) ?: return@apply
        textLocales = LocaleList.forLanguageTags(case.locale)
      }
      canvas.drawTextRun(
        case.text, range[0], range[1], range[0], range[1],
        (8f + run.left) * scale, (8f + run.baseline) * scale, run.rtl, paint,
      )
    }
  }

  /**
   * For every text run, Minikin's shaping of the run's text in Parley's font at the device's
   * [scale] against Parley's: glyph ids (API 31) and advances (every level), per variant. Runs of
   * blanks and line breaks alone, and justified variants (whose spaces Parley stretches), are
   * reported apart from the totals; [DETAILED] variants keep every glyph's position from both.
   */
  fun shaping(scale: Float): JSONObject {
    val list = plan.getJSONArray("variants")
    val out = JSONArray()
    var runsTotal = 0
    var idsCompared = 0
    var idsEqual = 0
    var worstDrift = 0f
    var worstAdvance = 0f
    val excluded = JSONArray()

    for (at in 0 until list.length()) {
      val variant = list.getJSONObject(at)
      val id = variant.getString("variant")
      val case = variants.getValue(id)
      val perLine = HashMap<Int, Float>()
      val worstLine = HashMap<Int, Float>()
      var variantDrift = 0f
      var variantIds = 0
      var variantEqual = 0
      val detail = JSONArray()
      val different = JSONArray()

      for (run in runs(variant)) {
        val range = run.text ?: continue
        val runText = case.text.substring(range[0], range[1])
        val blank = runText.all { it.isWhitespace() || it == '\u00AD' }

        if (blank || case.alignment == "justify") {
          excluded.put("$id ${if (blank) "blank run" else "justified"} ${range.toList()}")
          continue
        }

        val paint = paint(run, scale).apply {
          typeface = typeface(run) ?: return@apply
          textLocales = LocaleList.forLanguageTags(case.locale)
        }
        val advance = paint.getRunAdvance(case.text, range[0], range[1], range[0], range[1], run.rtl, range[1]) / scale
        val delta = advance - run.advance
        runsTotal++
        worstAdvance = max(worstAdvance, abs(delta))
        perLine[run.line] = (perLine[run.line] ?: 0f) + delta
        worstLine[run.line] = max(worstLine[run.line] ?: 0f, abs(perLine.getValue(run.line)))

        if (Build.VERSION.SDK_INT >= 31) {
          val shaped = TextRunShaper.shapeTextRun(case.text, range[0], range[1] - range[0], range[0], range[1] - range[0], 0f, 0f, run.rtl, paint)
          val ids = IntArray(shaped.glyphCount()) { shaped.getGlyphId(it) }
          val minikin = FloatArray(ids.size) { shaped.getGlyphX(it) / scale }
          idsCompared++
          variantIds++

          if (ids.contentEquals(run.glyphs)) {
            idsEqual++
            variantEqual++

            for (i in ids.indices) variantDrift = max(variantDrift, abs(minikin[i] - (run.xs[i] - run.left)))
          } else {
            different.put(JSONObject().put("text", runText).put("parley", JSONArray(run.glyphs.toList())).put("minikin", JSONArray(ids.toList())))
          }

          if (DETAILED.any { id.startsWith(it) }) {
            detail.put(JSONObject().apply {
              put("text", runText)
              put("parley_x", JSONArray(run.xs.take(64).map { ((it - run.left) * 1000).toInt() / 1000.0 }))
              put("minikin_x", JSONArray(minikin.take(64).map { (it * 1000).toInt() / 1000.0 }))
              put("parley_advance", run.advance.toDouble())
              put("minikin_advance", advance.toDouble())
            })
          }
        }
      }

      worstDrift = max(worstDrift, variantDrift)
      out.put(JSONObject().apply {
        put("variant", id)
        put("runs_compared", variantIds)
        put("runs_same_glyphs", variantEqual)
        put("max_glyph_drift", variantDrift.toDouble())
        put("max_line_end_drift", (worstLine.values.maxOrNull() ?: 0f).toDouble())
        if (different.length() > 0) put("different_glyphs", different)
        if (detail.length() > 0) put("detail", detail)
      })
    }

    return JSONObject().apply {
      put("scale", scale.toDouble())
      put("runs", runsTotal)
      put("runs_compared", idsCompared)
      put("runs_same_glyphs", idsEqual)
      put("max_glyph_drift", worstDrift.toDouble())
      put("max_run_advance_delta", worstAdvance.toDouble())
      put("excluded", excluded)
      put("variants", out)
    }
  }

  companion object {
    /** The variants whose every glyph position the shaping report keeps. */
    val DETAILED = listOf(
      "latin-kerning/17/max", "latin-ligatures/17/max", "arabic/17/max", "devanagari/17/max", "thai/17/160",
      "han-zh-hans/17/max", "latin-paragraph/17/max", "sizes-latin/", "burmese/17/90", "emoji-keycaps/17/max",
    )
  }
}
