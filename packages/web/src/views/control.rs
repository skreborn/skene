//! The one control builder, over a behavior.

use skene_foundation::{
  Activate,
  Behavior,
  ControlBase,
  DecorationBase,
  Listbox,
  ListboxOption,
  Menu,
  MenuOption,
  MotionBase,
  Radio,
  RadioGroup,
  Slider,
  TextEntry,
  Toggle,
  View,
  Wraps,
};

use crate::Active;

/// A control: focus, events, decoration, a label, parts and accessibility once, over
/// the [`Behavior`](skene_foundation::Behavior) that says what the interaction is.
///
/// The behavior runs first on every event it acts on and its verdict is the whole
/// verdict; every `handle_` here observes. The control is controlled: it reports, and
/// the signal its behavior is bound to is the truth. Its parts and its child resolve
/// the control-scoped conditions (`Checked`, `Disabled`, …) and their focus against it.
/// Its drawn body, which [`set_child`](Self::set_child) or
/// [`set_child_builder`](Self::set_child_builder) supplies, is optional: a control
/// with a label or parts alone builds without one. Its parts, its body and its label
/// sit in one row half an em apart, centered on the cross axis, the label where the
/// behavior places it ([`Behavior::label_placement`](skene_foundation::Behavior::label_placement)):
/// leading or trailing in the row, or beside the box, before it among its siblings.
///
/// ```
/// use skene_web::{Activate, prelude::*};
///
/// let count = Signal::new(0);
/// let button = BasicButton::new(Activate::new())
///   .set_label("Add")
///   .handle_activate(move || count.update(|count| *count += 1));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Control<B: Behavior<Active>>(skene_foundation::Control<Active, B>);

impl<B: Behavior<Active> + Default> Default for Control<B> {
  #[inline(always)]
  fn default() -> Self {
    Self::new(B::default())
  }
}

impl<B: Behavior<Active>> ControlBase<Active, B> for Control<B> {}

impl<B: Behavior<Active>> DecorationBase<Active> for Control<B> {}

impl<B: Behavior<Active>> MotionBase<Active> for Control<B> {}

/// A button: a control over [`Activate`].
pub type BasicButton = Control<Activate>;
/// A checkbox: a control over [`Toggle`].
pub type BasicCheckbox = Control<Toggle>;
/// A slider: a control over [`Slider`].
pub type BasicSlider = Control<Slider>;
/// A radio group: a control over [`RadioGroup`].
pub type BasicRadioGroup = Control<RadioGroup>;
/// A radio inside a group: a control over [`Radio`].
pub type BasicRadio = Control<Radio>;
/// A single-line text entry: a control over [`TextEntry`].
pub type BasicTextField = Control<TextEntry>;
/// A multi-line text entry: a control over [`TextEntry`].
pub type BasicTextEditor = Control<TextEntry>;
/// A listbox: a control over [`Listbox`].
pub type BasicListbox = Control<Listbox>;
/// An option inside a listbox or a picker: a control over [`ListboxOption`].
pub type BasicListboxOption = Control<ListboxOption>;
/// A menu: a control over [`Menu`].
pub type BasicMenu = Control<Menu>;
/// An item inside a menu: a control over [`MenuOption`].
pub type BasicMenuItem = Control<MenuOption>;
