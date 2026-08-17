//! A ready-made button in the Simple design language, in cells.

use skene_core::{MouseCursor, NodeId, Source, View};

use crate::{
  HasDecoration,
  RawButton,
  props::Condition,
  simple::{self, SimpleVariant, ThemedControl, ThemedText},
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

    let variant = self.variant;
    let text = match self.label {
      Source::Static(label) => crate::Text::new(label),
      Source::Derived(label) => crate::Text::derive(move || label()),
    };
    let mut button = self
      .raw
      .set_padding(crate::Inset::horizontal(2))
      .theme_color(move |palette| variant.look(palette).surface)
      .set_cursor(MouseCursor::Click);

    // A bordered variant takes its feedback on the border, a filled one on the fill: a
    // terminal border is a cell rather than a hairline, so an invisible one would still
    // cost the room. Which of the two a variant is comes from the palette in force,
    // since the border is the palette's to carry.
    button = match variant.look(&simple::painted_palette()).border {
      Some(_) => button
        .set_border_glyphs(simple::GLYPHS)
        .theme_border(move |palette| variant.look(palette).border.unwrap_or(palette.border))
        .theme_border_when(Condition::Hovered, |palette| palette.ring)
        .theme_border_when(Condition::Focused, move |palette| variant.look(palette).ink),
      None => button
        .theme_color_when(Condition::Hovered, move |palette| {
          variant.look(palette).hover
        })
        .theme_color_when(Condition::Focused, move |palette| {
          variant.look(palette).focus
        }),
    };

    button
      .set_child(
        text
          .set_bold(true)
          .theme_color(move |palette| variant.look(palette).ink),
      )
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
