//! A slider, drawn with the platform's colors.

use std::rc::Rc;

use skene_foundation::{
  Alignment,
  AnyKind,
  Condition,
  ContainerBase,
  ControlBase,
  DecorationBase,
  Dimension,
  FlexChild,
  Flexibility,
  HasBoxStyle,
  HasChild,
  HasChildren,
  HasControl,
  Inset,
  IntoChildOf,
  ItemAlignment,
  Length,
  MotionBase,
  MouseCursor,
  NodeHandle,
  NodeId,
  Overflow,
  Part,
  Scene,
  SliderRange,
  StackBase,
  State,
  SystemColors,
  View,
  When,
  Wraps,
};
use skene_reactive::{Signal, untrack};

use crate::Active;

/// The platform's colors in cells.
type Colors = SystemColors<i32>;

/// A slider: Skene's, drawn in cells, a [`Control`](crate::Control) over a
/// [`Slider`](crate::behavior::Slider).
///
/// The arrows step it, the page keys move it by a tenth, `Home` and `End` take it to its
/// ends, and a press or a drag along its track jumps to where the pointer lands.
///
/// Controlled: its value is what [`derive_value`](Self::derive_value) reads, and a move
/// only calls [`handle_change`](Self::handle_change) with the value it asks for;
/// [`bind_value`](Self::bind_value) wires both to one signal.
///
/// Its track is its [`Part::Track`], holding its fill, the [`Part::Fill`], and its thumb,
/// the [`Part::Thumb`], its caption trailing it ([`set_label`](ControlBase::set_label)).
/// Its defaults: 20 cells wide, the track taking what its caption leaves. Its default
/// track, which [`set_part(Part::Track, ..)`](ControlBase::set_part) replaces with its
/// fill and its thumb (the replacement's thumb [as wide](Self::set_thumb_extent) as the
/// pointer is to measure it): a rail of one row of cells in [`SystemColors::TRACK`],
/// clipping its fill, cells in [`SystemColors::ACCENT`], to the thumb's leading edge; its
/// thumb a cell in [`SystemColors::KNOB`], [`SystemColors::ACCENT_PRESSED`] while focus
/// shows, travelling the track less its own width. The fill and the thumb follow the
/// value by offset rather than layout. Its default cursor
/// is the click cursor, the default one while disabled; every other default is the
/// control's.
///
/// ```
/// use skene_terminal::{behavior::SliderRange, prelude::*};
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
  /// The width of the default thumb, in cells.
  pub const THUMB: i32 = 1;

  /// A slider over `0..=100` stepping by `1`, holding its minimum, with no handler.
  pub fn new() -> Self {
    Self(skene_foundation::Control::new(
      skene_foundation::Slider::new().set_thumb_extent(Self::THUMB),
    ))
  }

  /// Sets the range: its bounds and the step values snap to.
  pub fn set_range(self, range: SliderRange) -> Self {
    self.map_control(|control| control.map_behavior(|slider| slider.set_range(range)))
  }

  /// Derives the range from the signals `range` reads.
  pub fn derive_range(self, range: impl Fn() -> SliderRange + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|slider| slider.derive_range(range)))
  }

  /// Sets the value.
  pub fn set_value(self, value: f32) -> Self {
    self.map_control(|control| control.map_behavior(|slider| slider.set_value(value)))
  }

  /// Derives the value from the signals `value` reads.
  pub fn derive_value(self, value: impl Fn() -> f32 + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|slider| slider.derive_value(value)))
  }

  /// Runs `action` with the value a move asks for.
  pub fn handle_change(self, action: impl Fn(f32) + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|slider| slider.handle_change(action)))
  }

  /// Binds the value to `signal` in both directions.
  pub fn bind_value(self, signal: Signal<f32>) -> Self {
    self.map_control(|control| control.map_behavior(|slider| slider.bind_value(signal)))
  }

  /// Sets the width of the thumb the pointer is measured against, in cells:
  /// its center travels from half of it past the track's start to half of it short of
  /// its end; the default thumb's unless told otherwise.
  pub fn set_thumb_extent(self, extent: impl Into<Length>) -> Self {
    let extent = extent.into();

    self.map_control(|control| control.map_behavior(|slider| slider.set_thumb_extent(extent)))
  }

  crate::views::terminal_decoration!(box_style_mut);
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
      top: 0,
      end: Self::THUMB,
      bottom: 0,
      start: 0,
    };
    let filled = Rc::clone(&fraction);
    let rail = skene_foundation::Container::<Active>::new()
      .set_width(Dimension::Fraction(1.))
      .set_height(Dimension::Length(1))
      .set_padding(spared)
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
          .set_background(
            Colors::KNOB.when(Condition::from(State::FocusVisible), Colors::ACCENT_PRESSED),
          )
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

impl DecorationBase<Active> for Slider {}

impl MotionBase<Active> for Slider {}

impl AnyKind for Slider {}

impl View<Active> for Slider {
  fn build(self) -> NodeId {
    let (fill, thumb) = (NodeHandle::new(), NodeHandle::new());
    let fraction = self.behavior().reads_fraction();
    let defaults = skene_foundation::Control::new(skene_foundation::Slider::new())
      .set_width(Dimension::Length(20))
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
