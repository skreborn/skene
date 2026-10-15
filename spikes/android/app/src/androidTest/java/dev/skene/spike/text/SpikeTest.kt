package dev.skene.spike.text

import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.Build
import android.os.Looper
import android.util.Log
import android.view.View.MeasureSpec
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.createFontFamilyResolver
import androidx.compose.ui.text.style.Hyphens
import androidx.compose.ui.text.style.LineBreak
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.Callable
import java.util.concurrent.Executors
import kotlin.math.abs
import kotlin.math.max
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The spike's measurements. Each test is meant to run in a process of its own (one
 * `am instrument` per test), so a cold pass is cold for Minikin's process-wide caches too.
 * Results go to logcat under [TAG] as `key=value` lines.
 */
@RunWith(AndroidJUnit4::class)
class SpikeTest {
  private val instrumentation = InstrumentationRegistry.getInstrumentation()
  private val runId = InstrumentationRegistry.getArguments().getString("runId") ?: "local"

  private fun report(key: String, value: Any) {
    Log.i(TAG, "RID:$runId $key=$value")
  }

  private fun ms(nanos: Long) = "%.3f".format(nanos / 1e6)

  private fun <T> onMain(block: () -> T): T {
    var result: Result<T>? = null
    instrumentation.runOnMainSync { result = runCatching(block) }
    return result!!.getOrThrow()
  }

  private fun passes(prefix: String, name: String, runs: List<LongArray>) {
    val nanos = runs.map { it[0] }.sorted()
    report("$prefix.$name.ms", ms(nanos[nanos.size / 2]))
    report("$prefix.$name.runs", runs.size)
    report("$prefix.$name.crossings", runs.maxOf { it[1] })
    report("$prefix.$name.intrinsicsCalls", runs.maxOf { it[2] })
    report("$prefix.$name.layoutCalls", runs.maxOf { it[3] })
    report("$prefix.$name.maxPerLeaf", runs.maxOf { it[4] })
    report("$prefix.$name.leavesOver1", runs.maxOf { it[5] })
    report("$prefix.$name.measureCallbacks", runs.maxOf { it[6] })
    report("$prefix.$name.leavesCrossing", runs.maxOf { it[7] })
  }

  /** The engine as designed: PrecomputedText once per text, layouts reusing it. */
  @Test
  fun taffy() = taffyPasses("taffy", precompute = true)

  /** The engine without PrecomputedText: every layout measures its text afresh. */
  @Test
  fun taffyPlain() = taffyPasses("taffyPlain", precompute = false)

  private fun taffyPasses(prefix: String, precompute: Boolean) {
    report("device.sdk", Build.VERSION.SDK_INT)
    report("device.abi", Build.SUPPORTED_ABIS.joinToString(","))

    onMain {
      val corpus = Corpus(CARDS)
      val start = System.nanoTime()
      val engine = TextEngine(corpus, precompute = precompute)
      val tree = NativeTree(engine, CARDS)
      report("$prefix.leaves", tree.leaves)
      report("$prefix.useBoring", engine.useBoring)
      report("$prefix.create.ms", ms(System.nanoTime() - start))
      fun passes(name: String, runs: List<LongArray>) = passes(prefix, name, runs)

      // A single pass, with the time spent inside the Kotlin engine's entry points beside it.
      fun timed(name: String, width: Float) {
        val intrinsics = engine.intrinsicsNanos
        val layouts = engine.layoutNanos
        passes(name, listOf(tree.layout(width)))
        report("$prefix.$name.engineIntrinsicsMs", ms(engine.intrinsicsNanos - intrinsics))
        report("$prefix.$name.engineLayoutMs", ms(engine.layoutNanos - layouts))
      }

      timed("cold", W1)
      passes("warmSameWidth", List(5) { tree.layout(W1, NativeTree.DIRTY) })
      timed("widthChange", W2)
      passes("backToFirstWidth", listOf(tree.layout(W1)))
      passes("alternate", List(10) { tree.layout(if (it % 2 == 0) W2 else W1) })
      timed("newWidth", W3)
      passes("javaCacheOnly", List(5) { tree.layout(W1, NativeTree.DIRTY or NativeTree.CLEAR_LAID) })
      passes(
        "javaCacheOnlyWithIntrinsics",
        List(5) {
          tree.layout(W1, NativeTree.DIRTY or NativeTree.CLEAR_LAID or NativeTree.CLEAR_INTRINSICS)
        },
      )

      val rects = tree.rects()
      report("$prefix.rootHeightAtW1", rects[rects.size - 1])
      report("$prefix.engineHits", engine.hits)
      report("$prefix.engineMisses", engine.misses)
      tree.close()
    }
  }

  @Test
  fun compose() {
    report("device.sdk", Build.VERSION.SDK_INT)
    val scenario = ActivityScenario.launch(SpikeActivity::class.java)

    scenario.onActivity { activity ->
      val corpus = Corpus(CARDS)
      val view = ComposeView(activity)
      activity.setContentView(view)

      val start = System.nanoTime()
      view.setContent { SpikeTree(corpus) }
      report("compose.composition.ms", ms(System.nanoTime() - start))
      report("compose.attached", view.isAttachedToWindow)

      fun measure(width: Float): Long {
        val begin = System.nanoTime()
        view.measure(
          MeasureSpec.makeMeasureSpec(width.toInt(), MeasureSpec.EXACTLY),
          MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED),
        )
        view.layout(0, 0, width.toInt(), view.measuredHeight)
        return System.nanoTime() - begin
      }

      fun passes(name: String, runs: List<Long>) {
        val sorted = runs.sorted()
        report("compose.$name.ms", ms(sorted[sorted.size / 2]))
        report("compose.$name.runs", runs.size)
      }

      passes("cold", listOf(measure(W1)))
      val atW1 = view.measuredHeight
      passes("widthChange", listOf(measure(W2)))
      passes("backToFirstWidth", listOf(measure(W1)))
      passes("alternate", List(10) { measure(if (it % 2 == 0) W2 else W1) })
      passes("newWidth", listOf(measure(W3)))
      passes("sameWidthNoop", List(5) { measure(W3) })
      report("compose.rootHeightAtW1", atW1)
    }

    scenario.close()
  }

  /** Lays the tree out at W1, then W2, then W1 again, returning both sets of rectangles. */
  private fun measureAll(corpus: Corpus): FloatArray {
    val engine = TextEngine(corpus)
    val tree = NativeTree(engine, CARDS)
    tree.layout(W1)
    tree.layout(W2)
    val second = tree.rects()
    tree.layout(W1, NativeTree.DIRTY or NativeTree.CLEAR_LAID or NativeTree.CLEAR_INTRINSICS)
    val first = tree.rects()
    tree.close()
    return first + second
  }

  private fun compare(name: String, a: FloatArray, b: FloatArray) {
    var mismatched = 0
    var largest = 0f

    for (i in a.indices) {
      val difference = abs(a[i] - b[i])

      if (difference != 0f) mismatched++

      largest = max(largest, difference)
    }

    report("offMain.$name.values", a.size)
    report("offMain.$name.mismatched", mismatched)
    report("offMain.$name.maxDifference", largest)
  }

  @Test
  fun offMain() {
    val corpus = Corpus(CARDS)
    val executor = Executors.newSingleThreadExecutor()

    val main = onMain {
      check(Looper.myLooper() == Looper.getMainLooper())
      measureAll(corpus)
    }

    val background = executor.submit(
      Callable {
        check(Looper.myLooper() != Looper.getMainLooper())
        val begin = System.nanoTime()
        val engine = TextEngine(corpus)
        val tree = NativeTree(engine, CARDS)
        val stats = tree.layout(W1)
        report("offMain.backgroundColdPass.ms", ms(stats[0]))
        report("offMain.backgroundColdPass.crossings", stats[1])
        tree.close()
        report("offMain.backgroundTotal.ms", ms(System.nanoTime() - begin))
        report("offMain.backgroundThread", Thread.currentThread().name)
        measureAll(corpus)
      },
    ).get()

    compare("mainVsBackground", main, background)

    val concurrent = executor.submit(Callable { measureAll(corpus) })
    val mainAlongside = onMain { measureAll(corpus) }
    val backgroundAlongside = concurrent.get()
    compare("mainVsConcurrentMain", main, mainAlongside)
    compare("mainVsConcurrentBackground", main, backgroundAlongside)

    // Per leaf: a StaticLayout built on each thread at the leaf's final width.
    val widths = IntArray(corpus.leaves) { (main[it * 4 + 2] + 0.001f).toInt() }

    fun numbers(): IntArray {
      val engine = TextEngine(corpus)
      return IntArray(corpus.leaves * 3) {
        val layout = engine.build(it / 3, widths[it / 3])

        when (it % 3) {
          0 -> layout.height
          1 -> layout.lineCount
          else -> layout.getLineBaseline(0)
        }
      }
    }

    val onMainThread = onMain { numbers() }
    val offMainThread = executor.submit(Callable { numbers() }).get()
    report("offMain.staticLayoutNumbers", onMainThread.size)
    report(
      "offMain.staticLayoutMismatched",
      onMainThread.indices.count { onMainThread[it] != offMainThread[it] },
    )
    executor.shutdown()
  }

  @Test
  fun ink() {
    val context = instrumentation.targetContext
    val corpus = Corpus(CARDS)
    val margin = 200
    val bitmap = Bitmap.createBitmap(W1.toInt() + 2 * margin, 1400, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(bitmap)
    val density = Density(context)
    val measurer = TextMeasurer(
      createFontFamilyResolver(context),
      density,
      LayoutDirection.Ltr,
      0,
    )
    val styles = Corpus.SIZES.map { px ->
      with(density) {
        TextStyle(fontSize = px.toSp(), lineBreak = LineBreak.Simple, hyphens = Hyphens.None)
      }
    }

    val variants = listOf(
      "ink.default" to (Build.VERSION.SDK_INT >= 33),
      "ink.boringOn" to true,
      "ink.boringOff" to false,
    )

    for ((prefix, boring) in variants) {
      val engine = TextEngine(corpus, boring)
      val tree = NativeTree(engine, CARDS)
      tree.layout(W1)
      val rects = tree.rects()
      tree.close()

      val fresh = TextEngine(corpus, boring)
      var heightMismatch = 0
      var inkAbove = 0
      var inkBelow = 0
      var inkBelowOver1 = 0
      var inkBeyondWidth = 0
      var worstAbove = 0
      var worstBelow = 0
      var boringLayouts = 0
      var composeMismatch = 0
      var composeWorst = 0
      val perScript = HashMap<String, Int>()
      val examples = ArrayList<String>()

      for (id in 0 until corpus.leaves) {
        val width = (rects[id * 4 + 2] + 0.001f).toInt()
        val measured = rects[id * 4 + 3].toInt()
        val layout = fresh.build(id, width)

        if (layout is android.text.BoringLayout) boringLayouts++

        if (layout.height != measured) heightMismatch++

        val height = layout.height

        val composed = measurer.measure(
          corpus.text(id),
          styles[corpus.style(id)],
          constraints = Constraints(maxWidth = width),
        )

        if (composed.size.height != height) {
          composeMismatch++
          composeWorst = max(composeWorst, abs(composed.size.height - height))
        }

        val boxHeight = height + 2 * margin

        if (boxHeight > bitmap.height) continue

        bitmap.eraseColor(0)
        canvas.save()
        canvas.translate(margin.toFloat(), margin.toFloat())
        layout.draw(canvas)
        canvas.restore()

        val w = width + 2 * margin
        val pixels = IntArray(w * boxHeight)
        bitmap.getPixels(pixels, 0, w, 0, 0, w, boxHeight)
        var top = Int.MAX_VALUE
        var bottom = Int.MIN_VALUE
        var right = Int.MIN_VALUE

        for (y in 0 until boxHeight) {
          for (x in 0 until w) {
            if (pixels[y * w + x] ushr 24 != 0) {
              top = minOf(top, y)
              bottom = maxOf(bottom, y)
              right = maxOf(right, x)
            }
          }
        }

        if (top == Int.MAX_VALUE) continue

        val above = margin - top
        val below = (bottom + 1) - (margin + height)

        if (above > 0) {
          inkAbove++
          worstAbove = max(worstAbove, above)
        }

        if (below > 0) {
          inkBelow++
          worstBelow = max(worstBelow, below)

          if (below > 1) inkBelowOver1++
        }

        if (right + 1 > margin + width) inkBeyondWidth++

        if ((above > 0 || below > 0)) {
          val script = corpus.script(id)
          perScript[script] = (perScript[script] ?: 0) + 1

          if (examples.size < 8) {
            examples.add("id$id/$script/${if (layout is android.text.BoringLayout) "boring" else "static"}/h$height/above$above/below$below")
          }
        }
      }

      report("$prefix.leaves", corpus.leaves)
      report("$prefix.boringLayouts", boringLayouts)
      report("$prefix.heightMismatchVsFreshLayout", heightMismatch)
      report("$prefix.inkAboveBox", inkAbove)
      report("$prefix.inkBelowBox", inkBelow)
      report("$prefix.inkBelowBoxOver1px", inkBelowOver1)
      report("$prefix.worstAbovePx", worstAbove)
      report("$prefix.worstBelowPx", worstBelow)
      report("$prefix.inkBeyondLayoutWidth", inkBeyondWidth)
      report("$prefix.overflowByScript", perScript.toSortedMap().toString().replace(" ", ""))
      report("$prefix.examples", examples.joinToString(";").replace(" ", "_"))
      report("$prefix.composeHeightMismatch", composeMismatch)
      report("$prefix.composeWorstHeightDiffPx", composeWorst)
    }
  }

  companion object {
    const val TAG = "SKENE_SPIKE"
    const val CARDS = 100
    const val W1 = 1080f
    const val W2 = 720f
    const val W3 = 900f
  }
}
