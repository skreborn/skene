//! What a control's interaction is: the shipped behaviors, the trait a library's own
//! implements, the handle a behavior acts through and the contexts a collection's members
//! join.
//!
//! A [`Control`](crate::Control) runs over one [`Behavior`], and each `Basic*` view is
//! a control over one of these (`BasicButton` over [`Activate`], `BasicCheckbox` over
//! [`Toggle`]). The behaviors carry the keyboard tables, so a third party's control gets
//! the same keys as the shipped ones; a behavior names no realization and reaches the
//! build's one through its [`ControlHandle`]'s [`ControlOps`].
//!
//! ```
//! use skene_web::{behavior::Toggle, prelude::*};
//!
//! let checked = Signal::new(false);
//! let _ = BasicCheckbox::new(Toggle::new().bind_checked(checked)).set_label("Subscribe");
//! ```

pub use skene_foundation::{
  Activate,
  Behavior,
  ControlHandle,
  ControlOps,
  Listbox,
  ListboxContext,
  ListboxOption,
  Menu,
  MenuContext,
  MenuOption,
  NativePicker,
  Popup,
  Radio,
  RadioGroup,
  RadioGroupContext,
  Roster,
  Slider,
  SliderRange,
  TextEntry,
  Toggle,
};
