//! A streaming boundary: a subtree the server can flush around and stream in place.

use skene_foundation::{View, Wraps, web::DeferredBase};

use crate::Active;

/// A subtree whose async work the server settles apart from the shell's and streams
/// in place once it does.
///
/// Its content builds in place, between anchors of its own. On the server its async
/// work reports to a boundary of its own rather than the shell's, so a streaming render
/// emits everything up to the boundary, blocks until that work settles, then streams
/// the content in document order; a whole render settles through it and serializes it
/// in place. On the client the content builds directly, and hydration adopts the
/// server's already-settled markup. A streamed page's head flushes with the shell, so a
/// head element registered inside the subtree reaches that page only once the client
/// hydrates. Without the content [`set_child_builder`](Self::set_child_builder)
/// supplies, it renders nothing.
///
/// ```
/// use skene_web::{Deferred, prelude::*};
///
/// let comments = Deferred::new().set_child_builder(|| Text::new("The comments"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Deferred(skene_foundation::web::Deferred<Active>);

impl DeferredBase<Active> for Deferred {}
