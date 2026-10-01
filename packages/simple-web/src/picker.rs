//! A ready-made dropdown choice in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::SimplePickerBase;
use skene_web::ActivePlatform;

/// A ready-made dropdown choice.
///
/// Field chrome whose hairline becomes the ring while focused, an indicator after the
/// chosen label, and a list on the card surface whose rows highlight while focused and
/// fill in the primary look while chosen.
///
/// The indicator is the face's [`Part::Face`](skene_web::Part::Face), the list surface the
/// [`Part::List`](skene_web::Part::List) and each row's check the
/// [`Part::Item`](skene_web::Part::Item), so all three follow their control's state. The chosen
/// value flows both ways, exactly as on [`BasicPicker`](skene_web::BasicPicker); for custom chrome,
/// compose the raw control directly.
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct SimplePicker(skene_simple::SimplePicker<ActivePlatform>);

impl SimplePickerBase<ActivePlatform> for SimplePicker {}
