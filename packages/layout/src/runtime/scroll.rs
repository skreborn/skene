//! What the drawn runtime keeps of scrolling: each box's overflow and its scroll offsets,
//! clamped to the limits its last layout allows, a text entry's window standing for its
//! offset through the platform's own state of it, the writes that move them with the
//! notification their scroll handlers hear, and the node a frame scrolls into view with
//! the offsets that reveal it.

use skene_foundation::{Axes, NodeId, Overflow, Point, Rect, ScrollEvent, Size, notify_scroll};

use super::{Drawn, Runtime};

impl<P: Drawn> Runtime<P> {
  /// A node's overflow per axis as CSS computes it, `Visible` when unset or the node has
  /// no box style.
  pub fn overflow(&self, node: NodeId) -> Axes<Overflow> {
    self
      .record(node)
      .and_then(|record| record.kind.box_style())
      .and_then(|style| style.overflow)
      .map_or(Axes::all(Overflow::Visible), Axes::computed)
  }

  /// Whether the node's content scrolls: a box overflowing by its style on either axis,
  /// or a text entry whose window the platform scrolls as an offset
  /// ([`Drawn::entry_scroll`]).
  #[must_use]
  pub fn is_scrollable(&self, node: NodeId) -> bool {
    self.entry_scroll(node).is_some() || self.overflow(node).scrolls()
  }

  /// Whether the node clips its children when painting.
  #[must_use]
  pub fn clips(&self, node: NodeId) -> bool {
    self.overflow(node) != Axes::all(Overflow::Visible)
  }

  /// A node's scroll offsets, clamped to what the last layout allows; a text entry's is
  /// its window as the platform scrolls it ([`Drawn::entry_scroll`]).
  pub fn scroll_offset(&self, node: NodeId) -> Point<f32> {
    if let Some((window, _)) = self.entry_scroll(node) {
      return window;
    }

    let Some(offset) = self.record(node).map(|record| record.scroll)
    else {
      return Point::ZERO;
    };

    if offset == Point::ZERO {
      return Point::ZERO;
    }

    let limits = self.scroll_limits(node);

    Point::new(offset.x.min(limits.width), offset.y.min(limits.height))
  }

  /// A node's maximum scroll offsets, from the last computed layout, in whole units where
  /// the platform's layout rounds ([`Drawn::ROUNDED`]) and zero on an axis that does not
  /// scroll; a text entry's is how far its window can scroll ([`Drawn::entry_scroll`]).
  pub fn scroll_limits(&self, node: NodeId) -> Size<f32> {
    if let Some((_, limit)) = self.entry_scroll(node) {
      return limit;
    }

    let overflow = self.overflow(node);
    let Some(layout) = self.tree.layout(node)
    else {
      return Size::ZERO;
    };
    let limit = |scrolls: bool, extent: f32| {
      if !scrolls {
        0.
      }
      else if P::ROUNDED {
        extent.round().max(0.)
      }
      else {
        extent.max(0.)
      }
    };

    Size::new(
      limit(overflow.horizontal.scrolls(), layout.scroll_limit.width),
      limit(overflow.vertical.scrolls(), layout.scroll_limit.height),
    )
  }

  /// The offsets `node` would scroll to by `delta`, clamped to its limits; `None` when it
  /// does not scroll or cannot move further.
  pub fn scrolled_by(&self, node: NodeId, delta: Point<f32>) -> Option<Point<f32>> {
    if !self.is_scrollable(node) {
      return None;
    }

    let limits = self.scroll_limits(node);
    let offset = self.scroll_offset(node);
    let next = Point::new(
      (offset.x + delta.x).min(limits.width).max(0.),
      (offset.y + delta.y).min(limits.height).max(0.),
    );

    (next != offset).then_some(next)
  }

  /// Stores a node's offsets, damaging it and moving the geometry; `true` where the
  /// caller notifies its scroll handlers. A text entry's window moves instead
  /// ([`Drawn::scroll_entry`]), notifying nothing.
  fn write_scroll(&mut self, node: NodeId, offset: Point<f32>) -> bool {
    self.damage(node);
    self.geometry_stale = true;

    let content = self.content_size(node);
    let Some(record) = self.records.get_mut(node)
    else {
      return false;
    };

    if record.kind.is_text_entry() && P::entry_scroll(node, &record.platform, content).is_some() {
      P::scroll_entry(node, &mut record.platform, content, offset);

      return false;
    }

    record.scroll = offset;

    true
  }

  /// Writes `offset` into `node` where it moves it and notifies its scroll handlers, once
  /// the platform's runtime is released; a text entry's window moves notifying nothing.
  ///
  /// # Panics
  ///
  /// Panics if no runtime is installed for this thread, or if the platform's state is
  /// borrowed already.
  pub fn scroll_to(node: NodeId, offset: Point<f32>) {
    let written = Self::with(|runtime| {
      runtime.scroll_offset(node) != offset && runtime.write_scroll(node, offset)
    });

    if written {
      notify_scroll::<P>(
        node,
        &ScrollEvent {
          x: offset.x,
          y: offset.y,
        },
      );
    }
  }

  /// Scrolls `node` by `delta`, clamped to its limits, as
  /// [`scroll_to`](Self::scroll_to) does; `true` when it moved.
  ///
  /// # Panics
  ///
  /// Panics if no runtime is installed for this thread, or if the platform's state is
  /// borrowed already.
  #[must_use]
  pub fn scroll_by(node: NodeId, delta: Point<f32>) -> bool {
    let Some(offset) = Self::with(|runtime| runtime.scrolled_by(node, delta))
    else {
      return false;
    };

    Self::scroll_to(node, offset);

    true
  }

  /// The size of `node`'s content box, inside its border and padding, as the last layout
  /// placed it; zero for a node outside the layout.
  pub fn content_size(&self, node: NodeId) -> Size<f32> {
    self
      .tree
      .layout(node)
      .map_or(Size::ZERO, |layout| layout.content_size())
  }

  /// Queues `node` to be scrolled into view once the next layout is computed, in place
  /// of any node queued before it.
  #[inline(always)]
  pub const fn reveal(&mut self, node: NodeId) {
    self.reveal = Some(node);
  }

  /// Takes the node queued to be scrolled into view, `None` where none is.
  #[inline(always)]
  pub const fn take_reveal(&mut self) -> Option<NodeId> {
    self.reveal.take()
  }

  /// The scroll offsets each scrollable laid-out ancestor of `node` takes to show it,
  /// nearest first, as the last layout placed them, holding only those that move. A run
  /// flowing in a paragraph is shown as its paragraph.
  ///
  /// Each ancestor scrolls by as little as shows the target between its near and its far
  /// border, within its padding box; a target larger than that shows from its start.
  pub fn reveal_offsets(&self, node: NodeId) -> Vec<(NodeId, Point<f32>)> {
    let node = self.laid_owner(node);
    let target = self
      .tree
      .layout(node)
      .map_or_else(Rect::default, |layout| layout.rect);
    let mut relative = target.origin;
    let mut adjusted = Vec::new();
    let mut current = self.layout_parent(node);

    while let Some(ancestor) = current {
      let (frame, border) = self
        .tree
        .layout(ancestor)
        .map_or_else(Default::default, |layout| (layout.rect, layout.border));
      let mut offset = self.scroll_offset(ancestor);

      if self.is_scrollable(ancestor) {
        let limits = self.scroll_limits(ancestor);
        let next = Point::new(
          fit_axis(
            offset.x,
            relative.x,
            target.size.width,
            frame.size.width,
            (border.start, border.end),
            limits.width,
          ),
          fit_axis(
            offset.y,
            relative.y,
            target.size.height,
            frame.size.height,
            (border.top, border.bottom),
            limits.height,
          ),
        );

        if next != offset {
          adjusted.push((ancestor, next));
        }

        offset = next;
      }

      relative = Point::new(
        relative.x + frame.origin.x - offset.x,
        relative.y + frame.origin.y - offset.y,
      );
      current = self.layout_parent(ancestor);
    }

    adjusted
  }

  /// The nearest node at or above `node` the tree lays out: `node` itself, or the
  /// paragraph a text or group run flows in.
  fn laid_owner(&self, node: NodeId) -> NodeId {
    if self.is_inline_run(node) && !self.is_inline_box(node) {
      return self.enclosing_paragraph(node).unwrap_or(node);
    }

    node
  }

  /// A text entry's window as the platform scrolls it, as an offset and its limit, in
  /// its content box ([`Drawn::entry_scroll`]); `None` for any other node.
  fn entry_scroll(&self, node: NodeId) -> Option<(Point<f32>, Size<f32>)> {
    let record = self
      .record(node)
      .filter(|record| record.kind.is_text_entry())?;

    P::entry_scroll(node, &record.platform, self.content_size(node))
  }
}

/// The nearest offset along one axis showing the target `[relative, relative + extent)`,
/// placed from its scroll box's border-box corner before any scrolling, within a box of
/// `size` whose near and far borders (`borders`) bound what shows, clamped to
/// `[0, limit]`; a target larger than what shows aligns to its start.
const fn fit_axis(
  offset: f32,
  relative: f32,
  extent: f32,
  size: f32,
  borders: (f32, f32),
  limit: f32,
) -> f32 {
  let min_offset = relative + extent - size + borders.1;
  let max_offset = relative - borders.0;
  let next = if min_offset > max_offset || offset > max_offset {
    max_offset
  }
  else if offset < min_offset {
    min_offset
  }
  else {
    offset
  };

  next.min(limit).max(0.)
}

#[cfg(test)]
mod tests {
  use super::fit_axis;

  /// A target ten units below a scroll box's corner, two units tall, in a box five units
  /// tall that scrolls as far as twenty: past its bottom while the box is unscrolled.
  const BELOW: (f32, f32, f32, f32) = (10., 2., 5., 20.);

  /// Fits the target `BELOW` places at `offset` behind `borders`.
  const fn fit_below(offset: f32, borders: (f32, f32)) -> f32 {
    let (relative, extent, size, limit) = BELOW;

    fit_axis(offset, relative, extent, size, borders, limit)
  }

  #[test]
  fn equal_borders_bound_both_edges() {
    assert_eq!(fit_below(0., (1., 1.)), 8.);
    assert_eq!(fit_below(20., (1., 1.)), 9.);
  }

  #[test]
  fn a_far_border_alone_bounds_the_far_edge() {
    assert_eq!(fit_below(0., (0., 1.)), 8.);
    assert_eq!(fit_below(20., (0., 1.)), 10.);
  }

  #[test]
  fn a_near_border_alone_bounds_the_near_edge() {
    assert_eq!(fit_below(0., (1., 0.)), 7.);
    assert_eq!(fit_below(20., (1., 0.)), 9.);
  }

  #[test]
  fn an_oversized_target_shows_from_its_start() {
    assert_eq!(fit_axis(0., 10., 8., 5., (1., 1.), 20.), 9.);
    assert_eq!(fit_axis(20., 10., 8., 5., (1., 1.), 20.), 9.);
    assert_eq!(fit_axis(9.5, 10., 4., 5., (1., 1.), 20.), 9.);
  }

  #[test]
  fn an_offset_already_showing_the_target_stays() {
    assert_eq!(fit_below(8.5, (1., 1.)), 8.5);
    assert_eq!(fit_below(10., (0., 1.)), 10.);
    assert_eq!(fit_axis(3., 4., 1., 5., (0., 0.), 20.), 3.);
  }

  #[test]
  fn the_offset_clamps_at_zero() {
    assert_eq!(fit_axis(4., 1., 1., 5., (2., 0.), 20.), 0.);
  }

  #[test]
  fn the_offset_clamps_at_the_limit() {
    assert_eq!(fit_axis(0., 10., 2., 5., (0., 1.), 6.), 6.);
  }
}
