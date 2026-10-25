//! A picker: a popup face, an overlay and a listbox.

use skene_foundation::{
  AnyChosen,
  AnyKind,
  Arrangement,
  BasicPickerBase,
  BasicPickerCore,
  Choice,
  Color,
  Condition,
  DecorationBase,
  DecorationWithCornerRadius,
  Dimension,
  FlexBase,
  HasChildren,
  Inset,
  ItemAlignment,
  ListboxContext,
  MouseCursor,
  NodeId,
  RunBase,
  Spacing,
  State,
  SystemColors,
  Text,
  Thin,
  Value,
  View,
  When,
  Wraps,
};
use skene_reactive::{Get, Memo, use_context};

use crate::Active;

/// The platform's colors in logical pixels.
type Colors = SystemColors<f32>;

/// A choice among fixed options, drawn by the views it holds: the face draws the
/// control closed, the options draw the list it opens.
///
/// The face is a [`Popup`](crate::behavior::Popup) control decorated here, per property: a click,
/// `Enter` or `Space` toggles the list, `ArrowDown` and `ArrowUp` open it, and `Escape` closes it.
/// The list is a [`Listbox`](crate::behavior::Listbox) in a popup [`Overlay`](crate::Overlay)
/// anchored to the face: opening focuses the chosen option, or the first; the arrows move focus
/// without wrapping; `Enter`, `Space` or a click chooses, closes and returns focus to the face; a
/// dismiss closes and leaves the value alone. Typing over the closed face chooses the option
/// whose name the characters begin, a character repeated cycling through the options
/// starting with it; typing over the open list moves focus the same way.
///
/// Controlled: the chosen value, a value of the app's own type `T`, is what
/// [`derive_chosen`](BasicPickerBase::derive_chosen) reads, and a choice only calls
/// [`handle_choose`](BasicPickerBase::handle_choose) with the value asked for;
/// [`bind_chosen`](BasicPickerBase::bind_chosen) wires both to one signal. The face comes
/// from [`set_face`](BasicPickerBase::set_face), and a picker without one renders nothing;
/// the options come from [`set_options`](BasicPickerBase::set_options) and
/// [`derive_options`](BasicPickerBase::derive_options), each a
/// [`BasicListboxOption`](crate::BasicListboxOption) at any depth tagged with a `T`. Every
/// picker builds one core whatever its `T`, which it holds erased.
///
/// ```
/// use skene_web::{behavior::ListboxOption, prelude::*};
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// enum Size {
///   Small,
///   Medium,
/// }
///
/// let size = Signal::new(Size::Medium);
/// let picker = BasicPicker::new()
///   .set_label("Size")
///   .bind_chosen(size)
///   .set_options((
///     BasicListboxOption::new(ListboxOption::new(Size::Small)).set_label("Small"),
///     BasicListboxOption::new(ListboxOption::new(Size::Medium)).set_label("Medium"),
///   ));
/// ```
#[derive(Debug, View, Wraps)]
#[view(core = skene_foundation::BasicPickerCore<Active>)]
#[must_use]
pub struct BasicPicker<T: Choice>(skene_foundation::BasicPicker<Active, T>);

impl<T: Choice> Default for BasicPicker<T> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<T: Choice> BasicPickerBase<Active, T> for BasicPicker<T> {}

impl<T: Choice> DecorationBase<Active> for BasicPicker<T> {}

impl<T: Choice> BasicPicker<T> {
  /// A picker with no options and nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::BasicPicker::new())
  }
}

/// A choice among fixed options: Skene's picker, a [`BasicPicker`] styled here.
///
/// It is drawn once the page is live, and before that it is the page's own `<select>`,
/// which the server renders and whose choice carries across the swap. It behaves as a
/// [`BasicPicker`] does. Its options are [`ListboxOption`](crate::ListboxOption)s, or any
/// [`BasicListboxOption`](crate::BasicListboxOption), from
/// [`set_options`](BasicPickerBase::set_options); a caption
/// ([`set_label`](BasicPickerBase::set_label)) sits before it among its siblings.
///
/// Its defaults, each a value its setters replace: a face at least 32 pixels tall, padded
/// 4 above and below and 8 at the sides inside a border of 1 in
/// [`SystemColors::BORDER`], the accent while focus shows or the list is open, its
/// corners rounded by 6, filled with [`SystemColors::CONTROL`] and its hover and pressed
/// variants; the click cursor, the default one while disabled. The face
/// ([`set_face`](BasicPickerBase::set_face)) shows the chosen option's name in
/// [`SystemColors::INK`] ([`SystemColors::MUTED_INK`] while disabled), then the indicator
/// `▾` in [`SystemColors::MUTED_INK`]. The list ([`set_list`](BasicPickerBase::set_list))
/// is padded 4 inside a border of 1 in [`SystemColors::BORDER`], its corners rounded by
/// 6, filled with [`SystemColors::FIELD`]. The `<select>` takes the face's decoration.
///
/// ```
/// use skene_web::prelude::*;
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// enum Size {
///   Small,
///   Medium,
/// }
///
/// let size = Signal::new(Size::Medium);
/// let picker = Picker::new()
///   .set_label("Size")
///   .bind_chosen(size)
///   .set_options((
///     ListboxOption::new(Size::Small).set_label("Small"),
///     ListboxOption::new(Size::Medium).set_label("Medium"),
///   ));
/// ```
#[derive(Debug, Wraps)]
#[must_use]
pub struct Picker<T: Choice>(skene_foundation::BasicPicker<Active, T>);

impl<T: Choice> Default for Picker<T> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<T: Choice> Picker<T> {
  /// A picker with no options and nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::BasicPicker::new())
  }
}

impl<T: Choice> BasicPickerBase<Active, T> for Picker<T> {}

impl<T: Choice> DecorationBase<Active> for Picker<T> {}

impl<T: Choice> AnyKind for Picker<T> {}

impl<T: Choice> View<Active> for Picker<T> {
  #[inline(always)]
  fn build(self) -> NodeId {
    styled(self.0.into_core())
  }
}

/// Builds `picker` over the styled picker's defaults.
fn styled(picker: BasicPickerCore<Active>) -> NodeId {
  let ring = Colors::BORDER
    .when(Condition::from(State::FocusVisible), Colors::ACCENT)
    .when(State::Open.into(), Colors::ACCENT);
  let list = skene_foundation::Container::<Active>::new()
    .set_padding(Inset::all(4))
    .set_border_width(Inset::all(1))
    .set_border_color(Colors::BORDER)
    .set_corner_radius(6)
    .set_background(Colors::FIELD);
  let defaults = BasicPickerCore::<Active>::new()
    .set_min_height(Dimension::Length(32.))
    .set_padding(Inset::symmetric(8, 4))
    .set_border_width(Inset::all(1))
    .set_border_color(ring)
    .set_corner_radius(6)
    .set_background(
      Value::<Color, Condition<f32>>::from(Colors::CONTROL)
        .when(State::Hovered.into(), Colors::CONTROL_HOVER)
        .when(State::Pressed.into(), Colors::CONTROL_PRESSED)
        .when(State::Disabled.into(), Colors::CONTROL),
    )
    .set_cursor(MouseCursor::Click.when(State::Disabled, MouseCursor::Default))
    .set_list(list)
    .set_face(face);

  picker.over(defaults).build()
}

/// The default face: the name of the option `chosen` holds, then the indicator.
fn face(chosen: Memo<AnyChosen>) -> skene_foundation::Row<Active> {
  let options = use_context::<ListboxContext>();
  let name = move || {
    options
      .as_ref()
      .and_then(|options| options.roster().name_of(&chosen.get()))
      .unwrap_or_default()
  };

  skene_foundation::Row::<Active>::new()
    .set_spacing(Spacing::all(8))
    .set_arrangement(Arrangement::SpaceBetween)
    .set_item_alignment(ItemAlignment::Center)
    .set_children((
      Text::<Active>::derive(name).set_color(Colors::INK.when(State::Disabled, Colors::MUTED_INK)),
      Text::<Active>::new("\u{25be}").set_color(Colors::MUTED_INK),
    ))
}
