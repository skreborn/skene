//! A ready-made button in the Simple design language.

use std::rc::Rc;

use skene_core::{Border, BoxStyle, Condition, HasChildren, Inset, View};

use crate::{RawButton, SimpleVariant, Text, simple};

/// A ready-made button: a text label on the variant's surface, with hover feedback and
/// the platform's full keyboard behavior.
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

  /// Sets the click handler (activation by pointer, Enter, or Space).
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
  fn build(self) -> skene_core::NodeId {
    let look = self.variant.look();
    let mut button = RawButton::new()
      .set_style(
        BoxStyle::new()
          .set_padding(Inset::symmetric(16., 8.))
          .set_color(look.surface)
          .set_border(Border::new(1., look.border))
          .set_border_radius(simple::RADIUS),
      )
      .set_when(Condition::Hovered, BoxStyle::new().set_color(look.hover))
      .set_child(
        Text::new(self.label)
          .set_font_size(14.)
          .set_font_weight(500.)
          .set_color(look.ink),
      );

    if let Some(action) = self.click_handler {
      button = button.handle_click(move || action());
    }

    button.build()
  }
}
