//! A ready-made button in the Simple design language, in cells.

use skene_core::{MouseCursor, NodeId, Source, View};

use crate::{
  HasStyle,
  RawButton,
  props::{BoxStyle, Condition},
  simple::{self, SimpleVariant},
};

/// A ready-made button: a one-row text label on the variant's surface, with the
/// terminal's full keyboard behavior.
///
/// The surface shifts on hover and focus; the bordered `Outline` variant shifts its
/// border instead—to the ring under the pointer, to its own ink while focused. Pick
/// the emphasis with [`set_variant`](Self::set_variant); a derived label comes from
/// the [`derive`](Self::derive) constructor, exactly as on [`Text`](crate::Text). For
/// anything beyond a labeled action—custom content, custom styling—compose
/// [`RawButton`] directly.
#[must_use]
pub struct SimpleButton {
  raw: RawButton,
  label: Source<String>,
  variant: SimpleVariant,
}

impl SimpleButton {
  /// Creates a button carrying `label`.
  #[inline(always)]
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      raw: RawButton::new(),
      label: Source::Static(label.into()),
      variant: SimpleVariant::Primary,
    }
  }

  /// Creates a button with a derived label, re-rendered whenever it changes.
  #[inline(always)]
  pub fn derive(label: impl Fn() -> String + 'static) -> Self {
    Self {
      raw: RawButton::new(),
      label: Source::Derived(std::rc::Rc::new(label)),
      variant: SimpleVariant::Primary,
    }
  }

  /// Sets the emphasis variant.
  #[inline(always)]
  pub fn set_variant(mut self, variant: SimpleVariant) -> Self {
    self.variant = variant;

    self
  }
}

impl View for SimpleButton {
  fn build(self) -> NodeId {
    use skene_core::HasChildren;

    let look = self.variant.look();
    let text = match self.label {
      Source::Static(label) => crate::Text::new(label),
      Source::Derived(label) => crate::Text::derive(move || label()),
    };
    let mut style = BoxStyle::new()
      .set_padding(crate::Inset::horizontal(2))
      .set_color(look.surface)
      .set_cursor(MouseCursor::Click);

    let mut button = self.raw;

    if let Some(border) = look.border {
      style = style.set_border(border).set_border_glyphs(simple::GLYPHS);
      button = button
        .set_when(
          Condition::Hovered,
          BoxStyle::new().set_border(crate::simple::SimplePalette::ring()),
        )
        .set_when(Condition::Focused, BoxStyle::new().set_border(look.ink));
    }
    else {
      button = button
        .set_when(Condition::Hovered, BoxStyle::new().set_color(look.hover))
        .set_when(Condition::Focused, BoxStyle::new().set_color(look.focus));
    }

    button
      .set_style(style)
      .set_child(text.set_bold(true).set_color(look.ink))
      .build()
  }
}

impl crate::HasEvents for SimpleButton {
  #[inline(always)]
  fn event_core(&mut self) -> &mut crate::EventCore {
    self.raw.event_core()
  }
}

impl crate::HasFocus for SimpleButton {
  #[inline(always)]
  fn focus_core(&mut self) -> &mut crate::FocusCore {
    crate::HasFocus::focus_core(&mut self.raw)
  }
}
