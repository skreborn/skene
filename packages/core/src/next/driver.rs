//! The seam between the shared machinery and a medium.

use std::{cell::RefCell, fmt::Debug, rc::Rc, thread::LocalKey};

use super::{
  BoxProps,
  BoxValue,
  ControlProps,
  ControlState,
  Definition,
  Definitions,
  EventKind,
  MediaSignals,
  NodeId,
  OverlayProps,
  RunProps,
  RunValue,
  Scalar,
  Scene,
  Slot,
  StyleExtension,
};
use crate::{AccessProp, Color, FocusOrder, Size};

/// A medium that realizes the portable vocabulary: boxes, controls, overlays, runs,
/// anchors, scopes, tree mutations, tokens, event registration, focus, values, sizes
/// and media.
///
/// Zero-sized and monomorphized: a facade names exactly one driver, and every view is
/// written once against the trait. Conditions follow from [`Scalar`](Self::Scalar) as
/// `Condition<Self::Scalar>`. Core owns the [`Scene`] and every operation on it; a
/// driver declares the thread-local holding it and is called only to mirror each
/// mutation into its sink. `Painted` and embeds are outside this trait: their payloads
/// arrive through a driver's extension traits.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a driver",
  label = "a driver is a zero-sized type realizing the portable vocabulary: boxes, runs, anchors, scopes, tokens and events",
  note = "`Painted` and embeds are outside `Driver`; a driver offers them through its own extension traits"
)]
pub trait Driver: Copy + Default + 'static {
  /// The platform handle a node owns: an element, a layout id, a record.
  type Sink: 'static;
  /// The unit lengths are measured in.
  type Scalar: Scalar;
  /// The properties only this medium has.
  type StyleExt: StyleExtension;
  /// The erased token definition this medium accepts.
  type TokenValue: Clone
    + PartialEq
    + Debug
    + From<Definition<Color, Self::Scalar>>
    + From<Definition<f32, Self::Scalar>>
    + 'static;

  /// The thread-local holding this driver's scene.
  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>>;

  /// Tells the driver the id the scene minted for `sink`, once, right after the sink
  /// was created.
  #[inline(always)]
  fn bind_node(&self, _sink: &Self::Sink, _node: NodeId) {}

  /// Realizes a box.
  fn create_box(&self, props: &BoxProps<Self>) -> Self::Sink;

  /// Writes the value a derived leaf computed into its slot on a box.
  fn update_box(
    &self,
    sink: &Self::Sink,
    slot: Slot,
    value: &BoxValue<Self::Scalar, Self::StyleExt>,
  );

  /// Replaces a box's place in focus traversal; `None` takes it out of focus.
  fn update_focus_order(&self, sink: &Self::Sink, focus_order: Option<FocusOrder>);

  /// Replaces the accessibility properties a box reports.
  fn update_access(&self, sink: &Self::Sink, access: &[AccessProp]);

  /// Realizes a control: a box a driver may stand a native element in for.
  fn create_control(&self, props: &ControlProps<Self>) -> Self::Sink;

  /// Mirrors a control's state, for a driver whose platform holds it too.
  fn set_control_state(&self, sink: &Self::Sink, state: &ControlState);

  /// Realizes an overlay: a box shown above the app while open, closed at first.
  fn create_overlay(&self, props: &OverlayProps<Self>) -> Self::Sink;

  /// Opens or closes an overlay; an open overlay placed against `anchor` sits by it
  /// rather than by the viewport.
  fn set_overlay_open(&self, sink: &Self::Sink, open: bool, anchor: Option<&Self::Sink>);

  /// Realizes a text run.
  fn create_run(&self, props: &RunProps<Self>) -> Self::Sink;

  /// Writes the value a derived leaf computed into its slot on a run.
  fn update_run(&self, sink: &Self::Sink, slot: Slot, value: &RunValue<Self::StyleExt>);

  /// Replaces a run's text.
  fn set_text(&self, sink: &Self::Sink, text: &str);

  /// Realizes an anchor: a layout-invisible marker a reactive subtree mounts between.
  fn create_anchor(&self) -> Self::Sink;

  /// Realizes a scope: a layout-invisible node token definitions hang off.
  fn create_scope(&self) -> Self::Sink;

  /// Places `child` under `parent`, immediately before `before` when given, else last.
  fn attach(&self, parent: &Self::Sink, child: &Self::Sink, before: Option<&Self::Sink>);

  /// Removes `child` from under `parent`.
  fn detach(&self, parent: &Self::Sink, child: &Self::Sink);

  /// Releases a sink the scene no longer holds.
  fn destroy(&self, sink: Self::Sink);

  /// Defines tokens on a scope, replacing what the scope defined before.
  fn define_tokens(&self, scope: &Self::Sink, definitions: &Definitions<Self::TokenValue>);

  /// Defines the tokens every node falls back to, replacing the previous set.
  fn define_root_tokens(&self, definitions: &Definitions<Self::TokenValue>);

  /// Notes that a node has a handler for `kind`; dispatch itself is core's.
  fn listen(&self, sink: &Self::Sink, kind: EventKind);

  /// Moves focus to a node.
  fn focus(&self, sink: &Self::Sink);

  /// Takes focus off a node.
  fn blur(&self, sink: &Self::Sink);

  /// Writes a control's textual value.
  fn set_value(&self, sink: &Self::Sink, value: &str);

  /// Writes a control's checked state.
  fn set_checked(&self, sink: &Self::Sink, checked: bool);

  /// Reports a node's measured size to `report`, now and whenever it changes.
  fn observe_size(&self, sink: &Self::Sink, report: Rc<dyn Fn(Size<Self::Scalar>)>);

  /// The environment as signals.
  fn media(&self) -> MediaSignals<Self::Scalar>;
}
