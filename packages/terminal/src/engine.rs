//! The terminal's text engine: Parley over a typeface of cells, so the terminal lays its text
//! out, breaks it and moves through it by the same engine and rules as every pixel platform,
//! in units of cells, a line one row at one cell; and what a laid-out text draws, cluster by
//! cluster, in the engine's visual order.

use parley::PositionedLayoutItem;
use skene_foundation::text::{
  AttributedText,
  BaseDirection,
  Laid as _,
  LineHeight,
  Paragraph as _,
  ParagraphStyle,
  Round,
  Substitute,
  Substituted,
  SubstitutedLaid,
  TextEngine as _,
  TextIndex,
  TextRun,
};
use skene_parley::{ParleyEngine, ParleyLaid, TabSize, Typeface};

use crate::text::{self, CellScale};

/// The typeface the terminal measures its text in, generated at build time: every code
/// point as many cells wide as the terminal draws it on its own, zero, one or two, a cell
/// an em and its ascent the whole cell.
pub(crate) const CELLS: Typeface = Typeface::new(
  "Skene Cells",
  include_bytes!(concat!(env!("OUT_DIR"), "/cells.otf")),
);

thread_local! {
  static ENGINE: CellEngine = {
    let engine = ParleyEngine::with_typefaces(&[CELLS]);

    engine.set_tab_size(TabSize::Spaces(1.));

    Substituted::new(engine, StandIns)
  };
}

/// The terminal's text engine: Parley over the cell typeface, a tab's stops a cell apart,
/// laying out in place of a cluster the stand-in [`StandIns`] names, every position it
/// reports and takes a byte of the real text.
pub(crate) type CellEngine = Substituted<ParleyEngine, StandIns>;

/// A text laid out through the [`CellEngine`].
pub(crate) type CellLaid = SubstitutedLaid<ParleyLaid>;

/// The engine of this thread: one font context every text shares.
pub(crate) fn engine() -> CellEngine {
  ENGINE.with(Clone::clone)
}

/// The paragraph style every terminal text lays out in: its lines one row tall at the
/// least and wrapping where they fill, left to right as the document reads.
pub(crate) fn paragraph_style() -> ParagraphStyle {
  ParagraphStyle::default()
    .with_direction(BaseDirection::LeftToRight)
    .with_line_height(LineHeight::Absolute(1.))
}

/// What the cell typeface lays out in place of a cluster: a tab as the blank it shows as,
/// one cell at its run's scale, and where the terminal clusters (mode 2027) and draws a
/// cluster wider or narrower than its code points sum to, one character as wide as the
/// terminal draws it, which a line breaks around as it does around an ideograph.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StandIns;

impl Substitute for StandIns {
  fn substitute(&self, cluster: &str, _start: usize) -> Option<char> {
    text::stand_in(cluster)
  }
}

/// One grapheme cluster a laid-out text draws: the bytes of the text it holds, the cell
/// its block's left edge is in, the row its block's top is on, and the cells it advances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Drawn {
  pub(crate) at: usize,
  pub(crate) end: usize,
  pub(crate) column: usize,
  pub(crate) top: usize,
  pub(crate) advance: usize,
}

/// Every cluster `laid`, of `text`, draws, line by line in visual order: each where the
/// engine places it, as wide as the terminal draws it at its run's size and as tall as the
/// size's rows, its block's foot on its line's baseline. A line's break and the blanks
/// hanging past its end, wholly or in part, draw nothing.
pub(crate) fn drawn(laid: &CellLaid, text: &str, index: &TextIndex) -> Vec<Drawn> {
  let mut drawn = Vec::new();
  let lines = laid.lines();

  for (line, info) in laid.inner().layout().lines().zip(&lines) {
    let visible = info.left + info.width;

    for item in line.items() {
      let PositionedLayoutItem::GlyphRun(glyphs) = item
      else {
        continue;
      };
      let rows = glyphs.run().font_size().round().max(1.);
      let top = whole(glyphs.baseline() - rows);
      let mut x = glyphs.offset();

      for cluster in glyphs.run().visual_clusters() {
        let left = x;
        let shown = cluster.text_range();
        let cluster_advance = cluster.advance();

        x += cluster_advance;

        if cluster.is_hard_line_break() {
          continue;
        }

        let end = laid.real_index(shown.end);
        let mut column = whole(left);
        // A cluster the engine shaped as one may hold several of the text's clusters, each
        // drawn after the last; one starting inside a cluster of the text draws nothing, the
        // cluster drawn whole where it starts.
        let mut at = index.snap_to_grapheme(laid.real_index(shown.start), Round::Up);

        while at < end {
          let next = index.next_grapheme(at);
          let grapheme = &text[at..next];

          if left + cluster_advance > visible + 1e-3 && grapheme.chars().all(char::is_whitespace) {
            break;
          }

          let advance = text::width(grapheme) * whole(rows);

          drawn.push(Drawn {
            at,
            end: next,
            column,
            top,
            advance,
          });
          column += advance;
          at = next;
        }
      }
    }
  }

  drawn
}

/// The lines `text` wraps into at `width` cells, every cluster at one cell's scale: each
/// line's first cluster and the clusters it shows, short of its break and the blanks
/// hanging past its end; at least one.
pub(crate) fn wrap_spans(text: &str, width: usize) -> Vec<(usize, String)> {
  let style = CellScale::ONE.style();
  let paragraph = engine().paragraph(
    &AttributedText::new(
      text,
      style.clone(),
      vec![TextRun {
        range: 0..text.len(),
        style,
      }],
    ),
    &paragraph_style(),
  );
  #[expect(
    clippy::cast_precision_loss,
    reason = "a width in cells stays far below the integers f32 holds exactly"
  )]
  let laid = paragraph.layout(Some(width.max(1) as f32));
  let index = TextIndex::new(text);

  laid
    .lines()
    .into_iter()
    .map(|line| {
      let start = line.range.start.min(text.len());
      let mut shown = &text[start..line.range.end.min(text.len())];

      shown = shown
        .strip_suffix("\r\n")
        .or_else(|| shown.strip_suffix('\n'))
        .or_else(|| shown.strip_suffix('\r'))
        .unwrap_or(shown);

      while text::cells(shown) > whole(line.width) {
        match shown.char_indices().next_back() {
          Some((at, blank)) if blank.is_whitespace() => shown = &shown[..at],
          _ => break,
        }
      }

      (index.grapheme_at_byte(start, Round::Down), shown.to_owned())
    })
    .collect()
}

/// The whole cells or rows `length` spans, rounded.
#[expect(
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  reason = "a length in cells is non-negative and far below usize::MAX"
)]
pub(crate) const fn whole(length: f32) -> usize {
  length.max(0.).round() as usize
}

#[cfg(test)]
mod tests {
  use skene_foundation::{
    Color,
    text::{
      AttributedText,
      Intrinsic,
      Laid as _,
      PaintStyle,
      Paragraph as _,
      TextEngine as _,
      TextIndex,
      TextRun,
    },
  };

  use super::{CellLaid, drawn, engine, paragraph_style, wrap_spans};
  use crate::text::{self, CellScale};

  /// `text` at one cell's scale.
  fn plain(text: &str) -> AttributedText {
    AttributedText::plain(text, CellScale::ONE.style())
  }

  fn widths(text: &str) -> Intrinsic {
    engine()
      .paragraph(&plain(text), &paragraph_style())
      .intrinsic()
  }

  /// `text` laid out at `width` cells, unbounded where `None`.
  fn laid(text: &str, width: Option<f32>) -> CellLaid {
    engine()
      .paragraph(&plain(text), &paragraph_style())
      .layout(width)
  }

  /// The rows of `text` laid out at `width` cells, each as it draws.
  fn rows(text: &str, width: f32) -> Vec<String> {
    let laid = laid(text, Some(width));
    let mut rows = vec![String::new(); laid.line_count()];

    for cluster in drawn(&laid, text, &TextIndex::new(text)) {
      let row = &mut rows[cluster.top];

      while text::cells(row) < cluster.column {
        row.push(' ');
      }

      row.push_str(&text[cluster.at..cluster.end]);
    }

    rows
  }

  #[test]
  fn text_is_measured_in_cells() {
    text::reset();

    assert_eq!(widths("abc").max, 3.);
    assert_eq!(widths("日本").max, 4., "a wide cluster takes two cells");
    assert_eq!(widths("e\u{301}").max, 1., "a combining mark takes none");
    assert_eq!(widths("a\tb").max, 3., "a tab is one cell");
    assert_eq!(widths("alpha beta").min, 5.);
  }

  #[test]
  fn a_line_is_one_row() {
    text::reset();

    let laid = laid("ab\ncd", None);

    assert_eq!(laid.size().height, 2.);
    assert_eq!(laid.lines()[1].top, 1.);
  }

  /// A whole scale lays out at as many rows and its clusters as many times as wide; a
  /// smaller run on its line stands on the line's bottom row.
  #[test]
  fn a_scale_lays_out_at_its_rows() {
    text::reset();

    let text = "ab";
    let paragraph = engine().paragraph(
      &AttributedText::new(
        text,
        CellScale::ONE.style(),
        vec![
          TextRun {
            range: 0..1,
            style: CellScale::Whole(2).style(),
          },
          TextRun {
            range: 1..2,
            style: CellScale::ONE.style(),
          },
        ],
      ),
      &paragraph_style(),
    );
    let laid = paragraph.layout(None);
    let clusters = drawn(&laid, text, &TextIndex::new(text));

    assert_eq!(laid.size().height, 2.);
    assert_eq!(
      clusters
        .iter()
        .map(|cluster| (cluster.column, cluster.top, cluster.advance))
        .collect::<Vec<_>>(),
      [(0, 0, 2), (2, 1, 1)]
    );
  }

  /// The guaranteed core: blanks and newlines are kept, a word too wide for its line breaks
  /// where the line fills, an empty text lays out one line and a trailing newline opens
  /// one.
  #[test]
  fn the_guaranteed_core_holds() {
    text::reset();

    assert_eq!(widths("a  b").max, 4., "two blanks take two cells");
    assert_eq!(laid("a\nb", None).line_count(), 2);
    assert_eq!(rows("abcdefgh", 3.), ["abc", "def", "gh"]);
    assert_eq!(laid("", None).line_count(), 1);
    assert_eq!(laid("", None).size().height, 1.);
    assert_eq!(laid("ab\n", None).line_count(), 2);
    assert_eq!(
      wrap_spans("ab\n", 5),
      [(0, "ab".to_owned()), (3, String::new())]
    );
  }

  /// A layout's geometry never changes: a later layout at another width moves nothing in
  /// one handed out, and a repaint shows in every layout and moves nothing.
  #[test]
  fn a_layout_is_immutable_and_its_paint_the_paragraphs() {
    text::reset();

    let mut paragraph = engine().paragraph(&plain("alpha beta gamma"), &paragraph_style());
    let wide = paragraph.layout(Some(20.));
    let lines = wide.lines();
    let narrow = paragraph.layout(Some(6.));

    assert_eq!(narrow.line_count(), 3);
    assert_eq!(wide.lines(), lines);

    let red = PaintStyle::new(Color::srgb(1., 0., 0.));

    assert!(paragraph.repaint(0..5, &red));
    assert_eq!(wide.lines(), lines);
    assert_eq!(wide.inner().paint_at(0), red);
    assert_eq!(narrow.inner().paint_at(0), red);
  }

  /// Where the terminal clusters, a cluster it draws wider than its code points sum to
  /// lays out as wide as it draws, every position still a byte of the real text.
  #[test]
  fn a_clustered_cluster_lays_out_as_the_terminal_draws_it() {
    text::reset();

    let family = "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}x";

    assert_eq!(widths(family).max, 7.);

    text::set_clustering(true);

    let laid = laid(family, None);
    let clusters = drawn(&laid, family, &TextIndex::new(family));

    assert_eq!(laid.size().width, 3.);
    assert_eq!(
      clusters
        .iter()
        .map(|cluster| (cluster.at, cluster.column, cluster.advance))
        .collect::<Vec<_>>(),
      [(0, 0, 2), (family.len() - 1, 2, 1)]
    );
    assert_eq!(
      laid.hit_test(2.2, 0.5).position.index,
      family.len() - 1,
      "a hit past the cluster lands on the next byte of the real text"
    );

    text::reset();
  }

  /// Right-to-left text draws in the engine's visual order, its first character rightmost.
  #[test]
  fn right_to_left_text_draws_in_visual_order() {
    text::reset();

    assert_eq!(
      rows("ab \u{5d0}\u{5d1}\u{5d2}", 20.),
      ["ab \u{5d2}\u{5d1}\u{5d0}"]
    );
  }
}
