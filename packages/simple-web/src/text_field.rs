//! A ready-made single-line text field in the Simple design language.

use skene_web::TextField;

use crate::field_chrome;

/// A ready-made single-line text field, a preset of [`TextField`]: full-width field
/// chrome on the card surface whose hairline border becomes the ring while focused.
///
/// [`new`](Self::new) returns the field, so every member of a text field applies, its
/// value flowing both ways; [`styled`](Self::styled) dresses a field built otherwise.
/// Its value is selected in the palette's background on its foreground. Its colors are
/// [`SimplePalette`](crate::SimplePalette)'s tokens, which a
/// [`Theme`](skene_web::Theme) of the palette above it defines, or a read of the palette
/// where no theme does.
///
/// ```
/// use skene_simple_web::prelude::*;
/// use skene_web::prelude::*;
///
/// let name = Signal::new(String::new());
/// let field = SimpleTextField::new()
///   .set_placeholder("Your name")
///   .bind_value(name);
/// ```
#[derive(Debug)]
pub enum SimpleTextField {}

impl SimpleTextField {
  /// An empty field.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new() -> TextField {
    Self::styled(TextField::new())
  }

  /// `field` in the Simple field chrome: full width, padded, on the card surface inside
  /// a hairline in the border color that becomes the ring while focus shows, rounded,
  /// and on the disabled surface under the default cursor while disabled.
  pub fn styled(field: TextField) -> TextField {
    field_chrome(field)
  }
}
