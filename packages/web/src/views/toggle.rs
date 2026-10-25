//! A switch, drawn with the platform's colors.

use skene_foundation::{
  Alignment,
  AnyKind,
  Color,
  Condition,
  ContainerBase,
  ControlBase,
  DecorationBase,
  DecorationWithCornerRadius,
  Dimension,
  HasChild,
  Inset,
  ItemAlignment,
  MotionBase,
  MouseCursor,
  NodeHandle,
  NodeId,
  Part,
  Scene,
  State,
  SystemColors,
  ToggleBase,
  View,
  When,
  Wraps,
};
use skene_reactive::untrack;

use crate::Active;

/// The platform's colors in logical pixels.
type Colors = SystemColors<f32>;

/// A switch: Skene's, drawn on every engine, a [`Control`](crate::Control) over a
/// [`Toggle`](crate::behavior::Toggle) made a [`switch`](crate::behavior::Toggle::switch),
/// reported as [`Role::Switch`](crate::Role::Switch): a click, `Space` or the platform's
/// own toggle turns it on or off.
///
/// Its track is its [`Part::Track`], holding its thumb, the [`Part::Thumb`], its caption
/// trailing it ([`set_label`](ControlBase::set_label)), and the page's own checkbox made a
/// switch keeps it scriptless, covering the caption unseen. Its default track, which
/// [`set_part(Part::Track, ..)`](ControlBase::set_part) replaces with its thumb: 40 by 24
/// pixels, padded 2 inside a border of 2, rounded to a pill, filled with
/// [`SystemColors::TRACK`], the accent while on, its border invisible but while focus
/// shows, a ring in the accent ([`SystemColors::INK`] while on); its thumb a disc of 16 in
/// [`SystemColors::KNOB`], at the start while off and shifted to the end while on, by
/// offset rather than layout. Its default cursor is the click cursor, the default one
/// while disabled; every other default is the control's.
///
/// ```
/// use skene_web::prelude::*;
///
/// let wifi = Signal::new(true);
/// let switch = Toggle::new().set_label("Wi-Fi").bind_checked(wifi);
/// ```
#[derive(Debug, Wraps)]
#[must_use]
pub struct Toggle(skene_foundation::Control<Active, skene_foundation::Toggle>);

impl Default for Toggle {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Toggle {
  /// A switch that is off, with no handler.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::Control::new(
      skene_foundation::Toggle::switch(),
    ))
  }
}

impl Toggle {
  /// The default track, its thumb bound to `thumb`: a pill that fills with the accent
  /// while on, the thumb shifting to its end.
  fn track(thumb: NodeHandle<Active>) -> skene_foundation::Container<Active> {
    let on = Condition::from(State::Checked);
    let focused = Condition::from(State::FocusVisible);
    let start = |start: f32| Inset {
      start: Dimension::Length(start),
      top: Dimension::Auto,
      end: Dimension::Auto,
      bottom: Dimension::Auto,
    };

    skene_foundation::Container::<Active>::new()
      .set_width(Dimension::Length(40.))
      .set_height(Dimension::Length(24.))
      .set_padding(Inset::all(2))
      .set_corner_radius(12)
      .set_border_width(Inset::all(2))
      .set_border_color(
        Color::TRANSPARENT
          .when(focused.clone(), Colors::ACCENT)
          .when(Condition::and([on.clone(), focused]), Colors::INK),
      )
      .set_background(Colors::TRACK.when(on.clone(), Colors::ACCENT))
      .set_alignment(Alignment::new(ItemAlignment::Start, ItemAlignment::Center))
      .set_child(
        skene_foundation::Container::new()
          .bind_handle(thumb)
          .set_width(Dimension::Length(16.))
          .set_height(Dimension::Length(16.))
          .set_corner_radius(8)
          .set_background(Colors::KNOB)
          .set_offset(start(0.).when(on, start(16.))),
      )
  }
}

impl ControlBase<Active, skene_foundation::Toggle> for Toggle {}

impl ToggleBase<Active> for Toggle {}

impl DecorationBase<Active> for Toggle {}

impl MotionBase<Active> for Toggle {}

impl AnyKind for Toggle {}

impl View<Active> for Toggle {
  fn build(self) -> NodeId {
    let thumb = NodeHandle::new();
    let defaults = skene_foundation::Control::new(skene_foundation::Toggle::switch())
      .set_cursor(MouseCursor::Click.when(State::Disabled, MouseCursor::Default))
      .set_part(Part::Track, Self::track(thumb));
    let node = self.0.over(defaults).build();

    if let Some(thumb) = untrack(|| thumb.node()) {
      Scene::<Active>::with(|scene| scene.set_part(thumb, Part::Thumb));
    }

    node
  }
}
