//! A checkbox.

use skene_foundation::{
  Alignment, AxisAlignment, Border, Dimension, State, SystemColors, Value,
  behavior::Toggle as Behavior, seam::control::Part,
};

use crate::{
  AsPart, Container, Control, Painted, Signal,
  control::control_members,
  controls::{focus_ring, marks},
  view::{AnyView, Cx, Node, View},
};

/// A checkbox with its caption: Skene's checkbox, drawn on every platform. It is a
/// [`Control`] over a checkbox's [`skene_foundation::behavior::Toggle`]: `Space` flips it at
/// its release, a click anywhere on it, its caption included. A mixed checkbox
/// ([`Checkbox::derive_mixed`]) turns on when flipped.
///
/// It is controlled: it shows what [`Checkbox::derive_checked`] returns, asks
/// [`Checkbox::handle_change`] for each flip, and shows the truth.
///
/// Its parts: its face ([`Checkbox::default_face`]) holding its mark
/// ([`Checkbox::default_mark`]), then its caption, 8 pixels after.
#[must_use]
pub struct Checkbox {
  pub(super) control: Control<Behavior>,
  pub(super) face: Container,
  mark: AnyView,
}

impl Checkbox {
  /// A checkbox labelled `label`, unchecked.
  pub fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();
    let control = Control::new(Behavior::checkbox())
      .set_item_alignment(AxisAlignment::Center)
      .set_spacing(8)
      .set_label(label);

    Self {
      control,
      face: Self::default_face(),
      mark: Self::default_mark().into_any(),
    }
  }

  /// The default face: 18 pixels square, its corners rounded by 4, a border of 1.5 in
  /// [`SystemColors::BORDER`] around [`SystemColors::FIELD`] and its hover and pressed
  /// variants, the accent while checked or mixed, its mark centered and inked in
  /// [`SystemColors::ON_ACCENT`]; a ring of 2 in the accent while focus shows.
  pub fn default_face() -> Container {
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
  pub fn default_mark() -> Painted {
    Painted::new(marks::check)
      .set_width(Dimension::Fraction(1.))
      .set_height(Dimension::Fraction(1.))
  }

  /// Whether the checkbox is checked.
  pub fn set_checked(self, checked: bool) -> Self {
    self.derive_checked(move || checked)
  }

  /// Whether the checkbox is checked, kept current.
  pub fn derive_checked(mut self, checked: impl Fn() -> bool + 'static) -> Self {
    self.control = self
      .control
      .map_behavior(|behavior| behavior.derive_checked(checked));

    self
  }

  /// Whether the checkbox is mixed: neither checked nor unchecked, drawn with a dash.
  pub fn set_mixed(self, mixed: bool) -> Self {
    self.derive_mixed(move || mixed)
  }

  /// Whether the checkbox is mixed, kept current.
  pub fn derive_mixed(mut self, mixed: impl Fn() -> bool + 'static) -> Self {
    self.control = self
      .control
      .map_behavior(|behavior| behavior.derive_mixed(mixed));

    self
  }

  /// Runs `change` with the state the reader flipped the checkbox to.
  pub fn handle_change(mut self, change: impl Fn(bool) + 'static) -> Self {
    self.control = self
      .control
      .map_behavior(|behavior| behavior.handle_change(change));

    self
  }

  /// Shows `checked` and writes each flip into it.
  pub fn bind_checked(mut self, checked: Signal<bool>) -> Self {
    self.control = self
      .control
      .map_behavior(|behavior| behavior.bind_checked(checked));

    self
  }

  /// The face, in place of [`Checkbox::default_face`]; the mark is placed in it, so a child
  /// it holds is replaced.
  pub fn set_face(mut self, face: Container) -> Self {
    self.face = face;

    self
  }

  /// The mark, in place of [`Checkbox::default_mark`]: any view, drawn under the checkbox's
  /// states (`State::Checked`, `State::Indeterminate`).
  pub fn set_mark(mut self, mark: impl View) -> Self {
    self.mark = mark.into_any();

    self
  }

  control_members!(control.members);
}

impl View for Checkbox {
  fn build(self, cx: &mut Cx) -> Node {
    let face = self
      .face
      .set_child(AsPart::new(Part::Mark).set_child(self.mark));

    self.control.add_part(Part::Face, face).build(cx)
  }
}
