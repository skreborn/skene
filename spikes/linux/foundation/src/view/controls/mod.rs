//! The built-in controls: each a [`crate::view::Control`] over a shipped behavior, composed of
//! Skene's views and styled by ordinary values, so every default is a value its user
//! overrides and every part a view its user replaces (`set_<part>`), starting from the
//! default the control hands back (`default_<part>()`).
//!
//! Colors are [`crate::SystemColors`] tokens, which the platform defines at the root and any
//! `Theme` beneath redefines; lengths are the literals each default's documentation states. No
//! default changes a layout property with a state or a value: a press, a check, a flip or a
//! drag changes paint alone (a switch's thumb and a slider's fill move by `offset`), so a
//! change costs no layout.

mod button;
mod checkbox;
mod context_menu;
pub mod marks;
mod picker;
mod radio;
mod slider;
mod text_field;
mod toggle;

pub use self::{
  button::{Button, ButtonBase},
  checkbox::{Checkbox, CheckboxBase},
  context_menu::{ContextMenu, ContextMenuBase, MenuEntry, MenuItem, MenuItemBase},
  picker::{BasicPicker, Picker, PickerBase, PickerItem, PickerItemBase},
  radio::{Radio, RadioBase, RadioGroup, RadioGroupBase},
  slider::{Slider, SliderBase},
  text_field::{
    EditingBase, HasEntry, Keyboard, TextEditor, TextEditorBase, TextField, TextFieldBase,
  },
  toggle::{Toggle, ToggleBase},
};
use crate::{Ring, State, Value, When as _};

/// The ring a built-in shows while focus shows on it: two pixels wide, at its edge.
fn focus_ring() -> Value<Ring> {
  Ring::NONE.when(State::FocusVisible, Ring::new(2))
}

/// Implements the surfaces every control shares for a styled control whose root is the
/// [`crate::view::Control`] at `$path`: its record and members, and [`ControlMembersBase`],
/// [`DecorationBase`], [`RunBase`], [`RunWithEntries`] and [`MotionBase`].
///
/// [`ControlMembersBase`]: crate::view::ControlMembersBase
/// [`DecorationBase`]: crate::view::DecorationBase
/// [`RunBase`]: crate::view::RunBase
/// [`RunWithEntries`]: crate::view::RunWithEntries
/// [`MotionBase`]: crate::view::MotionBase
macro_rules! control_surfaces {
  ([$($generics:tt)*] $name:ty, $realization:ident => $($path:ident).+) => {
    impl<$($generics)*> $crate::view::HasRecord for $name {
      #[inline(always)]
      fn record_mut(&mut self) -> &mut $crate::seam::record::Record {
        $crate::view::HasRecord::record_mut(&mut self.$($path).+)
      }
    }

    impl<$($generics)*> $crate::view::HasMembers<$realization> for $name {
      #[inline(always)]
      fn members_mut(&mut self) -> &mut $crate::view::Members<$realization> {
        $crate::view::HasMembers::members_mut(&mut self.$($path).+)
      }
    }

    impl<$($generics)*> $crate::view::ControlMembersBase<$realization> for $name {}

    impl<$($generics)*> $crate::view::DecorationBase for $name {}

    impl<$($generics)*> $crate::view::RunBase for $name {}

    impl<$($generics)*> $crate::view::RunWithEntries for $name {}

    impl<$($generics)*> $crate::view::MotionBase for $name {}
  };
}

use control_surfaces;
