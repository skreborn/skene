//! A vertically scrolling list rendering only its visible slice.

use std::rc::Rc;

use skene_core::{AnyView, BoxStyle, Condition, IntoView};

use crate::HasSize;

/// A scrollable column of `count()` fixed-height items, only the visible slice (plus an
/// overscan margin) built at any moment; the rest is empty space.
///
/// Items are keyed by index: scrolling reuses the views whose indices stay in the
/// window, and an item expresses its own reactivity with `derive_` properties as usual.
/// A server render measures no viewport, so it serializes only the overscan margin.
#[must_use]
pub struct LazyColumn {
  item_height: f32,
  count: Rc<dyn Fn() -> usize>,
  item: Rc<dyn Fn(usize) -> AnyView>,
  style: BoxStyle,
  variants: Vec<(Condition, BoxStyle)>,
  overscan: usize,
}

impl LazyColumn {
  /// Creates a list of `count()` items, each `item_height` logical pixels tall, built
  /// by `item`.
  pub fn new<V: IntoView>(
    item_height: f32,
    count: impl Fn() -> usize + 'static,
    item: impl Fn(usize) -> V + 'static,
  ) -> Self {
    Self {
      item_height,
      count: Rc::new(count),
      item: Rc::new(move |index| item(index).into_view()),
      style: BoxStyle::default(),
      variants: Vec::new(),
      overscan: 3,
    }
  }

  /// Sets the scroll box's static style; its overflow defaults to `Auto`.
  #[inline(always)]
  pub fn set_style(mut self, style: BoxStyle) -> Self {
    self.style = style;

    self
  }

  /// Adds a conditional style variant, applied while `condition` holds.
  #[inline(always)]
  pub fn set_when(mut self, condition: Condition, style: BoxStyle) -> Self {
    self.variants.push((condition, style));

    self
  }

  /// Sets how many extra items build beyond each edge of the visible slice.
  #[inline(always)]
  pub fn set_overscan(mut self, overscan: usize) -> Self {
    self.overscan = overscan;

    self
  }
}

impl skene_core::View for LazyColumn {
  fn build(self) -> skene_core::NodeId {
    use skene_core::{BoxProps, HasChildren, Layout, Overflow, ScrollEvent, TreeOps, item_window};
    use skene_reactive::{Get, Memo, Set, Signal};

    use crate::{GuiOps, StyledBox, driver};

    let mut style = self.style;

    style.overflow = Some(style.overflow.unwrap_or(Overflow::Auto));

    let offset = Signal::new(0.0f32);
    let viewport = Signal::new(0.0f32);
    let outer = driver::create_box(&BoxProps {
      layout: Layout::Flow,
      role: None,
      style,
      variants: self.variants,
      transitions: Vec::new(),
      animations: Vec::new(),
    });

    driver::handle_scroll(
      outer,
      Rc::new(move |event: &ScrollEvent| offset.set(event.y)),
    );
    driver::observe_size(outer, Rc::new(move |_, height| viewport.set(height)));

    let item_height = self.item_height;
    let count = self.count;
    let overscan = self.overscan;
    let window = Memo::new(move |_| {
      let count = count();

      (
        item_window(offset.get(), viewport.get(), item_height, count, overscan),
        count,
      )
    });

    #[expect(clippy::cast_precision_loss)]
    let extent = move |items: usize| items as f32 * item_height;
    let leading = StyledBox::new()
      .derive_height(move || extent(window.get().0.start))
      .build();
    let items = GuiOps.create_group();
    let trailing = StyledBox::new()
      .derive_height(move || {
        let (window, count) = window.get();

        extent(count - window.end)
      })
      .build();

    GuiOps.attach(outer, leading, None);
    GuiOps.attach(outer, items, None);
    GuiOps.attach(outer, trailing, None);

    let item = self.item;

    crate::reconcile(
      items,
      move || {
        let (window, _) = window.get();

        window.start..window.end
      },
      |index| *index,
      move |index| {
        StyledBox::new()
          .set_height(item_height)
          .set_child(item(index))
          .build()
      },
    );

    outer
  }
}
