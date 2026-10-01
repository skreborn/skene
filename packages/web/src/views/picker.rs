//! A picker: a popup face, an overlay and a listbox.

use skene_foundation::NoChild;

use crate::ActivePlatform;

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
/// Controlled: the chosen value is what [`derive_value`](Self::derive_value) reads, and
/// a choice only calls [`handle_change`](Self::handle_change) with the value asked for;
/// [`bind_value`](Self::bind_value) wires both to one signal. Holds [`NoChild`] until
/// [`set_face`](Self::set_face) supplies the face; the options come from
/// [`set_options`](Self::set_options) and [`derive_options`](Self::derive_options),
/// each a [`BasicListboxOption`](crate::BasicListboxOption) at any depth.
#[must_use]
pub struct BasicPicker<F = NoChild>(skene_foundation::BasicPicker<ActivePlatform, F>);

skene_foundation::impl_basic_picker!(BasicPicker, ActivePlatform);
