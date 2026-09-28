//! # Headless platform
//!
//! The host platform: realizes the portable vocabulary into an inspectable record store,
//! so view logic tests run without a platform. Every mirrored call is bookkeeping, not
//! emulation: no layout runs, and no platform response fires. Tests build views exactly
//! as an app does, then inspect the records ([`outline`], [`box_props`], [`slot_leaf`],
//! [`resolve_color`]) and drive events through the `trigger_` seams, each routed through
//! core's dispatch so bubbling is the one implementation every platform shares.
//!
//! The environment starts at a 1024 by 768 viewport, the light scheme and no reduced
//! motion; [`set_viewport`], [`set_scheme`] and [`set_reduced_motion`] move it,
//! [`choose_scheme`] lets the app override the platform's scheme, and the signals
//! [`Platform::media`] reports follow.
//!
//! Motion runs on a clock that stands still until [`advance`] steps it: a property
//! whose resolved value moved under a transition, or that an animation sets, resolves
//! to the in-flight value at the clock's reading, so [`resolve_color`] and
//! [`resolve_length`] read what a frame would show.
//!
//! A control is realized as a box carrying its [`ControlProps`], an overlay as one
//! carrying its [`OverlayProps`], an open flag ([`is_open`]) and the anchor it was last
//! opened against ([`overlay_anchor`]); [`control_state`],
//! [`part`], [`focus_order`], [`is_tab_stop`] and [`accessible_name`] read what the
//! control and its parts report, and [`trigger_size`] gives a control the measured size
//! its pointer handling maps a position onto. A painted box records that a painter was
//! bound to it ([`is_painted`]); nothing is drawn.
//!
//! Tasks run on an executor that stands still until [`pump_tasks`] drains it:
//! [`install_executor`] makes it the thread's spawner ([`reset`] does too), so a
//! `Resource` built in a test loads when the test pumps.

mod resolve;
mod store;

use std::{cell::RefCell, fmt::Write as _, rc::Rc, thread::LocalKey, time::Duration};

use skene_common::gui::{Canvas, Painter, Painting};
use skene_core::{
  AccessProp,
  Availability,
  Border,
  BoxProps,
  BoxValue,
  ClipboardAction,
  Color,
  ColorScheme,
  ContainerName,
  ControlProps,
  ControlState,
  Definitions,
  Dimension,
  EventKind,
  FocusOrder,
  Inset,
  KeyEvent,
  MediaSignals,
  NoProperty,
  NodeId,
  NodeRecord,
  OverlayProps,
  Part,
  Placement,
  Platform,
  Point,
  PointerEvent,
  Prevented,
  PropertyKey,
  RunProps,
  RunValue,
  Scene,
  ScrollEvent,
  Size,
  Slot,
  Spacing,
  StaticLeaf,
  TextSelection,
  Timer,
  dispatch_click,
  dispatch_context_menu,
  dispatch_key_down,
  dispatch_key_up,
  dispatch_pointer_down,
  dispatch_pointer_move,
  dispatch_pointer_up,
  notify_blur,
  notify_change,
  notify_dismiss,
  notify_focus,
  notify_pointer_enter,
  notify_pointer_leave,
  notify_scroll,
  notify_selection_change,
  notify_toggle,
};
use skene_reactive::{ReadSignal, Set};
use unicode_segmentation::UnicodeSegmentation;

pub use crate::store::HeadElement;
use crate::store::{Kind, Record, Store};

/// The host platform: a zero-sized [`Platform`] whose sink is a [`Handle`] into this
/// crate's record store.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Headless;

/// The platform's sink: the index of a node's record and the generation it was minted
/// in, so a handle whose record was destroyed reaches nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub struct Handle {
  index: u32,
  generation: u32,
}

impl Handle {
  #[inline(always)]
  const fn new(index: u32, generation: u32) -> Self {
    Self { index, generation }
  }

  /// The record's index.
  #[inline(always)]
  #[must_use]
  pub const fn index(self) -> u32 {
    self.index
  }

  /// The generation the handle was minted in.
  #[inline(always)]
  #[must_use]
  pub const fn generation(self) -> u32 {
    self.generation
  }
}

/// The erased token definition the headless platform stores per token; see
/// [`skene_core::TokenValue`].
pub type TokenValue = skene_core::TokenValue<f32>;

/// A property's value erased over the property; see [`skene_core::SlotValue`].
pub type SlotValue = skene_core::SlotValue<f32>;

/// What a derived leaf last wrote into a node's slot; see [`skene_core::SlotLeaf`].
pub type SlotLeaf = skene_core::SlotLeaf<f32>;

thread_local! {
  static SCENE: RefCell<Scene<Headless>> = RefCell::new(Scene::new());
  static STORE: RefCell<Store> = RefCell::new(Store::default());
  /// The flush the turn's writes scheduled, run when the turn ends.
  static TURN: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}

/// Ends the turn: runs the flush the writes since the last turn's end scheduled, so
/// their effects have taken hold.
///
/// Every `trigger_` and setter here is a turn of its own and every query ends the
/// pending one first; a test calls this where it observes a direct write's effects
/// some other way.
pub fn end_turn() {
  let flush = TURN.with(|turn| turn.borrow_mut().take());

  if let Some(flush) = flush {
    flush();
  }
}

/// Runs `event` as one turn: what earlier writes scheduled takes effect first, and
/// what the event's own writes schedule once it is handled.
fn trigger<R>(event: impl FnOnce() -> R) -> R {
  end_turn();

  let outcome = event();

  end_turn();

  outcome
}

/// Runs `operation` on the store; nothing of the user's runs inside it.
fn with_store<R>(operation: impl FnOnce(&mut Store) -> R) -> R {
  STORE.with(|store| operation(&mut store.borrow_mut()))
}

/// Runs `operation` on the record of `handle`, if it is live.
fn with_record<R>(handle: Handle, operation: impl FnOnce(&mut Record) -> R) -> Option<R> {
  with_store(|store| store.record_mut(handle).map(operation))
}

/// The handle of `node`, if it is live.
#[must_use]
pub fn handle(node: NodeId) -> Option<Handle> {
  end_turn();

  Scene::<Headless>::with(|scene| scene.record(node).map(|record| *record.sink()))
}

impl Platform for Headless {
  type Scalar = f32;
  type Sink = Handle;
  type StyleExt = ();
  type TokenValue = TokenValue;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create_box(&self, props: &BoxProps<Self>) -> Handle {
    with_store(|store| store.create(Record::new(Kind::Box(Box::new(props.clone())))))
  }

  fn update_box(&self, sink: &Handle, slot: Slot, leaf: &StaticLeaf<BoxValue<f32>>) {
    with_record(*sink, |record| {
      record.slots.insert(slot, leaf.clone().map(SlotValue::Box));
    });
  }

  fn update_focus_order(&self, sink: &Handle, focus_order: Option<FocusOrder>) {
    with_record(*sink, |record| {
      if let Some(props) = record.kind.box_props_mut() {
        props.focus_order = focus_order;
      }
    });
  }

  fn update_access(&self, sink: &Handle, access: &[AccessProp]) {
    with_record(*sink, |record| {
      if let Some(props) = record.kind.box_props_mut() {
        access.clone_into(&mut props.access);
      }
    });
  }

  fn create_control(&self, props: &ControlProps<Self>) -> Handle {
    with_store(|store| store.create(Record::new(Kind::Control(Box::new(props.clone())))))
  }

  fn set_control_state(&self, sink: &Handle, state: &ControlState) {
    with_record(*sink, |record| record.control_state = *state);
  }

  fn create_overlay(&self, props: &OverlayProps<Self>) -> Handle {
    with_store(|store| store.create(Record::new(Kind::Overlay(Box::new(props.clone())))))
  }

  fn set_overlay_open(&self, sink: &Handle, open: bool, anchor: Option<&Handle>) {
    with_record(*sink, |record| {
      record.open = open;
      record.anchor = anchor.copied();
    });
  }

  fn set_overlay_placement(&self, sink: &Handle, placement: Placement) {
    with_record(*sink, |record| {
      if let Kind::Overlay(props) = &mut record.kind {
        props.placement = placement;
      }
    });
  }

  fn set_overlay_position(&self, sink: &Handle, position: Option<Point<f32>>) {
    with_record(*sink, |record| record.position = position);
  }

  fn create_run(&self, props: &RunProps<Self>) -> Handle {
    with_store(|store| store.create(Record::new(Kind::Run(Box::new(props.clone())))))
  }

  fn update_run(&self, sink: &Handle, slot: Slot, leaf: &StaticLeaf<RunValue>) {
    with_record(*sink, |record| {
      record.slots.insert(slot, leaf.clone().map(SlotValue::Run));
    });
  }

  fn set_text(&self, sink: &Handle, text: &str) {
    with_record(*sink, |record| text.clone_into(&mut record.text));
  }

  fn create_anchor(&self) -> Handle {
    with_store(|store| store.create(Record::new(Kind::Anchor)))
  }

  fn create_scope(&self) -> Handle {
    with_store(|store| store.create(Record::new(Kind::Scope)))
  }

  fn attach(&self, parent: &Handle, child: &Handle, before: Option<&Handle>) {
    with_record(*parent, |record| {
      record.children.retain(|existing| existing != child);

      let index = before
        .and_then(|before| record.children.iter().position(|node| node == before))
        .unwrap_or(record.children.len());

      record.children.insert(index, *child);
    });
  }

  fn detach(&self, parent: &Handle, child: &Handle) {
    with_record(*parent, |record| {
      record.children.retain(|existing| existing != child);
    });
  }

  fn destroy(&self, sink: Handle) {
    with_store(|store| store.destroy(sink));
  }

  fn define_tokens(&self, scope: &Handle, definitions: &Definitions<TokenValue>) {
    with_record(*scope, |record| {
      record.definitions = Some(definitions.clone());
    });
  }

  fn define_root_tokens(&self, definitions: &Definitions<TokenValue>) {
    with_store(|store| {
      store.root_definitions = definitions.clone();
      store.root_definition_writes += 1;
    });
  }

  fn listen(&self, sink: &Handle, kind: EventKind) {
    with_record(*sink, |record| {
      if !record.listened.contains(&kind) {
        record.listened.push(kind);
      }
    });
  }

  fn focus(&self, sink: &Handle) {
    with_store(|store| store.focused = Some(*sink));
  }

  fn blur(&self, sink: &Handle) {
    with_store(|store| {
      if store.focused == Some(*sink) {
        store.focused = None;
      }
    });
  }

  fn set_value(&self, sink: &Handle, value: &str) {
    with_record(*sink, |record| value.clone_into(&mut record.value));
  }

  fn set_options(&self, sink: &Handle, options: &[(String, String)]) {
    with_record(*sink, |record| options.clone_into(&mut record.options));
  }

  fn set_selection(&self, sink: &Handle, selection: TextSelection) {
    with_record(*sink, |record| {
      record.selection = selection.clamped(record.value.graphemes(true).count());
    });
  }

  fn set_checked(&self, sink: &Handle, checked: bool) {
    with_record(*sink, |record| record.checked = checked);
  }

  fn observe_size(&self, sink: &Handle, report: Rc<dyn Fn(Size<f32>)>) {
    with_record(*sink, |record| record.size_observers.push(report));
  }

  fn media(&self) -> MediaSignals<f32> {
    with_store(|store| store.media().signals)
  }

  fn hydration(&self) -> Option<ReadSignal<bool>> {
    Some(with_store(|store| store.media().hydrated))
  }

  fn set_scheme(&self, scheme: Option<ColorScheme>) {
    record_chosen_scheme(scheme);
  }

  fn schedule(&self, delay: Duration, action: Box<dyn FnOnce()>) -> Timer {
    with_store(|store| {
      let timer = Timer(store.next_timer);

      store.next_timer += 1;
      store
        .timers
        .push((store.clock + delay.as_secs_f64(), timer, action));

      timer
    })
  }

  fn cancel(&self, timer: Timer) {
    with_store(|store| store.timers.retain(|(_, held, _)| *held != timer));
  }

  fn clipboard_availability(&self, action: ClipboardAction) -> Availability {
    with_store(|store| {
      store
        .availability
        .get(&action)
        .copied()
        .unwrap_or(Availability::Enabled)
    })
  }

  fn clipboard(&self, action: ClipboardAction) {
    with_store(|store| store.clipboard.push(action));
  }

  fn stored_scheme(&self) -> Option<ColorScheme> {
    chosen_scheme()
  }
}

impl Painting for Headless {
  fn paint(&self, sink: &Handle, painter: Box<Painter>) {
    with_record(*sink, |record| record.painter = Some(Rc::from(painter)));
  }

  fn available(&self) -> ReadSignal<bool> {
    with_store(|store| store.media().graphics)
  }
}

/// Whether a paint callback was handed to the box `node` through [`Painting`].
#[must_use]
pub fn is_painted(node: NodeId) -> bool {
  end_turn();

  handle(node)
    .is_some_and(|handle| with_record(handle, |record| record.painter.is_some()) == Some(true))
}

/// Paints the box `node` at `size` logical pixels, one device pixel each, and returns
/// the unpremultiplied RGBA8 pixels row by row; `None` for a box nothing paints.
#[must_use]
pub fn paint(node: NodeId, size: Size<f32>) -> Option<Vec<u8>> {
  end_turn();

  let painter =
    handle(node).and_then(|handle| with_record(handle, |record| record.painter.clone()))??;
  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  let mut canvas = Canvas::new(size.width as u16, size.height as u16);

  painter(&mut canvas, size);

  Some(canvas.pixels())
}

/// Empties the scene, the store and the task queue, returns the environment to its
/// defaults, installs the executor and holds every flush until its turn ends.
pub fn reset() {
  TURN.with(|turn| *turn.borrow_mut() = None);
  skene_reactive::Runtime::set_scheduler(|flush| {
    TURN.with(|turn| *turn.borrow_mut() = Some(flush));
  });
  SCENE.with(|scene| *scene.borrow_mut() = Scene::new());

  let owner = with_store(Store::reset);

  if let Some(owner) = owner {
    owner.dispose();
  }

  skene_core::executor::clear();
  install_executor();
}

/// Installs the headless executor as this thread's spawner: every task `spawn` and
/// `Resource` start on this thread queues here until [`pump_tasks`] polls it.
pub fn install_executor() {
  skene_core::executor::install();
}

/// Polls every queued task until none is ready, so a resource whose loader is ready
/// lands its value; `true` when any task ran.
#[must_use]
pub fn pump_tasks() -> bool {
  trigger(skene_core::executor::pump)
}

/// Sets the document title.
pub fn set_document_title(title: &str) {
  with_store(|store| store.title = Some(title.to_owned()));
}

/// Sets the document's language, a BCP 47 tag.
pub fn set_document_language(language: &str) {
  with_store(|store| store.language = Some(language.to_owned()));
}

/// Registers an element for the document head.
pub fn add_head_element(tag: &str, attributes: &[(&str, &str)]) {
  let element = HeadElement {
    tag: tag.to_owned(),
    attributes: attributes
      .iter()
      .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
      .collect(),
  };

  with_store(|store| store.head.push(element));
}

/// The document title last set.
#[must_use]
pub fn document_title() -> Option<String> {
  end_turn();

  with_store(|store| store.title.clone())
}

/// The document language last set.
#[must_use]
pub fn document_language() -> Option<String> {
  end_turn();

  with_store(|store| store.language.clone())
}

/// The elements registered for the document head, in order.
#[must_use]
pub fn head_elements() -> Vec<HeadElement> {
  end_turn();

  with_store(|store| store.head.clone())
}

/// Dispatches a click at `node`, bubbling through the scene.
pub fn trigger_click(node: NodeId) -> Prevented {
  trigger(|| dispatch_click::<Headless>(node))
}

/// Dispatches a key press at `node`, bubbling through the scene; focus shows from
/// here on.
pub fn trigger_key_down(node: NodeId, event: &KeyEvent) -> Prevented {
  trigger(|| {
    with_store(|store| store.focus_visible = true);

    dispatch_key_down::<Headless>(node, event)
  })
}

/// Dispatches a key release at `node`, bubbling through the scene.
pub fn trigger_key_up(node: NodeId, event: &KeyEvent) -> Prevented {
  trigger(|| dispatch_key_up::<Headless>(node, event))
}

/// Dispatches a pointer press at `node`, bubbling through the scene; focus stops
/// showing from here on.
pub fn trigger_pointer_down(node: NodeId, event: &PointerEvent) -> Prevented {
  trigger(|| {
    with_store(|store| store.focus_visible = false);

    dispatch_pointer_down::<Headless>(node, event)
  })
}

/// Dispatches a pointer release at `node`, bubbling through the scene.
pub fn trigger_pointer_up(node: NodeId, event: &PointerEvent) -> Prevented {
  trigger(|| dispatch_pointer_up::<Headless>(node, event))
}

/// Dispatches a pointer move at `node`, bubbling through the scene.
pub fn trigger_pointer_move(node: NodeId, event: &PointerEvent) -> Prevented {
  trigger(|| dispatch_pointer_move::<Headless>(node, event))
}

/// Marks `node` hovered and notifies its pointer-enter handlers.
pub fn trigger_pointer_enter(node: NodeId) {
  trigger(|| {
    mark_hovered(node, true);
    notify_pointer_enter::<Headless>(node);
  });
}

/// Marks `node` no longer hovered and notifies its pointer-leave handlers.
pub fn trigger_pointer_leave(node: NodeId) {
  trigger(|| {
    mark_hovered(node, false);
    notify_pointer_leave::<Headless>(node);
  });
}

/// Moves focus to `node` and notifies its focus handlers; no blur is implied on the
/// node focus left.
pub fn trigger_focus(node: NodeId) {
  trigger(|| {
    move_focus(Some(node));
    notify_focus::<Headless>(node);
  });
}

/// Takes focus off `node`, if it holds it, and notifies its blur handlers.
pub fn trigger_blur(node: NodeId) {
  trigger(|| {
    if let Some(handle) = handle(node) {
      Headless.blur(&handle);
    }

    notify_blur::<Headless>(node);
  });
}

/// Records `value` as the node's textual value and notifies its change handlers.
pub fn trigger_change(node: NodeId, value: &str) {
  trigger(|| {
    if let Some(handle) = handle(node) {
      Headless.set_value(&handle, value);
    }

    notify_change::<Headless>(node, value);
  });
}

/// Records `selection` as the node's selection and notifies its selection handlers.
pub fn trigger_selection_change(node: NodeId, selection: TextSelection) {
  trigger(|| {
    if let Some(handle) = handle(node) {
      Headless.set_selection(&handle, selection);
    }

    notify_selection_change::<Headless>(node, selection);
  });
}

/// Records `checked` as the node's checked state and notifies its toggle handlers.
pub fn trigger_toggle(node: NodeId, checked: bool) {
  trigger(|| {
    if let Some(handle) = handle(node) {
      Headless.set_checked(&handle, checked);
    }

    notify_toggle::<Headless>(node, checked);
  });
}

/// Notifies `node` of its new scroll offsets.
pub fn trigger_scroll(node: NodeId, event: &ScrollEvent) {
  trigger(|| notify_scroll::<Headless>(node, event));
}

/// Notifies the overlay `node` that the platform asked it to close; it stays open until
/// the app closes it.
pub fn trigger_dismiss(node: NodeId) {
  trigger(|| notify_dismiss::<Headless>(node));
}

/// Dispatches a context menu request at `node`, as a secondary press there or the
/// menu key would; the verdict says whether the platform's own menu was prevented.
pub fn trigger_context_menu(node: NodeId, event: &PointerEvent) -> Prevented {
  trigger(|| dispatch_context_menu::<Headless>(node, event))
}

/// The clipboard actions performed since the last call, in order.
#[must_use]
pub fn take_clipboard_actions() -> Vec<ClipboardAction> {
  end_turn();

  with_store(|store| std::mem::take(&mut store.clipboard))
}

/// Sets what `action` answers when a view asks whether it can be offered; enabled
/// unless set.
pub fn set_clipboard_availability(action: ClipboardAction, availability: Availability) {
  with_store(|store| {
    store.availability.insert(action, availability);
  });
}

/// Reports `size` as the measured size of `node` to every observer bound to it; a box
/// named for container queries also publishes it as that container's size.
pub fn trigger_size(node: NodeId, size: Size<f32>) {
  trigger(|| {
    let (handle, name) = Scene::<Headless>::with(|scene| {
      let record = scene.record(node);

      (
        record.map(|record| *record.sink()),
        record.and_then(NodeRecord::container),
      )
    });

    if let Some(name) = name {
      record_container_size(name, size);
    }

    let observers = handle
      .and_then(|handle| with_record(handle, |record| record.size_observers.clone()))
      .unwrap_or_default();

    for observer in observers {
      observer(size);
    }
  });
}

/// Sets the viewport's size.
pub fn set_viewport(size: Size<f32>) {
  trigger(|| {
    let write = with_store(|store| {
      store.viewport = Some(size);

      store.media().set_viewport
    });

    write.set(size);
  });
}

/// Sets the platform's color scheme; the scheme in force follows it unless the app
/// chose one through [`choose_scheme`].
pub fn set_scheme(scheme: ColorScheme) {
  trigger(|| {
    let (write, in_force) = with_store(|store| {
      store.scheme = Some(scheme);

      (store.media().set_scheme, store.scheme_in_force())
    });

    write.set(in_force);
  });
}

/// Sets the scheme the app chose, overriding the platform's; `None` returns to
/// following the platform.
pub fn choose_scheme(scheme: Option<ColorScheme>) {
  trigger(|| record_chosen_scheme(scheme));
}

/// Records the scheme the app chose, the scheme in force following.
fn record_chosen_scheme(scheme: Option<ColorScheme>) {
  let (write, in_force) = with_store(|store| {
    store.chosen_scheme = scheme;

    (store.media().set_scheme, store.scheme_in_force())
  });

  write.set(in_force);
}

/// The scheme the app chose through [`choose_scheme`], if any.
#[must_use]
pub fn chosen_scheme() -> Option<ColorScheme> {
  end_turn();

  with_store(|store| store.chosen_scheme)
}

/// Sets whether the render stands hydrated, as the web leaves answer it: false
/// stands in for a server render or a hydration still adopting, true for a live page;
/// hydrated until set.
pub fn set_hydrated(hydrated: bool) {
  trigger(|| {
    let write = with_store(|store| {
      store.hydrated = Some(hydrated);

      store.media().set_hydrated
    });

    write.set(hydrated);
  });
}

/// Sets whether a painted box shows as pixels; shown until set.
pub fn set_graphics(shown: bool) {
  trigger(|| {
    let write = with_store(|store| {
      store.graphics = Some(shown);

      store.media().set_graphics
    });

    write.set(shown);
  });
}

/// Sets whether the reader prefers reduced motion.
pub fn set_reduced_motion(reduced: bool) {
  trigger(|| {
    let write = with_store(|store| {
      store.reduced_motion = Some(reduced);

      store.media().set_reduced_motion
    });

    write.set(reduced);
  });
}

/// How many scheduled actions are still waiting for the clock, for a test asserting that
/// a view cancels its own.
#[must_use]
pub fn pending_timers() -> usize {
  end_turn();

  with_store(|store| store.timers.len())
}

/// Passes a frame: every property a resolution has observed is judged again at the
/// clock's reading, so a value that moved since starts its transition there, and the
/// clock then steps forward by `elapsed`; the next resolution samples every transition
/// and animation at the new reading.
pub fn advance(elapsed: std::time::Duration) {
  trigger(|| {
    let tracked = with_store(|store| store.tracked());

    for (node, keys) in tracked {
      for key in keys {
        let _ = resolve::resolve(node, key);
      }
    }

    let due = with_store(|store| {
      store.clock += elapsed.as_secs_f64();

      let now = store.clock;
      let (due, pending): (Vec<_>, Vec<_>) = std::mem::take(&mut store.timers)
        .into_iter()
        .partition(|(at, ..)| *at <= now);

      store.timers = pending;

      due
    });

    for (_, _, action) in due {
      action();
    }
  });
}

/// Sets the measured size of the query container `name`.
pub fn set_container_size(name: ContainerName, size: Size<f32>) {
  trigger(|| record_container_size(name, size));
}

/// Records the measured size of the query container `name`.
fn record_container_size(name: ContainerName, size: Size<f32>) {
  let sizes = with_store(|store| {
    store.container_sizes.insert(name, size);

    store.media().signals.container_sizes
  });

  sizes.set(name, size);
}

/// Sets whether the pointer is over `node`.
pub fn set_hovered(node: NodeId, hovered: bool) {
  trigger(|| mark_hovered(node, hovered));
}

/// Records whether the pointer is over `node`.
fn mark_hovered(node: NodeId, hovered: bool) {
  if let Some(handle) = handle(node) {
    with_record(handle, |record| record.hovered = hovered);
  }
}

/// Moves focus to `node`, or clears it for `None`.
pub fn set_focused(node: Option<NodeId>) {
  trigger(|| move_focus(node));
}

/// Records focus on `node`, or on nothing for `None`.
fn move_focus(node: Option<NodeId>) {
  let handle = node.and_then(handle);

  with_store(|store| store.focused = handle);
}

/// Whether the pointer is over `node`.
#[must_use]
pub fn is_hovered(node: NodeId) -> bool {
  end_turn();

  handle(node).is_some_and(|handle| with_record(handle, |record| record.hovered) == Some(true))
}

/// Whether `node` holds focus.
#[must_use]
pub fn is_focused(node: NodeId) -> bool {
  end_turn();

  let handle = handle(node);

  handle.is_some() && with_store(|store| store.focused == handle)
}

/// The tree under `node` as the layout sees it: one line per box or run, indented by
/// depth, a run followed by its text; anchors and scopes are elided.
#[must_use]
pub fn outline(node: NodeId) -> String {
  end_turn();

  render_outline(node, false)
}

/// The tree under `node` with every node shown, anchors and scopes included.
#[must_use]
pub fn full_outline(node: NodeId) -> String {
  end_turn();

  render_outline(node, true)
}

fn render_outline(node: NodeId, full: bool) -> String {
  let mut out = String::new();

  if let Some(handle) = handle(node) {
    outline_into(handle, 0, full, &mut out);
  }

  out
}

fn outline_into(handle: Handle, depth: usize, full: bool, out: &mut String) {
  let Some((line, children)) = with_store(|store| {
    let record = store.record(handle)?;
    let line = match &record.kind {
      Kind::Box(_) => Some("Box".to_owned()),
      Kind::Control(props) => Some(format!("Control {:?}", props.kind)),
      Kind::Overlay(props) => Some(format!(
        "Overlay {}",
        if props.modal { "modal" } else { "popup" }
      )),
      Kind::Run(_) => Some(format!("Run {:?}", record.text)),
      Kind::Anchor => full.then(|| "Anchor".to_owned()),
      Kind::Scope => full.then(|| "Scope".to_owned()),
    };

    Some((line, record.children.clone()))
  })
  else {
    return;
  };
  let child_depth = if let Some(line) = line {
    for _ in 0..depth {
      out.push_str("  ");
    }

    let _ = writeln!(out, "{line}");

    depth + 1
  }
  else {
    depth
  };

  for child in children {
    outline_into(child, child_depth, full, out);
  }
}

/// Whether `node` is live in the scene, attached or not.
#[must_use]
pub fn is_live(node: NodeId) -> bool {
  end_turn();

  handle(node).is_some()
}

/// The props the box or control `node` was created with, carrying the focus order and
/// accessibility properties last written to it.
#[must_use]
pub fn box_props(node: NodeId) -> Option<BoxProps<Headless>> {
  end_turn();

  with_record(handle(node)?, |record| record.kind.box_props().cloned())?
}

/// The props the control `node` was created with.
#[must_use]
pub fn control_props(node: NodeId) -> Option<ControlProps<Headless>> {
  end_turn();

  with_record(handle(node)?, |record| match &record.kind {
    Kind::Control(props) => Some((**props).clone()),
    Kind::Box(_) | Kind::Overlay(_) | Kind::Run(_) | Kind::Anchor | Kind::Scope => None,
  })?
}

/// The props the overlay `node` was created with.
#[must_use]
pub fn overlay_props(node: NodeId) -> Option<OverlayProps<Headless>> {
  end_turn();

  with_record(handle(node)?, |record| match &record.kind {
    Kind::Overlay(props) => Some((**props).clone()),
    Kind::Box(_) | Kind::Control(_) | Kind::Run(_) | Kind::Anchor | Kind::Scope => None,
  })?
}

/// Whether the overlay `node` is open.
#[must_use]
pub fn is_open(node: NodeId) -> bool {
  end_turn();

  handle(node).is_some_and(|handle| with_record(handle, |record| record.open) == Some(true))
}

/// The anchor the overlay `node` was last opened or closed against.
#[must_use]
pub fn overlay_anchor(node: NodeId) -> Option<Handle> {
  end_turn();

  with_record(handle(node)?, |record| record.anchor)?
}

/// Where the overlay `node` is pinned, if it is.
#[must_use]
pub fn overlay_position(node: NodeId) -> Option<Point<f32>> {
  end_turn();

  with_record(handle(node)?, |record| record.position)?
}

/// The state last mirrored into the control `node`.
#[must_use]
pub fn control_state(node: NodeId) -> Option<ControlState> {
  end_turn();

  with_record(handle(node)?, |record| match record.kind {
    Kind::Control(_) => Some(record.control_state),
    Kind::Box(_) | Kind::Overlay(_) | Kind::Run(_) | Kind::Anchor | Kind::Scope => None,
  })?
}

/// The part of its enclosing control `node` is recorded as; a control node is its own
/// [`Part::Root`].
#[must_use]
pub fn part(node: NodeId) -> Option<Part> {
  end_turn();

  Scene::<Headless>::with(|scene| scene.part(node))
}

/// The place in focus traversal last written to the box or control `node`, or a link
/// run's automatic one; `None` when it takes no focus.
#[must_use]
pub fn focus_order(node: NodeId) -> Option<FocusOrder> {
  end_turn();

  match box_props(node) {
    Some(props) => props.focus_order,
    None => run_props(node)?.link.map(|_| FocusOrder::Auto),
  }
}

/// Whether sequential focus traversal reaches `node`: it takes focus and is not
/// skipped.
#[must_use]
pub fn is_tab_stop(node: NodeId) -> bool {
  end_turn();

  matches!(
    focus_order(node),
    Some(FocusOrder::Auto | FocusOrder::Position(_))
  )
}

/// The name the control `node` reports to assistive technology: the text of its
/// [`Part::Label`], else the label its accessibility properties carry, else the one its
/// props were created with.
#[must_use]
pub fn accessible_name(node: NodeId) -> Option<String> {
  end_turn();

  let props = control_props(node)?;
  let label = Scene::<Headless>::with(|scene| {
    scene
      .children(node)
      .find(|child| scene.part(*child) == Some(Part::Label))
  });

  if let Some(label) = label {
    let text = subtree_text(label);

    if !text.is_empty() {
      return Some(text);
    }
  }

  props
    .box_props
    .access
    .iter()
    .find_map(|prop| match prop {
      AccessProp::LabelledBy(node) => Some(subtree_text(*node)).filter(|text| !text.is_empty()),
      AccessProp::Label(label) => Some(label.clone()),
      _ => None,
    })
    .or(props.label)
}

/// The text of every run under `node`, in order.
fn subtree_text(node: NodeId) -> String {
  let mut out = String::new();
  let mut pending = vec![node];

  while let Some(node) = pending.pop() {
    if let Some(text) = text(node) {
      out.push_str(&text);
    }

    let children: Vec<NodeId> = children(node);

    pending.extend(children.into_iter().rev());
  }

  out
}

/// The props the run `node` was created with.
#[must_use]
pub fn run_props(node: NodeId) -> Option<RunProps<Headless>> {
  end_turn();

  with_record(handle(node)?, |record| match &record.kind {
    Kind::Run(props) => Some((**props).clone()),
    Kind::Box(_) | Kind::Control(_) | Kind::Overlay(_) | Kind::Anchor | Kind::Scope => None,
  })?
}

/// The text of the run `node`.
#[must_use]
pub fn text(node: NodeId) -> Option<String> {
  end_turn();

  with_record(handle(node)?, |record| match record.kind {
    Kind::Run(_) => Some(record.text.clone()),
    Kind::Box(_) | Kind::Control(_) | Kind::Overlay(_) | Kind::Anchor | Kind::Scope => None,
  })?
}

/// The textual value last written to `node`.
#[must_use]
pub fn value(node: NodeId) -> Option<String> {
  end_turn();

  with_record(handle(node)?, |record| record.value.clone())
}

/// The options last written to `node`, each a value and its name.
#[must_use]
pub fn options(node: NodeId) -> Option<Vec<(String, String)>> {
  end_turn();

  with_record(handle(node)?, |record| record.options.clone())
}

/// The selection last written to `node`.
#[must_use]
pub fn selection(node: NodeId) -> Option<TextSelection> {
  end_turn();

  with_record(handle(node)?, |record| record.selection)
}

/// The checked state last written to `node`.
#[must_use]
pub fn checked(node: NodeId) -> Option<bool> {
  end_turn();

  with_record(handle(node)?, |record| record.checked)
}

/// The latest leaf a render effect wrote into `slot` on `node`: a value, or a token or
/// expression resolved where the slot is read.
#[must_use]
pub fn slot_leaf(node: NodeId, slot: Slot) -> Option<SlotLeaf> {
  end_turn();

  with_record(handle(node)?, |record| record.slots.get(&slot).cloned())?
}

/// The event kinds `node` has handlers for, in registration order.
#[must_use]
pub fn listened(node: NodeId) -> Vec<EventKind> {
  end_turn();

  handle(node)
    .and_then(|handle| with_record(handle, |record| record.listened.clone()))
    .unwrap_or_default()
}

/// The token definitions placed on the scope `node`.
#[must_use]
pub fn definitions(node: NodeId) -> Option<Definitions<TokenValue>> {
  end_turn();

  with_record(handle(node)?, |record| record.definitions.clone())?
}

/// The token definitions every node falls back to.
pub fn root_definitions() -> Definitions<TokenValue> {
  end_turn();

  with_store(|store| store.root_definitions.clone())
}

/// How many times the root definitions have been written since the last reset.
#[must_use]
pub fn root_definition_writes() -> usize {
  end_turn();

  with_store(|store| store.root_definition_writes)
}

/// The children of `node` as the scene holds them, in order.
#[must_use]
pub fn children(node: NodeId) -> Vec<NodeId> {
  end_turn();

  Scene::<Headless>::with(|scene| scene.children(node).collect())
}

/// The children of `node` as the platform mirrored them, in order.
#[must_use]
pub fn mirrored_children(node: NodeId) -> Vec<Handle> {
  end_turn();

  handle(node)
    .and_then(|handle| with_record(handle, |record| record.children.clone()))
    .unwrap_or_default()
}

/// The color the property `key` of `node` resolves to: the variant in force under the
/// environment and the node's hover and focus state, then its leaf through the
/// scene's tokens, the expression evaluator or the recorded slot value.
#[must_use]
pub fn resolve_color(node: NodeId, key: PropertyKey<NoProperty>) -> Option<Color> {
  end_turn();

  match resolve::resolve(node, key)? {
    SlotValue::Box(BoxValue::Color(color)) | SlotValue::Run(RunValue::Color(color)) => Some(color),
    SlotValue::Box(_) | SlotValue::Run(_) => None,
  }
}

/// The border of the box `node`, by the same resolution as [`resolve_color`], with its
/// color leaf resolved to a fixed color.
#[must_use]
pub fn resolve_border(node: NodeId) -> Option<Border<f32>> {
  end_turn();

  resolve::resolve_border(node)
}

/// The length the property `key` of `node` resolves to, by the same resolution as
/// [`resolve_color`]: a fixed-length dimension, a number, or a uniform inset or spacing.
#[must_use]
pub fn resolve_length(node: NodeId, key: PropertyKey<NoProperty>) -> Option<f32> {
  end_turn();

  match resolve::resolve(node, key)? {
    SlotValue::Box(BoxValue::Dimension(Dimension::Length(length))) => Some(length),
    SlotValue::Box(BoxValue::Number(number)) => Some(number),
    SlotValue::Box(BoxValue::Inset(inset)) => uniform_inset(inset),
    SlotValue::Box(BoxValue::DimensionInset(inset)) => match uniform_inset(inset)? {
      Dimension::Length(length) => Some(length),
      Dimension::Fraction(_) | Dimension::Auto => None,
    },
    SlotValue::Box(BoxValue::Spacing(spacing)) => uniform_spacing(spacing),
    SlotValue::Box(_) | SlotValue::Run(_) => None,
  }
}

fn uniform_spacing<T: Copy + PartialEq>(spacing: Spacing<T>) -> Option<T> {
  (spacing.row == spacing.column).then_some(spacing.row)
}

fn uniform_inset<T: Copy + PartialEq>(inset: Inset<T>) -> Option<T> {
  (inset.top == inset.right && inset.top == inset.bottom && inset.top == inset.left)
    .then_some(inset.top)
}
