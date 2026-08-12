//! A ready-made button in the Simple design language, in cells.

use std::rc::Rc;

use skene_core::{MouseCursor, NodeId, View};

use crate::{
  RawButton,
  props::{BoxStyle, Condition},
  simple::{self, SimpleVariant},
};

/// A ready-made button: a text label on the variant's surface, with hover and focus
/// feedback and the terminal's full keyboard behavior.
///
/// Pick the emphasis with [`set_variant`](Self::set_variant). For anything beyond a
/// labeled action—custom content, custom styling—compose [`RawButton`] directly.
#[must_use]
pub struct SimpleButton {
  label: String,
  variant: SimpleVariant,
  click_handler: Option<Rc<skene_core::ClickHandler>>,
}

impl SimpleButton {
  /// Creates a button carrying `label`.
  #[inline(always)]
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label: label.into(),
      variant: SimpleVariant::Primary,
      click_handler: None,
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
    self.click_handler = Some(Rc::new(move || action().into()));

    self
  }
}

impl View for SimpleButton {
  fn build(self) -> NodeId {
    use skene_core::HasChildren;

    let look = self.variant.look();
    let mut style = BoxStyle::new()
      .set_padding(crate::Inset::horizontal(2))
      .set_color(look.surface)
      .set_cursor(MouseCursor::Click);

    if let Some(border) = look.border {
      style = style.set_border(border).set_border_glyphs(simple::GLYPHS);
    }

    let mut button = RawButton::new()
      .set_style(style)
      .set_when(Condition::Hovered, BoxStyle::new().set_color(look.hover))
      .set_when(Condition::Focused, BoxStyle::new().set_color(look.focus))
      .set_child(
        crate::Text::new(self.label)
          .set_bold(true)
          .set_color(look.ink),
      );

    if let Some(action) = self.click_handler {
      button = button.handle_click(move || action());
    }

    button.build()
  }
}
