//! A text run on its own line.

use crate::ActivePlatform;

/// A run of text on its own line.
///
/// A property left unset is inherited through its [`typography`](crate::typography)
/// token.
#[must_use]
pub struct Text(skene_foundation::Text<ActivePlatform>);

skene_foundation::impl_text!(Text, ActivePlatform);

impl Text {
  skene_foundation::properties! { style_mut, ActivePlatform;
    set_bold / derive_bold (bold) => ext.bold: bool, "whether the text is bold";
  }
}
