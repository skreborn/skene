//! # skene-simple-tui
//!
//! The Simple views on the `skene-tui` facade: every `Simple*` type of
//! [`skene_simple`] aliased to the facade's `ActiveDriver` and unit, with a [`prelude`]
//! an app imports beside the facade's own.

#![cfg(not(target_arch = "wasm32"))]

pub use skene_simple::SimpleVariant;
use skene_tui::ActiveDriver;
/// The Simple palette in the tui's unit; see [`skene_simple::SimplePalette`].
pub type SimplePalette = skene_simple::SimplePalette<u32>;
/// One emphasis variant's colors; see [`skene_simple::SimpleLook`].
pub type SimpleLook = skene_simple::SimpleLook<u32>;
/// See [`skene_simple::SimpleButton`].
pub type SimpleButton = skene_simple::SimpleButton<ActiveDriver>;
/// See [`skene_simple::SimpleCard`].
pub type SimpleCard = skene_simple::SimpleCard<ActiveDriver>;
/// See [`skene_simple::SimpleCheckbox`].
pub type SimpleCheckbox = skene_simple::SimpleCheckbox<ActiveDriver>;
/// See [`skene_simple::SimpleContextMenu`].
pub type SimpleContextMenu<C = skene_core::NoChild> =
  skene_simple::SimpleContextMenu<ActiveDriver, C>;
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
/// See [`skene_simple::SimpleTooltip`].
pub type SimpleTooltip<C = skene_core::NoChild> = skene_simple::SimpleTooltip<ActiveDriver, C>;
/// See [`skene_simple::SimpleRadioGroup`].
pub type SimpleRadioGroup<C = skene_core::NoChild> =
  skene_simple::SimpleRadioGroup<ActiveDriver, C>;
pub use skene_simple::{SimpleGlyphs, glyphs as glyph};

/// Everything above, for `use skene_simple_tui::prelude::*` beside the facade's own
/// prelude.
pub mod prelude {
  pub use super::*;
}
