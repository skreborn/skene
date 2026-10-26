//! A checkbox, drawn with the platform's colors.

use skene_foundation::{
  Alignment,
  AnyKind,
  Condition,
  ContainerBase,
  ControlBase,
  DecorationBase,
  DecorationWithCornerRadius,
  Dimension,
  HasChild,
  Inset,
  MotionBase,
  MouseCursor,
  NodeId,
  Part,
  RunBase,
  State,
  SystemColors,
  Text,
  Toggle,
  ToggleBase,
  View,
  When,
  Wraps,
};

use crate::Active;

/// The platform's colors in logical pixels.
type Colors = SystemColors<f32>;

/// A checkbox: Skene's, drawn on every engine, a [`Control`](crate::Control) over
/// [`Toggle`](crate::behavior::Toggle): a click, `Space` or the platform's own toggle
/// flips it.
///
/// Its mark is its [`Part::Mark`], its caption trailing it
/// ([`set_label`](ControlBase::set_label)), and the page's own checkbox keeps it
/// scriptless, covering the caption unseen. Its default mark, which
/// [`set_part(Part::Mark, ..)`](ControlBase::set_part) replaces: a box of 18 pixels
/// rounded by 4 in a border of 2, filled with [`SystemColors::FIELD`] and bordered with
/// [`SystemColors::BORDER`], both the accent while checked, a check in
/// [`SystemColors::ON_ACCENT`] showing while checked, and its border the accent while
/// focus shows ([`SystemColors::INK`] while checked too). Its default cursor is the click
/// cursor, the default one while disabled; every other default is the control's.
///
/// ```
/// use skene_gui::prelude::*;
///
/// let agreed = Signal::new(false);
/// let terms = Checkbox::new().set_label("I agree").bind_checked(agreed);
/// ```
#[derive(Debug, Wraps)]
#[must_use]
pub struct Checkbox(skene_foundation::Control<Active, Toggle>);

impl Default for Checkbox {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Checkbox {
  /// An unchecked checkbox with no handler.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::Control::new(Toggle::new()))
  }
}

impl Checkbox {
  /// The default mark: a rounded box that fills with the accent while checked, its check
  /// showing then.
  fn mark() -> skene_foundation::Container<Active> {
    let checked = Condition::from(State::Checked);
    let focused = Condition::from(State::FocusVisible);

    skene_foundation::Container::<Active>::new()
      .set_width(Dimension::Length(18.))
      .set_height(Dimension::Length(18.))
      .set_corner_radius(4)
      .set_border_width(Inset::all(2))
      .set_border_color(
        Colors::BORDER
          .when(checked.clone(), Colors::ACCENT)
          .when(focused.clone(), Colors::ACCENT)
          .when(Condition::and([checked.clone(), focused]), Colors::INK),
      )
      .set_background(Colors::FIELD.when(checked, Colors::ACCENT))
      .set_alignment(Alignment::CENTER)
      .set_child(
        skene_foundation::Container::new()
          .set_hidden(true.when(State::Checked, false))
          .set_child(Text::new("\u{2713}").set_color(Colors::ON_ACCENT)),
      )
  }
}

impl ControlBase<Active, Toggle> for Checkbox {}

impl ToggleBase<Active> for Checkbox {}

impl DecorationBase<Active> for Checkbox {}

impl MotionBase<Active> for Checkbox {}

impl AnyKind for Checkbox {}

impl View<Active> for Checkbox {
  fn build(self) -> NodeId {
    let defaults = skene_foundation::Control::new(Toggle::new())
      .set_cursor(MouseCursor::Click.when(State::Disabled, MouseCursor::Default))
      .set_part(Part::Mark, Self::mark());

    self.0.over(defaults).build()
  }
}
