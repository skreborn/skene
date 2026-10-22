//! A ready-made single-line text field in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::SimpleTextFieldBase;
use skene_terminal::Active;

/// A ready-made single-line text field: full-width field chrome whose hairline border
/// becomes the ring while focused.
///
/// The value flows both ways, exactly as on [`BasicTextField`](skene_terminal::BasicTextField); for
/// custom chrome, compose the raw control directly.
///
/// ```
/// use skene_simple_terminal::prelude::*;
/// use skene_terminal::prelude::*;
///
/// let name = Signal::new(String::new());
/// let field = SimpleTextField::new()
///   .set_placeholder("Your name")
///   .bind_value(name);
/// ```
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct SimpleTextField(skene_simple::SimpleTextField<Active>);

impl SimpleTextFieldBase<Active> for SimpleTextField {}
