//! A checkbox, drawn with the platform's colors.

use skene_foundation::{
  AnyKind,
  Condition,
  ControlBase,
  DecorationBase,
  HasBoxStyle,
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

/// The platform's colors in cells.
type Colors = SystemColors<i32>;

/// A checkbox: Skene's, drawn in cells, a [`Control`](crate::Control) over
/// [`Toggle`](crate::behavior::Toggle): a click or `Space` flips it.
///
/// Its mark is its [`Part::Mark`], its caption trailing it
/// ([`set_label`](ControlBase::set_label)). Its default mark, which
/// [`set_part(Part::Mark, ..)`](ControlBase::set_part) replaces: the glyphs `[ ]`, and
/// `[x]` while checked, in [`SystemColors::INK`], the accent while focus shows and
/// [`SystemColors::MUTED_INK`] while disabled. Its default cursor is the click cursor,
/// the default one while disabled; every other default is the control's.
///
/// ```
/// use skene_terminal::prelude::*;
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

  crate::views::terminal_decoration!(box_style_mut);
}

impl Checkbox {
  /// The default mark: its glyphs as `checked` reads, inked in the accent while focus
  /// shows.
  fn mark(checked: std::rc::Rc<dyn Fn() -> bool>) -> Text<Active> {
    Text::<Active>::derive(move || if checked() { "[x]" } else { "[ ]" }.to_owned()).set_color(
      Colors::INK
        .when(Condition::from(State::FocusVisible), Colors::ACCENT)
        .when(State::Disabled.into(), Colors::MUTED_INK),
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
      .set_part(Part::Mark, Self::mark(self.behavior().reads_checked()));

    self.0.over(defaults).build()
  }
}
