//! The Simple component library's facade-side machinery: the shape vocabulary in
//! cells and the shared builder surface. The palette and the emphasis variants come
//! from `skene-simple`, shared with every other facade.
//!
//! Terminal interaction feedback rides the border, never the backdrop: hovering
//! brightens a Simple control's border to the ring, and focusing it inks the border
//! in the control's own ink.
//!
//! Every `Simple*` view composes the raw controls through their public surface alone,
//! so a third-party component library sits on exactly the same footing.

mod button;
mod card;
mod checkbox;
mod picker;
mod radio;
mod slider;
mod text_field;

pub use skene_simple::{SimplePalette, SimpleVariant};

pub use self::{
  button::SimpleButton,
  card::SimpleCard,
  checkbox::SimpleCheckbox,
  picker::SimplePicker,
  radio::SimpleRadio,
  slider::SimpleSlider,
  text_field::SimpleTextField,
};
use crate::props::{BorderGlyphs, Condition};

/// The border every Simple surface draws with—arc corners, the terminal's radius.
pub(crate) const GLYPHS: BorderGlyphs = BorderGlyphs::ROUNDED;

/// The shared field chrome, applied per property: padded, bordered in the scheme's
/// hairline, brightening on hover and again on focus.
pub(crate) fn field_chrome<T: crate::HasDecoration>(control: T) -> T {
  control
    .set_padding(crate::Inset::horizontal(1))
    .set_border(SimplePalette::border())
    .set_border_glyphs(GLYPHS)
    .set_border_when(Condition::Hovered, SimplePalette::ring())
    .set_border_when(Condition::Focused, SimplePalette::foreground())
}
