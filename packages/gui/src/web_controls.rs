//! The browser's own controls, as views.
//!
//! Each wraps the native element and exposes only what a real browser delivers on:
//! the platform's whole keyboard, focus, form, and accessibility behavior, and exactly
//! the customization that works across engines. Where that is not enough to draw the
//! control a design calls for, the drawn [`Raw`](crate::RawButton) controls are the
//! other half of the pair.

mod checkbox;
mod picker;
mod radio;
mod slider;

pub use self::{checkbox::WebCheckbox, picker::WebPicker, radio::WebRadio, slider::WebSlider};
