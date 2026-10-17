//! A switch.

use skene_foundation::{
  Alignment, AxisAlignment, Dimension, Flexibility, Inset, Offset, State, SystemColors, Value,
  When as _, behavior::Toggle as Behavior, seam::control::Part,
};

use crate::{
  AsPart, Container, Control, Flexible, Label, Signal, Text,
  control::control_members,
  controls::focus_ring,
  view::{Cx, Node, View},
};

/// An on/off control with its caption: Skene's switch, drawn on every platform. It is a
/// [`Control`] over a switch's [`skene_foundation::behavior::Toggle`]: `Space` flips it at its release, a click
/// anywhere on it, its caption included.
///
/// It is controlled: it shows what [`Toggle::derive_on`] returns, asks
/// [`Toggle::handle_change`] for each flip, and shows the truth.
///
/// Its parts: the caption first, taking the room the switch leaves, 12 pixels before its
/// track ([`Toggle::default_track`]), which holds its thumb ([`Toggle::default_thumb`]). The
/// thumb moves by its own width while on, by paint alone.
#[must_use]
pub struct Toggle {
  /// The control it composes, its defaults set.
  pub(super) control: Control<Behavior>,
  /// The track.
  pub(super) track: Container,
  /// The thumb.
  pub(super) thumb: Container,
}

impl Toggle {
  /// A switch labelled `label`, off.
  pub fn new(label: impl Into<String>) -> Self {
    let label = label.into();
    let caption = Flexible::new()
      .set_flexibility(Flexibility {
        grow: 1.,
        shrink: 1.,
        initial: Dimension::Auto,
      })
      .set_child(Text::new(label.clone()));
    let control = Control::new(Behavior::switch())
      .set_item_alignment(AxisAlignment::Center)
      .set_spacing(12)
      .set_label(Label::new(caption).set_name(label));

    Self {
      control,
      track: Self::default_track(),
      thumb: Self::default_thumb(),
    }
  }

  /// The default track: 44 by 24 pixels, its corners rounded by 12, padded 2, its thumb at
  /// its leading end; filled with [`SystemColors::TRACK`], its hover and pressed variants,
  /// and the accent while on; a ring of 2 in the accent while focus shows.
  pub fn default_track() -> Container {
    Container::new()
      .set_width(44)
      .set_height(24)
      .set_corner_radius(12)
      .set_padding(Inset::all(2))
      .set_alignment(Alignment::new(AxisAlignment::Start, AxisAlignment::Center))
      .set_background(
        Value::from(SystemColors::TRACK)
          .when(State::Hovered, SystemColors::CONTROL_HOVER)
          .when(State::Pressed, SystemColors::CONTROL_PRESSED)
          .when(State::Disabled, SystemColors::TRACK)
          .when(State::Checked, SystemColors::ACCENT),
      )
      .set_ring(focus_ring())
      .set_ring_color(SystemColors::ACCENT)
  }

  /// The default thumb: a circle 20 pixels across in [`SystemColors::KNOB`], moved by its
  /// own width while on.
  pub fn default_thumb() -> Container {
    Container::new()
      .set_width(20)
      .set_height(20)
      .set_corner_radius(10)
      .set_background(SystemColors::KNOB)
      .set_offset(Offset::ZERO.when(State::Checked, Offset::of_size(1., 0.)))
  }

  /// Whether the switch is on.
  pub fn set_on(self, on: bool) -> Self {
    self.derive_on(move || on)
  }

  /// Whether the switch is on, kept current.
  pub fn derive_on(mut self, on: impl Fn() -> bool + 'static) -> Self {
    self.control = self
      .control
      .map_behavior(|behavior| behavior.derive_checked(on));

    self
  }

  /// Runs `change` with the state the reader flipped the switch to.
  pub fn handle_change(mut self, change: impl Fn(bool) + 'static) -> Self {
    self.control = self
      .control
      .map_behavior(|behavior| behavior.handle_change(change));

    self
  }

  /// Shows `on` and writes each flip into it.
  pub fn bind_on(mut self, on: Signal<bool>) -> Self {
    self.control = self
      .control
      .map_behavior(|behavior| behavior.bind_checked(on));

    self
  }

  /// The track, in place of [`Toggle::default_track`]; the thumb is placed in it, so a child
  /// it holds is replaced.
  pub fn set_track(mut self, track: Container) -> Self {
    self.track = track;

    self
  }

  /// The thumb, in place of [`Toggle::default_thumb`]. A track whose content is not twice the
  /// thumb's width moves the thumb by another `offset`.
  pub fn set_thumb(mut self, thumb: Container) -> Self {
    self.thumb = thumb;

    self
  }

  control_members!(control.members);
}

impl View for Toggle {
  fn build(self, cx: &mut Cx) -> Node {
    let track = self
      .track
      .set_child(AsPart::new(Part::Thumb).set_child(self.thumb));

    self.control.add_part(Part::Track, track).build(cx)
  }
}
