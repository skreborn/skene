//!
//! The Simple views on the `skene-gui` facade: every `Simple*` type of
//! this crate aliased to the facade's `ActivePlatform` and unit, with a [`prelude`]
//! an app imports beside the facade's own.

use skene_gui::ActivePlatform;

pub use crate::SimpleVariant;
/// The Simple palette in the gui's unit; see [`crate::SimplePalette`].
pub type SimplePalette = crate::SimplePalette<f32>;
/// One emphasis variant's colors; see [`crate::SimpleLook`].
pub type SimpleLook = crate::SimpleLook<f32>;
/// See [`crate::SimpleButton`].
pub type SimpleButton = crate::SimpleButton<ActivePlatform>;
/// See [`crate::SimpleCard`].
pub type SimpleCard = crate::SimpleCard<ActivePlatform>;
/// See [`crate::SimpleCheckbox`].
pub type SimpleCheckbox = crate::SimpleCheckbox<ActivePlatform>;
/// See [`crate::SimpleContextMenu`].
pub type SimpleContextMenu<C = skene_core::NoChild> = crate::SimpleContextMenu<ActivePlatform, C>;
/// See [`crate::SimplePicker`].
pub type SimplePicker = crate::SimplePicker<ActivePlatform>;
/// See [`crate::SimpleRadio`].
pub type SimpleRadio = crate::SimpleRadio<ActivePlatform>;
/// See [`crate::SimpleSlider`].
pub type SimpleSlider = crate::SimpleSlider<ActivePlatform>;
/// See [`crate::SimpleTextEditor`].
pub type SimpleTextEditor = crate::SimpleTextEditor<ActivePlatform>;
/// See [`crate::SimpleTextField`].
pub type SimpleTextField = crate::SimpleTextField<ActivePlatform>;
/// See [`crate::SimpleTooltip`].
pub type SimpleTooltip<C = skene_core::NoChild> = crate::SimpleTooltip<ActivePlatform, C>;
/// See [`crate::SimpleRadioGroup`].
pub type SimpleRadioGroup<C = skene_core::NoChild> = crate::SimpleRadioGroup<ActivePlatform, C>;

/// Everything above, for `use skene_simple::gui::prelude::*` beside the facade's own
/// prelude.
pub mod prelude {
  pub use super::*;
}
