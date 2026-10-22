//! A ready-made multi-line text editor in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::SimpleTextEditorBase;
use skene_web::Active;

/// A ready-made multi-line text editor: full-width field chrome whose hairline border
/// becomes the ring while focused, showing several lines at once.
///
/// The value flows both ways, exactly as on [`BasicTextEditor`](skene_web::BasicTextEditor); for
/// custom chrome, compose the raw control directly.
///
/// ```
/// use skene_simple_web::prelude::*;
/// use skene_web::prelude::*;
///
/// let notes = Signal::new(String::new());
/// let editor = SimpleTextEditor::new()
///   .set_lines(4)
///   .set_placeholder("Notes")
///   .bind_value(notes);
/// ```
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct SimpleTextEditor(skene_simple::SimpleTextEditor<Active>);

impl SimpleTextEditorBase<Active> for SimpleTextEditor {}
