//! A switch.

use crate::{
  Alignment, AxisAlignment, Dimension, Flexibility, Inset, Offset, Signal, State, SystemColors,
  Value, When as _,
  behavior::Toggle as Behavior,
  seam::{Realization, control::Part},
  view::{
    AnyKind, AsPart, AsPartBase, Container, ContainerBase, ControlBase, ControlMembersBase, Cx,
    DecorationBase, Flexible, FlexibleBase, Label, Node, Text, TextBase, View, Wraps,
    controls::{control_surfaces, focus_ring},
    wraps_itself,
  },
};

/// An on/off control with its caption: Skene's switch, drawn on every platform. It is a
/// [`crate::view::Control`] over a switch's [`crate::behavior::Toggle`]: `Space` flips it at
/// its release, a click anywhere on it, its caption included.
///
/// It is controlled: it shows what [`ToggleBase::derive_on`] returns, asks
/// [`ToggleBase::handle_change`] for each flip, and shows the truth.
///
/// Its parts: the caption first, taking the room the switch leaves, 12 pixels before its
/// track ([`Toggle::default_track`]), which holds its thumb ([`Toggle::default_thumb`]). The
/// thumb moves by its own width while on, by paint alone.
#[must_use]
pub struct Toggle<R: Realization> {
  /// The control it composes, its defaults set.
  control: crate::view::Control<Behavior, R>,
  /// The track.
  track: Container<R>,
  /// The thumb.
  thumb: Container<R>,
}

wraps_itself!(Toggle<R> where R: Realization);

control_surfaces!([R: Realization] Toggle<R>, R => control);

impl<R: Realization> Toggle<R> {
  /// The default track: 44 by 24 pixels, its corners rounded by 12, padded 2, its thumb at
  /// its leading end; filled with [`SystemColors::TRACK`], its hover and pressed variants,
  /// and the accent while on; a ring of 2 in the accent while focus shows.
  pub fn default_track() -> Container<R> {
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
  pub fn default_thumb() -> Container<R> {
    Container::new()
      .set_width(20)
      .set_height(20)
      .set_corner_radius(10)
      .set_background(SystemColors::KNOB)
      .set_offset(Offset::ZERO.when(State::Checked, Offset::of_size(1., 0.)))
  }
}

/// What a [`Toggle`] carries beside every control's members: whether it is on, its track and
/// its thumb.
pub trait ToggleBase<R: Realization>: Wraps<Inner = Toggle<R>> {
  /// A switch labelled `label`, off.
  fn new(label: impl Into<String>) -> Self {
    let label = label.into();
    let caption = Flexible::new()
      .set_flexibility(Flexibility {
        grow: 1.,
        shrink: 1.,
        initial: Dimension::Auto,
      })
      .set_child(Text::<R>::new(label.clone()));
    let control = crate::view::Control::new(Behavior::switch())
      .set_item_alignment(AxisAlignment::Center)
      .set_spacing(12)
      .set_label(Label::new(caption).set_name(label));

    Self::wrap(Toggle {
      control,
      track: Toggle::<R>::default_track(),
      thumb: Toggle::<R>::default_thumb(),
    })
  }

  /// Whether the switch is on.
  fn set_on(self, on: bool) -> Self {
    self.derive_on(move || on)
  }

  /// Whether the switch is on, kept current.
  fn derive_on(self, on: impl Fn() -> bool + 'static) -> Self {
    self.map_inner(|mut toggle| {
      toggle.control = toggle
        .control
        .map_behavior(|behavior| behavior.derive_checked(on));

      toggle
    })
  }

  /// Runs `change` with the state the reader flipped the switch to.
  fn handle_change(self, change: impl Fn(bool) + 'static) -> Self {
    self.map_inner(|mut toggle| {
      toggle.control = toggle
        .control
        .map_behavior(|behavior| behavior.handle_change(change));

      toggle
    })
  }

  /// Shows `on` and writes each flip into it.
  fn bind_on(self, on: Signal<bool>) -> Self {
    self.map_inner(|mut toggle| {
      toggle.control = toggle
        .control
        .map_behavior(|behavior| behavior.bind_checked(on));

      toggle
    })
  }

  /// The track, in place of [`Toggle::default_track`]; the thumb is placed in it, so a child
  /// it holds is replaced.
  fn set_track(mut self, track: impl Wraps<Inner = Container<R>>) -> Self {
    self.inner_mut().track = track.into_inner();

    self
  }

  /// The thumb, in place of [`Toggle::default_thumb`]. A track whose content is not twice the
  /// thumb's width moves the thumb by another `offset`.
  fn set_thumb(mut self, thumb: impl Wraps<Inner = Container<R>>) -> Self {
    self.inner_mut().thumb = thumb.into_inner();

    self
  }
}

impl<R: Realization> ToggleBase<R> for Toggle<R> {}

impl<R: Realization> AnyKind for Toggle<R> {}

impl<R: Realization> View<R> for Toggle<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let track = self
      .track
      .set_child(AsPart::new(Part::Thumb).set_child(self.thumb));

    self.control.add_part(Part::Track, track).build(cx)
  }
}
