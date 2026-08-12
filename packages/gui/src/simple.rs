//! The Simple component library's facade-side machinery: the shape vocabulary in
//! logical pixels and the shared builder surface. The palette and the emphasis
//! variants come from `skene-simple`, shared with every other facade.
//!
//! Every `Simple*` view composes the raw controls through their public surface alone,
//! so a third-party component library sits on exactly the same footing.

mod button;
mod card;
mod checkbox;
mod picker;
mod radio;
mod slider;
mod text_editor;
mod text_field;

use skene_core::{Border, BoxStyle, Condition, Inset};
pub use skene_simple::{SimplePalette, SimpleVariant};

pub use self::{
  button::SimpleButton,
  card::SimpleCard,
  checkbox::SimpleCheckbox,
  picker::SimplePicker,
  radio::SimpleRadio,
  slider::SimpleSlider,
  text_editor::SimpleTextEditor,
  text_field::SimpleTextField,
};

/// The corner radius of a Simple control.
pub(crate) const RADIUS: f32 = 8.;
/// The corner radius of a [`SimpleCard`](crate::SimpleCard).
pub(crate) const CARD_RADIUS: f32 = 12.;

/// The shared field chrome: padded, hairline-bordered, rounded.
pub(crate) fn field_style() -> BoxStyle {
  BoxStyle::new()
    .set_padding(Inset::symmetric(12., 8.))
    .set_border(Border::new(1., SimplePalette::border()))
    .set_border_radius(RADIUS)
}

/// The shared field focus variant: the border brightens to the ring.
pub(crate) fn field_focus() -> (Condition, BoxStyle) {
  (
    Condition::Focused,
    BoxStyle::new().set_border(Border::new(1., SimplePalette::ring())),
  )
}
