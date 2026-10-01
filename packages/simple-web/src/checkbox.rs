//! A ready-made checkbox in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::SimpleCheckboxBase;
use skene_web::ActivePlatform;

/// A ready-made checkbox: a rounded mark on the card surface that fills with the
/// primary surface while checked and rings while focused, beside its caption.
///
/// The mark is the control's [`Part::Mark`](skene_web::Part::Mark), so it follows the
/// control's state. The checked state flows both ways; for a checkbox drawn some
/// other way, compose [`BasicCheckbox`](skene_web::BasicCheckbox) directly.
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct SimpleCheckbox(skene_simple::SimpleCheckbox<ActivePlatform>);

impl SimpleCheckboxBase<ActivePlatform> for SimpleCheckbox {}
