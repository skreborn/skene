//! The seam between the shared machinery and a medium.

use std::{cell::RefCell, fmt::Debug, rc::Rc, thread::LocalKey, time::Duration};

use skene_reactive::{Get, Memo, ReadSignal};

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
  StaticLeaf,
  StyleExtension,
};
use crate::{
  AccessProp,
  Availability,
  ClipboardAction,
  Color,
  ColorScheme,
  ContainerName,
  FocusOrder,
  Media,
  MediaEnv,
  Placement,
  Point,
  PointerEvent,
  Size,
  TextSelection,
};

/// A one-shot action a driver scheduled on its clock, to cancel it by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct Timer(pub u64);

/// A medium that realizes the portable vocabulary: boxes, controls, overlays, runs,
/// anchors, scopes, tree mutations, tokens, event registration, focus, values, sizes,
/// media and a clock.
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
  ///
  /// Cloned out of the scene before the driver is called with it, so a driver call
  /// never runs under the scene's borrow and may reach the scene itself.
  type Sink: Clone + 'static;
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

  /// Writes the leaf a derived leaf computed into its slot on a box: a value, or a
  /// token or expression the driver resolves where it reads the slot.
  fn update_box(
    &self,
    sink: &Self::Sink,
    slot: Slot,
    leaf: &StaticLeaf<BoxValue<Self::Scalar, Self::StyleExt>>,
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

  /// Replaces where an overlay sits; an open overlay is re-placed by the
  /// [`set_overlay_open`](Self::set_overlay_open) that follows.
  #[inline(always)]
  fn set_overlay_placement(&self, _sink: &Self::Sink, _placement: Placement) {}

  /// Pins an overlay's top-start corner to `position`, in the units a pointer event
  /// reports, or unpins it for `None`; a pinned overlay ignores its placement and
  /// anchor, and is shifted to stay within the viewport. An open overlay moves at the
  /// [`set_overlay_open`](Self::set_overlay_open) that follows.
  #[inline(always)]
  fn set_overlay_position(&self, _sink: &Self::Sink, _position: Option<Point<f32>>) {}

  /// Realizes a text run.
  fn create_run(&self, props: &RunProps<Self>) -> Self::Sink;

  /// Writes the leaf a derived leaf computed into its slot on a run: a value, or a
  /// token or expression the driver resolves where it reads the slot.
  fn update_run(&self, sink: &Self::Sink, slot: Slot, leaf: &StaticLeaf<RunValue<Self::StyleExt>>);

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

  /// `event` with its local position measured from `sink`'s top-left corner; unchanged
  /// on a driver that lays nothing out.
  #[inline(always)]
  fn localize_pointer(&self, _sink: &Self::Sink, event: &PointerEvent) -> PointerEvent {
    *event
  }

  /// Moves focus to a node.
  fn focus(&self, sink: &Self::Sink);

  /// Takes focus off a node.
  fn blur(&self, sink: &Self::Sink);

  /// Writes the options a platform-drawn picker offers, each a value and the name
  /// shown for it, in order; a driver without one ignores them.
  #[inline(always)]
  fn set_options(&self, _sink: &Self::Sink, _options: &[(String, String)]) {}

  /// Writes a control's textual value.
  fn set_value(&self, sink: &Self::Sink, value: &str);

  /// Writes a text entry's selection.
  fn set_selection(&self, sink: &Self::Sink, selection: TextSelection);

  /// Writes a control's checked state.
  fn set_checked(&self, sink: &Self::Sink, checked: bool);

  /// Reports a node's measured size to `report`, now and whenever it changes.
  fn observe_size(&self, sink: &Self::Sink, report: Rc<dyn Fn(Size<Self::Scalar>)>);

  /// The environment as signals.
  fn media(&self) -> MediaSignals<Self::Scalar>;

  /// The signal that turns true once a render made without script has been hydrated,
  /// and stays false on a render that never is; `None` where the tree is live from
  /// the start.
  ///
  /// Only a driver with a scriptless render answers: the web leaves, and the headless
  /// driver standing in for them under a test. Every other driver keeps the default.
  #[inline(always)]
  fn hydration(&self) -> Option<ReadSignal<bool>> {
    None
  }

  /// Puts `scheme` in force throughout, overriding the platform's preference and
  /// touching no node; `None` returns to following the platform.
  fn set_scheme(&self, scheme: Option<ColorScheme>);

  /// The scheme the app put in force, or a previous run stored, when there is one.
  fn stored_scheme(&self) -> Option<ColorScheme>;

  /// Runs `action` once `delay` has passed on the driver's clock, unless cancelled
  /// first; a driver without a clock never runs it.
  fn schedule(&self, delay: Duration, action: Box<dyn FnOnce()>) -> Timer;

  /// Drops the action `timer` names, if it has not run.
  fn cancel(&self, timer: Timer);

  /// Whether `action` can be offered on what the platform holds selected or focused
  /// right now; a driver without a clipboard offers nothing.
  #[inline(always)]
  fn clipboard_availability(&self, _action: ClipboardAction) -> Availability {
    Availability::Absent
  }

  /// Performs `action` on what the platform holds selected or focused, when it is
  /// [`Enabled`](Availability::Enabled); a driver without a clipboard does nothing.
  #[inline(always)]
  fn clipboard(&self, _action: ClipboardAction) {}

  /// Whether `media` holds, as a memo following the environment; a container condition
  /// never holds here, since it is answered per node by the box it names.
  fn watch_media(&self, media: &Media<Self::Scalar>) -> Memo<bool> {
    let media = media.clone();
    let signals = self.media();

    Memo::new(move |_| {
      let container = |_: Option<ContainerName>| None;

      media.holds_in(&MediaEnv {
        viewport: signals.viewport.get(),
        scheme: signals.scheme.get(),
        reduced_motion: signals.reduced_motion.get(),
        reduced_transparency: false,
        high_contrast: false,
        container: &container,
      })
    })
  }
}
