//! The seam between the shared machinery and a medium.

use std::{cell::RefCell, fmt::Debug, hash::Hash, rc::Rc, thread::LocalKey, time::Duration};

use skene_reactive::{Memo, ReadSignal};

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
  PickerOption,
  RunProps,
  RunValue,
  Scalar,
  Scene,
  Slot,
  StaticBoxStyle,
  StaticLeaf,
  StyleExtension,
};
use crate::{
  AccessProp,
  Availability,
  ClipboardAction,
  Color,
  ColorScheme,
  Dimension,
  FocusOrder,
  Length,
  Media,
  Placement,
  Point,
  PointerEvent,
  Size,
  TextSelection,
};

/// A one-shot action a platform scheduled on its clock, to cancel it by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct Timer(pub u64);

/// A size observation a platform keeps: [`stop`](Self::stop) ends it, and it ends with
/// its node otherwise.
pub struct Observation {
  stop: Option<Box<dyn FnOnce()>>,
}

impl Observation {
  /// An observation `stop` ends.
  #[inline(always)]
  pub fn new(stop: impl FnOnce() + 'static) -> Self {
    Self {
      stop: Some(Box::new(stop)),
    }
  }

  /// An observation with nothing to end, from a platform that never measures.
  #[inline(always)]
  #[must_use]
  pub const fn unmeasured() -> Self {
    Self { stop: None }
  }

  /// Ends the observation: its report is not called again.
  #[inline(always)]
  pub fn stop(self) {
    if let Some(stop) = self.stop {
      stop();
    }
  }
}

impl Debug for Observation {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Observation")
      .finish_non_exhaustive()
  }
}

/// The one-shot actions a platform scheduled on its clock, each due at a reading.
///
/// A platform runs them earliest due first, those due together in the order they were
/// scheduled, as a browser runs its timers.
#[must_use]
pub struct Timers<A> {
  /// Each action, with the reading it is due at and its timer.
  entries: Vec<(f64, Timer, A)>,
  /// The timer the next action scheduled takes.
  next: u64,
}

impl<A> Default for Timers<A> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<A> Debug for Timers<A> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Timers")
      .field("pending", &self.entries.len())
      .finish_non_exhaustive()
  }
}

impl<A> Timers<A> {
  /// No action scheduled.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      entries: Vec::new(),
      next: 0,
    }
  }

  /// Schedules `action`, due at the clock reading `at`; the timer cancels it.
  pub fn schedule(&mut self, at: f64, action: A) -> Timer {
    let timer = Timer(self.next);

    self.next += 1;
    self.entries.push((at, timer, action));

    timer
  }

  /// Cancels `timer`'s action; `false` when it already ran or was cancelled.
  pub fn cancel(&mut self, timer: Timer) -> bool {
    let before = self.entries.len();

    self.entries.retain(|(_, held, _)| *held != timer);

    self.entries.len() != before
  }

  /// The timer the next action scheduled takes, which bounds a batch of
  /// [`pop_due`](Self::pop_due) calls to the actions scheduled before it began.
  #[inline(always)]
  pub const fn next_timer(&self) -> Timer {
    Timer(self.next)
  }

  /// Takes the earliest action due by the clock reading `now` among those scheduled
  /// before `before`: the first scheduled of those due together.
  pub fn pop_due(&mut self, now: f64, before: Timer) -> Option<A> {
    let index = self
      .entries
      .iter()
      .enumerate()
      .filter(|(_, (at, timer, _))| *at <= now && timer.0 < before.0)
      .min_by(|(_, (left, left_timer, _)), (_, (right, right_timer, _))| {
        left.total_cmp(right).then(left_timer.0.cmp(&right_timer.0))
      })
      .map(|(index, _)| index)?;

    Some(self.entries.remove(index).2)
  }

  /// The clock reading the earliest action is due at.
  #[must_use]
  pub fn next_due(&self) -> Option<f64> {
    self
      .entries
      .iter()
      .map(|(at, ..)| *at)
      .min_by(f64::total_cmp)
  }

  /// How many actions are waiting.
  #[inline(always)]
  #[must_use]
  pub const fn len(&self) -> usize {
    self.entries.len()
  }

  /// Whether no action is waiting.
  #[inline(always)]
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }
}

/// A medium that realizes the portable vocabulary: boxes, controls, overlays, runs,
/// anchors, scopes, tree mutations, tokens, event registration, focus, values, sizes,
/// media and a clock.
///
/// Zero-sized and monomorphized: a facade names exactly one platform, every view is
/// written once against the trait, and every operation is an associated function over
/// sinks passed by value. Conditions follow from [`Scalar`](Self::Scalar) as
/// `Condition<Self::Scalar>`. Core owns the [`Scene`] and every operation on it; a
/// platform declares the thread-local holding it and is called only to mirror each
/// mutation into its sink. `Painted` and embeds are outside this trait: their payloads
/// arrive through a platform's extension traits.
///
/// A platform holds every flush until its turn ends—an event, a mount, a round of task
/// polls—through [`Runtime::set_scheduler`](skene_reactive::Runtime::set_scheduler), so
/// what a view's build notifies is delivered once the build is whole.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a platform",
  label = "a platform is a zero-sized type realizing the portable vocabulary: boxes, runs, anchors, scopes, tokens and events",
  note = "`Painted` and embeds are outside `Platform`; a platform offers them through its own extension traits"
)]
pub trait Platform: Copy + Default + 'static {
  /// The platform handle a node owns: an element's index, a layout id, a record's.
  ///
  /// Copied out of the scene before the platform is called with it, so a platform call
  /// never runs under the scene's borrow and may reach the scene itself.
  type Sink: Copy + Eq + Hash + Debug + 'static;
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
    + From<Definition<Length, Self::Scalar>>
    + From<Definition<Dimension<Self::Scalar>, Self::Scalar>>
    + 'static;

  /// The thread-local holding this platform's scene.
  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>>;

  /// Tells the platform the id the scene minted for `sink`, once, right after the sink
  /// was created.
  #[inline(always)]
  fn bind_node(_sink: Self::Sink, _node: NodeId) {}

  /// Realizes a box.
  fn create_box(props: &BoxProps<Self>) -> Self::Sink;

  /// Writes the leaf a derived leaf computed into its slot on a box, or in the layout a
  /// wrapper gave a run or a low-level node: a value, or a token or expression the
  /// platform resolves where it reads the slot.
  fn update_box(
    sink: Self::Sink,
    slot: Slot,
    leaf: &StaticLeaf<BoxValue<Self::Scalar, Self::StyleExt>>,
  );

  /// Replaces a box's place in focus traversal; `None` takes it out of focus.
  fn update_focus_order(sink: Self::Sink, focus_order: Option<FocusOrder>);

  /// Replaces the accessibility properties a box reports.
  fn update_access(sink: Self::Sink, access: &[AccessProp]);

  /// Realizes a control: a box a platform may stand a native element in for.
  fn create_control(props: &ControlProps<Self>) -> Self::Sink;

  /// Mirrors a control's state, for a platform whose native element holds it too.
  fn set_control_state(sink: Self::Sink, state: &ControlState);

  /// Realizes an overlay: a box shown above the app while open, closed at first.
  fn create_overlay(props: &OverlayProps<Self>) -> Self::Sink;

  /// Opens or closes an overlay; an open overlay placed against `anchor` sits by it
  /// rather than by the viewport.
  fn set_overlay_open(sink: Self::Sink, open: bool, anchor: Option<Self::Sink>);

  /// Replaces where an overlay sits; an open overlay is re-placed by the
  /// [`set_overlay_open`](Self::set_overlay_open) that follows.
  #[inline(always)]
  fn set_overlay_placement(_sink: Self::Sink, _placement: Placement) {}

  /// Pins an overlay's top-start corner to `position`, in the units a pointer event
  /// reports, or unpins it for `None`; a pinned overlay ignores its placement and
  /// anchor, and is shifted to stay within the viewport. An open overlay moves at the
  /// [`set_overlay_open`](Self::set_overlay_open) that follows.
  #[inline(always)]
  fn set_overlay_position(_sink: Self::Sink, _position: Option<Point<f32>>) {}

  /// Gives the node a low-level view made at `sink` the per-child layout a wrapper
  /// annotated it with: its flexibility, alignment, pinning and grid placement, as a
  /// box style holding nothing else, each derived leaf a slot
  /// [`update_box`](Self::update_box) writes. A platform that lays out from the
  /// scene's annotations needs nothing here.
  #[inline(always)]
  fn apply_layout(_sink: Self::Sink, _layout: &StaticBoxStyle<Self>) {}

  /// Realizes a text run.
  fn create_run(props: &RunProps<Self>) -> Self::Sink;

  /// Writes the leaf a derived leaf computed into its slot on a run: a value, or a
  /// token or expression the platform resolves where it reads the slot.
  fn update_run(sink: Self::Sink, slot: Slot, leaf: &StaticLeaf<RunValue<Self::StyleExt>>);

  /// Replaces a run's text.
  fn set_text(sink: Self::Sink, text: &str);

  /// Realizes an anchor: a layout-invisible marker a reactive subtree mounts between.
  fn create_anchor() -> Self::Sink;

  /// Realizes a scope: a layout-invisible node token definitions hang off.
  fn create_scope() -> Self::Sink;

  /// Places `child` under `parent`, immediately before `before` when given, else last.
  fn attach(parent: Self::Sink, child: Self::Sink, before: Option<Self::Sink>);

  /// Removes `child` from under `parent`.
  fn detach(parent: Self::Sink, child: Self::Sink);

  /// Releases a sink the scene no longer holds.
  fn destroy(sink: Self::Sink);

  /// Defines tokens on a scope, replacing what the scope defined before.
  fn define_tokens(scope: Self::Sink, definitions: &Definitions<Self::TokenValue>);

  /// Defines the tokens every node falls back to, replacing the previous set.
  fn define_root_tokens(definitions: &Definitions<Self::TokenValue>);

  /// Notes that a node has a handler for `kind`; dispatch itself is core's.
  fn listen(sink: Self::Sink, kind: EventKind);

  /// `event` with its local position measured from `sink`'s top-left corner; unchanged
  /// on a platform that lays nothing out.
  #[inline(always)]
  fn localize_pointer(_sink: Self::Sink, event: &PointerEvent) -> PointerEvent {
    *event
  }

  /// Moves focus to a node.
  fn focus(sink: Self::Sink);

  /// Takes focus off a node.
  fn blur(sink: Self::Sink);

  /// Writes the options a platform-drawn picker offers, in order; a platform without
  /// one ignores them.
  #[inline(always)]
  fn set_options(_sink: Self::Sink, _options: &[PickerOption]) {}

  /// Writes a control's textual value; a focused entry keeps its caret, clamped into
  /// the value.
  fn set_value(sink: Self::Sink, value: &str);

  /// Writes a text entry's selection.
  fn set_selection(sink: Self::Sink, selection: TextSelection);

  /// Writes a control's checked state.
  fn set_checked(sink: Self::Sink, checked: bool);

  /// Reports a node's measured size to `report`, now and whenever it changes, until
  /// the observation this returns is stopped or the node is destroyed.
  fn observe_size(sink: Self::Sink, report: Rc<dyn Fn(Size<Self::Scalar>)>) -> Observation;

  /// The environment as signals.
  fn media() -> MediaSignals<Self::Scalar>;

  /// The signal that turns true once a render made without script has been hydrated,
  /// and stays false on a render that never is; `None` where the tree is live from
  /// the start.
  ///
  /// Only a platform with a scriptless render answers: the web leaves, and the headless
  /// platform standing in for them under a test. Every other platform keeps the default.
  #[inline(always)]
  #[must_use]
  fn hydration() -> Option<ReadSignal<bool>> {
    None
  }

  /// Puts `scheme` in force throughout, overriding the platform's preference and
  /// touching no node; `None` returns to following the platform.
  fn set_scheme(scheme: Option<ColorScheme>);

  /// The scheme the app put in force, or a previous run stored, when there is one.
  fn stored_scheme() -> Option<ColorScheme>;

  /// Runs `action` once `delay` has passed on the platform's clock, unless cancelled
  /// first; a platform without a clock never runs it.
  fn schedule(delay: Duration, action: Box<dyn FnOnce()>) -> Timer;

  /// Drops the action `timer` names, if it has not run.
  fn cancel(timer: Timer);

  /// Whether `action` can be offered on what the platform holds selected or focused
  /// right now; a platform without a clipboard offers nothing.
  #[inline(always)]
  fn clipboard_availability(_action: ClipboardAction) -> Availability {
    Availability::Absent
  }

  /// Performs `action` on what the platform holds selected or focused, when it is
  /// [`Enabled`](Availability::Enabled); a platform without a clipboard does nothing.
  #[inline(always)]
  fn clipboard(_action: ClipboardAction) {}

  /// Whether `media` holds, as a memo following the environment; a container condition
  /// never holds here, since it is answered per node by the box it names.
  ///
  /// [`MediaSignals::watch`] answers it from the platform's signals.
  fn watch_media(media: &Media<Self::Scalar>) -> Memo<bool>;
}

#[cfg(test)]
mod tests {
  use super::Timers;

  /// Actions run earliest due first, those due together in the order scheduled, and
  /// none before its reading.
  #[test]
  fn timers_pop_the_earliest_due() {
    let mut timers = Timers::new();

    let _ = timers.schedule(2., "late");
    let _ = timers.schedule(1., "first");
    let _ = timers.schedule(1., "second");
    let _ = timers.schedule(3., "later");

    let batch = timers.next_timer();

    assert_eq!(timers.pop_due(2., batch), Some("first"));
    assert_eq!(timers.pop_due(2., batch), Some("second"));
    assert_eq!(timers.pop_due(2., batch), Some("late"));
    assert_eq!(timers.pop_due(2., batch), None);
    assert_eq!(timers.len(), 1);
    assert_eq!(timers.next_due(), Some(3.));
  }

  /// A cancelled action never runs, and a batch leaves out what was scheduled after it
  /// began.
  #[test]
  fn a_cancelled_or_later_action_stays_out_of_a_batch() {
    let mut timers = Timers::new();
    let cancelled = timers.schedule(0., "cancelled");

    let _ = timers.schedule(0., "kept");

    let batch = timers.next_timer();

    let _ = timers.schedule(0., "after");

    assert!(timers.cancel(cancelled));
    assert!(!timers.cancel(cancelled));
    assert_eq!(timers.pop_due(0., batch), Some("kept"));
    assert_eq!(timers.pop_due(0., batch), None);
    assert_eq!(timers.pop_due(0., timers.next_timer()), Some("after"));
    assert!(timers.is_empty());
  }
}
