//! A checkbox.

use crate::{
  Alignment, AxisAlignment, Border, Dimension, Signal, State, SystemColors, Value,
  behavior::Toggle as Behavior,
  seam::{Realization, control::Part},
  view::{
    AnyKind, AnyView, AsPart, AsPartBase, Container, ContainerBase, ControlBase,
    ControlMembersBase, Cx, DecorationBase, Label, Node, Painted, PaintedBase, RunBase, View,
    Wraps,
    controls::{Pending, control_surfaces, focus_ring, marks},
    wraps_itself,
  },
};

/// A checkbox with its caption: Skene's checkbox, drawn on every platform. It is a
/// [`crate::view::Control`] over a checkbox's [`crate::behavior::Toggle`]: `Space` flips it at
/// its release, a click anywhere on it, its caption included. A mixed checkbox
/// ([`CheckboxBase::derive_mixed`]) turns on when flipped.
///
/// It is controlled: it shows what [`CheckboxBase::derive_checked`] returns, asks
/// [`CheckboxBase::handle_change`] for each flip, and shows the truth.
///
/// Its parts: its face ([`Checkbox::default_face`]) holding its mark
/// ([`Checkbox::default_mark`]), then its caption, 8 pixels after. The face follows the
/// control's states, its hover included, so hovering the caption highlights it.
#[must_use]
pub struct Checkbox<R: Realization> {
  /// The control it composes, its defaults set as it is built.
  control: crate::view::Control<Behavior, R>,
  /// Its caption.
  label: String,
  /// The face the mark sits in; the default where unset.
  face: Option<Container<R>>,
  /// The mark; the default where unset.
  mark: Option<AnyView<R>>,
}

wraps_itself!(Checkbox<R> where R: Realization);

control_surfaces!([R: Realization] Checkbox<R>, R => control);

impl<R: Realization> Checkbox<R> {
  /// The default face: 18 pixels square, its corners rounded by 4, a border of 1.5 in
  /// [`SystemColors::BORDER`] around [`SystemColors::FIELD`] and its hover and pressed
  /// variants, the accent while checked or mixed, its mark centered and inked in
  /// [`SystemColors::ON_ACCENT`]; a ring of 2 in the accent while focus shows.
  pub fn default_face() -> Container<R> {
    Container::new()
      .set_width(18)
      .set_height(18)
      .set_corner_radius(4)
      .set_border(Border::new(1.5))
      .set_alignment(Alignment::CENTER)
      .set_background(
        Value::from(SystemColors::FIELD)
          .when(State::Hovered, SystemColors::CONTROL_HOVER)
          .when(State::Pressed, SystemColors::CONTROL_PRESSED)
          .when(State::Disabled, SystemColors::FIELD)
          .when(State::Checked, SystemColors::ACCENT)
          .when(State::Indeterminate, SystemColors::ACCENT),
      )
      .set_border_color(
        Value::from(SystemColors::BORDER)
          .when(State::Checked, SystemColors::ACCENT)
          .when(State::Indeterminate, SystemColors::ACCENT),
      )
      .set_color(SystemColors::ON_ACCENT)
      .set_ring(focus_ring())
      .set_ring_color(SystemColors::ACCENT)
  }

  /// The default mark: [`marks::check`], filling the face.
  pub fn default_mark() -> Painted<R> {
    Painted::new(marks::check)
      .set_width(Dimension::Fraction(1.))
      .set_height(Dimension::Fraction(1.))
  }
}

/// What a [`Checkbox`] carries beside every control's members: its check, its face and its
/// mark.
pub trait CheckboxBase<R: Realization>: Wraps<Inner = Checkbox<R>> {
  /// A checkbox labelled `label`, unchecked.
  fn new(label: impl Into<String>) -> Self {
    Self::wrap(Checkbox {
      control: crate::view::Control::new(Behavior::checkbox()),
      label: label.into(),
      face: None,
      mark: None,
    })
  }

  /// Whether the checkbox is checked.
  fn set_checked(self, checked: bool) -> Self {
    self.derive_checked(move || checked)
  }

  /// Whether the checkbox is checked, kept current.
  fn derive_checked(self, checked: impl Fn() -> bool + 'static) -> Self {
    self.map_inner(|mut checkbox| {
      checkbox.control = checkbox
        .control
        .map_behavior(|behavior| behavior.derive_checked(checked));

      checkbox
    })
  }

  /// Whether the checkbox is mixed: neither checked nor unchecked, drawn with a dash.
  fn set_mixed(self, mixed: bool) -> Self {
    self.derive_mixed(move || mixed)
  }

  /// Whether the checkbox is mixed, kept current.
  fn derive_mixed(self, mixed: impl Fn() -> bool + 'static) -> Self {
    self.map_inner(|mut checkbox| {
      checkbox.control = checkbox
        .control
        .map_behavior(|behavior| behavior.derive_mixed(mixed));

      checkbox
    })
  }

  /// Runs `change` with the state the reader flipped the checkbox to.
  fn handle_change(self, change: impl Fn(bool) + 'static) -> Self {
    self.map_inner(|mut checkbox| {
      checkbox.control = checkbox
        .control
        .map_behavior(|behavior| behavior.handle_change(change));

      checkbox
    })
  }

  /// Shows `checked` and writes each flip into it.
  fn bind_checked(self, checked: Signal<bool>) -> Self {
    self.map_inner(|mut checkbox| {
      checkbox.control = checkbox
        .control
        .map_behavior(|behavior| behavior.bind_checked(checked));

      checkbox
    })
  }

  /// The face, in place of [`Checkbox::default_face`]; the mark is placed in it, so a child
  /// it holds is replaced.
  fn set_face(mut self, face: impl Wraps<Inner = Container<R>>) -> Self {
    self.inner_mut().face = Some(face.into_inner());

    self
  }

  /// The mark, in place of [`Checkbox::default_mark`]: any view, drawn under the checkbox's
  /// states (`State::Checked`, `State::Indeterminate`).
  fn set_mark(mut self, mark: impl View<R>) -> Self {
    self.inner_mut().mark = Some(mark.into_any());

    self
  }
}

impl<R: Realization> CheckboxBase<R> for Checkbox<R> {}

impl<R: Realization> AnyKind for Checkbox<R> {}

impl<R: Realization> View<R> for Checkbox<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let Self {
      control,
      label,
      face,
      mark,
    } = self;
    let defaults = crate::view::Control::new(Pending)
      .set_item_alignment(AxisAlignment::Center)
      .set_spacing(8)
      .set_label(Label::from(label));
    let mark = mark.unwrap_or_else(|| Self::default_mark().into_any());
    let face = face
      .unwrap_or_else(Self::default_face)
      .set_child(AsPart::new(Part::Mark).set_child(mark));

    control.over(defaults).add_part(Part::Face, face).build(cx)
  }
}
