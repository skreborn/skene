//! The one resolution of a node's style: the environment the runtime keeps and the node's
//! interaction feed the shared [`Resolver`], the node's style as the shared half applied
//! it resolves through the shared [`Keep::Nothing`](skene_foundation::Keep::Nothing)
//! entry ([`ResolutionContext::resolve`]), and the platform fills in what its own
//! vocabulary takes ([`Drawn::finish_box`], [`Drawn::finish_run`]).

use skene_foundation::{
  ContainerName,
  Environment,
  Frame,
  NodeId,
  ResolutionContext,
  ResolutionSource,
  ResolvedStyle,
  Resolver,
  Size,
  Slot,
};

use super::{Drawn, Kind, PlainBox, PlainRun, Runtime, SlotLeaf};

impl<P: Drawn> ResolutionSource<P> for Runtime<P> {
  fn environment(&self) -> Environment<P::Scalar> {
    Self::environment(self)
  }

  fn container_size(&self, name: ContainerName) -> Option<Size<P::Scalar>> {
    self.window().container_size(name)
  }

  fn slot(&self, node: NodeId, slot: Slot) -> Option<SlotLeaf<P>> {
    self.record(node)?.slots.get(&slot).cloned()
  }

  fn sizes_text(&self) -> bool {
    P::sizes_text()
  }

  fn generation(&self) -> u64 {
    self.restyle.generation()
  }
}

/// What one node's properties resolve against, with the runtime it reads.
#[must_use]
pub struct Env<'a, P: Drawn> {
  runtime: &'a Runtime<P>,
  node: NodeId,
  context: ResolutionContext<'a, P, Runtime<P>>,
}

impl<P: Drawn> std::fmt::Debug for Env<'_, P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Env")
      .field("node", &self.node)
      .finish_non_exhaustive()
  }
}

impl<'a, P: Drawn> Env<'a, P> {
  /// The environment of `node` in `runtime`; the root's where the scene holds no
  /// `node`.
  pub fn of(runtime: &'a Runtime<P>, node: NodeId) -> Self {
    Self {
      runtime,
      node,
      context: ResolutionContext::of(runtime, node)
        .unwrap_or_else(|| ResolutionContext::root(runtime)),
    }
  }

  /// Whether a resolution in this environment has read the node's interaction.
  #[inline(always)]
  #[must_use]
  pub const fn reads_interaction(&self) -> bool {
    self.context.read_interaction()
  }

  /// Runs `operation` with the resolver of this environment, reading tokens where they
  /// are defined and slots through the runtime's records.
  pub fn with_resolver<R>(
    &self,
    operation: impl FnOnce(&Resolver<'_, P::Scalar, P::StyleExt>) -> R,
  ) -> R {
    self.context.with_resolver(operation)
  }

  /// A box keyframe of the node's style with every leaf resolved at the node.
  pub fn resolve_box_keyframe(&self, frame: &Frame<P::Scalar, P::StyleExt>) -> PlainBox<P> {
    self.context.resolve_frame(frame).boxed
  }

  /// A run keyframe of the node's style with every leaf resolved at the node.
  pub fn resolve_run_keyframe(&self, frame: &Frame<P::Scalar, P::StyleExt>) -> PlainRun<P> {
    self.context.resolve_frame(frame).run
  }

  /// The box style of the node, every property resolved and the platform's fill-ins
  /// made; `None` for a node that keeps no box style.
  #[must_use]
  pub fn resolve_box(&self) -> Option<PlainBox<P>> {
    let kept = &self.runtime.record(self.node)?.kind.box_record()?.kept;
    let mut style = self.context.resolve(kept).boxed;

    P::finish_box(&mut style);

    Some(style)
  }

  /// The run style of the node, every property resolved and the platform's fill-ins
  /// made, with the per-child layout a wrapper annotated the run with, resolved into an
  /// otherwise empty box style; `None` for a node that is no run.
  #[must_use]
  pub fn resolve_run(&self) -> Option<(PlainRun<P>, PlainBox<P>)> {
    let Kind::Run { kept, .. } = &self.runtime.record(self.node)?.kind
    else {
      return None;
    };
    let ResolvedStyle {
      boxed: layout,
      run: mut style,
    } = self.context.resolve(kept);

    self.with_resolver(|resolver| P::finish_run(&mut style, resolver));

    Some((style, layout))
  }
}
