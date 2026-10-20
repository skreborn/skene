//! The windowing engine: a list's items by key, laid out in lines, each line's extent measured
//! or estimated; the lines a viewport shows; and the anchor that keeps content still under the
//! reader as extents and items change. It touches no view and no reactive state: the list
//! view drives it ([`crate::view::Lazy`]).

use std::{cell::RefCell, ops::Range};

use crate::lazy::{ItemExtent, Key, KeyMap, extents::Extents};

/// Where an item sits relative to the viewport: what keeps it still while the extents and the
/// items around it change.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Anchor {
  /// The item, by key.
  pub key: Key,
  /// Its place when the anchor was taken: where a search for it starts.
  pub index: usize,
  /// Its line's leading edge less the viewport's, along the axis.
  pub distance: f64,
}

/// A list's items laid out in lines along its axis: their keys, which line each falls in (one
/// item a line in a column or a row, up to `per_line` in a grid, an item taking a whole line
/// where it says so), each line's extent, and the padding around them.
#[derive(Debug)]
pub struct Engine {
  keys: Vec<Key>,
  whole: Vec<usize>,
  per_line: usize,
  starts: Option<Vec<usize>>,
  extents: Extents,
  extent: ItemExtent,
  estimate: f64,
  measured: KeyMap<f32>,
  places: RefCell<Option<KeyMap<usize>>>,
  padding: (f64, f64),
}

impl Engine {
  /// No items yet, each `extent` along the axis, lines `spacing` apart, `padding` before the
  /// first and after the last.
  #[must_use]
  pub fn new(extent: ItemExtent, spacing: f32, padding: (f32, f32)) -> Self {
    Self {
      keys: Vec::new(),
      whole: Vec::new(),
      per_line: 1,
      starts: None,
      extents: Extents::new(Vec::new(), f64::from(spacing)),
      extent,
      estimate: f64::from(extent.length()),
      measured: KeyMap::default(),
      places: RefCell::new(None),
      padding: (f64::from(padding.0), f64::from(padding.1)),
    }
  }

  /// The items, by key in order, the items at `whole` (in order) taking a line each, the rest
  /// up to `per_line` a line: the measures of the keys kept are kept, the others forgotten.
  ///
  /// It costs the items' count.
  ///
  /// # Panics
  ///
  /// In a debug build, where two items share a key.
  pub fn set_items(&mut self, keys: Vec<Key>, whole: Vec<usize>, per_line: usize) {
    // The keys' places are found once a key must be: at once where a measure must be kept, or
    // where a debug build checks the keys apart.
    let places = (cfg!(debug_assertions) || !self.measured.is_empty()).then(|| {
      let places: KeyMap<usize> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| (*key, index))
        .collect();

      debug_assert_eq!(places.len(), keys.len(), "two items of a list share a key");

      places
    });

    if let Some(places) = &places {
      self.measured.retain(|key, _| places.contains_key(key));
    }

    self.keys = keys;
    self.whole = whole;
    self.per_line = per_line.max(1);
    *self.places.borrow_mut() = places;
    self.rebuild();
  }

  /// Lays the items out `spacing` apart, `per_line` a line, with `padding` around them,
  /// where any changed.
  pub fn set_layout(&mut self, spacing: f32, padding: (f32, f32), per_line: usize) {
    let padding = (f64::from(padding.0), f64::from(padding.1));
    let per_line = per_line.max(1);

    self.padding = padding;

    if f64::from(spacing) != self.extents.spacing() || per_line != self.per_line {
      self.extents = Extents::new(Vec::new(), f64::from(spacing));
      self.per_line = per_line;
      self.rebuild();
    }
  }

  /// Takes `extent` for every item from now on; a fixed extent forgets every measure.
  pub fn set_extent(&mut self, extent: ItemExtent) {
    if extent == self.extent {
      return;
    }

    if matches!(extent, ItemExtent::Fixed(_)) {
      self.measured.clear();
    }

    self.extent = extent;
    self.estimate = f64::from(extent.length());
    self.rebuild();
  }

  /// The lines and their extents, from the keys, the whole-line items and the measures.
  fn rebuild(&mut self) {
    self.starts = (self.per_line > 1 || !self.whole.is_empty()).then(|| {
      let mut starts = Vec::with_capacity(self.keys.len() / self.per_line + self.whole.len() + 1);
      let mut whole = self.whole.iter().peekable();
      let mut in_line = 0;

      for index in 0..self.keys.len() {
        let takes_line = whole.next_if_eq(&&index).is_some();

        if takes_line || in_line == 0 || in_line == self.per_line {
          starts.push(index);
          in_line = 0;
        }

        in_line = if takes_line {
          self.per_line
        } else {
          in_line + 1
        };
      }

      starts
    });

    let lines = self.line_count();
    let sizes = (0..lines).map(|line| self.line_extent(line)).collect();

    self.extents = Extents::new(sizes, self.extents.spacing());
  }

  /// A line's extent: the most its items measure, an unmeasured one counting the estimate in
  /// force; a fixed extent's in a column, a row or a grid alike.
  fn line_extent(&self, line: usize) -> f64 {
    if let ItemExtent::Fixed(length) = self.extent {
      return f64::from(length);
    }

    if self.measured.is_empty() {
      return self.estimate;
    }

    self
      .items_of(line)
      .map(|index| {
        self
          .measured
          .get(&self.keys[index])
          .map_or(self.estimate, |measure| f64::from(*measure))
      })
      .fold(0., f64::max)
  }

  /// How many items.
  #[inline(always)]
  #[must_use]
  pub const fn count(&self) -> usize {
    self.keys.len()
  }

  /// How many lines.
  #[must_use]
  pub fn line_count(&self) -> usize {
    match &self.starts {
      Some(starts) => starts.len(),
      None => self.keys.len(),
    }
  }

  /// The item at `index`'s key.
  #[must_use]
  pub fn key(&self, index: usize) -> Option<Key> {
    self.keys.get(index).copied()
  }

  /// Where the item `key` is, looked for at `hint` and near it first, then through the keys'
  /// map (built once per change of the items).
  #[must_use]
  pub fn index_of(&self, key: Key, hint: Option<usize>) -> Option<usize> {
    if let Some(hint) = hint {
      for index in hint.saturating_sub(4)..(hint + 5).min(self.keys.len()) {
        if self.keys[index] == key {
          return Some(index);
        }
      }
    }

    let mut places = self.places.borrow_mut();
    let places = places.get_or_insert_with(|| {
      self
        .keys
        .iter()
        .enumerate()
        .map(|(index, key)| (*key, index))
        .collect::<KeyMap<usize>>()
    });

    places.get(&key).copied()
  }

  /// The line the item at `index` falls in.
  #[must_use]
  pub fn line_of(&self, index: usize) -> usize {
    match &self.starts {
      Some(starts) => starts
        .partition_point(|start| *start <= index)
        .saturating_sub(1),
      None => index,
    }
  }

  /// The items line `line` holds.
  #[must_use]
  pub fn items_of(&self, line: usize) -> Range<usize> {
    match &self.starts {
      Some(starts) => {
        let start = starts.get(line).copied().unwrap_or(self.keys.len());
        let end = starts.get(line + 1).copied().unwrap_or(self.keys.len());

        start..end
      }
      None => line.min(self.keys.len())..(line + 1).min(self.keys.len()),
    }
  }

  /// The items lines `lines` hold.
  #[must_use]
  pub fn items_of_lines(&self, lines: Range<usize>) -> Range<usize> {
    if lines.is_empty() {
      let at = self.items_of(lines.start).start;

      return at..at;
    }

    self.items_of(lines.start).start..self.items_of(lines.end - 1).end
  }

  /// Takes `size` as the measure of the item at `index` along the axis, where its extent is
  /// estimated: whether its line's extent changed.
  pub fn measure(&mut self, index: usize, size: f32) -> bool {
    if matches!(self.extent, ItemExtent::Fixed(_)) {
      return false;
    }

    let Some(key) = self.key(index) else {
      return false;
    };

    if self.measured.insert(key, size) == Some(size) {
      return false;
    }

    let line = self.line_of(index);
    let extent = self.line_extent(line);

    if self.extents.size(line) == extent {
      return false;
    }

    self.extents.set(line, extent);

    true
  }

  /// The measure of the item `key`, where it was measured.
  #[must_use]
  pub fn measured(&self, key: Key) -> Option<f32> {
    self.measured.get(&key).copied()
  }

  /// Takes the mean of the measured items as the estimate for the unmeasured ones, where it
  /// moved by half a pixel or more: what a list does while it is at rest, the anchor absorbing
  /// the change. Whether it moved. It costs the items' count where it did.
  pub fn rebase(&mut self) -> bool {
    if matches!(self.extent, ItemExtent::Fixed(_)) || self.measured.is_empty() {
      return false;
    }

    let mean = self
      .measured
      .values()
      .map(|size| f64::from(*size))
      .sum::<f64>()
      / self.measured.len() as f64;

    if (mean - self.estimate).abs() < 0.5 {
      return false;
    }

    self.estimate = mean;
    self.rebuild();

    true
  }

  /// The extent it takes for each item.
  #[inline(always)]
  #[must_use]
  pub const fn extent(&self) -> ItemExtent {
    self.extent
  }

  /// The estimate for an unmeasured item in force.
  #[inline(always)]
  #[must_use]
  pub const fn estimate(&self) -> f64 {
    self.estimate
  }

  /// Line `line`'s extent.
  #[must_use]
  pub fn line_size(&self, line: usize) -> f64 {
    self.extents.size(line)
  }

  /// The spacing between lines.
  #[must_use]
  pub const fn spacing(&self) -> f64 {
    self.extents.spacing()
  }

  /// The padding before the first line and after the last.
  #[inline(always)]
  #[must_use]
  pub const fn padding(&self) -> (f64, f64) {
    self.padding
  }

  /// Where line `line` starts, from the content's leading edge (its padding included).
  #[must_use]
  pub fn line_start(&self, line: usize) -> f64 {
    self.padding.0 + self.extents.start(line)
  }

  /// Where line `line` ends, from the content's leading edge.
  #[must_use]
  pub fn line_end(&self, line: usize) -> f64 {
    self.padding.0 + self.extents.end(line)
  }

  /// The content's whole extent, its padding included.
  #[must_use]
  pub fn total(&self) -> f64 {
    self.padding.0 + self.extents.total() + self.padding.1
  }

  /// The extent of lines `lines` from the first's start to the last's end.
  #[must_use]
  pub fn span(&self, lines: Range<usize>) -> f64 {
    if lines.is_empty() {
      0.
    } else {
      self.extents.end(lines.end - 1) - self.extents.start(lines.start)
    }
  }

  /// The line holding `offset` from the content's leading edge, the first before it and the
  /// last past it; none where there are no lines.
  #[must_use]
  pub fn line_at(&self, offset: f64) -> Option<usize> {
    self.extents.line_at(offset - self.padding.0)
  }

  /// The lines a viewport `viewport` long scrolled to `offset` shows.
  #[must_use]
  pub fn visible(&self, offset: f64, viewport: f64) -> Range<usize> {
    let (Some(first), Some(last)) = (
      self.line_at(offset),
      self.line_at(offset + viewport.max(0.) - 0.5),
    ) else {
      return 0..0;
    };

    first..last.max(first) + 1
  }

  /// The lines a viewport `viewport` long scrolled to `offset` realizes: those it shows and
  /// `overscan` more beyond each edge (at least one).
  #[must_use]
  pub fn window(&self, offset: f64, viewport: f64, overscan: usize) -> Range<usize> {
    let visible = self.visible(offset, viewport);
    let overscan = overscan.max(1);

    if visible.is_empty() {
      return 0..0;
    }

    visible.start.saturating_sub(overscan)..(visible.end + overscan).min(self.line_count())
  }

  /// The anchor of a viewport `viewport` long scrolled to `offset`: the item at `preferred`
  /// where it is in view, else the first item of the first line in view; none where nothing
  /// is in view.
  #[must_use]
  pub fn anchor(&self, offset: f64, viewport: f64, preferred: Option<usize>) -> Option<Anchor> {
    let visible = self.visible(offset, viewport);

    if visible.is_empty() {
      return None;
    }

    let index = preferred
      .filter(|index| visible.contains(&self.line_of(*index)))
      .unwrap_or_else(|| self.items_of(visible.start).start);
    let key = self.key(index)?;

    Some(Anchor {
      key,
      index,
      distance: self.line_start(self.line_of(index)) - offset,
    })
  }

  /// The offset that keeps `anchor` where it was in the viewport, the content's edges aside;
  /// none where its item is gone.
  #[must_use]
  pub fn anchored(&self, anchor: Anchor) -> Option<f64> {
    let index = self.index_of(anchor.key, Some(anchor.index))?;

    Some(self.line_start(self.line_of(index)) - anchor.distance)
  }

  /// The offset that places the item at `index` at `alignment` of a viewport `viewport` long,
  /// `offset` from its matching point, along the axis: `0` the start, `0.5` the center and `1`
  /// the end.
  #[must_use]
  pub fn offset_of(&self, index: usize, alignment: f64, offset: f64, viewport: f64) -> f64 {
    let line = self.line_of(index.min(self.count().saturating_sub(1)));
    let start = self.line_start(line);
    let size = self.line_size(line);

    start + size * alignment - viewport * alignment - offset
  }
}

#[cfg(test)]
mod tests {
  use super::Engine;
  use crate::lazy::{ItemExtent, Key};

  fn keys(count: usize) -> Vec<Key> {
    (0..count).map(|index| Key::of(0, &index)).collect()
  }

  #[test]
  fn an_estimated_list_windows_its_viewport_with_overscan() {
    let mut engine = Engine::new(ItemExtent::Estimated(50.), 0., (0., 0.));

    engine.set_items(keys(100_000), Vec::new(), 1);

    assert_eq!(engine.total(), 5_000_000.);
    assert_eq!(engine.visible(0., 300.), 0..6);
    assert_eq!(engine.window(0., 300., 2), 0..8);
    assert_eq!(engine.window(2_500_000., 300., 2), 49_998..50_008);
    assert_eq!(engine.window(4_999_700., 300., 2), 99_992..100_000);
  }

  #[test]
  fn a_measure_moves_the_lines_after_it_and_the_anchor_keeps_the_view_still() {
    let mut engine = Engine::new(ItemExtent::Estimated(50.), 4., (8., 8.));

    engine.set_items(keys(1000), Vec::new(), 1);

    let offset = engine.line_start(500) + 20.;
    let anchor = engine.anchor(offset, 300., None).expect("an anchor");

    assert_eq!(anchor.index, 500);
    assert_eq!(anchor.distance, -20.);

    // Rows above the anchor measure taller: the anchor's line moves down by as much.
    assert!(engine.measure(498, 80.));
    assert!(engine.measure(499, 70.));
    assert!(
      !engine.measure(499, 70.),
      "the same measure changes nothing"
    );

    let moved = engine.anchored(anchor).expect("the anchor stays");

    assert_eq!(moved - offset, 50.);
    assert_eq!(engine.line_start(500), 8. + 500. * 54. + 50.);
  }

  #[test]
  fn the_anchor_follows_its_item_through_insertions_and_falls_back_when_it_is_gone() {
    let mut engine = Engine::new(ItemExtent::Fixed(10.), 0., (0., 0.));
    let mut items = keys(100);

    engine.set_items(items.clone(), Vec::new(), 1);

    let anchor = engine.anchor(505., 50., None).expect("an anchor");

    assert_eq!(anchor.index, 50);

    // Three items inserted above.
    for at in 0..3 {
      items.insert(0, Key::of(1, &at));
    }

    engine.set_items(items.clone(), Vec::new(), 1);
    assert_eq!(engine.anchored(anchor), Some(535.));

    items.retain(|key| *key != anchor.key);
    engine.set_items(items, Vec::new(), 1);
    assert_eq!(engine.anchored(anchor), None);
  }

  #[test]
  fn a_grid_lays_its_items_in_lines_and_a_header_takes_one_alone() {
    let mut engine = Engine::new(ItemExtent::Estimated(100.), 8., (0., 0.));

    // A header, seven cells, a header, two cells: three a line.
    engine.set_items(keys(11), vec![0, 8], 3);

    assert_eq!(engine.line_count(), 6);
    assert_eq!(engine.items_of(0), 0..1);
    assert_eq!(engine.items_of(1), 1..4);
    assert_eq!(engine.items_of(3), 7..8);
    assert_eq!(engine.items_of(4), 8..9);
    assert_eq!(engine.items_of(5), 9..11);
    assert_eq!(engine.line_of(6), 2);
    assert_eq!(engine.line_of(10), 5);

    // A line is as tall as its tallest item.
    assert!(engine.measure(2, 140.));
    assert!(!engine.measure(3, 120.), "a shorter cell leaves its line");
    assert_eq!(engine.line_size(1), 140.);

    // Four a line: the lines recompose and keep the measures.
    engine.set_layout(8., (0., 0.), 4);
    assert_eq!(engine.items_of(1), 1..5);
    assert_eq!(engine.line_size(1), 140.);
  }

  #[test]
  fn rebasing_takes_the_measured_mean_for_the_unmeasured() {
    let mut engine = Engine::new(ItemExtent::Estimated(40.), 0., (0., 0.));

    engine.set_items(keys(10), Vec::new(), 1);

    for index in 0..4 {
      engine.measure(index, 60.);
    }

    assert!(engine.rebase());
    assert_eq!(engine.estimate(), 60.);
    assert_eq!(engine.total(), 600.);
    assert!(!engine.rebase());
  }

  #[test]
  fn an_item_is_placed_at_the_start_center_or_end_of_the_viewport() {
    let mut engine = Engine::new(ItemExtent::Fixed(20.), 0., (0., 0.));

    engine.set_items(keys(100), Vec::new(), 1);

    assert_eq!(engine.offset_of(50, 0., 0., 200.), 1000.);
    assert_eq!(engine.offset_of(50, 0.5, 0., 200.), 910.);
    assert_eq!(engine.offset_of(50, 1., 0., 200.), 820.);
  }
}
