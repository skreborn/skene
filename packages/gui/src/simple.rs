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
mod theme;

use skene_core::{Border, Condition, Inset};
pub use skene_simple::{SimpleLook, SimpleVariant};

/// The Simple palette, over the GUI facade's conditions.
pub type SimplePalette = skene_simple::SimplePalette<Condition>;

pub use self::{
  button::SimpleButton,
  card::SimpleCard,
  checkbox::SimpleCheckbox,
  picker::SimplePicker,
  radio::{SimpleRadio, SimpleRadioGroup},
  slider::SimpleSlider,
  text_editor::SimpleTextEditor,
  text_field::SimpleTextField,
  theme::SimpleTheme,
};

/// The corner radius of a Simple control.
pub(crate) const RADIUS: f32 = 8.;
/// The corner radius of a [`SimpleCard`](crate::SimpleCard).
pub(crate) const CARD_RADIUS: f32 = 12.;

/// The shared field chrome, applied per property: padded, hairline-bordered, rounded,
/// with the border brightening to the ring on focus.
pub(crate) fn field_chrome<T: crate::HasDecoration>(control: T, palette: &SimplePalette) -> T {
  control
    .set_padding(Inset::symmetric(12., 8.))
    .set_border_radius(RADIUS)
    .set_border(palette.border.clone().map(|color| Border::new(1., color)))
    .set_border_when(
      Condition::Focused,
      palette.ring.clone().map(|color| Border::new(1., color)),
    )
}
