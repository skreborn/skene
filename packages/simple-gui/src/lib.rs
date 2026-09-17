//! # skene-simple-gui
//!
//! The Simple views on the `skene-gui` facade: every `Simple*` type of
//! [`skene_simple`] aliased to the facade's `ActivePlatform` and unit, with a [`prelude`]
//! an app imports beside the facade's own.

use skene_gui::ActivePlatform;
pub use skene_simple::SimpleVariant;
/// The Simple palette in the gui's unit; see [`skene_simple::SimplePalette`].
pub type SimplePalette = skene_simple::SimplePalette<f32>;
/// One emphasis variant's colors; see [`skene_simple::SimpleLook`].
pub type SimpleLook = skene_simple::SimpleLook<f32>;
/// See [`skene_simple::SimpleButton`].
pub type SimpleButton = skene_simple::SimpleButton<ActivePlatform>;
/// See [`skene_simple::SimpleCard`].
pub type SimpleCard = skene_simple::SimpleCard<ActivePlatform>;
/// See [`skene_simple::SimpleCheckbox`].
pub type SimpleCheckbox = skene_simple::SimpleCheckbox<ActivePlatform>;
/// See [`skene_simple::SimpleContextMenu`].
pub type SimpleContextMenu<C = skene_core::NoChild> =
  skene_simple::SimpleContextMenu<ActivePlatform, C>;
/// See [`skene_simple::SimplePicker`].
pub type SimplePicker = skene_simple::SimplePicker<ActivePlatform>;
/// See [`skene_simple::SimpleRadio`].
pub type SimpleRadio = skene_simple::SimpleRadio<ActivePlatform>;
/// See [`skene_simple::SimpleSlider`].
pub type SimpleSlider = skene_simple::SimpleSlider<ActivePlatform>;
/// See [`skene_simple::SimpleTextEditor`].
pub type SimpleTextEditor = skene_simple::SimpleTextEditor<ActivePlatform>;
/// See [`skene_simple::SimpleTextField`].
pub type SimpleTextField = skene_simple::SimpleTextField<ActivePlatform>;
/// See [`skene_simple::SimpleTooltip`].
pub type SimpleTooltip<C = skene_core::NoChild> = skene_simple::SimpleTooltip<ActivePlatform, C>;
/// See [`skene_simple::SimpleRadioGroup`].
pub type SimpleRadioGroup<C = skene_core::NoChild> =
  skene_simple::SimpleRadioGroup<ActivePlatform, C>;

/// Everything above, for `use skene_simple_gui::prelude::*` beside the facade's own
/// prelude.
pub mod prelude {
  pub use super::*;
}
