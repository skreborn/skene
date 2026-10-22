//! A picker: a popup face, an overlay and a listbox.

use skene_foundation::{
  BasicPickerBase,
  DecorationBase,
  Face,
  NoChild,
  View,
  Wraps,
  reactive::Memo,
};

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
/// [`bind_chosen`](Self::bind_chosen) wires both to one signal. Holds [`NoChild`] until
/// [`set_face`](Self::set_face) supplies the face; the options come from
/// [`set_options`](Self::set_options) and [`derive_options`](Self::derive_options),
/// each a [`BasicListboxOption`](crate::BasicListboxOption) at any depth.
///
/// ```
/// use skene_terminal::{ListboxOption, prelude::*};
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
pub struct BasicPicker<F = NoChild>(skene_foundation::BasicPicker<Active, F>);

impl Default for BasicPicker {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<F> BasicPickerBase<Active, F> for BasicPicker<F> {}

impl<F> DecorationBase<Active> for BasicPicker<F> {}

impl BasicPicker {
  /// A picker with no options and nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self(skene_foundation::BasicPicker::new())
  }

  /// Sets the view drawing the control closed, built from the chosen value.
  #[inline(always)]
  pub fn set_face<V: View<Active>>(
    self,
    face: impl FnOnce(Memo<String>) -> V + 'static,
  ) -> BasicPicker<Face<Active>> {
    BasicPicker(self.0.set_face(face))
  }
}
