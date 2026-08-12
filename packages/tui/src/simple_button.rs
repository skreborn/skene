//! A ready-made button in the Simple design language, in cells.

use skene_core::{MouseCursor, NodeId, Source, View};

use crate::{
  RawButton,
  props::{BoxStyle, Condition},
  simple::{self, SimpleVariant, simple_surface},
};

/// A ready-made button: a bordered text label on the variant's surface, with the
/// terminal's full keyboard behavior.
///
/// Interaction feedback rides the border: hovering brightens it to the ring, focusing
/// inks it in the label's own ink; the surface never changes. Pick the emphasis with
/// [`set_variant`](Self::set_variant); a derived label comes from the
/// [`derive`](Self::derive) constructor, exactly as on [`Text`](crate::Text). For
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

  /// Sets the click handler (activation by mouse click, `Enter`, or `Space`).
  #[inline(always)]
  pub fn handle_click<R: Into<skene_core::Prevent>>(
    mut self,
    action: impl Fn() -> R + 'static,
  ) -> Self {
    self.raw = self.raw.handle_click(action);

    self
  }
}

simple_surface!(SimpleButton);

impl View for SimpleButton {
  fn build(self) -> NodeId {
    use skene_core::HasChildren;

    let look = self.variant.look();
    let text = match self.label {
      Source::Static(label) => crate::Text::new(label),
      Source::Derived(label) => crate::Text::derive(move || label()),
    };

    self
      .raw
      .set_style(
        BoxStyle::new()
          .set_padding(crate::Inset::horizontal(2))
          .set_color(look.surface)
          .set_border(look.border.unwrap_or(look.surface))
          .set_border_glyphs(simple::GLYPHS)
          .set_cursor(MouseCursor::Click),
      )
      .set_when(
        Condition::Hovered,
        BoxStyle::new().set_border(crate::simple::SimplePalette::ring()),
      )
      .set_when(Condition::Focused, BoxStyle::new().set_border(look.ink))
      .set_child(text.set_bold(true).set_color(look.ink))
      .build()
  }
}
