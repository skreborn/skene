//! Text measured in cells: extended grapheme clusters, each as wide as the terminal
//! draws it and as many times as its scale says, and the glyph a painted cell holds.

use std::{
  cell::{Cell, RefCell},
  fmt::{self, Write as _},
};

use rustc_hash::FxHashMap;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

thread_local! {
  static CLUSTERING: Cell<bool> = const { Cell::new(false) };
  static SIZING: Cell<bool> = const { Cell::new(false) };
  static INTERNED: RefCell<Interned> = RefCell::new(Interned::default());
}

/// The largest whole scale the text sizing protocol draws.
const MAX_SCALE: u8 = 7;

/// The largest denominator of a fractional scale.
const MAX_DENOMINATOR: u8 = 15;

/// How one cluster is sized: whole cells, or one cell drawn at a fraction of its size.
///
/// A whole scale of `n` takes a block `n` rows tall and `n` times the cluster's
/// columns wide; a fraction takes the cluster's own cells, its glyph drawn smaller at
/// the cell's foot. For a headless test asserting paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum CellScale {
  /// Whole cells: the scale, one to seven.
  Whole(u8),
  /// A fraction of one cell: the numerator and the denominator, the numerator smaller.
  Fraction(u8, u8),
}

impl CellScale {
  /// One cell.
  pub const ONE: Self = Self::Whole(1);

  /// The scale a font size in cells resolves to: a size of one and above rounds to
  /// a whole scale of at most seven, a size below one is the nearest fraction with a
  /// denominator of at most fifteen, and a size that is not positive is one.
  pub fn of(size: f32) -> Self {
    if !size.is_finite() || size <= 0. {
      return Self::ONE;
    }

    if size >= 1. {
      #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
      let whole = size.round().min(f32::from(MAX_SCALE)) as u8;

      return Self::Whole(whole.max(1));
    }

    let (mut best, mut error) = ((1, 2), f32::INFINITY);

    for denominator in 2..=MAX_DENOMINATOR {
      #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
      let numerator = ((size * f32::from(denominator)).round() as u8).clamp(1, denominator - 1);
      let distance = (f32::from(numerator) / f32::from(denominator) - size).abs();

      if distance < error {
        (best, error) = ((numerator, denominator), distance);
      }
    }

    Self::Fraction(best.0, best.1)
  }

  /// The rows the scale takes: the whole scale, one for a fraction.
  #[inline(always)]
  #[must_use]
  pub const fn rows(self) -> usize {
    match self {
      Self::Whole(scale) => scale as usize,
      Self::Fraction(..) => 1,
    }
  }
}

impl Default for CellScale {
  #[inline(always)]
  fn default() -> Self {
    Self::ONE
  }
}

/// Puts the measurement on the terminal's text sizing (OSC 66) or off it; `true` when
/// that changes it.
pub(crate) fn set_sizing(on: bool) -> bool {
  SIZING.with(|sizing| sizing.replace(on) != on)
}

/// Whether the terminal proved it sizes text.
pub(crate) fn sizing() -> bool {
  SIZING.with(Cell::get)
}

/// The scale a run's resolved size draws at: its own where the terminal sizes text,
/// one cell elsewhere.
pub(crate) fn scale_of(size: Option<f32>) -> CellScale {
  match size {
    Some(size) if sizing() => CellScale::of(size),
    _ => CellScale::ONE,
  }
}

/// What one stretch of a text spans: its clusters at a scale, or an inline box's one
/// placeholder cluster, as many cells wide and rows tall as the box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Extent {
  Scaled(CellScale),
  Box { cells: usize, rows: usize },
}

impl From<CellScale> for Extent {
  fn from(scale: CellScale) -> Self {
    Self::Scaled(scale)
  }
}

/// The extent of each stretch of a text: `(end, extent)` pairs in order, `end` the
/// cluster index the stretch ends before; the last stretch reaches past the end.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Scales(Vec<(usize, Extent)>);

impl Scales {
  /// One scale over the whole text.
  pub(crate) fn uniform(scale: CellScale) -> Self {
    Self(vec![(usize::MAX, Extent::Scaled(scale))])
  }

  /// Scales from the stretches `(count, extent)`, each `count` clusters long, in order.
  pub(crate) fn of<E: Into<Extent>>(stretches: impl IntoIterator<Item = (usize, E)>) -> Self {
    let mut end = 0;
    let mut scales = Vec::new();

    for (count, extent) in stretches {
      end += count;
      scales.push((end, extent.into()));
    }

    Self(scales)
  }

  /// The extent of the cluster at `index`; one cell past every stretch.
  fn extent(&self, index: usize) -> Extent {
    self
      .0
      .iter()
      .find(|(end, _)| index < *end)
      .map_or(Extent::Scaled(CellScale::ONE), |(_, extent)| *extent)
  }

  /// The scale the cluster at `index` draws at: one cell for an inline box's.
  pub(crate) fn at(&self, index: usize) -> CellScale {
    match self.extent(index) {
      Extent::Scaled(scale) => scale,
      Extent::Box { .. } => CellScale::ONE,
    }
  }

  /// The cells the cluster at `index` spans on one row.
  pub(crate) fn cells(&self, index: usize, cluster: &str) -> usize {
    match self.extent(index) {
      Extent::Scaled(scale) => width(cluster) * scale.rows(),
      Extent::Box { cells, .. } => cells,
    }
  }

  /// The rows the cluster at `index` spans.
  pub(crate) fn rows(&self, index: usize) -> usize {
    match self.extent(index) {
      Extent::Scaled(scale) => scale.rows(),
      Extent::Box { rows, .. } => rows,
    }
  }
}

/// Puts the measurement on the terminal's grapheme clustering (mode 2027) or off it;
/// `true` when that changes it.
pub(crate) fn set_clustering(on: bool) -> bool {
  CLUSTERING.with(|clustering| clustering.replace(on) != on)
}

/// Whether the terminal confirmed grapheme clustering.
pub(crate) fn clustering() -> bool {
  CLUSTERING.with(Cell::get)
}

/// Forgets the terminal's clustering and sizing and the clusters interned, for a
/// fresh install.
pub(crate) fn reset() {
  CLUSTERING.with(|clustering| clustering.set(false));
  SIZING.with(|sizing| sizing.set(false));
  INTERNED.with(|interned| *interned.borrow_mut() = Interned::default());
}

/// The character the terminal shows for `character`: a space for a tab or a carriage
/// return, the replacement character for any other control, else `character` itself.
pub(crate) const fn shown(character: char) -> char {
  match character {
    '\t' | '\r' => ' ',
    '\0'..='\u{1f}' | '\u{7f}'..='\u{9f}' => '\u{fffd}',
    _ => character,
  }
}

/// The cells one cluster occupies: one for a control, which shows as one character;
/// else its width where the terminal clusters, else the sum of its code points' widths,
/// as a terminal drawing each on its own advances.
pub(crate) fn width(cluster: &str) -> usize {
  if cluster.starts_with(char::is_control) {
    return 1;
  }

  if clustering() {
    cluster.width()
  }
  else {
    cluster
      .chars()
      .map(|character| character.width().unwrap_or(0))
      .sum()
  }
}

/// The clusters of `text`, each at its byte offset.
pub(crate) fn clusters(text: &str) -> impl Iterator<Item = (usize, &str)> {
  text.grapheme_indices(true)
}

/// The cells `text` spans on one row.
pub(crate) fn cells(text: &str) -> usize {
  text.graphemes(true).map(width).sum()
}

/// How many clusters `text` holds.
pub(crate) fn count(text: &str) -> usize {
  text.graphemes(true).count()
}

/// The byte offset of the cluster at `index` in `text`, or the text's end.
pub(crate) fn byte_of(text: &str, index: usize) -> usize {
  text
    .grapheme_indices(true)
    .nth(index)
    .map_or(text.len(), |(byte, _)| byte)
}

/// The clusters `from..to` of `text`.
pub(crate) fn slice(text: &str, from: usize, to: usize) -> &str {
  &text[byte_of(text, from)..byte_of(text, to.max(from))]
}

/// The longest prefix of `text` spanning at most `cells` cells.
pub(crate) fn fit(text: &str, cells: usize) -> &str {
  let mut spanned = 0;

  for (byte, cluster) in clusters(text) {
    spanned += width(cluster);

    if spanned > cells {
      return &text[..byte];
    }
  }

  text
}

/// The cluster a caret goes to for the cell `column` of `text` on one row: the one
/// whose first cell it is, the next for a wide cluster's later cells, and the count
/// past the text.
pub(crate) fn index_at_column(text: &str, column: usize) -> usize {
  let mut cell = 0;

  for (index, cluster) in text.graphemes(true).enumerate() {
    let next = cell + width(cluster);

    if column < next {
      return if column == cell { index } else { index + 1 };
    }

    cell = next;
  }

  count(text)
}

/// A value's clusters: the byte each starts at and the cell its head is in, with the
/// value's length and its width after the last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Clusters {
  bytes: Vec<usize>,
  columns: Vec<usize>,
}

impl Clusters {
  /// The clusters of nothing.
  pub(crate) const EMPTY: Self = Self {
    bytes: Vec::new(),
    columns: Vec::new(),
  };

  /// The clusters of `text`.
  pub(crate) fn of(text: &str) -> Self {
    let mut bytes = Vec::new();
    let mut columns = Vec::new();
    let mut column = 0;

    for (byte, cluster) in clusters(text) {
      bytes.push(byte);
      columns.push(column);
      column += width(cluster);
    }

    bytes.push(text.len());
    columns.push(column);

    Self { bytes, columns }
  }

  /// How many clusters there are.
  pub(crate) fn count(&self) -> usize {
    self.bytes.len().saturating_sub(1)
  }

  /// The byte offset of the cluster at `index`, or the value's end.
  pub(crate) fn byte_of(&self, index: usize) -> usize {
    self
      .bytes
      .get(index.min(self.count()))
      .copied()
      .unwrap_or(0)
  }

  /// The cell the cluster at `index` starts in, or the value's width.
  pub(crate) fn column_of(&self, index: usize) -> usize {
    self
      .columns
      .get(index.min(self.count()))
      .copied()
      .unwrap_or(0)
  }

  /// The cells the value spans.
  pub(crate) fn cells(&self) -> usize {
    self.column_of(self.count())
  }

  /// The cluster holding the byte `offset`, and the count at or past the value's end.
  pub(crate) fn index_of_byte(&self, offset: usize) -> usize {
    match self.bytes.binary_search(&offset) {
      Ok(index) => index.min(self.count()),
      Err(index) => index.saturating_sub(1).min(self.count()),
    }
  }

  /// The cluster a caret goes to for the cell `column` counted from the cluster
  /// `from`'s first cell: the one whose first cell it is, the next for a wide
  /// cluster's later cells, and the count past the value.
  pub(crate) fn index_at_column(&self, from: usize, column: usize) -> usize {
    let count = self.count();
    let from = from.min(count);
    let base = self.column_of(from);

    for index in from..count {
      let cell = self.column_of(index) - base;
      let next = self.column_of(index + 1) - base;

      if column < next {
        return if column == cell { index } else { index + 1 };
      }
    }

    count
  }

  /// The last cluster a window of `span` cells can start at and still show the
  /// value's end with the caret's cell after it.
  pub(crate) fn last_window(&self, span: usize) -> usize {
    let count = self.count();
    let total = self.cells() + 1;

    (0..=count)
      .find(|window| total - self.column_of(*window) <= span)
      .unwrap_or(count)
  }

  /// A window of `span` cells starting at the cluster `window`, settled around the
  /// caret `at`: held where the caret's cell stays in view, moved the least that
  /// brings it in, and never past [`last_window`](Self::last_window).
  pub(crate) fn settled_window(&self, window: usize, at: usize, span: usize) -> usize {
    let window = window.min(self.last_window(span));
    let at = at.min(self.count());
    let caret_end = self.column_of(at) + 1;

    if at < window {
      at
    }
    else if caret_end - self.column_of(window) > span {
      (window..=at)
        .find(|window| caret_end - self.column_of(*window) <= span)
        .unwrap_or(at)
    }
    else {
      window
    }
  }
}

/// What one painted cell shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Glyph {
  /// One code point.
  Char(char),
  /// A cluster of several code points, interned.
  Cluster(u32),
  /// A later cell of a wide cluster, drawing nothing of its own.
  Continuation,
}

impl Glyph {
  /// A blank cell.
  pub(crate) const SPACE: Self = Self::Char(' ');

  /// The cells the glyph's cluster spans at one cell, at least one; none for a
  /// continuation.
  pub(crate) fn cells(self) -> usize {
    match self {
      Self::Char(character) => character.width().unwrap_or(0).max(1),
      Self::Cluster(id) => INTERNED
        .with(|interned| width(interned.borrow().text(id)))
        .max(1),
      Self::Continuation => 0,
    }
  }

  /// The glyph showing `cluster`; a control shows as the character [`shown`] makes of
  /// it.
  pub(crate) fn of(cluster: &str) -> Self {
    let mut characters = cluster.chars();

    match (characters.next(), characters.next()) {
      (None, _) => Self::SPACE,
      (Some(character), _) if character.is_control() => Self::Char(shown(character)),
      (Some(character), None) => Self::Char(character),
      _ => Self::Cluster(INTERNED.with(|interned| interned.borrow_mut().intern(cluster))),
    }
  }
}

/// Writes the glyph as the terminal shows it, so no control character reaches the
/// terminal.
impl fmt::Display for Glyph {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Char(character) => out.write_char(shown(*character)),
      Self::Cluster(id) => INTERNED.with(|interned| {
        interned
          .borrow()
          .text(*id)
          .chars()
          .try_for_each(|character| out.write_char(shown(character)))
      }),
      Self::Continuation => Ok(()),
    }
  }
}

/// The multi-code-point clusters painted so far, each under one id.
#[derive(Default)]
struct Interned {
  texts: Vec<String>,
  ids: FxHashMap<String, u32>,
}

impl Interned {
  fn intern(&mut self, cluster: &str) -> u32 {
    if let Some(id) = self.ids.get(cluster) {
      return *id;
    }

    let id = u32::try_from(self.texts.len()).expect("fewer clusters than ids");

    self.texts.push(cluster.to_owned());
    self.ids.insert(cluster.to_owned(), id);

    id
  }

  fn text(&self, id: u32) -> &str {
    self.texts.get(id as usize).map_or("", String::as_str)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// A combining sequence is one cluster of one cell, a wide character one of two,
  /// and an emoji sequence is as wide as its parts until the terminal clusters.
  #[test]
  fn clusters_are_measured_as_the_terminal_draws_them() {
    reset();

    assert_eq!(count("e\u{301}x"), 2);
    assert_eq!(cells("e\u{301}x"), 2);
    assert_eq!(cells("日本"), 4);
    assert_eq!(cells("\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}"), 6);
    assert!(set_clustering(true));
    assert_eq!(cells("\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}"), 2);
    assert!(!set_clustering(true));
    reset();
  }

  /// A control shows as one cell whether or not the terminal clusters: a tab and a
  /// carriage return as a space, any other as the replacement character.
  #[test]
  fn a_control_shows_as_one_cell() {
    reset();

    for clustering in [false, true] {
      set_clustering(clustering);

      assert_eq!(cells("a\u{1b}b\tc\r\u{7f}"), 7);
      assert_eq!(Glyph::of("\u{1b}").to_string(), "\u{fffd}");
      assert_eq!(Glyph::of("\t").to_string(), " ");
      assert_eq!(Glyph::of("\r\n").to_string(), " ");
      assert_eq!(Glyph::Char('\u{7}').to_string(), "\u{fffd}");
    }

    reset();
  }

  /// A caret lands before the cluster whose first cell is hit and after a wide
  /// cluster hit on a later cell; a byte inside a cluster reads as the cluster's.
  #[test]
  fn positions_map_between_bytes_cells_and_clusters() {
    reset();

    let clusters = Clusters::of("a日b");

    assert_eq!(clusters.count(), 3);
    assert_eq!(clusters.cells(), 4);
    assert_eq!(clusters.byte_of(2), 4);
    assert_eq!(clusters.byte_of(9), 5);
    assert_eq!(clusters.column_of(2), 3);
    assert_eq!(clusters.index_of_byte(2), 1);
    assert_eq!(clusters.index_of_byte(4), 2);
    assert_eq!(clusters.index_of_byte(50), 3);
    assert_eq!(clusters.index_at_column(0, 1), 1);
    assert_eq!(clusters.index_at_column(0, 2), 2);
    assert_eq!(clusters.index_at_column(0, 3), 2);
    assert_eq!(clusters.index_at_column(0, 4), 3);
    assert_eq!(clusters.index_at_column(1, 2), 2);
    assert_eq!(index_at_column("a日b", 2), 2);
    assert_eq!(slice("a日b", 1, 2), "日");
    assert_eq!(fit("a日b", 2), "a");
    assert_eq!(fit("a日b", 3), "a日");
  }

  /// A size rounds to a whole scale of at most seven, a size below one is the
  /// nearest fraction, and a size that is not positive is one cell.
  #[test]
  fn sizes_resolve_to_scales() {
    assert_eq!(CellScale::of(1.), CellScale::ONE);
    assert_eq!(CellScale::of(2.4), CellScale::Whole(2));
    assert_eq!(CellScale::of(1.5), CellScale::Whole(2));
    assert_eq!(CellScale::of(12.), CellScale::Whole(7));
    assert_eq!(CellScale::of(0.5), CellScale::Fraction(1, 2));
    assert_eq!(CellScale::of(0.75), CellScale::Fraction(3, 4));
    assert_eq!(CellScale::of(0.3), CellScale::Fraction(3, 10));
    assert_eq!(CellScale::of(0.), CellScale::ONE);
    assert_eq!(CellScale::of(f32::NAN), CellScale::ONE);
    assert_eq!(CellScale::Fraction(1, 2).rows(), 1);
    assert_eq!(CellScale::Whole(3).rows(), 3);
  }

  /// Stretches map cluster indices to their scales, the last reaching past the end,
  /// and a cluster's cells are its width times its scale.
  #[test]
  fn scales_run_in_stretches() {
    reset();

    let scales = Scales::of([(2, CellScale::Whole(2)), (1, CellScale::ONE)]);

    assert_eq!(scales.at(0), CellScale::Whole(2));
    assert_eq!(scales.at(1), CellScale::Whole(2));
    assert_eq!(scales.at(2), CellScale::ONE);
    assert_eq!(scales.at(9), CellScale::ONE);
    assert_eq!(scales.cells(0, "日"), 4);
    assert_eq!(
      Scales::uniform(CellScale::Whole(3)).at(50),
      CellScale::Whole(3)
    );
    assert_eq!(Scales::default().at(0), CellScale::ONE);
  }

  /// A window over wide clusters settles by cells: the caret's cell stays in view and
  /// the window never passes where the value's end fits.
  #[test]
  fn a_window_settles_by_cells() {
    reset();

    let clusters = Clusters::of("日本語");

    assert_eq!(clusters.last_window(4), 2);
    assert_eq!(clusters.last_window(7), 0);
    assert_eq!(clusters.settled_window(0, 1, 4), 0);
    assert_eq!(clusters.settled_window(0, 2, 4), 1);
    assert_eq!(clusters.settled_window(0, 3, 4), 2);
    assert_eq!(clusters.settled_window(2, 0, 4), 0);
    assert_eq!(clusters.settled_window(2, 2, 4), 2);
  }
}
