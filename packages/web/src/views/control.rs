//! The one control builder, over a behavior.

use skene_foundation::{
  Activate,
  AnyView,
  Behavior,
  ControlBase,
  DecorationBase,
  Listbox,
  ListboxOption,
  Menu,
  MenuOption,
  MotionBase,
  NoChild,
  Radio,
  RadioGroup,
  Slider,
  TextEntry,
  Toggle,
  View,
  Wraps,
};

use crate::ActivePlatform;

/// A control: focus, events, decoration, a label, parts and accessibility once, over
/// the [`Behavior`](skene_foundation::Behavior) that says what the interaction is.
///
/// The behavior runs first on every event it acts on and its verdict is the whole
/// verdict; every `handle_` here observes. The control is controlled: it reports, and
/// the signal its behavior is bound to is the truth. Its parts and its child resolve
/// the control-scoped conditions (`Checked`, `Disabled`, …) and their focus against it.
/// Holds [`NoChild`] until [`set_child`](Self::set_child) or
/// [`set_child_builder`](Self::set_child_builder) supplies the drawn body; a control
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
pub struct Control<B: Behavior<ActivePlatform>, C = NoChild>(
  skene_foundation::Control<ActivePlatform, B, C>,
);

impl<B: Behavior<ActivePlatform> + Default> Default for Control<B> {
  #[inline(always)]
  fn default() -> Self {
    Self::new(B::default())
  }
}

impl<B: Behavior<ActivePlatform>> ControlBase<ActivePlatform, B> for Control<B> {}

impl<B: Behavior<ActivePlatform>> DecorationBase<ActivePlatform> for Control<B> {}

impl<B: Behavior<ActivePlatform>> MotionBase<ActivePlatform> for Control<B> {}

impl<B: Behavior<ActivePlatform>> Control<B> {
  /// Sets the drawn body.
  #[inline(always)]
  pub fn set_child(self, child: impl View<ActivePlatform>) -> Control<B, AnyView<ActivePlatform>> {
    Control(self.0.set_child(child))
  }

  /// Sets the drawn body, built by `child` under the control.
  #[inline(always)]
  pub fn set_child_builder<V: View<ActivePlatform>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Control<B, AnyView<ActivePlatform>> {
    Control(self.0.set_child_builder(child))
  }
}

/// A button: a control over [`Activate`].
pub type BasicButton<C = NoChild> = Control<Activate, C>;
/// A checkbox: a control over [`Toggle`].
pub type BasicCheckbox<C = NoChild> = Control<Toggle, C>;
/// A slider: a control over [`Slider`].
pub type BasicSlider<C = NoChild> = Control<Slider, C>;
/// A radio group: a control over [`RadioGroup`].
pub type BasicRadioGroup<C = NoChild> = Control<RadioGroup, C>;
/// A radio inside a group: a control over [`Radio`].
pub type BasicRadio<C = NoChild> = Control<Radio, C>;
/// A single-line text entry: a control over [`TextEntry`].
pub type BasicTextField<C = NoChild> = Control<TextEntry, C>;
/// A multi-line text entry: a control over [`TextEntry`].
pub type BasicTextEditor<C = NoChild> = Control<TextEntry, C>;
/// A listbox: a control over [`Listbox`].
pub type BasicListbox<C = NoChild> = Control<Listbox, C>;
/// An option inside a listbox or a picker: a control over [`ListboxOption`].
pub type BasicListboxOption<C = NoChild> = Control<ListboxOption, C>;
/// A menu: a control over [`Menu`].
pub type BasicMenu<C = NoChild> = Control<Menu, C>;
/// An item inside a menu: a control over [`MenuOption`].
pub type BasicMenuItem<C = NoChild> = Control<MenuOption, C>;
