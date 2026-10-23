//! A picker: a popup face, an overlay and a listbox.

use skene_foundation::{BasicPickerBase, DecorationBase, View, Wraps};

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
/// Controlled: the chosen value is what [`derive_chosen`](Self::derive_chosen) reads, and
/// a choice only calls [`handle_choose`](Self::handle_choose) with the value asked for;
/// [`bind_chosen`](Self::bind_chosen) wires both to one signal. The face comes from
/// [`set_face`](Self::set_face), and a picker without one renders nothing; the options
/// come from [`set_options`](Self::set_options) and
/// [`derive_options`](Self::derive_options), each a
/// [`BasicListboxOption`](crate::BasicListboxOption) at any depth.
///
/// ```
/// use skene_web::{ListboxOption, prelude::*};
///
/// let size = Signal::new(String::from("m"));
/// let picker = BasicPicker::new()
///   .set_label("Size")
///   .bind_chosen(size)
///   .set_options((
///     BasicListboxOption::new(ListboxOption::new("s")).set_label("Small"),
///     BasicListboxOption::new(ListboxOption::new("m")).set_label("Medium"),
///   ));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct BasicPicker(skene_foundation::BasicPicker<Active>);

impl Default for BasicPicker {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl BasicPickerBase<Active> for BasicPicker {}

impl DecorationBase<Active> for BasicPicker {}

impl BasicPicker {
  /// A picker with no options and nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::BasicPicker::new())
  }
}
