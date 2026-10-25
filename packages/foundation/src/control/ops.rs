//! The realization's operations a behavior reaches, as plain functions.

use std::{rc::Rc, time::Duration};

use crate::{
  ClickEvent,
  ContentOps,
  NodeId,
  Observation,
  Part,
  PointerEvent,
  Prevented,
  Realization,
  Scalar,
  Scene,
  Size,
  Timer,
  dispatch_click,
};

/// A visitor of the nodes of a subtree, in tree order.
type Visit<'a> = dyn FnMut(NodeId) + 'a;

/// What hears a node's size, in `f32`.
type SizeReport = Rc<dyn Fn(Size<f32>)>;

/// The operations of the build's realization a behavior reaches, as plain functions.
///
/// They are filled once from it ([`ControlOps::of`]): monomorphized, never a `dyn`
/// realization, so a behavior names no realization and acts through its
/// [`ControlHandle`] or these. Every size and position crosses as `f32`, whatever the
/// realization's unit.
///
/// [`ControlHandle`]: crate::ControlHandle
#[derive(Clone, Copy)]
#[must_use]
pub struct ControlOps {
  /// Reads, writes and controls a node's contents.
  content: ContentOps,
  /// Moves focus to a node.
  focus: fn(NodeId),
  /// Dispatches a click at a node, bubbling.
  click: fn(NodeId, &ClickEvent) -> Prevented,
  /// A control's first node recorded as a part.
  find_part: fn(NodeId, Part) -> Option<NodeId>,
  /// Visits a node and every node beneath it, in tree order.
  walk: fn(NodeId, &mut Visit<'_>),
  /// Runs an action once after a delay on the platform's clock.
  schedule: fn(Duration, Box<dyn FnOnce()>) -> Timer,
  /// Drops a scheduled action that has not run.
  cancel: fn(Timer),
  /// Reports a node's size each time it is laid out at a new one.
  observe_size: fn(NodeId, SizeReport) -> Observation,
  /// A pointer event with its local position measured from a node.
  localize_pointer: fn(NodeId, &PointerEvent) -> PointerEvent,
  /// The length nearest another that the realization's unit holds.
  round_length: fn(f32) -> f32,
}

impl std::fmt::Debug for ControlOps {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("ControlOps").finish_non_exhaustive()
  }
}

impl ControlOps {
  /// The operations of the realization `R`.
  #[inline(always)]
  pub const fn of<R: Realization>() -> Self {
    Self {
      content: ContentOps::of::<R>(),
      focus: R::focus,
      click: dispatch_click::<R>,
      find_part: Self::find_part_of::<R>,
      walk: Self::walk_of::<R>,
      schedule: R::schedule,
      cancel: R::cancel,
      observe_size: Self::observe_size_of::<R>,
      localize_pointer: R::localize_pointer,
      round_length: Self::round_length_of::<R>,
    }
  }

  /// The operations on a node's contents, for a [`ContentKey`](crate::ContentKey)'s
  /// `_with` forms and a [`Controls`](crate::Controls)' registration.
  #[inline(always)]
  pub const fn content(&self) -> ContentOps {
    self.content
  }

  /// Moves focus to `node`.
  #[inline(always)]
  pub fn focus(&self, node: NodeId) {
    (self.focus)(node);
  }

  /// Dispatches `event` as a click at `node`, so its behavior and handlers see it, then
  /// its ancestors'.
  #[inline(always)]
  pub fn click(&self, node: NodeId, event: &ClickEvent) -> Prevented {
    (self.click)(node, event)
  }

  /// The first node inside the control `control` recorded as `part`.
  #[inline(always)]
  #[must_use]
  pub fn find_part(&self, control: NodeId, part: Part) -> Option<NodeId> {
    (self.find_part)(control, part)
  }

  /// Runs `visit` with `root` and every node beneath it in the scene, in tree order.
  ///
  /// The scene is borrowed while `visit` runs, so it reads what it needs and touches
  /// no view.
  #[inline(always)]
  pub fn walk(&self, root: NodeId, mut visit: impl FnMut(NodeId)) {
    (self.walk)(root, &mut visit);
  }

  /// Runs `action` once after `delay` on the platform's clock, answering the timer that
  /// cancels it.
  #[inline(always)]
  pub fn schedule(&self, delay: Duration, action: impl FnOnce() + 'static) -> Timer {
    (self.schedule)(delay, Box::new(action))
  }

  /// Drops the action `timer` names, if it has not run.
  #[inline(always)]
  pub fn cancel(&self, timer: Timer) {
    (self.cancel)(timer);
  }

  /// Reports `node`'s size to `report` each time it is laid out at a new one, until
  /// the observation is stopped or the node goes.
  #[inline(always)]
  pub fn observe_size(&self, node: NodeId, report: Rc<dyn Fn(Size<f32>)>) -> Observation {
    (self.observe_size)(node, report)
  }

  /// `event` with its local position measured from `node`'s top-left corner.
  #[inline(always)]
  pub fn localize_pointer(&self, node: NodeId, event: &PointerEvent) -> PointerEvent {
    (self.localize_pointer)(node, event)
  }

  /// The length nearest `length` that the realization's unit holds.
  #[inline(always)]
  #[must_use]
  pub fn round_length(&self, length: f32) -> f32 {
    (self.round_length)(length)
  }

  /// The first node inside `control` recorded as `part` in `R`'s scene.
  fn find_part_of<R: Realization>(control: NodeId, part: Part) -> Option<NodeId> {
    Scene::<R>::with(|scene| scene.find_part(control, part))
  }

  /// Runs `visit` with `root` and every node beneath it in `R`'s scene, in tree order.
  fn walk_of<R: Realization>(root: NodeId, visit: &mut Visit<'_>) {
    Scene::<R>::with(|scene| {
      let mut pending = vec![root];

      while let Some(node) = pending.pop() {
        visit(node);

        let children: Vec<NodeId> = scene.children(node).collect();

        pending.extend(children.into_iter().rev());
      }
    });
  }

  /// Reports `node`'s size to `report` as `R` lays it out, in `f32`.
  fn observe_size_of<R: Realization>(node: NodeId, report: SizeReport) -> Observation {
    R::observe_size(
      node,
      Rc::new(move |size| report(Size::new(size.width.to_f32(), size.height.to_f32()))),
    )
  }

  /// The length nearest `length` that `R`'s unit holds, in `f32`.
  fn round_length_of<R: Realization>(length: f32) -> f32 {
    R::Scalar::from_f32(length).to_f32()
  }
}
