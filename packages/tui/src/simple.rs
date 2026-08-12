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
use crate::props::{BorderGlyphs, BoxStyle, Condition};

/// The border every Simple surface draws with—arc corners, the terminal's radius.
pub(crate) const GLYPHS: BorderGlyphs = BorderGlyphs::ROUNDED;

/// The shared field chrome: padded, hairline-bordered, rounded.
pub(crate) fn field_style() -> BoxStyle {
  BoxStyle::new()
    .set_padding(crate::Inset::horizontal(1))
    .set_border(SimplePalette::border())
    .set_border_glyphs(GLYPHS)
}

/// The shared field hover variant: the border brightens to the ring.
pub(crate) fn field_hover() -> (Condition, BoxStyle) {
  (
    Condition::Hovered,
    BoxStyle::new().set_border(SimplePalette::ring()),
  )
}

/// The shared field focus variant: the border inks fully.
pub(crate) fn field_focus() -> (Condition, BoxStyle) {
  (
    Condition::Focused,
    BoxStyle::new().set_border(SimplePalette::foreground()),
  )
}
