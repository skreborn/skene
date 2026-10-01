//! A text run on its own line.

use skene_foundation::{HasRun, RunBase, TextBase, View, Wraps};

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

impl Text {
  skene_foundation::properties! { run_style_mut, ActivePlatform;
    set_bold / derive_bold (bold) => ext.bold: bool, "whether the text is bold";
  }
}
