//! The one control builder, over a behavior.

use std::marker::PhantomData;

use skene_foundation::{
  Activate,
  BasicListboxBase,
  BasicRadioGroupBase,
  Behavior,
  Choice,
  Chosen,
  ControlBase,
  DecorationBase,
  HasBoxStyle,
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
/// use skene_terminal::{behavior::Activate, prelude::*};
///
/// let count = Signal::new(0);
/// let button = BasicButton::new(Activate::new())
///   .set_label("Add")
///   .handle_activate(move || count.update(|count| *count += 1));
/// ```
#[derive(Debug, View, Wraps)]
#[view(core = skene_foundation::ControlCore<Active>)]
#[must_use]
pub struct Control<B: Behavior>(skene_foundation::Control<Active, B>);

impl<B: Behavior + Default> Default for Control<B> {
  #[inline(always)]
  fn default() -> Self {
    Self::new(B::default())
  }
}

impl<B: Behavior> ControlBase<Active, B> for Control<B> {}

impl<B: Behavior> DecorationBase<Active> for Control<B> {}

impl<B: Behavior> MotionBase<Active> for Control<B> {}

impl<B: Behavior> Control<B> {
  crate::views::terminal_decoration!(box_style_mut);
}

/// A button: a control over [`Activate`].
pub type BasicButton = Control<Activate>;
/// A checkbox: a control over [`Toggle`].
pub type BasicCheckbox = Control<Toggle>;
/// A slider: a control over [`Slider`].
pub type BasicSlider = Control<Slider>;
/// A radio inside a group: a control over [`Radio`].
pub type BasicRadio = Control<Radio>;
/// A single-line text entry: a control over [`TextEntry`].
pub type BasicTextField = Control<TextEntry>;
/// A multi-line text entry: a control over [`TextEntry`].
pub type BasicTextEditor = Control<TextEntry>;
/// An option inside a listbox or a picker: a control over [`ListboxOption`].
pub type BasicListboxOption = Control<ListboxOption>;
/// A menu: a control over [`Menu`].
pub type BasicMenu = Control<Menu>;
/// An item inside a menu: a control over [`MenuOption`].
pub type BasicMenuItem = Control<MenuOption>;

/// A radio group: a control over a [`RadioGroup`] whose radios are tagged with values of
/// the app's own type `T`, one of them chosen.
///
/// Its radios are [`BasicRadio`]s at any depth inside it, one tab stop; the arrows move to
/// the next or the previous enabled radio, wrapping, and choose it; `Space` and a click
/// choose. Controlled: the chosen radio is the one tagged with what
/// [`derive_chosen`](BasicRadioGroupBase::derive_chosen) reads, and a choice only calls
/// [`handle_choose`](BasicRadioGroupBase::handle_choose) with the value asked for;
/// [`bind_chosen`](BasicRadioGroupBase::bind_chosen) wires both to one signal. A group that
/// starts with nothing chosen holds an `Option`, its radios tagged `Some(..)`. Every group
/// is one control whatever its `T`, which it holds erased.
///
/// ```
/// use skene_terminal::{behavior::Radio, prelude::*};
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// enum Size {
///   Small,
///   Medium,
/// }
///
/// let size = Signal::new(Size::Medium);
/// let group = BasicRadioGroup::new()
///   .set_label("Size")
///   .bind_chosen(size)
///   .set_child(Column::new().set_children((
///     BasicRadio::new(Radio::new(Size::Small)).set_label("Small"),
///     BasicRadio::new(Radio::new(Size::Medium)).set_label("Medium"),
///   )));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct BasicRadioGroup<T: Choice>(
  skene_foundation::Control<Active, RadioGroup>,
  PhantomData<fn(T) -> T>,
);

impl<T: Choice> Default for BasicRadioGroup<T> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<T: Choice> ControlBase<Active, RadioGroup> for BasicRadioGroup<T> {}

impl<T: Choice> DecorationBase<Active> for BasicRadioGroup<T> {}

impl<T: Choice> MotionBase<Active> for BasicRadioGroup<T> {}

impl<T: Choice> BasicRadioGroupBase<Active, T> for BasicRadioGroup<T> {}

impl<T: Choice> BasicRadioGroup<T> {
  /// A radio group with nothing chosen and no radios.
  #[inline(always)]
  pub fn new() -> Self {
    Self(
      skene_foundation::Control::new(RadioGroup::typed::<Option<T>>()),
      PhantomData,
    )
  }
}

impl<T: Choice> BasicRadioGroup<T> {
  crate::views::terminal_decoration!(box_style_mut);
}

/// A listbox: a control over a [`Listbox`] whose options are tagged with values of the
/// app's own type, what it has chosen a [`Chosen`] of them, `Option<T>` for one choice.
///
/// Its options are [`BasicListboxOption`]s at any depth inside it, one tab stop; the
/// vertical arrows, `Home`, `End` and typing move focus, and `Enter`, `Space` or a click
/// choose. Controlled: what is chosen is what
/// [`derive_chosen`](BasicListboxBase::derive_chosen) reads, and a choice only calls
/// [`handle_choose`](BasicListboxBase::handle_choose) with what it asks for;
/// [`bind_chosen`](BasicListboxBase::bind_chosen) wires both to one signal. Every listbox is
/// one control whatever its `C`, which it holds erased.
///
/// ```
/// use skene_terminal::{BasicListbox, behavior::ListboxOption, prelude::*};
///
/// let fruit = Signal::new(Some(2));
/// let listbox = BasicListbox::new()
///   .bind_chosen(fruit)
///   .set_child(Column::new().set_children((
///     BasicListboxOption::new(ListboxOption::new(1)).set_label("Apple"),
///     BasicListboxOption::new(ListboxOption::new(2)).set_label("Banana"),
///   )));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct BasicListbox<C: Chosen>(
  skene_foundation::Control<Active, Listbox>,
  PhantomData<fn(C) -> C>,
);

impl<C: Chosen> Default for BasicListbox<C> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<C: Chosen> ControlBase<Active, Listbox> for BasicListbox<C> {}

impl<C: Chosen> DecorationBase<Active> for BasicListbox<C> {}

impl<C: Chosen> MotionBase<Active> for BasicListbox<C> {}

impl<C: Chosen> BasicListboxBase<Active, C> for BasicListbox<C> {}

impl<C: Chosen> BasicListbox<C> {
  /// A listbox with nothing chosen and no options.
  #[inline(always)]
  pub fn new() -> Self {
    Self(
      skene_foundation::Control::new(Listbox::typed::<C>()),
      PhantomData,
    )
  }
}

impl<C: Chosen> BasicListbox<C> {
  crate::views::terminal_decoration!(box_style_mut);
}
