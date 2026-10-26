//! A switch, drawn with the platform's colors.

use skene_foundation::{
  Alignment,
  AnyKind,
  Condition,
  ContainerBase,
  ControlBase,
  DecorationBase,
  Dimension,
  HasBoxStyle,
  HasChild,
  Inset,
  ItemAlignment,
  MotionBase,
  MouseCursor,
  NodeHandle,
  NodeId,
  Part,
  RunBase,
  State,
  SystemColors,
  Text,
  ToggleBase,
  View,
  When,
  Wraps,
};
use skene_reactive::untrack;

use crate::{Active, Scene};

/// The platform's colors in cells.
type Colors = SystemColors<i32>;

/// A switch: Skene's, drawn in cells, a [`Control`](crate::Control) over a
/// [`Toggle`](crate::behavior::Toggle) made a [`switch`](crate::behavior::Toggle::switch),
/// reported as [`Role::Switch`](crate::Role::Switch): a click or `Space` turns it on or
/// off.
///
/// Its track is its [`Part::Track`], holding its thumb, the [`Part::Thumb`], its caption
/// trailing it ([`set_label`](ControlBase::set_label)). Its default track, which
/// [`set_part(Part::Track, ..)`](ControlBase::set_part) replaces with its thumb: three
/// cells in [`SystemColors::TRACK`], the accent while on and the pressed accent while on
/// with focus showing; its thumb the glyph `●` in [`SystemColors::KNOB`], the accent while
/// off with focus showing, in the first cell while off and shifted to the last while on,
/// by offset rather than layout. Its default cursor is the click cursor, the default one
/// while disabled; every other default is the control's.
///
/// ```
/// use skene_tui::prelude::*;
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

  crate::views::terminal_decoration!(box_style_mut);
}

impl Toggle {
  /// The default track, its thumb bound to `thumb`: three cells that fill with the
  /// accent while on, the thumb shifting to the last.
  fn track(thumb: NodeHandle<Active>) -> skene_foundation::Container<Active> {
    let on = Condition::from(State::Checked);
    let focused = Condition::from(State::FocusVisible);
    let start = |start: i32| Inset {
      start: Dimension::Length(start),
      top: Dimension::Auto,
      end: Dimension::Auto,
      bottom: Dimension::Auto,
    };

    skene_foundation::Container::<Active>::new()
      .set_width(Dimension::Length(3))
      .set_height(Dimension::Length(1))
      .set_background(Colors::TRACK.when(on.clone(), Colors::ACCENT).when(
        Condition::and([on.clone(), focused.clone()]),
        Colors::ACCENT_PRESSED,
      ))
      .set_alignment(Alignment::new(ItemAlignment::Start, ItemAlignment::Center))
      .set_child(
        skene_foundation::Container::new()
          .bind_handle(thumb)
          .set_width(Dimension::Length(1))
          .set_offset(start(0).when(on.clone(), start(2)))
          .set_child(
            Text::new("\u{25cf}").set_color(
              Colors::KNOB
                .when(focused, Colors::ACCENT)
                .when(on, Colors::KNOB),
            ),
          ),
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
      Scene::with(|scene| scene.set_part(thumb, Part::Thumb));
    }

    node
  }
}
