use skene_foundation::{
  NodeId,
  Rect,
  Size,
  text::{Paragraph, ParagraphStyle, TextEngine, TextStyle},
};
use taffy::prelude::AvailableSpace;

use crate::{Leaf, TextFlow};

/// A text leaf of the drawn runtime: a paragraph's runs and inline boxes laid out as one
/// text through the platform's engine `E`.
///
/// Its [`TextFlow`] is made into a paragraph once a measure, a paint or a hit first asks,
/// and kept between frames. A flow in whole units lays its text out the way a layout rounded to
/// whole units lays out its boxes: measured and placed at its width floored to whole units, at
/// least one, and each inline box as large as the tree lays it out rounded to whole units, at least
/// one of each; a flow in fractional units keeps both as they are. Two flows are alike
/// where they were made under one text generation of the runtime and their flows are:
/// each run's node, groups, text and style, and each box's node, groups and size.
#[must_use]
pub struct Flow<E: TextEngine> {
  generation: u64,
  whole: bool,
  text: TextFlow<E>,
}

impl<E: TextEngine> Flow<E> {
  /// An empty flow laid out through `engine` in `style`, a line holding no run laid out in
  /// `base`, in whole units where `whole` holds, made under the runtime's text generation
  /// `generation`.
  pub const fn new(
    engine: E,
    base: TextStyle,
    style: ParagraphStyle,
    whole: bool,
    generation: u64,
  ) -> Self {
    Self {
      generation,
      whole,
      text: TextFlow::new(engine, base, style),
    }
  }

  /// Adds the run `node` holding `text` in `style`, inside `groups`, outermost first, after
  /// everything the flow holds.
  #[inline(always)]
  pub fn add_run(&mut self, node: NodeId, groups: &[NodeId], text: &str, style: TextStyle) {
    self.text.add_run(node, groups, text, style);
  }

  /// Adds the inline box `node`, inside `groups`, outermost first, after everything the
  /// flow holds, unsized until the tree sizes it: one unit by one in whole units.
  pub fn add_box(&mut self, node: NodeId, groups: &[NodeId]) {
    self.text.add_box(node, groups, self.box_size(Size::ZERO));
  }

  /// The width a flow in whole units lays out across when laid out `width` wide: `width`
  /// floored to whole units, at least one.
  #[must_use]
  pub const fn whole_width(width: f32) -> f32 {
    width.max(0.).floor().max(1.)
  }

  /// The width the flow lays out across when laid out `width` wide.
  const fn width(&self, width: f32) -> f32 {
    if self.whole {
      Self::whole_width(width)
    }
    else {
      width
    }
  }

  /// The size an inline box laid out at `size` takes in the flow.
  const fn box_size(&self, size: Size<f32>) -> Size<f32> {
    if self.whole {
      Size::new(
        (size.width + 0.5).floor().max(1.),
        (size.height + 0.5).floor().max(1.),
      )
    }
    else {
      size
    }
  }

  /// The flow's content box laid out at `content`, its width as the flow lays out across.
  const fn content(&self, content: Rect<f32>) -> Rect<f32> {
    Rect::new(
      content.origin,
      Size::new(self.width(content.size.width), content.size.height),
    )
  }

  /// The flow the leaf lays out.
  #[inline(always)]
  pub const fn flow(&self) -> &TextFlow<E> {
    &self.text
  }

  /// The flow laid out in a content box at `content`, across the width it measures and
  /// places at, its first line's top at the content box's top.
  pub fn laid(&self, content: Rect<f32>) -> <E::Paragraph as Paragraph>::Laid {
    self.text.layout_in(self.content(content)).0
  }
}

impl<E: TextEngine> std::fmt::Debug for Flow<E> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Flow")
      .field("generation", &self.generation)
      .field("whole", &self.whole)
      .field("text", &self.text)
      .finish()
  }
}

impl<E: TextEngine> PartialEq for Flow<E> {
  fn eq(&self, other: &Self) -> bool {
    self.generation == other.generation && self.whole == other.whole && self.text == other.text
  }
}

impl<E: TextEngine> Leaf for Flow<E> {
  /// The flow measured across the width it lays out across.
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    available: taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32> {
    let measured = self.text.measure(
      taffy::geometry::Size {
        width: known.width.map(|width| self.width(width)),
        height: known.height,
      },
      taffy::geometry::Size {
        width: match available.width {
          AvailableSpace::Definite(width) => AvailableSpace::Definite(self.width(width)),
          space => space,
        },
        height: available.height,
      },
    );

    Size::new(known.width.unwrap_or(measured.width), measured.height)
  }

  /// The flow's places, its content box as wide as it lays out across.
  fn place(&self, content: Rect<f32>) -> Vec<(NodeId, Rect<f32>)> {
    self.text.place(self.content(content))
  }

  /// The flow's inline boxes.
  fn boxes(&self) -> impl Iterator<Item = NodeId> {
    self.text.boxes()
  }

  /// Sizes the inline box `node` as the flow takes a box laid out at `size`.
  fn size_box(&mut self, node: NodeId, size: Size<f32>) {
    let size = self.box_size(size);

    self.text.size_box(node, size);
  }

  /// Takes `next`'s paint where both were made under one text generation in the same
  /// units and their flows lay out alike.
  fn repaint_as(&mut self, next: &Self) -> bool {
    self.generation == next.generation
      && self.whole == next.whole
      && self.text.repaint_as(&next.text)
  }
}

/// What the drawn runtime's layout measures a leaf by: a text leaf's flow, or a text
/// entry's native extent, its content box's size, as an `<input>`'s intrinsic size.
#[must_use]
pub enum Measured<E: TextEngine> {
  /// A text leaf, measured by its flow.
  Flow(Box<Flow<E>>),
  /// A text entry, measured at its native extent.
  Entry(Size<f32>),
}

impl<E: TextEngine> Measured<E> {
  /// The flow a text leaf lays out as; `None` for an entry.
  #[inline(always)]
  #[must_use]
  pub fn flow(&self) -> Option<&Flow<E>> {
    match self {
      Self::Flow(flow) => Some(flow),
      Self::Entry(_) => None,
    }
  }
}

impl<E: TextEngine> std::fmt::Debug for Measured<E> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Flow(flow) => formatter.debug_tuple("Flow").field(flow).finish(),
      Self::Entry(native) => formatter.debug_tuple("Entry").field(native).finish(),
    }
  }
}

impl<E: TextEngine> PartialEq for Measured<E> {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Flow(held), Self::Flow(flow)) => held == flow,
      (Self::Entry(held), Self::Entry(native)) => held == native,
      _ => false,
    }
  }
}

impl<E: TextEngine> Leaf for Measured<E> {
  /// A flow's measure, or an entry's native extent in each dimension not `known`.
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    available: taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32> {
    match self {
      Self::Flow(flow) => flow.measure(known, available),
      Self::Entry(native) => Size::new(
        known.width.unwrap_or(native.width),
        known.height.unwrap_or(native.height),
      ),
    }
  }

  /// A flow's places; an entry places none.
  fn place(&self, content: Rect<f32>) -> Vec<(NodeId, Rect<f32>)> {
    match self {
      Self::Flow(flow) => flow.place(content),
      Self::Entry(_) => Vec::new(),
    }
  }

  /// A flow's inline boxes; an entry names none.
  fn boxes(&self) -> impl Iterator<Item = NodeId> {
    self.flow().into_iter().flat_map(Leaf::boxes)
  }

  /// Sizes a flow's inline box `node`; an entry holds none.
  fn size_box(&mut self, node: NodeId, size: Size<f32>) {
    if let Self::Flow(flow) = self {
      flow.size_box(node, size);
    }
  }

  /// A flow takes `next`'s paint as [`Flow`] does; an entry keeps no paint of its own.
  fn repaint_as(&mut self, next: &Self) -> bool {
    match (self, next) {
      (Self::Flow(held), Self::Flow(flow)) => held.repaint_as(flow),
      _ => false,
    }
  }
}

#[cfg(test)]
mod tests {
  use std::sync::atomic::{AtomicU32, Ordering};

  use skene_foundation::{
    NodeId,
    Point,
    Rect,
    Size,
    text::{Laid as _, LineHeight, ParagraphStyle, TextStyle},
  };
  use skene_parley::{ParleyEngine, Typeface};
  use taffy::prelude::AvailableSpace;

  use super::{Flow, Measured};
  use crate::Leaf;

  /// The face the tests lay text out in.
  const INTER: Typeface = Typeface::new(
    "Inter",
    include_bytes!("../../../parley/fonts/inter-regular.otf"),
  );

  /// The text the tests lay out: two words a line holds side by side.
  const TEXT: &str = "wide words";

  /// A node no other node of the tests is.
  fn node() -> NodeId {
    static NEXT: AtomicU32 = AtomicU32::new(0);

    NodeId::new(NEXT.fetch_add(1, Ordering::Relaxed), 0)
  }

  /// The style of the tests' text: the face at 16, its lines 24 tall.
  fn style() -> TextStyle {
    TextStyle::new(16.).with_line_height(LineHeight::Absolute(24.))
  }

  /// An empty flow over the face alone, its line metrics fractional, in whole units where
  /// `whole` holds, made under `generation`.
  fn empty(whole: bool, generation: u64) -> Flow<ParleyEngine> {
    let engine = ParleyEngine::with_typefaces(&[INTER]);

    engine.set_quantize(false);

    Flow::new(
      engine,
      style(),
      ParagraphStyle::default(),
      whole,
      generation,
    )
  }

  /// A flow of one run, `run`, holding the tests' text, in whole units where `whole`
  /// holds.
  fn flow(run: NodeId, whole: bool) -> Flow<ParleyEngine> {
    let mut flow = empty(whole, 0);

    flow.add_run(run, &[], TEXT, style());

    flow
  }

  /// How tall `flow` measures `width` wide.
  fn height_at(flow: &Flow<ParleyEngine>, width: f32) -> f32 {
    flow
      .measure(
        taffy::geometry::Size {
          width: None,
          height: None,
        },
        taffy::geometry::Size {
          width: AvailableSpace::Definite(width),
          height: AvailableSpace::MaxContent,
        },
      )
      .height
  }

  /// Where `flow` places its nodes in a content box `width` wide.
  fn placed(flow: &Flow<ParleyEngine>, width: f32) -> Vec<(NodeId, Rect<f32>)> {
    flow.place(Rect::new(Point::ZERO, Size::new(width, 0.)))
  }

  /// A flow in whole units measures and places at its width floored to whole units, so a
  /// line exactly as wide as its text wraps where the width's fraction is cut off, and a
  /// width below one lays out one unit wide.
  #[test]
  fn a_whole_unit_flow_lays_out_across_its_floored_width() {
    let run = node();
    let flow = flow(run, true);
    let line = flow.flow().leaf().laid(None).size().width;

    assert!(line.fract() > 0., "the text's line is {line} wide");
    assert_eq!(height_at(&flow, line), height_at(&flow, line.floor()));
    assert_eq!(height_at(&flow, line), 48.);
    assert_eq!(height_at(&flow, line.ceil()), 24.);
    assert_eq!(placed(&flow, line), placed(&flow, line.floor()));
    assert_eq!(placed(&flow, 0.25), placed(&flow, 1.));
    assert_eq!(
      flow
        .laid(Rect::new(Point::ZERO, Size::new(line, 0.)))
        .size(),
      flow
        .laid(Rect::new(Point::ZERO, Size::new(line.floor(), 0.)))
        .size()
    );
  }

  /// A flow in whole units sizes an inline box to whole units, at least one of each.
  #[test]
  fn a_whole_unit_flow_sizes_its_boxes_in_whole_units() {
    let inline = node();
    let mut flow = empty(true, 0);

    flow.add_box(inline, &[]);

    let sized = |flow: &Flow<ParleyEngine>| {
      placed(flow, 100.)
        .into_iter()
        .find_map(|(node, rect)| (node == inline).then_some(rect.size))
        .expect("the flow places its box")
    };

    assert_eq!(sized(&flow), Size::new(1., 1.));

    flow.size_box(inline, Size::new(2.6, 0.2));

    assert_eq!(sized(&flow), Size::new(3., 1.));
  }

  /// A flow in fractional units measures and places across its whole width and sizes an
  /// inline box at the size it is laid out at.
  #[test]
  fn a_fractional_flow_keeps_its_widths_and_box_sizes() {
    let (run, inline) = (node(), node());
    let mut flow = flow(run, false);
    let line = flow.flow().leaf().laid(None).size().width;

    assert_eq!(height_at(&flow, line), 24.);
    assert_eq!(
      placed(&flow, line),
      flow
        .flow()
        .place(Rect::new(Point::ZERO, Size::new(line, 0.)))
    );

    flow.add_box(inline, &[]);
    flow.size_box(inline, Size::new(2.6, 0.2));

    let size = placed(&flow, 1000.)
      .into_iter()
      .find_map(|(node, rect)| (node == inline).then_some(rect.size));

    assert_eq!(size, Some(Size::new(2.6, 0.2)));
  }

  /// Two flows of the same runs are alike under one text generation and unlike under two,
  /// and neither takes the other's paint across them.
  #[test]
  fn a_flow_made_under_another_generation_is_unlike() {
    let run = node();
    let make = |generation| {
      let mut flow = empty(true, generation);

      flow.add_run(run, &[], TEXT, style());

      flow
    };
    let mut held = make(0);

    assert_eq!(held, make(0));
    assert_ne!(held, make(1));
    assert!(!held.repaint_as(&make(1)));
  }

  /// An entry measures at its native extent in each dimension the layout leaves unknown,
  /// and places nothing.
  #[test]
  fn an_entry_measures_at_its_native_extent() {
    let entry = Measured::<ParleyEngine>::Entry(Size::new(9., 2.));
    let available = taffy::geometry::Size {
      width: AvailableSpace::Definite(40.),
      height: AvailableSpace::MaxContent,
    };

    assert_eq!(
      entry.measure(
        taffy::geometry::Size {
          width: None,
          height: None,
        },
        available,
      ),
      Size::new(9., 2.)
    );
    assert_eq!(
      entry.measure(
        taffy::geometry::Size {
          width: Some(20.),
          height: None,
        },
        available,
      ),
      Size::new(20., 2.)
    );
    assert_eq!(
      entry.place(Rect::new(Point::ZERO, Size::new(40., 2.))),
      Vec::new()
    );
    assert_eq!(entry.boxes().count(), 0);
  }
}
