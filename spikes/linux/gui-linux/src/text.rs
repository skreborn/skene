//! Text through the foundation's engine contract: one Parley engine per thread (over
//! fontconfig's per-character fallback), and a [`TextCell`] per paragraph, the layout engine's
//! text leaf, rebuilt only where its content or a metric changed and repainted where only its
//! paint did.

use std::{cell::RefCell, ops::Range, rc::Rc};

use skene_foundation::{
  seam::{content::Run, record::Prop},
  text::{
    AttributedText, Intrinsic, LeafMeasure, MeasureText, Paragraph as _, ParagraphStyle, RunStyle,
    Space, TextEngine as _, TextLeaf, TextRun,
  },
};
use skene_parley::{ParleyEngine, ParleyLaid, ParleyParagraph};

use crate::look::RunProps;

thread_local! {
  static ENGINE: ParleyEngine = ParleyEngine::system();
}

/// This thread's engine.
pub fn engine() -> ParleyEngine {
  ENGINE.with(Clone::clone)
}

/// The link runs of a paragraph: each byte range and where it leads.
pub type Links = Vec<(Range<usize>, Rc<str>)>;

/// What setting a paragraph's text changed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Change {
  /// Nothing.
  None,
  /// Its paint alone: it is drawn again where it is.
  Paint,
  /// Its content or a metric: it is laid out again.
  Layout,
}

/// A paragraph, as the layout engine measures it and the painter draws it.
#[derive(Default)]
pub struct TextCell {
  leaf: RefCell<Option<TextLeaf<ParleyParagraph>>>,
  source: RefCell<Option<(AttributedText, ParagraphStyle)>>,
  links: RefCell<Links>,
}

impl TextCell {
  /// An empty cell, laid out as nothing until its text is set.
  pub fn new() -> Rc<Self> {
    Rc::new(Self::default())
  }

  /// Shows `text` in `style`: the paragraph rebuilt where its content or a metric changed,
  /// repainted where only its paint changed.
  pub fn set(&self, text: AttributedText, style: ParagraphStyle) -> Change {
    let mut source = self.source.borrow_mut();

    if let Some((old, old_style)) = source.as_ref() {
      if *old == text && *old_style == style {
        return Change::None;
      }

      let reshapes = old.value() != text.value()
        || *old_style != style
        || old.spans().len() != text.spans().len()
        || old
          .spans()
          .iter()
          .zip(text.spans())
          .any(|((a, a_style), (b, b_style))| *a != b || !a_style.shapes_like(b_style));

      if !reshapes {
        let mut leaf = self.leaf.borrow_mut();
        let repainted = leaf.as_mut().is_some_and(|leaf| {
          text
            .spans()
            .into_iter()
            .all(|(range, run)| leaf.repaint(range, &run.paint))
        });

        if repainted {
          *source = Some((text, style));

          return Change::Paint;
        }
      }
    }

    let paragraph = engine().paragraph(&text, &style);
    let mut leaf = self.leaf.borrow_mut();

    match leaf.as_mut() {
      Some(leaf) => leaf.replace(paragraph),
      None => *leaf = Some(TextLeaf::new(paragraph)),
    }

    *source = Some((text, style));

    Change::Layout
  }

  /// Sets the destinations of the link runs, by byte range.
  pub fn set_links(&self, links: Links) {
    *self.links.borrow_mut() = links;
  }

  /// The link runs, in the text's order.
  pub fn links(&self) -> Links {
    self.links.borrow().clone()
  }

  /// The text shown.
  pub fn value(&self) -> Option<Rc<str>> {
    self
      .source
      .borrow()
      .as_ref()
      .map(|(text, _)| text.shared_value())
  }

  /// The intrinsic widths `text` has laid out by the engine in this paragraph's base style
  /// and paragraph style: another value the same box may show (a picker's other options).
  pub fn intrinsic_of(&self, text: &str) -> Intrinsic {
    self
      .source
      .borrow()
      .as_ref()
      .map(|(source, style)| {
        engine()
          .paragraph(&AttributedText::plain(text, source.base().clone()), style)
          .intrinsic()
      })
      .unwrap_or_default()
  }

  /// The paragraph's intrinsic widths.
  pub fn intrinsic(&self) -> Intrinsic {
    self
      .leaf
      .borrow()
      .as_ref()
      .map(TextLeaf::intrinsic)
      .unwrap_or_default()
  }

  /// The paragraph laid out within `width` (unbounded where `None`), from the leaf's cache.
  pub fn laid(&self, width: Option<f32>) -> Option<ParleyLaid> {
    self.leaf.borrow().as_ref().map(|leaf| leaf.laid(width))
  }

  /// How many layouts the engine made for this paragraph.
  pub fn layouts(&self) -> u32 {
    self.leaf.borrow().as_ref().map_or(0, TextLeaf::layouts)
  }
}

impl MeasureText for TextCell {
  fn measure(&self, known: (Option<f32>, Option<f32>), available: (Space, Space)) -> LeafMeasure {
    self
      .leaf
      .borrow()
      .as_ref()
      .map(|leaf| leaf.measure(known, available))
      .unwrap_or_default()
  }
}

/// Rich text's runs as attributed text: each run's properties over `inherited`, the link runs'
/// destinations by byte range.
pub fn attributed_runs(
  runs: &[Run],
  inherited: &RunProps,
  base: &RunStyle,
  ink: skene_foundation::Color,
  size: f32,
) -> (AttributedText, Links) {
  let mut value = String::new();
  let mut styled = Vec::new();
  let mut links = Vec::new();

  for run in runs {
    let start = value.len();

    value.push_str(&run.text);

    let mut props = inherited.clone();

    for (prop, resolved) in run.props.iter() {
      props.put(*prop, resolved);
    }

    if run.destination.is_some() && !run.props.iter().any(|(prop, _)| *prop == Prop::Decoration) {
      props.decoration = Some(skene_foundation::TextDecoration::UNDERLINE);
    }

    if let Some(destination) = &run.destination {
      links.push((start..value.len(), Rc::clone(destination)));
    }

    styled.push(TextRun {
      range: start..value.len(),
      style: props.run_style(ink, size),
    });
  }

  (AttributedText::new(value, base.clone(), styled), links)
}
