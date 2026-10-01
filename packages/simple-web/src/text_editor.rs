//! A ready-made multi-line text editor in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::SimpleTextEditorBase;
use skene_web::ActivePlatform;

/// A ready-made multi-line text editor: full-width field chrome whose hairline border
/// becomes the ring while focused, showing several lines at once.
///
/// The value flows both ways, exactly as on [`BasicTextEditor`](skene_web::BasicTextEditor); for
/// custom chrome, compose the raw control directly.
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct SimpleTextEditor(skene_simple::SimpleTextEditor<ActivePlatform>);

impl SimpleTextEditorBase<ActivePlatform> for SimpleTextEditor {}
