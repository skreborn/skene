//! The Simple views on the gui facade: every `Simple*` type aliased to its
//! `ActiveDriver` and unit, so an app imports this module beside the facade's prelude.

use skene_gui::ActiveDriver;

pub use crate::SimpleVariant;
/// The Simple palette in the gui's unit; see [`crate::SimplePalette`].
pub type SimplePalette = crate::SimplePalette<f32>;
/// One emphasis variant's colors; see [`crate::SimpleLook`].
pub type SimpleLook = crate::SimpleLook<f32>;
/// See [`crate::SimpleButton`].
pub type SimpleButton = crate::SimpleButton<ActiveDriver>;
/// See [`crate::SimpleCard`].
pub type SimpleCard = crate::SimpleCard<ActiveDriver>;
/// See [`crate::SimpleCheckbox`].
pub type SimpleCheckbox = crate::SimpleCheckbox<ActiveDriver>;
/// See [`crate::SimplePicker`].
pub type SimplePicker = crate::SimplePicker<ActiveDriver>;
/// See [`crate::SimpleRadio`].
pub type SimpleRadio = crate::SimpleRadio<ActiveDriver>;
/// See [`crate::SimpleSlider`].
pub type SimpleSlider = crate::SimpleSlider<ActiveDriver>;
/// See [`crate::SimpleTextEditor`].
pub type SimpleTextEditor = crate::SimpleTextEditor<ActiveDriver>;
/// See [`crate::SimpleTextField`].
pub type SimpleTextField = crate::SimpleTextField<ActiveDriver>;
/// See [`crate::SimpleRadioGroup`].
pub type SimpleRadioGroup<C = skene_core::NoChild> = crate::SimpleRadioGroup<ActiveDriver, C>;

/// Everything above, for `use skene_simple::gui::prelude::*` beside the facade's own
/// prelude.
pub mod prelude {
  pub use super::*;
}
