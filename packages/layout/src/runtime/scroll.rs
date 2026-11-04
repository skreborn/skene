//! What the drawn runtime keeps of scrolling: each box's overflow and its scroll offsets,
//! clamped to the limits its last layout allows, a text entry's window standing for its
//! offset through the platform's own state of it, and the writes that move them with
//! the notification their scroll handlers hear.

use skene_foundation::{Axes, NodeId, Overflow, Point, ScrollEvent, Size, notify_scroll};

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

  /// A text entry's window as the platform scrolls it, as an offset and its limit, in
  /// its content box ([`Drawn::entry_scroll`]); `None` for any other node.
  fn entry_scroll(&self, node: NodeId) -> Option<(Point<f32>, Size<f32>)> {
    let record = self
      .record(node)
      .filter(|record| record.kind.is_text_entry())?;

    P::entry_scroll(node, &record.platform, self.content_size(node))
  }
}
