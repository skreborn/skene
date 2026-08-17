//! A ready-made button in the Simple design language.

use skene_core::{Color, Condition, HasChildren, Inset, Source, View};

use crate::{HasDecoration, RawButton, SimpleVariant, Text, ThemedControl, ThemedText, simple};

/// A ready-made button: a text label on the variant's surface, with hover feedback and
/// the platform's full keyboard behavior.
///
/// Pick the emphasis with [`set_variant`](Self::set_variant); a derived label comes
/// from the [`derive`](Self::derive) constructor, exactly as on [`Text`]. For anything
/// beyond a labeled action—custom content, custom styling—compose [`RawButton`]
/// directly.
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

  /// Sets the click handler (activation by pointer, Enter, or Space).
  #[inline(always)]
  pub fn handle_click<R: Into<skene_core::Prevent>>(
    mut self,
    action: impl Fn() -> R + 'static,
  ) -> Self {
    self.raw = self.raw.handle_click(action);

    self
  }
}

impl View for SimpleButton {
  fn build(self) -> skene_core::NodeId {
    let variant = self.variant;
    let text = match self.label {
      Source::Static(label) => Text::new(label),
      Source::Derived(label) => Text::derive(move || label()),
    };

    self
      .raw
      .set_padding(Inset::symmetric(16., 8.))
      .theme_color(move |palette| variant.look(palette).surface)
      .theme_border(1., move |palette| {
        variant.look(palette).border.unwrap_or(Color::TRANSPARENT)
      })
      .set_border_radius(simple::RADIUS)
      .theme_color_when(Condition::Hovered, move |palette| {
        variant.look(palette).hover
      })
      .set_child(
        text
          .set_font_size(14.)
          .set_font_weight(500.)
          .theme_color(move |palette| variant.look(palette).ink),
      )
      .build()
  }
}

impl crate::HasFocus for SimpleButton {
  #[inline(always)]
  fn focus_core(&mut self) -> &mut crate::FocusCore {
    crate::HasFocus::focus_core(&mut self.raw)
  }
}
