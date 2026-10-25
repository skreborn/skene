//! A ready-made dropdown choice in the Simple design language.

use skene_foundation::{Choice, View, Wraps};
use skene_simple::SimplePickerBase;
use skene_web::Active;

/// A ready-made dropdown choice.
///
/// Field chrome whose hairline becomes the ring while focused, an indicator after the
/// chosen label, and a list on the card surface whose rows highlight while focused and
/// fill in the primary look while chosen.
///
/// The indicator is the face's [`Part::Face`](skene_web::Part::Face), the list surface the
/// [`Part::List`](skene_web::Part::List) and each row's check the
/// [`Part::Item`](skene_web::Part::Item), so all three follow their control's state. The chosen
/// value, a value of the app's own type, flows both ways, exactly as on
/// [`BasicPicker`](skene_web::BasicPicker); for custom chrome, compose the raw control directly.
///
/// ```
/// use skene_simple_web::prelude::*;
/// use skene_web::prelude::*;
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// enum Size {
///   Small,
///   Medium,
///   Large,
/// }
///
/// let size = Signal::new(Size::Medium);
/// let picker = SimplePicker::new()
///   .set_label("Size")
///   .set_options([
///     (Size::Small, "Small"),
///     (Size::Medium, "Medium"),
///     (Size::Large, "Large"),
///   ])
///   .bind_chosen(size);
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct SimplePicker<T: Choice>(skene_simple::SimplePicker<Active, T>);

impl<T: Choice> Default for SimplePicker<T> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<T: Choice> SimplePickerBase<Active, T> for SimplePicker<T> {}
