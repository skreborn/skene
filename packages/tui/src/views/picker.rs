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
  FlexBase,
  HasBoxStyle,
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

/// The platform's colors in cells.
type Colors = SystemColors<i32>;

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
/// use skene_tui::{behavior::ListboxOption, prelude::*};
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
/// It is drawn in cells. It behaves as a [`BasicPicker`] does. Its options are
/// [`ListboxOption`](crate::ListboxOption)s, or any
/// [`BasicListboxOption`](crate::BasicListboxOption), from
/// [`set_options`](BasicPickerBase::set_options); a caption
/// ([`set_label`](BasicPickerBase::set_label)) sits before it among its siblings.
///
/// Its defaults, each a value its setters replace: a face with a cell of padding at
/// either side, filled with [`SystemColors::CONTROL`] and its hover and pressed variants,
/// the accent while focus shows, the pressed variant while the list is open; the click
/// cursor, the default one while disabled. The face
/// ([`set_face`](BasicPickerBase::set_face)) shows the chosen option's name, then a cell
/// apart the indicator `▾`, in [`SystemColors::INK`] ([`SystemColors::ON_ACCENT`] while
/// focus shows, [`SystemColors::MUTED_INK`] while disabled). The list
/// ([`set_list`](BasicPickerBase::set_list)) sits inside a border of one cell in
/// [`SystemColors::BORDER`], filled with [`SystemColors::FIELD`].
///
/// ```
/// use skene_tui::prelude::*;
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

impl<T: Choice> Picker<T> {
  crate::views::terminal_decoration!(box_style_mut);
}

impl<T: Choice> View<Active> for Picker<T> {
  #[inline(always)]
  fn build(self) -> NodeId {
    styled(self.0.into_core())
  }
}

/// Builds `picker` over the styled picker's defaults.
fn styled(picker: BasicPickerCore<Active>) -> NodeId {
  let list = skene_foundation::Container::<Active>::new()
    .set_border_width(Inset::all(1))
    .set_border_color(Colors::BORDER)
    .set_background(Colors::FIELD);
  let defaults = BasicPickerCore::<Active>::new()
    .set_padding(Inset::symmetric(1, 0))
    .set_background(
      Value::<Color, Condition<i32>>::from(Colors::CONTROL)
        .when(State::Hovered.into(), Colors::CONTROL_HOVER)
        .when(State::Pressed.into(), Colors::CONTROL_PRESSED)
        .when(State::Open.into(), Colors::CONTROL_PRESSED)
        .when(Condition::from(State::FocusVisible), Colors::ACCENT)
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

  let ink = || {
    Value::<Color, Condition<i32>>::from(Colors::INK)
      .when(Condition::from(State::FocusVisible), Colors::ON_ACCENT)
      .when(State::Disabled.into(), Colors::MUTED_INK)
  };

  skene_foundation::Row::<Active>::new()
    .set_spacing(Spacing::all(1))
    .set_arrangement(Arrangement::SpaceBetween)
    .set_item_alignment(ItemAlignment::Center)
    .set_children((
      Text::<Active>::derive(name).set_color(ink()),
      Text::<Active>::new("\u{25be}").set_color(ink()),
    ))
}
