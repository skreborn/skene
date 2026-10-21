//! A slider.

use std::rc::Rc;

use crate::{
  Alignment, AxisAlignment, Dimension, Flexibility, Inset, Offset, Ring, Signal, State,
  SystemColors, Value, When as _,
  behavior::{Slider as Behavior, SliderKeys},
  seam::{Realization, control::Part},
  view::{
    AnyKind, AsPart, AsPartBase, Container, ContainerBase, ControlBase, ControlMembersBase, Cx,
    DecorationBase, Flexible, FlexibleBase, Node, Stack, StackBase, View, Wraps,
    controls::{Pending, control_surfaces},
    wraps_itself,
  },
};

/// A value along a range, with its caption: Skene's slider, drawn on every platform. It is a
/// [`crate::view::Control`] over [`crate::behavior::Slider`]: the arrows step it, the page keys
/// move it by a tenth, `Home` and `End` take it to its ends, a press jumps to where it lands
/// and a drag follows the pointer.
///
/// It is controlled: it shows what [`SliderBase::derive_value`] returns, asks
/// [`SliderBase::handle_change`] for each move, and shows the truth.
///
/// Its caption sits before it among its siblings, a press on it focusing the slider. Its
/// parts: a track ([`Slider::default_track`]) clipping its fill ([`Slider::default_fill`]),
/// and a thumb ([`Slider::default_thumb`]) centered on the value. The fill and the thumb move
/// by paint alone. It is 200 pixels wide unless its layout stretches or shrinks it, 80 at
/// least, and 24 tall, padded 9 at the sides; it takes a press on what it draws: its box, and
/// its thumb where the thumb overflows it.
#[must_use]
pub struct Slider<R: Realization> {
  /// The control it composes, its defaults set as it is built, over a behavior replaced then.
  control: crate::view::Control<Behavior, R>,
  /// Its caption.
  label: String,
  /// The value; 0 where unset.
  value: Option<Rc<dyn Fn() -> f32>>,
  /// The least and greatest values; 0 and 1 where unset.
  range: Option<Rc<dyn Fn() -> (f32, f32)>>,
  /// The step the value moves by; 0.01 where unset.
  step: Option<Rc<dyn Fn() -> f32>>,
  /// What hears each move.
  change: Option<Rc<dyn Fn(f32)>>,
  /// The track; the default where unset.
  track: Option<Container<R>>,
  /// The fill; the default where unset.
  fill: Option<Container<R>>,
  /// The thumb; the default where unset.
  thumb: Option<Container<R>>,
}

wraps_itself!(Slider<R> where R: Realization);

control_surfaces!([R: Realization] Slider<R>, R => control);

impl<R: Realization> Slider<R> {
  /// The default track: the slider's width, 4 pixels tall, its corners rounded by 2, in
  /// [`SystemColors::TRACK`], clipping the fill.
  pub fn default_track() -> Container<R> {
    Container::new()
      .set_width(Dimension::Fraction(1.))
      .set_height(4)
      .set_corner_radius(2)
      .set_background(SystemColors::TRACK)
      .set_clip(true)
  }

  /// The default fill: as large as the track, its corners rounded by 2, in the accent.
  pub fn default_fill() -> Container<R> {
    Container::new()
      .set_width(Dimension::Fraction(1.))
      .set_height(Dimension::Fraction(1.))
      .set_corner_radius(2)
      .set_background(SystemColors::ACCENT)
  }

  /// The default thumb: a circle 18 pixels across in [`SystemColors::KNOB`], its center on
  /// the value (moved back by half its width), edged by half a pixel of
  /// [`SystemColors::BORDER`], a ring of 2 in the accent while focus shows.
  pub fn default_thumb() -> Container<R> {
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
}

/// What a [`Slider`] carries beside every control's members: its value, its range, its step,
/// its handler, its track, its fill and its thumb.
pub trait SliderBase<R: Realization>: Wraps<Inner = Slider<R>> {
  /// A slider labelled `label`, at 0 of 0 to 1, stepping by a hundredth.
  fn new(label: impl Into<String>) -> Self {
    Self::wrap(Slider {
      control: crate::view::Control::new(Behavior::new()),
      label: label.into(),
      value: None,
      range: None,
      step: None,
      change: None,
      track: None,
      fill: None,
      thumb: None,
    })
  }

  /// The slider's value.
  fn set_value(self, value: f32) -> Self {
    self.derive_value(move || value)
  }

  /// The slider's value, kept current.
  fn derive_value(mut self, value: impl Fn() -> f32 + 'static) -> Self {
    self.inner_mut().value = Some(Rc::new(value));

    self
  }

  /// Shows `value` and writes each move into it.
  fn bind_value(self, value: Signal<f32>) -> Self {
    self
      .derive_value(move || value.get())
      .handle_change(move |moved| value.set(moved))
  }

  /// The least and greatest values; 0 to 1 where unset.
  fn set_range(self, minimum: f32, maximum: f32) -> Self {
    self.derive_range(move || (minimum, maximum))
  }

  /// The least and greatest values, kept current.
  fn derive_range(mut self, range: impl Fn() -> (f32, f32) + 'static) -> Self {
    self.inner_mut().range = Some(Rc::new(range));

    self
  }

  /// The step the value moves by; a hundredth where unset.
  fn set_step(self, step: f32) -> Self {
    self.derive_step(move || step)
  }

  /// The step the value moves by, kept current.
  fn derive_step(mut self, step: impl Fn() -> f32 + 'static) -> Self {
    self.inner_mut().step = Some(Rc::new(step));

    self
  }

  /// Runs `change` with the value the reader moved the slider to.
  fn handle_change(mut self, change: impl Fn(f32) + 'static) -> Self {
    self.inner_mut().change = Some(Rc::new(change));

    self
  }

  /// The track, in place of [`Slider::default_track`]; the fill is placed in it, so a child it
  /// holds is replaced.
  fn set_track(mut self, track: impl Wraps<Inner = Container<R>>) -> Self {
    self.inner_mut().track = Some(track.into_inner());

    self
  }

  /// The fill, in place of [`Slider::default_fill`]: as large as the track, it is moved back
  /// by the part of the range above the value, the track clipping it.
  fn set_fill(mut self, fill: impl Wraps<Inner = Container<R>>) -> Self {
    self.inner_mut().fill = Some(fill.into_inner());

    self
  }

  /// The thumb, in place of [`Slider::default_thumb`]: its leading edge on the value, unless
  /// its `offset` moves it.
  fn set_thumb(mut self, thumb: impl Wraps<Inner = Container<R>>) -> Self {
    self.inner_mut().thumb = Some(thumb.into_inner());

    self
  }
}

impl<R: Realization> SliderBase<R> for Slider<R> {}

impl<R: Realization> AnyKind for Slider<R> {}

impl<R: Realization> View<R> for Slider<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let Self {
      control,
      label,
      value,
      range,
      step,
      change,
      track,
      fill,
      thumb,
    } = self;
    let defaults = crate::view::Control::new(Pending)
      .set_min_width(80)
      .set_min_height(24)
      .set_padding(Inset::new(0, 9, 0, 9))
      .set_item_alignment(AxisAlignment::Center)
      .set_label(label);
    let value = value.unwrap_or_else(|| Rc::new(|| 0.));
    let range = range.unwrap_or_else(|| Rc::new(|| (0., 1.)));
    let step = step.unwrap_or_else(|| Rc::new(|| 0.01));
    let (track, fill, thumb) = (
      track.unwrap_or_else(Self::default_track),
      fill.unwrap_or_else(Self::default_fill),
      thumb.unwrap_or_else(Self::default_thumb),
    );
    let range = Rc::new(move || {
      let (minimum, maximum) = range();

      (f64::from(minimum), f64::from(maximum), f64::from(step()))
    });
    let fraction = {
      let (value, range) = (Rc::clone(&value), Rc::clone(&range));

      move || {
        let (minimum, maximum, step) = range();

        SliderKeys::new(crate::seam::content::Range {
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
    // wide it likes to be; no height, so it takes no press beyond the slider as it moves, its
    // top on the track's middle and the thumb centered on that line, overflowing it.
    let carrier = Container::new()
      .set_width(182)
      .set_min_width(Dimension::Fraction(1.))
      .set_max_width(Dimension::Fraction(1.))
      .set_height(0)
      .set_alignment(Alignment::new(AxisAlignment::Start, AxisAlignment::Start))
      .derive_offset(move || Offset::of_size(fraction(), 0.))
      .set_child(
        Container::new()
          .set_offset(Offset::of_size(0., -0.5))
          .set_child(AsPart::new(Part::Thumb).set_child(thumb)),
      );
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
      .over(defaults)
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
