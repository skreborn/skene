//! A ready-made button in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::SimpleButtonBase;
use skene_web::ActivePlatform;

/// A ready-made button: a text label on the variant's surface, with hover and focus
/// feedback and the platform's full keyboard behavior.
///
/// Pick the emphasis with [`set_variant`](Self::set_variant) or follow a signal with
/// [`derive_variant`](Self::derive_variant); the look is redefined on the button's own
/// node, so a change re-inks it without a rebuild. A disabled button takes the
/// palette's disabled surface and ink whatever its variant, and the default cursor.
/// [`set_child`](Self::set_child) draws a view in the label's place, inked in the look's ink. For
/// anything beyond that, compose [`BasicButton`](skene_web::BasicButton) directly.
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct SimpleButton(skene_simple::SimpleButton<ActivePlatform>);

impl SimpleButtonBase<ActivePlatform> for SimpleButton {}
