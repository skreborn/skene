//! A slider.

use std::rc::Rc;

use skene_foundation::{
  Alignment, AxisAlignment, Dimension, Flexibility, Inset, Offset, Ring, State, SystemColors,
  Value, When as _,
  behavior::{Slider as Behavior, SliderKeys},
  seam::control::Part,
};

use crate::{
  AsPart, Container, Control, Flexible, Signal, Stack,
  control::control_members,
  view::{Cx, Node, View},
};

/// A value along a range, with its caption: Skene's slider, drawn on every platform. It is a
/// [`Control`] over [`skene_foundation::behavior::Slider`]: the arrows step it, the page keys
/// move it by a tenth, `Home` and `End` take it to its ends, a press jumps to where it lands
/// and a drag follows the pointer.
///
/// It is controlled: it shows what [`Slider::derive_value`] returns, asks
/// [`Slider::handle_change`] for each move, and shows the truth.
///
/// Its caption sits before it among its siblings, a press on it focusing the slider. Its
/// parts: a track ([`Slider::default_track`]) clipping its fill ([`Slider::default_fill`]),
/// and a thumb ([`Slider::default_thumb`]) centered on the value. The fill and the thumb move
/// by paint alone. It is 200 pixels wide unless its layout stretches or shrinks it, 80 at
/// least, and 24 tall, padded 9 at the sides; it takes a press inside its box alone.
#[must_use]
pub struct Slider {
  /// The control it composes, its defaults set, over a behavior replaced as it is built.
  pub(super) control: Control<Behavior>,
  /// The value.
  value: Rc<dyn Fn() -> f32>,
  /// The least and greatest values.
  range: Rc<dyn Fn() -> (f32, f32)>,
  /// The step the value moves by.
  step: Rc<dyn Fn() -> f32>,
  /// What hears each move.
  change: Option<Rc<dyn Fn(f32)>>,
  /// The track.
  pub(super) track: Container,
  /// The fill.
  pub(super) fill: Container,
  /// The thumb.
  pub(super) thumb: Container,
}

impl Slider {
  /// A slider labelled `label`, at 0 of 0 to 1, stepping by a hundredth.
  pub fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();

    Self {
      control: Control::new(Behavior::new())
        .set_min_width(80)
        .set_min_height(24)
        .set_padding(Inset::new(0, 9, 0, 9))
        .set_item_alignment(AxisAlignment::Center)
        .set_label(label),
      value: Rc::new(|| 0.),
      range: Rc::new(|| (0., 1.)),
      step: Rc::new(|| 0.01),
      change: None,
      track: Self::default_track(),
      fill: Self::default_fill(),
      thumb: Self::default_thumb(),
    }
  }

  /// The default track: the slider's width, 4 pixels tall, its corners rounded by 2, in
  /// [`SystemColors::TRACK`], clipping the fill.
  pub fn default_track() -> Container {
    Container::new()
      .set_width(Dimension::Fraction(1.))
      .set_height(4)
      .set_corner_radius(2)
      .set_background(SystemColors::TRACK)
      .set_clip(true)
  }

  /// The default fill: as large as the track, its corners rounded by 2, in the accent.
  pub fn default_fill() -> Container {
    Container::new()
      .set_width(Dimension::Fraction(1.))
      .set_height(Dimension::Fraction(1.))
      .set_corner_radius(2)
      .set_background(SystemColors::ACCENT)
  }

  /// The default thumb: a circle 18 pixels across in [`SystemColors::KNOB`], its center on
  /// the value (moved back by half its width), edged by half a pixel of
  /// [`SystemColors::BORDER`], a ring of 2 in the accent while focus shows.
  pub fn default_thumb() -> Container {
    Container::new()
      .set_width(18)
      .set_height(18)
      .set_corner_radius(9)
      .set_offset(Offset::of_size(-0.5, 0.))
      .set_background(SystemColors::KNOB)
      .set_ring(Ring::new(0.5).when(State::FocusVisible, Ring::new(2)))
      .set_ring_color(
        Value::from(SystemColors::BORDER).when(State::FocusVisible, SystemColors::ACCENT),
      )
  }

  /// The slider's value.
  pub fn set_value(self, value: f32) -> Self {
    self.derive_value(move || value)
  }

  /// The slider's value, kept current.
  pub fn derive_value(mut self, value: impl Fn() -> f32 + 'static) -> Self {
    self.value = Rc::new(value);

    self
  }

  /// Shows `value` and writes each move into it.
  pub fn bind_value(self, value: Signal<f32>) -> Self {
    self
      .derive_value(move || value.get())
      .handle_change(move |moved| value.set(moved))
  }

  /// The least and greatest values; 0 to 1 where unset.
  pub fn set_range(self, minimum: f32, maximum: f32) -> Self {
    self.derive_range(move || (minimum, maximum))
  }

  /// The least and greatest values, kept current.
  pub fn derive_range(mut self, range: impl Fn() -> (f32, f32) + 'static) -> Self {
    self.range = Rc::new(range);

    self
  }

  /// The step the value moves by; a hundredth where unset.
  pub fn set_step(self, step: f32) -> Self {
    self.derive_step(move || step)
  }

  /// The step the value moves by, kept current.
  pub fn derive_step(mut self, step: impl Fn() -> f32 + 'static) -> Self {
    self.step = Rc::new(step);

    self
  }

  /// Runs `change` with the value the reader moved the slider to.
  pub fn handle_change(mut self, change: impl Fn(f32) + 'static) -> Self {
    self.change = Some(Rc::new(change));

    self
  }

  /// The track, in place of [`Slider::default_track`]; the fill is placed in it, so a child it
  /// holds is replaced.
  pub fn set_track(mut self, track: Container) -> Self {
    self.track = track;

    self
  }

  /// The fill, in place of [`Slider::default_fill`]: as large as the track, it is moved back
  /// by the part of the range above the value, the track clipping it.
  pub fn set_fill(mut self, fill: Container) -> Self {
    self.fill = fill;

    self
  }

  /// The thumb, in place of [`Slider::default_thumb`]: its leading edge on the value, unless
  /// its `offset` moves it.
  pub fn set_thumb(mut self, thumb: Container) -> Self {
    self.thumb = thumb;

    self
  }

  control_members!(control.members);
}

impl View for Slider {
  fn build(self, cx: &mut Cx) -> Node {
    let Self {
      control,
      value,
      range,
      step,
      change,
      track,
      fill,
      thumb,
    } = self;
    let range = Rc::new(move || {
      let (minimum, maximum) = range();

      (f64::from(minimum), f64::from(maximum), f64::from(step()))
    });
    let fraction = {
      let (value, range) = (Rc::clone(&value), Rc::clone(&range));

      move || {
        let (minimum, maximum, step) = range();

        SliderKeys::new(skene_foundation::seam::content::Range {
          value: f64::from(value()),
          minimum,
          maximum,
          step,
        })
        .fraction() as f32
      }
    };
    let fill_fraction = fraction.clone();
    let fill = fill.derive_offset(move || Offset::of_size(fill_fraction() - 1., 0.));
    // As wide as the track where the track's width is known, 182 where its layout asks how
    // wide it likes to be.
    let carrier = Container::new()
      .set_width(182)
      .set_min_width(Dimension::Fraction(1.))
      .set_max_width(Dimension::Fraction(1.))
      .set_height(18)
      .set_alignment(Alignment::new(AxisAlignment::Start, AxisAlignment::Center))
      .derive_offset(move || Offset::of_size(fraction(), 0.))
      .set_child(AsPart::new(Part::Thumb).set_child(thumb));
    let body = Stack::new()
      .set_item_alignment(Alignment::new(AxisAlignment::Start, AxisAlignment::Center))
      .add_child(
        AsPart::new(Part::Track)
          .set_child(track.set_child(AsPart::new(Part::Fill).set_child(fill))),
      )
      .add_child(carrier);
    let mut behavior = Behavior::new()
      .derive_value(move || f64::from(value()))
      .derive_range(move || range());

    if let Some(change) = change {
      behavior = behavior.handle_change(move |moved| change(moved as f32));
    }

    control
      .map_behavior(|_| behavior)
      .set_child(
        Flexible::new()
          .set_flexibility(Flexibility {
            grow: 1.,
            shrink: 1.,
            initial: Dimension::Length(182.),
          })
          .set_child(body),
      )
      .build(cx)
  }
}
