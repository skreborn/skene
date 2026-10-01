//! A text run on its own line.

use skene_foundation::{RunBase, TextBase, View, Wraps};

use crate::ActivePlatform;

/// A run of text on its own line.
///
/// A property left unset is inherited through its [`typography`](crate::typography)
/// token.
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Text(skene_foundation::Text<ActivePlatform>);

impl TextBase<ActivePlatform> for Text {}

impl RunBase<ActivePlatform> for Text {}
