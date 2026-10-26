//! A text run on its own line.

use skene_foundation::{HasRun, RunBase, TextBase, View, Wraps};

use crate::Active;

/// A run of text on its own line.
///
/// A property left unset is inherited through its [`typography`](crate::typography)
/// token.
///
/// ```
/// use skene_terminal::prelude::*;
///
/// let count = Signal::new(0);
/// let label =
///   Text::derive(move || format!("{} items", count.get())).set_color(Color::srgb(0.2, 0.2, 0.2));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Text(skene_foundation::Text<Active>);

impl TextBase<Active> for Text {}

impl RunBase<Active> for Text {}

impl Text {
  skene_foundation::properties! { run_style_mut, crate::Condition;
    set_bold / derive_bold (bold) => ext.bold: bool, "whether the text is bold";
  }
}
