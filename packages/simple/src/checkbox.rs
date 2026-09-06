//! A ready-made checkbox in the Simple design language.

use std::marker::PhantomData;

use skene_core::{
  Alignment,
  AnyKind,
  AnyView,
  Dimension,
  Driver,
  HasChildren,
  IntoView,
  MouseCursor,
  NodeId,
  Part,
  State,
  View,
  When,
};
use skene_reactive::{Get, Memo, Signal, use_context};
use skene_views::{Container, CornerRadius, RawCheckbox, Text, Toggle};

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
pub struct SimpleCheckbox<D: Driver> {
  toggle: Toggle,
  label: Option<String>,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> SimpleCheckbox<D> {
  /// An unchecked checkbox.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      toggle: Toggle::new(),
      label: None,
      driver: PhantomData,
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

impl<D: Driver> Default for SimpleCheckbox<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> AnyKind for SimpleCheckbox<D> {}

impl<D: Driver> View<D> for SimpleCheckbox<D>
where
  D::Scalar: SimpleUnit,
  D::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<D>();

    let mark: AnyView<D> = match D::Scalar::MARKS {
      SimpleMarks::Drawn => drawn_mark::<D>().into_view(),
      SimpleMarks::Glyph => {
        let glyphs = use_context::<Memo<SimpleGlyphs>>();
        let checked = self.toggle.reads_checked();

        Text::<D>::derive(move || {
          glyphs
            .map_or_else(SimpleGlyphs::default, |glyphs| glyphs.get())
            .checkbox(checked())
        })
        .set_color(
          SimplePalette::<D::Scalar>::FOREGROUND
            .when(State::FocusVisible, SimplePalette::<D::Scalar>::RING),
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
fn drawn_mark<D: Driver>() -> Container<D>
where
  D::Scalar: SimpleUnit,
  D::StyleExt: CornerRadius,
{
  let shape = shape::<D>();

  Container::<D>::new()
    .set_width(Dimension::Length(shape.mark_size))
    .set_height(Dimension::Length(shape.mark_size))
    .set_color(
      shape
        .mark_surface
        .when(State::Checked, SimplePalette::<D::Scalar>::PRIMARY_SURFACE),
    )
    .set_border(
      border(shape.mark_ring, SimplePalette::<D::Scalar>::BORDER)
        .when(
          State::Checked,
          border(shape.mark_ring, SimplePalette::<D::Scalar>::PRIMARY_SURFACE),
        )
        .when(
          State::FocusVisible,
          border(shape.mark_ring, SimplePalette::<D::Scalar>::RING),
        ),
    )
    .set_corner_radius(MARK_RADIUS)
    .set_alignment(Alignment::CENTER)
    .set_child(
      Text::new("✓").set_color(
        shape
          .mark_surface
          .when(State::Checked, SimplePalette::<D::Scalar>::PRIMARY_INK),
      ),
    )
}
