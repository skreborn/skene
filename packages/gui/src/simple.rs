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

use skene_core::{Condition, Inset};
pub use skene_simple::{Look, Palette, SimpleVariant};

pub use self::{
  button::SimpleButton,
  card::SimpleCard,
  checkbox::SimpleCheckbox,
  picker::SimplePicker,
  radio::{SimpleRadio, SimpleRadioGroup},
  slider::SimpleSlider,
  text_editor::SimpleTextEditor,
  text_field::SimpleTextField,
  theme::{SimpleTheme, ThemedBox, ThemedControl, ThemedText},
};

/// The corner radius of a Simple control.
pub(crate) const RADIUS: f32 = 8.;
/// The corner radius of a [`SimpleCard`](crate::SimpleCard).
pub(crate) const CARD_RADIUS: f32 = 12.;

/// The shared field chrome, applied per property: padded, hairline-bordered, rounded,
/// with the border brightening to the ring on focus.
pub(crate) fn field_chrome<T: crate::HasDecoration>(control: T) -> T {
  use self::theme::ThemedControl;

  control
    .set_padding(Inset::symmetric(12., 8.))
    .set_border_radius(RADIUS)
    .theme_border(1., |palette| palette.border)
    .theme_border_when(Condition::Focused, 1., |palette| palette.ring)
}
