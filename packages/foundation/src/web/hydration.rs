//! Hydration: the state and markup a server render carries to the client, and what the
//! client adopts of them.

use crate::{NodeId, Realization};

/// A resource's serializer a server render drains into the hydration state once the
/// resource settles: its value as JSON, `None` while it has none.
pub type HydrationSource = Box<dyn Fn() -> Option<String>>;

/// A platform of the web medium: one a render's state and markup cross on, from a server
/// render to the client that hydrates it.
///
/// The server leaf carries what settles, the browser leaf adopts it, and the headless
/// platform standing in for both off the web carries and adopts nothing.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a platform of the web medium",
  label = "hydration crosses from the web's server leaf to its browser leaf"
)]
pub trait WebHydration: Realization {
  /// A lazy segment's place among the hydrating resources.
  type LazyRegion: Unpin + 'static;

  /// Installs the platform's executor as this thread's spawner, where it is not yet.
  fn install_executor();

  /// Whether the platform renders on a server, ahead of any client.
  #[must_use]
  fn is_server() -> bool;

  /// The value a server render carried for the next resource created, in creation
  /// order within the lazy segment being built, as JSON.
  #[must_use]
  fn adopt_next() -> Option<String>;

  /// The value a server render carried for the resource tagged `tag`, as JSON.
  #[must_use]
  fn adopt_tagged(tag: &str) -> Option<String>;

  /// Registers `source`, drained into the render once its resource settles and matched
  /// in creation order within the lazy segment being built.
  fn register_source(source: HydrationSource);

  /// Registers `source` under `tag`, drained into the render once its resource settles.
  fn register_tagged_source(tag: &str, source: HydrationSource);

  /// Takes the place of the lazy segment being built among the hydrating resources.
  fn enter_lazy() -> Self::LazyRegion;

  /// Runs `work` as the content of the lazy segment at `region`, so the resources it
  /// creates register or adopt under that segment.
  fn within_lazy<R>(region: &Self::LazyRegion, work: impl FnOnce() -> R) -> R;

  /// Claims the server's markup in the lazy segment `start` begins, where hydrating over
  /// some: it stands until [`discard_lazy`](Self::discard_lazy) removes it.
  #[must_use]
  fn claim_lazy(start: NodeId) -> bool;

  /// Removes the markup [`claim_lazy`](Self::claim_lazy) claimed in the segment `start`
  /// begins.
  fn discard_lazy(start: NodeId);

  /// Registers the segment `start` begins as a boundary a streaming render flushes
  /// around, its content settled once `settled` reports so.
  fn register_boundary(start: NodeId, settled: Box<dyn Fn() -> bool>);
}
