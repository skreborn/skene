package dev.skene.spike.text

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.Hyphens
import androidx.compose.ui.text.style.LineBreak

/** Text styles matching [TextEngine]'s pinned parameters, per style index. */
@Composable
fun spikeStyles(): List<TextStyle> {
  val density = LocalDensity.current

  return Corpus.SIZES.map { px ->
    with(density) {
      TextStyle(fontSize = px.toSp(), lineBreak = LineBreak.Simple, hyphens = Hyphens.None)
    }
  }
}

/** The same tree as the Rust one: cards of a paragraph, three weighted items, a fixed label
 * beside a weighted item, and four wrapping tags. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SpikeTree(corpus: Corpus) {
  val density = LocalDensity.current
  val styles = spikeStyles()

  @Composable
  fun leaf(id: Int, modifier: Modifier = Modifier) {
    BasicText(corpus.text(id), modifier, style = styles[corpus.style(id)])
  }

  with(density) {
    Column(
      Modifier.fillMaxWidth(),
      verticalArrangement = Arrangement.spacedBy(ROOT_GAP.toDp()),
    ) {
      for (card in 0 until corpus.cards) {
        val base = card * Corpus.LEAVES_PER_CARD

        Column(
          Modifier.fillMaxWidth().padding(CARD_PAD.toDp()),
          verticalArrangement = Arrangement.spacedBy(CARD_GAP.toDp()),
        ) {
          leaf(base)

          Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(ROW_GAP.toDp())) {
            leaf(base + 1, Modifier.weight(1f))
            leaf(base + 2, Modifier.weight(1f))
            leaf(base + 3, Modifier.weight(1f))
          }

          Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(ROW_GAP.toDp())) {
            leaf(base + 4, Modifier.width(FIXED.toDp()))
            leaf(base + 5, Modifier.weight(1f))
          }

          FlowRow(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(TAG_GAP_H.toDp()),
            verticalArrangement = Arrangement.spacedBy(TAG_GAP_V.toDp()),
          ) {
            for (tag in 6..9) leaf(base + tag)
          }
        }
      }
    }
  }
}

const val ROOT_GAP = 12f
const val CARD_PAD = 16f
const val CARD_GAP = 8f
const val ROW_GAP = 8f
const val FIXED = 240f
const val TAG_GAP_H = 12f
const val TAG_GAP_V = 8f
