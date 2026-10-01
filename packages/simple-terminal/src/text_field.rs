//! A ready-made single-line text field in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::SimpleTextFieldBase;
use skene_terminal::ActivePlatform;

/// A ready-made single-line text field: full-width field chrome whose hairline border
/// becomes the ring while focused.
///
/// The value flows both ways, exactly as on [`BasicTextField`](skene_terminal::BasicTextField); for
/// custom chrome, compose the raw control directly.
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct SimpleTextField(skene_simple::SimpleTextField<ActivePlatform>);

impl SimpleTextFieldBase<ActivePlatform> for SimpleTextField {}
