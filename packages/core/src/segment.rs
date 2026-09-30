//! Content mounted between two anchors, changed in place.

use skene_reactive::Owner;

use super::{Annotation, NodeId, Platform, Scene};

/// Content between a pair of anchors, standing among its parent's children as the
/// content itself: what a view whose content changes in place builds on.
///
/// The start anchor stands for the segment. Each [`Piece`] of content is built under a
/// scope of its own with the annotation pending as the segment was made, then mounted
/// or taken out live, and paused while it stands apart where its view wants; a piece's
/// nodes are destroyed with its scope.
#[must_use]
pub struct Segment<P: Platform> {
  start: NodeId,
  annotation: Option<Annotation<P>>,
}

impl<P: Platform> std::fmt::Debug for Segment<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Segment")
      .field("start", &self.start)
      .finish_non_exhaustive()
  }
}

impl<P: Platform> Segment<P> {
  /// An empty, detached segment, taking the annotation pending for the view making it.
  pub fn new() -> Self {
    let (start, _) = Scene::<P>::create_segment();
    let annotation = Scene::<P>::with(Scene::take_pending_annotation);

    Self { start, annotation }
  }

  /// The start anchor, which stands for the segment.
  #[inline(always)]
  pub const fn start(&self) -> NodeId {
    self.start
  }

  /// Builds a piece of content under `scope`, its first node taking the segment's
  /// annotation; the piece stands apart until mounted, and its nodes are destroyed
  /// when `scope` is disposed.
  pub fn build(&self, scope: Owner, build: impl FnOnce() -> NodeId) -> Piece {
    let node = Scene::<P>::annotate(self.annotation.clone(), || scope.enter(build));

    scope.on_cleanup(move || Scene::<P>::with(|scene| scene.destroy(node)));

    Piece { node, scope }
  }

  /// Mounts `piece` last in the segment, moving it there if it is mounted.
  #[inline(always)]
  pub fn mount(&self, piece: Piece) {
    Scene::<P>::with(|scene| scene.insert_in_segment(self.start, piece.node, None));
  }

  /// Mounts `piece` immediately before `before`, a piece mounted in the segment.
  #[inline(always)]
  pub fn mount_before(&self, piece: Piece, before: Piece) {
    Scene::<P>::with(|scene| scene.insert_in_segment(self.start, piece.node, Some(before.node)));
  }

  /// Takes `piece` out of the segment, leaving it live.
  #[inline(always)]
  pub fn unmount(&self, piece: Piece) {
    Scene::<P>::with(|scene| scene.remove_from_segment(self.start, piece.node));
  }
}

impl<P: Platform> Default for Segment<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

/// One piece of a segment's content: the node it built and the scope it was built
/// under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct Piece {
  node: NodeId,
  scope: Owner,
}

impl Piece {
  /// The node the piece built.
  #[inline(always)]
  pub const fn node(self) -> NodeId {
    self.node
  }

  /// The scope the piece was built under.
  #[inline(always)]
  pub const fn scope(self) -> Owner {
    self.scope
  }

  /// Pauses the piece's effects: invalidations accumulate and runs wait for
  /// [`resume`](Self::resume).
  #[inline(always)]
  pub fn pause(self) {
    self.scope.pause();
  }

  /// Lets the piece's effects run again, at once for each whose dependency changed
  /// while it was paused.
  #[inline(always)]
  pub fn resume(self) {
    self.scope.resume();
  }

  /// Discards the piece: its scope is disposed and its nodes destroyed.
  #[inline(always)]
  pub fn discard(self) {
    self.scope.dispose();
  }
}
