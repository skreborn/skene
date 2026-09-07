//! # skene-simple-gui
//!
//! The Simple views on the `skene-gui` facade: every `Simple*` type of
//! [`skene_simple`] aliased to the facade's `ActiveDriver` and unit, with a [`prelude`]
//! an app imports beside the facade's own.

use skene_gui::ActiveDriver;
pub use skene_simple::SimpleVariant;
/// The Simple palette in the gui's unit; see [`skene_simple::SimplePalette`].
pub type SimplePalette = skene_simple::SimplePalette<f32>;
/// One emphasis variant's colors; see [`skene_simple::SimpleLook`].
pub type SimpleLook = skene_simple::SimpleLook<f32>;
/// See [`skene_simple::SimpleButton`].
pub type SimpleButton = skene_simple::SimpleButton<ActiveDriver>;
/// See [`skene_simple::SimpleCard`].
pub type SimpleCard = skene_simple::SimpleCard<ActiveDriver>;
/// See [`skene_simple::SimpleCheckbox`].
pub type SimpleCheckbox = skene_simple::SimpleCheckbox<ActiveDriver>;
/// See [`skene_simple::SimplePicker`].
pub type SimplePicker = skene_simple::SimplePicker<ActiveDriver>;
/// See [`skene_simple::SimpleRadio`].
pub type SimpleRadio = skene_simple::SimpleRadio<ActiveDriver>;
/// See [`skene_simple::SimpleSlider`].
pub type SimpleSlider = skene_simple::SimpleSlider<ActiveDriver>;
/// See [`skene_simple::SimpleTextEditor`].
pub type SimpleTextEditor = skene_simple::SimpleTextEditor<ActiveDriver>;
/// See [`skene_simple::SimpleTextField`].
pub type SimpleTextField = skene_simple::SimpleTextField<ActiveDriver>;
/// See [`skene_simple::SimpleRadioGroup`].
pub type SimpleRadioGroup<C = skene_core::NoChild> =
  skene_simple::SimpleRadioGroup<ActiveDriver, C>;

/// Everything above, for `use skene_simple_gui::prelude::*` beside the facade's own
/// prelude.
pub mod prelude {
  pub use super::*;
}
