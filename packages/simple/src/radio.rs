//! A ready-made radio group and its members, in the Simple design language.

use std::marker::PhantomData;

use skene_core::{
  Alignment,
  AnyKind,
  AnyView,
  Color,
  Dimension,
  Driver,
  HasChildren,
  IntoView,
  MouseCursor,
  NoChild,
  NodeId,
  Part,
  State,
  View,
  When,
};
use skene_reactive::Signal;
use skene_views::{Container, CornerRadius, RadioMember, RawRadio, RawRadioGroup, Roving};

use super::{
  SimplePalette,
  shape::{ROUND, SimpleUnit, border, define_defaults, shape},
};

/// A ready-made group of radio buttons: the members inside it are one tab stop, the
/// arrow keys move between them, and exactly one is chosen.
///
/// Members are [`SimpleRadio`]s at any depth inside the child, so one may be wrapped
/// or interleaved with anything. The chosen value flows both ways. Holds [`NoChild`]
/// until [`set_child`](Self::set_child) supplies the view holding the members.
#[must_use]
pub struct SimpleRadioGroup<D: Driver, C = NoChild> {
  roving: Roving,
  label: Option<String>,
  child: C,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> SimpleRadioGroup<D> {
  /// A group with nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      roving: Roving::new(),
      label: None,
      child: NoChild,
      driver: PhantomData,
    }
  }
}

impl<D: Driver> Default for SimpleRadioGroup<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver, C> SimpleRadioGroup<D, C> {
  /// Sets the chosen member's value.
  #[inline(always)]
  pub fn set_chosen(mut self, chosen: impl Into<String>) -> Self {
    self.roving = self.roving.set_chosen(chosen);

    self
  }

  /// Derives the chosen member's value from the signals `chosen` reads.
  #[inline(always)]
  pub fn derive_chosen(mut self, chosen: impl Fn() -> Option<String> + 'static) -> Self {
    self.roving = self.roving.derive_chosen(chosen);

    self
  }

  /// Runs `action` with the value a choice asks for.
  #[inline(always)]
  pub fn handle_choose(mut self, action: impl Fn(String) + 'static) -> Self {
    self.roving = self.roving.handle_choose(action);

    self
  }

  /// Binds the chosen value to `signal` in both directions.
  #[inline(always)]
  pub fn bind_chosen(mut self, signal: Signal<Option<String>>) -> Self {
    self.roving = self.roving.bind_chosen(signal);

    self
  }

  /// Sets the group's accessible name.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }

  /// Sets the view holding the members.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView<D>) -> SimpleRadioGroup<D, AnyView<D>> {
    SimpleRadioGroup {
      roving: self.roving,
      label: self.label,
      child: child.into_view(),
      driver: PhantomData,
    }
  }
}

impl<D: Driver, C> AnyKind for SimpleRadioGroup<D, C> {}

impl<D: Driver> View<D> for SimpleRadioGroup<D, AnyView<D>> {
  fn build(self) -> NodeId {
    let mut group = RawRadioGroup::new(self.roving);

    if let Some(label) = self.label {
      group = group.set_label(label);
    }

    group.set_child(self.child).build()
  }
}

/// A ready-made radio button: a ring on the card surface whose dot appears while
/// chosen and which rings while focused, beside its caption.
///
/// The ring is the control's [`Part::Mark`], so it follows the control's state on
/// every facade. Outside a [`SimpleRadioGroup`] it draws, is never chosen and takes
/// focus like any control. For a radio drawn some other way, compose [`RawRadio`]
/// directly.
#[must_use]
pub struct SimpleRadio<D: Driver> {
  member: RadioMember,
  label: Option<String>,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> SimpleRadio<D> {
  /// The member standing for `value`, the string the group reports when it is chosen.
  #[inline(always)]
  pub fn new(value: impl Into<String>) -> Self {
    Self {
      member: RadioMember::new(value),
      label: None,
      driver: PhantomData,
    }
  }

  /// Sets the caption beside the ring, which is also the accessible name.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }
}

impl<D: Driver> AnyKind for SimpleRadio<D> {}

impl<D: Driver> View<D> for SimpleRadio<D>
where
  D::Scalar: SimpleUnit,
  D::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<D>();

    let shape = shape::<D>();
    let dot = Container::<D>::new()
      .set_width(Dimension::Length(shape.dot_size))
      .set_height(Dimension::Length(shape.dot_size))
      .set_corner_radius(ROUND / 2.)
      .set_color(
        Color::TRANSPARENT.when(State::Checked, SimplePalette::<D::Scalar>::PRIMARY_SURFACE),
      );
    let ring = Container::<D>::new()
      .set_width(Dimension::Length(shape.mark_size))
      .set_height(Dimension::Length(shape.mark_size))
      .set_color(shape.mark_surface)
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
      .set_corner_radius(ROUND)
      .set_alignment(Alignment::CENTER)
      .set_child(dot);
    let mut control = RawRadio::new(self.member)
      .set_cursor(MouseCursor::Click)
      .set_part(Part::Mark, ring);

    if let Some(label) = self.label {
      control = control.set_label(label);
    }

    control.build()
  }
}
