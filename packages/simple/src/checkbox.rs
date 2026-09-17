//! A ready-made checkbox in the Simple design language.

use std::marker::PhantomData;

use skene_common::{Container, CornerRadius, RawCheckbox, Text, Toggle};
use skene_core::{
  Alignment,
  AnyKind,
  AnyView,
  Dimension,
  HasChildren,
  IntoView,
  MouseCursor,
  NodeId,
  Part,
  Platform,
  State,
  View,
  When,
};
use skene_reactive::{Get, Memo, Signal, use_context};

use super::{
  SimpleGlyphs,
  SimplePalette,
  shape::{MARK_RADIUS, SimpleMarks, SimpleUnit, border, define_defaults, shape},
};

/// A ready-made checkbox: a rounded mark on the card surface that fills with the
/// primary surface while checked and rings while focused, beside its caption.
///
/// The mark is the control's [`Part::Mark`], so it follows the control's state on
/// every facade. The checked state flows both ways; for a checkbox drawn some other
/// way, compose [`RawCheckbox`] directly.
#[must_use]
pub struct SimpleCheckbox<P: Platform> {
  toggle: Toggle,
  label: Option<String>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> SimpleCheckbox<P> {
  /// An unchecked checkbox.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      toggle: Toggle::new(),
      label: None,
      platform: PhantomData,
    }
  }

  /// Sets the caption beside the mark, which is also the accessible name.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }

  /// Sets the checked state.
  #[inline(always)]
  pub fn set_checked(mut self, checked: bool) -> Self {
    self.toggle = self.toggle.set_checked(checked);

    self
  }

  /// Derives the checked state from the signals `checked` reads.
  #[inline(always)]
  pub fn derive_checked(mut self, checked: impl Fn() -> bool + 'static) -> Self {
    self.toggle = self.toggle.derive_checked(checked);

    self
  }

  /// Runs `action` with the state a flip asks for.
  #[inline(always)]
  pub fn handle_toggle(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.toggle = self.toggle.handle_toggle(action);

    self
  }

  /// Binds the checked state to `signal` in both directions.
  #[inline(always)]
  pub fn bind_checked(mut self, signal: Signal<bool>) -> Self {
    self.toggle = self.toggle.bind_checked(signal);

    self
  }
}

impl<P: Platform> Default for SimpleCheckbox<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> AnyKind for SimpleCheckbox<P> {}

impl<P: Platform> View<P> for SimpleCheckbox<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<P>();

    let mark: AnyView<P> = match P::Scalar::MARKS {
      SimpleMarks::Drawn => drawn_mark::<P>().into_view(),
      SimpleMarks::Glyph => {
        let glyphs = use_context::<Memo<SimpleGlyphs>>();
        let checked = self.toggle.reads_checked();

        Text::<P>::derive(move || {
          glyphs
            .map_or_else(SimpleGlyphs::default, |glyphs| glyphs.get())
            .checkbox(checked())
        })
        .set_color(
          SimplePalette::<P::Scalar>::FOREGROUND
            .when(State::FocusVisible, SimplePalette::<P::Scalar>::RING),
        )
        .into_view()
      }
    };
    let mut control = RawCheckbox::new(self.toggle)
      .set_cursor(MouseCursor::Click)
      .set_part(Part::Mark, mark);

    if let Some(label) = self.label {
      control = control.set_label(label);
    }

    control.build()
  }
}

/// The drawn mark: a rounded box on the card surface that fills with the primary
/// surface while checked and rings while focused; its check is inked in the mark's own
/// surface until checked, so it hides on every medium.
fn drawn_mark<P: Platform>() -> Container<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  let shape = shape::<P>();

  Container::<P>::new()
    .set_width(Dimension::Length(shape.mark_size))
    .set_height(Dimension::Length(shape.mark_size))
    .set_color(
      shape
        .mark_surface
        .when(State::Checked, SimplePalette::<P::Scalar>::PRIMARY_SURFACE),
    )
    .set_border(
      border(shape.mark_ring, SimplePalette::<P::Scalar>::BORDER)
        .when(
          State::Checked,
          border(shape.mark_ring, SimplePalette::<P::Scalar>::PRIMARY_SURFACE),
        )
        .when(
          State::FocusVisible,
          border(shape.mark_ring, SimplePalette::<P::Scalar>::RING),
        ),
    )
    .set_corner_radius(MARK_RADIUS)
    .set_alignment(Alignment::CENTER)
    .set_child(
      Text::new("✓").set_color(
        shape
          .mark_surface
          .when(State::Checked, SimplePalette::<P::Scalar>::PRIMARY_INK),
      ),
    )
}
