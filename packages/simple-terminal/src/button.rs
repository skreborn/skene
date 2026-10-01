//! A ready-made button in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::SimpleButtonBase;
use skene_terminal::ActivePlatform;

/// A ready-made button: a text label on the variant's surface, with hover and focus
/// feedback and the platform's full keyboard behavior.
///
/// Pick the emphasis with [`set_variant`](Self::set_variant) or follow a signal with
/// [`derive_variant`](Self::derive_variant); a change re-inks the button without a
/// rebuild. A disabled button takes the palette's disabled surface and ink whatever its
/// variant, and the default cursor. [`set_child`](Self::set_child) draws a view in the
/// label's place, inked in the look's ink. For anything beyond that, compose
/// [`BasicButton`](skene_terminal::BasicButton) directly.
///
/// ```
/// use skene_simple_terminal::prelude::*;
/// use skene_terminal::prelude::*;
///
/// let removed = Signal::new(0);
/// let delete = SimpleButton::new("Delete")
///   .set_variant(SimpleVariant::Destructive)
///   .handle_activate(move || removed.update(|count| *count += 1));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct SimpleButton(skene_simple::SimpleButton<ActivePlatform>);

impl SimpleButtonBase<ActivePlatform> for SimpleButton {}
