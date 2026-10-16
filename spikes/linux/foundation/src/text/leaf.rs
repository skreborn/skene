//! A text leaf of the layout engine: a paragraph measured as taffy asks, its intrinsic widths
//! cached for the paragraph's life and its layouts by width.

use std::{
  cell::{Cell, RefCell},
  ops::Range,
};

use crate::text::{Intrinsic, Laid, PaintStyle, Paragraph};

/// The space a leaf is measured in along one axis.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Space {
  /// As narrow as it can be: its min-content size.
  MinContent,
  /// As wide as it would be: its max-content size.
  MaxContent,
  /// Within this much.
  Definite(f32),
}

/// What a leaf measures at: its size, and its baselines from its top.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct LeafMeasure {
  /// The width.
  pub width: f32,
  /// The height.
  pub height: f32,
  /// The first line's baseline.
  pub first_baseline: Option<f32>,
  /// The last line's baseline.
  pub last_baseline: Option<f32>,
}

/// A leaf the layout engine measures itself rather than through a toolkit: text, through its
/// realization's [`crate::text::TextEngine`].
pub trait MeasureText {
  /// The leaf's size given what is known of it and the space it has.
  fn measure(&self, known: (Option<f32>, Option<f32>), available: (Space, Space)) -> LeafMeasure;

  /// The leaf's width alone in the space `available` along its width: the width
  /// [`MeasureText::measure`] gives there, its height not asked for.
  fn measure_width(&self, available: Space) -> f32 {
    self
      .measure((None, None), (available, Space::MaxContent))
      .width
  }
}

/// The most layouts a leaf keeps, by width.
const KEPT: usize = 3;

/// A paragraph as a leaf of the layout engine.
///
/// Min-content is the paragraph's narrowest width, max-content its widest; a definite width
/// lays it out within that width and reports the width it uses, so a width under min-content
/// breaks words where the line fills; a known width lays it out at that width. Its intrinsic
/// widths are measured once a paragraph, and its last three layouts are kept by width, a
/// width at or past its widest sharing the unbounded layout.
pub struct TextLeaf<P: Paragraph> {
  paragraph: P,
  epoch: u64,
  intrinsic: Cell<Option<Intrinsic>>,
  laid: RefCell<Vec<(Option<u32>, P::Laid)>>,
  layouts: Cell<u32>,
}

impl<P: Paragraph> TextLeaf<P> {
  /// The leaf of `paragraph`.
  pub fn new(paragraph: P) -> Self {
    Self {
      paragraph,
      epoch: 0,
      intrinsic: Cell::new(None),
      laid: RefCell::new(Vec::with_capacity(KEPT)),
      layouts: Cell::new(0),
    }
  }

  /// The paragraph.
  #[inline(always)]
  pub const fn paragraph(&self) -> &P {
    &self.paragraph
  }

  /// Replaces the paragraph after its content or a metric-affecting style changed: a new
  /// epoch, every cached measurement dropped.
  pub fn replace(&mut self, paragraph: P) {
    self.paragraph = paragraph;
    self.epoch += 1;
    self.intrinsic.set(None);
    self.laid.borrow_mut().clear();
  }

  /// Repaints `range` with `paint`, keeping every measurement and every layout, which show
  /// it: `false` where the engine needs the paragraph rebuilt ([`TextLeaf::replace`]).
  pub fn repaint(&mut self, range: Range<usize>, paint: &PaintStyle) -> bool {
    self.paragraph.repaint(range, paint)
  }

  /// The paragraph's epoch: how many times it was replaced.
  #[inline(always)]
  #[must_use]
  pub const fn epoch(&self) -> u64 {
    self.epoch
  }

  /// How many layouts the engine made for this leaf.
  #[inline(always)]
  #[must_use]
  pub const fn layouts(&self) -> u32 {
    self.layouts.get()
  }

  /// The paragraph's intrinsic widths, measured once an epoch.
  pub fn intrinsic(&self) -> Intrinsic {
    if let Some(intrinsic) = self.intrinsic.get() {
      return intrinsic;
    }

    let intrinsic = self.paragraph.intrinsic();

    self.intrinsic.set(Some(intrinsic));

    intrinsic
  }

  /// The paragraph laid out within `width` (unbounded where `None`), from the cache where a
  /// layout at that width is kept.
  pub fn laid(&self, width: Option<f32>) -> P::Laid {
    let key = width
      .filter(|width| *width < self.intrinsic().max)
      .map(f32::to_bits);
    let mut kept = self.laid.borrow_mut();

    if let Some(at) = kept.iter().position(|(width, _)| *width == key) {
      let entry = kept.remove(at);
      let laid = entry.1.clone();

      kept.insert(0, entry);

      return laid;
    }

    let laid = self.paragraph.layout(key.map(f32::from_bits));
    self.layouts.set(self.layouts.get() + 1);
    kept.truncate(KEPT - 1);
    kept.insert(0, (key, laid.clone()));

    laid
  }
}

impl<P: Paragraph> MeasureText for TextLeaf<P> {
  fn measure_width(&self, available: Space) -> f32 {
    match available {
      Space::MinContent => self.intrinsic().min,
      Space::MaxContent => self.intrinsic().max,
      Space::Definite(width) => self.laid(Some(width)).size().width,
    }
  }

  fn measure(&self, known: (Option<f32>, Option<f32>), available: (Space, Space)) -> LeafMeasure {
    let intrinsic = self.intrinsic();
    let within = match (known.0, available.0) {
      (Some(width), _) => Some(width),
      (None, Space::MinContent) => Some(intrinsic.min),
      (None, Space::MaxContent) => None,
      (None, Space::Definite(width)) => Some(width),
    };
    let laid = self.laid(within);
    let size = laid.size();
    let width = match (known.0, available.0) {
      (Some(width), _) => width,
      (None, Space::MinContent) => intrinsic.min,
      (None, Space::MaxContent) => intrinsic.max,
      (None, Space::Definite(_)) => size.width,
    };

    LeafMeasure {
      width,
      height: known.1.unwrap_or(size.height),
      first_baseline: Some(laid.first_baseline()),
      last_baseline: Some(laid.last_baseline()),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use super::{LeafMeasure, MeasureText, Space, TextLeaf};
  use crate::{
    layout::{Axis, ContainerLayout, LayoutHost, Measurement, Needed, Rect, ResolvedLayout},
    seam::node::LayoutKind,
    text::{
      AttributedText, ParagraphStyle, RunStyle, TextEngine,
      grid::{Grid, GridParagraph},
    },
    value::{AxisAlignment, Inset},
  };

  fn leaf(text: &str) -> TextLeaf<GridParagraph> {
    TextLeaf::new(Grid.paragraph(
      &AttributedText::plain(text, RunStyle::new(13.)),
      &ParagraphStyle::default(),
    ))
  }

  fn measured(leaf: &TextLeaf<GridParagraph>, width: Space) -> (f32, f32) {
    let LeafMeasure { width, height, .. } = leaf.measure((None, None), (width, Space::MaxContent));

    (width, height)
  }

  #[test]
  fn min_and_max_content_are_the_intrinsic_widths() {
    let leaf = leaf("alpha beta gamma");

    assert_eq!(measured(&leaf, Space::MinContent), (50., 60.));
    assert_eq!(measured(&leaf, Space::MaxContent), (160., 20.));
  }

  #[test]
  fn a_definite_width_reports_the_width_used() {
    let leaf = leaf("alpha beta gamma");

    assert_eq!(measured(&leaf, Space::Definite(125.)), (100., 40.));
    // Under min-content a word breaks where the line fills, as overflow-wrap: break-word
    // does: the leaf is as wide as the space, not clamped to its min-content.
    assert_eq!(measured(&leaf, Space::Definite(30.)), (30., 120.));
    // Past max-content it is as wide as its widest line.
    assert_eq!(measured(&leaf, Space::Definite(500.)), (160., 20.));
  }

  #[test]
  fn a_known_width_lays_out_at_it_with_both_baselines() {
    let leaf = leaf("alpha beta gamma");
    let measure = leaf.measure((Some(60.), None), (Space::MaxContent, Space::MaxContent));

    assert_eq!(measure.width, 60.);
    assert_eq!(measure.height, 60.);
    assert_eq!(measure.first_baseline, Some(15.));
    assert_eq!(measure.last_baseline, Some(55.));
  }

  #[test]
  fn intrinsics_are_measured_once_and_layouts_kept_by_width() {
    let mut leaf = leaf("alpha beta gamma");

    for _ in 0..3 {
      let _ = measured(&leaf, Space::Definite(125.));
      let _ = measured(&leaf, Space::MaxContent);
      let _ = measured(&leaf, Space::Definite(900.));
    }

    // Two layouts: at 125, and unbounded (shared by max-content and every width past it).
    assert_eq!(leaf.layouts(), 2);

    for width in [70., 80., 90., 125.] {
      let _ = measured(&leaf, Space::Definite(width));
    }

    // Three kept: 125 was pushed out by 70, 80 and 90.
    assert_eq!(leaf.layouts(), 6);

    let paint = crate::text::PaintStyle::new(crate::Color::WHITE);

    assert!(leaf.repaint(0..5, &paint), "paint never relayouts");
    let _ = measured(&leaf, Space::Definite(125.));
    assert_eq!(leaf.layouts(), 6);

    leaf.replace(Grid.paragraph(
      &AttributedText::plain("other", RunStyle::new(13.)),
      &ParagraphStyle::default(),
    ));
    assert_eq!(leaf.epoch(), 1);
    assert_eq!(measured(&leaf, Space::MaxContent), (50., 20.));
  }

  /// Text leaves under the layout engine, recording where they were placed.
  #[derive(Clone)]
  struct Child {
    leaf: Rc<TextLeaf<GridParagraph>>,
    placed: Rc<Cell<Option<Rect>>>,
  }

  impl Child {
    fn new(text: &str) -> Self {
      Self {
        leaf: Rc::new(leaf(text)),
        placed: Rc::new(Cell::new(None)),
      }
    }

    fn at(&self) -> (f32, f32, f32, f32) {
      let rect = self.placed.get().expect("the leaf was placed");

      (rect.x, rect.y, rect.width, rect.height)
    }
  }

  struct Texts;

  impl LayoutHost for Texts {
    type Child = Child;

    fn measure(_: &Child, _: Axis, _: Option<f32>, _: Needed) -> Measurement {
      unreachable!("a text leaf is measured through its engine")
    }

    fn constant_height(_: &Child) -> bool {
      false
    }

    fn is_visible(_: &Child) -> bool {
      true
    }

    fn is_container(_: &Child) -> bool {
      false
    }

    fn text(child: &Child) -> Option<Rc<dyn MeasureText>> {
      Some(child.leaf.clone())
    }

    fn allocate(child: &Child, rect: Rect, _: Rect, _: Option<f32>) {
      child.placed.set(Some(rect));
    }
  }

  #[test]
  fn a_row_shrinks_text_leaves_and_they_wrap() {
    let (a, b) = (Child::new("alpha beta"), Child::new("gamma delta"));
    let mut row = ContainerLayout::<Texts>::new(
      LayoutKind::Row,
      ResolvedLayout {
        item_alignment: Some(AxisAlignment::Start),
        ..ResolvedLayout::default()
      },
    );

    row.set_children(vec![
      (a.clone(), ResolvedLayout::default()),
      (b.clone(), ResolvedLayout::default()),
    ]);

    let natural = row.measure(Axis::Horizontal, None, Needed::Both);

    assert_eq!((natural.minimum, natural.natural), (100., 210.));

    row.allocate((120., 40.), None);

    let ((ax, _, aw, ah), (bx, _, bw, bh)) = (a.at(), b.at());

    assert_eq!((ax, ah, bh), (0., 40., 40.));
    assert_eq!(bx, aw);
    assert!((aw + bw - 120.).abs() < 1., "{aw} + {bw}");
  }

  #[test]
  fn a_box_takes_its_texts_baseline_under_its_padding() {
    let text = Child::new("alpha beta gamma");
    let mut boxed = ContainerLayout::<Texts>::new(
      LayoutKind::Box,
      ResolvedLayout {
        padding: Some(Inset::new(10., 0., 0., 0.)),
        ..ResolvedLayout::default()
      },
    );

    boxed.set_children(vec![(text, ResolvedLayout::default())]);

    let measured = boxed.measure(Axis::Vertical, Some(100.), Needed::Natural);

    assert_eq!((measured.natural, measured.baseline), (50., Some(25.)));
  }
}
