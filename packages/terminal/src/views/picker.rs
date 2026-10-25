//! A picker: a popup face, an overlay and a listbox.

use skene_foundation::{BasicPickerBase, Choice, DecorationBase, View, Wraps};

use crate::Active;

/// A choice among fixed options, drawn by the views it holds: the face draws the
/// control closed, the options draw the list it opens.
///
/// The face is a [`Popup`](crate::Popup) control decorated here, per property: a click, `Enter` or
/// `Space` toggles the list, `ArrowDown` and `ArrowUp` open it, and `Escape` closes it.
/// The list is a [`Listbox`](crate::Listbox) in a popup [`Overlay`](crate::Overlay) anchored to the
/// face: opening focuses the chosen option, or the first; the arrows move focus without wrapping;
/// `Enter`, `Space` or a click chooses, closes and returns focus to the face; a dismiss
/// closes and leaves the value alone. Typing over the closed face chooses the option
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
/// use skene_terminal::{ListboxOption, prelude::*};
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
