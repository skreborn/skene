//! A slider, drawn with the platform's colors.

use std::rc::Rc;

use skene_foundation::{
  Alignment,
  AnyKind,
  Condition,
  ContainerBase,
  ControlBase,
  DecorationBase,
  DecorationWithCornerRadius,
  Dimension,
  FlexChild,
  Flexibility,
  HasChild,
  HasChildren,
  Inset,
  IntoChildOf,
  ItemAlignment,
  MotionBase,
  MouseCursor,
  NodeHandle,
  NodeId,
  Overflow,
  Part,
  Scene,
  SliderBase,
  StackBase,
  State,
  SystemColors,
  View,
  When,
  Wraps,
};
use skene_reactive::untrack;

use crate::Active;

/// The platform's colors in logical pixels.
type Colors = SystemColors<f32>;

/// A slider: Skene's, drawn on every engine, a [`Control`](crate::Control) over a
/// [`Slider`](crate::behavior::Slider).
///
/// The arrows step it, the page keys move it by a tenth, `Home` and `End` take it to its
/// ends, and a press or a drag along its track jumps to where the pointer lands. The
/// page's own range input keeps it scriptless, covering it unseen.
///
/// Controlled: its value is what [`derive_value`](SliderBase::derive_value) reads, and a move
/// only calls [`handle_change`](SliderBase::handle_change) with the value it asks for;
/// [`bind_value`](SliderBase::bind_value) wires both to one signal.
///
/// Its track is its [`Part::Track`], holding its fill, the [`Part::Fill`], and its thumb,
/// the [`Part::Thumb`], its caption trailing it ([`set_label`](ControlBase::set_label)).
/// Its defaults: 160 pixels wide, the track taking what its caption leaves. Its default
/// track, which [`set_part(Part::Track, ..)`](ControlBase::set_part) replaces with its
/// fill and its thumb (the replacement's thumb [as
/// wide](SliderBase::set_thumb_extent) as the pointer is to measure it, both placed by
/// the fraction the slider shares with its parts, its
/// [`SliderContext`](crate::behavior::SliderContext)): a rail 4 pixels tall in
/// [`SystemColors::TRACK`], rounded, clipping its fill, in [`SystemColors::ACCENT`], to
/// the thumb's leading edge; its thumb a disc of 18 in [`SystemColors::KNOB`] in a
/// border of 2 in the accent, [`SystemColors::INK`] while focus shows, travelling the
/// track less its own width. The fill and the thumb follow the value by offset rather
/// than layout. Its default cursor is the click cursor, the default one while disabled;
/// every other default is the control's.
///
/// ```
/// use skene_web::{behavior::SliderRange, prelude::*};
///
/// let volume = Signal::new(50.);
/// let slider = Slider::new()
///   .set_label("Volume")
///   .set_range(SliderRange::new(0., 100., 5.))
///   .bind_value(volume);
/// ```
#[derive(Debug, Wraps)]
#[must_use]
pub struct Slider(skene_foundation::Control<Active, skene_foundation::Slider>);

impl Default for Slider {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Slider {
  /// The width of the default thumb, in logical pixels.
  pub const THUMB: f32 = 18.;

  /// A slider over `0..=100` stepping by `1`, holding its minimum, with no handler.
  pub fn new() -> Self {
    Self(skene_foundation::Control::new(
      skene_foundation::Slider::new().set_thumb_extent(Self::THUMB),
    ))
  }
}

impl Slider {
  /// The default track, its fill bound to `fill` and its thumb to `thumb`, both placed
  /// at the fraction of the range `fraction` reads: a rounded rail clipping the fill
  /// beneath a lane the thumb travels.
  fn track(
    fraction: Rc<dyn Fn() -> f32>,
    fill: NodeHandle<Active>,
    thumb: NodeHandle<Active>,
  ) -> impl View<Active> {
    let start = |start: f32| Inset {
      start: Dimension::Fraction(start),
      top: Dimension::Auto,
      end: Dimension::Auto,
      bottom: Dimension::Auto,
    };
    let spared = Inset {
      top: 0.,
      end: Self::THUMB,
      bottom: 0.,
      start: 0.,
    };
    let filled = Rc::clone(&fraction);
    let rail = skene_foundation::Container::<Active>::new()
      .set_width(Dimension::Fraction(1.))
      .set_height(Dimension::Length(4.))
      .set_padding(spared)
      .set_corner_radius(2)
      .set_overflow(Overflow::Clip)
      .set_background(Colors::TRACK)
      .set_child(
        skene_foundation::Container::new()
          .bind_handle(fill)
          .set_width(Dimension::Fraction(1.))
          .set_height(Dimension::Fraction(1.))
          .set_background(Colors::ACCENT)
          .derive_offset(move || start(filled() - 1.)),
      );
    let lane = skene_foundation::Container::<Active>::new()
      .set_width(Dimension::Fraction(1.))
      .set_height(Dimension::Length(Self::THUMB))
      .set_padding(spared)
      .set_alignment(Alignment::new(ItemAlignment::Start, ItemAlignment::Center))
      .set_child(
        skene_foundation::Container::new()
          .bind_handle(thumb)
          .set_width(Dimension::Length(Self::THUMB))
          .set_height(Dimension::Length(Self::THUMB))
          .set_corner_radius(Self::THUMB / 2.)
          .set_border_width(Inset::all(2))
          .set_border_color(Colors::ACCENT.when(Condition::from(State::FocusVisible), Colors::INK))
          .set_background(Colors::KNOB)
          .derive_offset(move || start(fraction())),
      );
    let track = skene_foundation::Container::<Active>::new()
      .set_height(Dimension::Length(Self::THUMB))
      .set_child(
        skene_foundation::Stack::new()
          .set_item_alignment(Alignment::new(ItemAlignment::Start, ItemAlignment::Center))
          .set_children((rail, lane)),
      );

    IntoChildOf::<Active, FlexChild>::into_child(
      skene_foundation::Flexible::new()
        .set_flexibility(Flexibility::new(0, 1., 1.))
        .set_child(track),
    )
  }
}

impl ControlBase<Active, skene_foundation::Slider> for Slider {}

impl SliderBase<Active> for Slider {}

impl DecorationBase<Active> for Slider {}

impl MotionBase<Active> for Slider {}

impl AnyKind for Slider {}

impl View<Active> for Slider {
  fn build(self) -> NodeId {
    let (fill, thumb) = (NodeHandle::new(), NodeHandle::new());
    let fraction = self.behavior().reads_fraction();
    let defaults = skene_foundation::Control::new(skene_foundation::Slider::new())
      .set_width(Dimension::Length(160.))
      .set_cursor(MouseCursor::Click.when(State::Disabled, MouseCursor::Default))
      .set_part(Part::Track, Self::track(fraction, fill, thumb));
    let node = self.0.over(defaults).build();

    untrack(|| {
      for (handle, part) in [(fill, Part::Fill), (thumb, Part::Thumb)] {
        if let Some(node) = handle.node() {
          Scene::<Active>::with(|scene| scene.set_part(node, part));
        }
      }
    });

    node
  }
}
