//! # Headless platform
//!
//! The host platform: realizes the portable vocabulary into inspectable records,
//! so view logic tests run without a platform. Every mirrored call is bookkeeping, not
//! emulation: no layout runs, and no platform response fires. Tests build views exactly
//! as an app does, then inspect the records ([`outline`], [`box_props`], [`slot_leaf`],
//! [`resolve_color`]) and drive events through the `trigger_` seams, each routed through
//! core's dispatch so bubbling is the one implementation every platform shares.
//!
//! The environment starts at a 1024 by 768 viewport, the light scheme and no reduced
//! motion; [`trigger_resize`], [`trigger_color_scheme`] and [`trigger_reduced_motion`]
//! move it, the app's choice through [`Platform::set_scheme`] overrides the platform's
//! scheme, and the signals [`Platform::media`] reports follow.
//!
//! Motion runs on a clock that stands still until [`trigger_frame`] steps it: a property
//! whose resolved value moved under a transition, or that an animation sets, resolves
//! to the in-flight value at the clock's reading, so [`resolve_color`] and
//! [`resolve_length`] read what a frame would show.
//!
//! A control is realized as a box carrying its [`ControlProps`], an overlay as one
//! carrying its [`OverlayProps`], an open flag ([`is_open`]) and the anchor it was last
//! opened against ([`overlay_anchor`]); [`control_state`],
//! [`part`], [`focus_order`], [`is_tab_stop`] and [`accessible_name`] read what the
//! control and its parts report, and [`trigger_size`] gives a control the measured size
//! its pointer handling maps a position onto. A painted box records the painter bound to
//! it ([`is_painted`]), which [`render_to_pixels`] draws to pixels a test reads.
//!
//! [`find_by_role`], [`find_all_by_role`] and [`find_by_text`] find nodes as a reader
//! does, by role and accessible name or by text, [`find_all_unnamed`] those a reader
//! could not name, and [`user`] drives them as a user does: a click, a key or typed
//! text becomes the sequence of events a browser dispatches for it.
//!
//! Tasks run on an executor that stands still until [`pump_tasks`] drains it:
//! [`install_executor`] makes it the thread's spawner ([`reset`] does too), so a
//! `Resource` built in a test loads when the test pumps.

mod records;
mod resolve;
pub mod user;

use std::{cell::RefCell, fmt::Write as _, rc::Rc, thread::LocalKey, time::Duration};

use skene_common::gui::{Canvas, GuiProperty, GuiStyle, Painter, Painting};
pub use skene_common::gui::{HeadElement, HeadId};
use skene_core::{
  AspectRatio,
  Availability,
  Axes,
  BoxProps,
  BoxValue,
  CheckState,
  ClickEvent,
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
  KeyDefault,
  KeyEvent,
  Media,
  MediaSignals,
  NameFacts,
  NativeElement,
  NodeId,
  NodeRecord,
  Observation,
  Overflow,
  OverlayProps,
  Part,
  PickerOption,
  Placement,
  Platform,
  Point,
  PointerButton,
  PointerEvent,
  PointerPrecision,
  Prevented,
  Property,
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
  Visibility,
  WheelEvent,
  dispatch_click,
  dispatch_context_menu,
  dispatch_double_click,
  dispatch_key_down,
  dispatch_key_up,
  dispatch_pointer_cancel,
  dispatch_pointer_down,
  dispatch_pointer_move,
  dispatch_pointer_up,
  dispatch_wheel,
  drop_drag,
  end_drag,
  move_drag,
  notify_change,
  notify_dismiss,
  notify_focus_moved,
  notify_pointer_moved,
  notify_scroll,
  notify_selection_change,
  notify_toggle,
  start_drag,
};
use skene_reactive::{Memo, ReadSignal, Set};
use unicode_segmentation::UnicodeSegmentation;

use crate::records::{Kind, Record, Records};

/// The host platform: a zero-sized [`Platform`] keeping a record per node in this
/// crate's records.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Headless;

/// The erased token definition the headless platform stores per token; see
/// [`skene_core::TokenValue`].
pub type TokenValue = skene_core::TokenValue<f32>;

/// A property's value erased over the property; see [`skene_core::SlotValue`].
pub type SlotValue = skene_core::SlotValue<f32, GuiStyle>;

/// What a derived leaf last wrote into a node's slot; see [`skene_core::SlotLeaf`].
pub type SlotLeaf = skene_core::SlotLeaf<f32, GuiStyle>;

thread_local! {
  static SCENE: RefCell<Scene<Headless>> = RefCell::new(Scene::new());
  static STORE: RefCell<Records> = RefCell::new(Records::default());
}

/// Ends the turn: runs the flush the writes since the last turn's end scheduled, so
/// their effects have taken hold.
///
/// Every `trigger_` and setter here is a turn of its own and every query ends the
/// pending one first; a test calls this where it observes a direct write's effects
/// some other way.
pub fn end_turn() {
  skene_core::end_turn();
}

/// Runs `event` as one turn: what earlier writes scheduled takes effect first, and
/// what the event's own writes schedule once it is handled.
fn trigger<R>(event: impl FnOnce() -> R) -> R {
  end_turn();

  let outcome = event();

  end_turn();

  outcome
}

/// Runs `operation` on the records; nothing of the user's runs inside it.
fn with_records<R>(operation: impl FnOnce(&mut Records) -> R) -> R {
  STORE.with(|records| operation(&mut records.borrow_mut()))
}

/// Runs `operation` on the record of `node`, if it is live.
fn with_record<R>(node: NodeId, operation: impl FnOnce(&mut Record) -> R) -> Option<R> {
  with_records(|records| records.record_mut(node).map(operation))
}

impl Platform for Headless {
  type Scalar = f32;
  type StyleExt = GuiStyle;
  type TokenValue = TokenValue;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create_box(node: NodeId, props: &BoxProps<Self>) {
    with_records(|records| records.insert(node, Record::new(Kind::Box(Box::new(props.clone())))));
  }

  fn update_box(node: NodeId, slot: Slot, leaf: &StaticLeaf<BoxValue<f32, GuiStyle>>) {
    with_record(node, |record| {
      record.slots.insert(slot, leaf.clone().into());
    });
  }

  fn create_control(node: NodeId, props: &ControlProps<Self>) {
    with_records(|records| {
      records.insert(node, Record::new(Kind::Control(Box::new(props.clone()))));
    });
  }

  fn set_control_state(node: NodeId, state: &ControlState) {
    with_record(node, |record| {
      record.checked = state.checked == CheckState::On;
    });
  }

  fn create_overlay(node: NodeId, props: &OverlayProps<Self>) {
    with_records(|records| {
      records.insert(node, Record::new(Kind::Overlay(Box::new(props.clone()))));
    });
  }

  fn set_overlay_open(node: NodeId, open: bool, _anchor: Option<NodeId>) {
    let opened = with_record(node, |record| {
      let opened = open && !record.open;

      record.open = open;

      opened
    });

    if opened == Some(true) && Scene::<Self>::read(|scene| scene.is_modal(node)) {
      let (_, order) = tab_order(node);
      let target = Scene::<Self>::read(|scene| scene.modal_focus(node, &order));

      notify_focus_moved::<Self>(Some(target));
    }
  }

  fn set_overlay_placement(node: NodeId, placement: Placement) {
    with_record(node, |record| {
      if let Kind::Overlay(props) = &mut record.kind {
        props.placement = placement;
      }
    });
  }

  fn set_overlay_position(node: NodeId, position: Option<Point<f32>>) {
    with_record(node, |record| record.position = position);
  }

  fn create_run(node: NodeId, props: &RunProps<Self>) {
    with_records(|records| records.insert(node, Record::new(Kind::Run(Box::new(props.clone())))));
  }

  fn update_run(node: NodeId, slot: Slot, leaf: &StaticLeaf<RunValue<GuiStyle>>) {
    with_record(node, |record| {
      record.slots.insert(slot, leaf.clone().into());
    });
  }

  fn set_text(node: NodeId, text: &str) {
    with_record(node, |record| text.clone_into(&mut record.text));
  }

  fn create_anchor(node: NodeId) {
    with_records(|records| records.insert(node, Record::new(Kind::Anchor)));
  }

  fn create_scope(node: NodeId) {
    with_records(|records| records.insert(node, Record::new(Kind::Scope)));
  }

  fn attach(_: NodeId, _: NodeId, _: Option<NodeId>) {}

  fn relocate(_: NodeId, _: NodeId, _: Option<NodeId>) {}

  fn hide(_: NodeId, _: NodeId) {}

  fn remove(_: NodeId, _: NodeId) {}

  fn destroy(node: NodeId) {
    with_records(|records| records.destroy(node));
  }

  fn define_tokens(scope: NodeId, definitions: &Definitions<TokenValue>) {
    with_record(scope, |record| {
      record.definitions = Some(definitions.clone());
    });
  }

  fn define_root_tokens(definitions: &Definitions<TokenValue>) {
    with_records(|records| {
      records.root_definitions = definitions.clone();
      records.root_definition_writes += 1;
    });
  }

  fn listen(node: NodeId, kind: EventKind) {
    with_record(node, |record| {
      if let Err(at) = record.listened.binary_search(&kind) {
        record.listened.insert(at, kind);
      }
    });
  }

  fn focus(node: NodeId) {
    if with_records(|records| records.record(node).is_some()) {
      notify_focus_moved::<Self>(Some(node));
    }
  }

  fn blur(node: NodeId) {
    let holds = Scene::<Self>::with(|scene| scene.focused() == Some(node));

    if holds {
      notify_focus_moved::<Self>(None);
    }
  }

  fn focus_visible() -> bool {
    Scene::<Self>::with(|scene| {
      scene
        .focused()
        .is_some_and(|focused| scene.focus_shows(focused))
    })
  }

  fn set_value(node: NodeId, value: &str) {
    with_record(node, |record| value.clone_into(&mut record.value));
  }

  fn set_options(node: NodeId, options: &[PickerOption]) {
    with_record(node, |record| options.clone_into(&mut record.options));
  }

  fn set_selection(node: NodeId, selection: TextSelection) {
    with_record(node, |record| {
      record.selection = selection.clamped(record.value.graphemes(true).count());
    });
  }

  fn set_checked(node: NodeId, checked: bool) {
    with_record(node, |record| record.checked = checked);
  }

  fn observe_size(node: NodeId, report: Rc<dyn Fn(Size<f32>)>) -> Observation {
    let kept = Rc::clone(&report);

    with_record(node, |record| record.size_observers.push(report));

    Observation::new(move || {
      with_record(node, |record| {
        record
          .size_observers
          .retain(|observer| !Rc::ptr_eq(observer, &kept));
      });
    })
  }

  fn media() -> MediaSignals<f32> {
    with_records(|records| records.media().signals)
  }

  fn watch_media(media: &Media<f32>) -> Memo<bool> {
    Self::media().watch(media)
  }

  fn hydration() -> Option<ReadSignal<bool>> {
    Some(with_records(|records| records.media().hydrated))
  }

  fn set_scheme(scheme: Option<ColorScheme>) {
    record_chosen_scheme(scheme);
  }

  fn schedule(delay: Duration, action: Box<dyn FnOnce()>) -> Timer {
    with_records(|records| {
      let at = records.clock + delay.as_secs_f64();

      records.timers.schedule(at, action)
    })
  }

  fn cancel(timer: Timer) {
    with_records(|records| records.timers.cancel(timer));
  }

  fn clipboard_availability(action: ClipboardAction) -> Availability {
    with_records(|records| {
      records
        .availability
        .get(&action)
        .copied()
        .unwrap_or(Availability::Enabled)
    })
  }

  fn clipboard(action: ClipboardAction) {
    with_records(|records| records.clipboard.push(action));
  }

  fn stored_scheme() -> Option<ColorScheme> {
    chosen_scheme()
  }
}

impl Painting for Headless {
  fn paint(node: NodeId, painter: Box<Painter>) {
    with_record(node, |record| record.painter = Some(Rc::from(painter)));
  }

  fn available() -> ReadSignal<bool> {
    with_records(|records| records.media().graphics)
  }
}

/// Whether a paint callback was handed to the box `node` through [`Painting`].
#[must_use]
pub fn is_painted(node: NodeId) -> bool {
  end_turn();

  with_record(node, |record| record.painter.is_some()) == Some(true)
}

/// Paints the box `node` at `size` logical pixels, one device pixel each, and returns
/// the unpremultiplied RGBA8 pixels row by row; `None` for a box nothing paints.
#[must_use]
pub fn render_to_pixels(node: NodeId, size: Size<f32>) -> Option<Vec<u8>> {
  end_turn();

  let painter = with_record(node, |record| record.painter.clone())??;
  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a test canvas's size is a small, non-negative number of pixels"
  )]
  let mut canvas = Canvas::new(size.width as u16, size.height as u16);

  painter(&mut canvas, size);

  Some(canvas.pixels())
}

/// Empties the scene, the records and the task queue, returns the environment to its
/// defaults, installs the executor and holds every flush until its turn ends.
pub fn reset() {
  end_turn();

  let owner = with_records(Records::reset);

  if let Some(owner) = owner {
    owner.dispose();
  }

  skene_core::reset_thread::<Headless>();
  skene_core::hold_flushes(|| {});
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

/// Registers `title` for the document, in force while it is the latest live one.
pub fn add_head_title(title: &str) -> HeadId {
  with_records(|records| records.head.add_title(title.to_owned()))
}

/// Rewrites the title registration `id`.
pub fn set_head_title(id: HeadId, title: &str) {
  with_records(|records| records.head.set_title(id, title.to_owned()));
}

/// Registers an element for the document head, in force while it is the latest live
/// one of what it stands for.
///
/// # Panics
///
/// Panics when `tag` is not an element a head holds (`base`, `link`, `meta` or
/// `script`) or an attribute name is not one.
pub fn add_head_element(tag: &str, attributes: &[(&str, &str)]) -> HeadId {
  with_records(|records| records.head.add_element(HeadElement::new(tag, attributes)))
}

/// Removes the head registration `id`.
pub fn remove_head_entry(id: HeadId) {
  with_records(|records| records.head.remove(id));
}

/// Sets the document's language, a BCP 47 tag.
pub fn set_document_language(language: &str) {
  with_records(|records| records.language = Some(language.to_owned()));
}

/// The document title in force.
#[must_use]
pub fn document_title() -> Option<String> {
  end_turn();

  with_records(|records| records.head.title().map(str::to_owned))
}

/// The document language last set.
#[must_use]
pub fn document_language() -> Option<String> {
  end_turn();

  with_records(|records| records.language.clone())
}

/// The elements in force in the document head, in their registrations' order.
#[must_use]
pub fn head_elements() -> Vec<HeadElement> {
  end_turn();

  with_records(|records| records.head.elements().cloned().collect())
}

/// Dispatches a single click with no modifier held at `node`, bubbling through the
/// scene.
pub fn trigger_click(node: NodeId) -> Prevented {
  trigger_click_event(node, &ClickEvent::new(1))
}

/// Dispatches `event` as a click at `node`, bubbling through the scene.
pub fn trigger_click_event(node: NodeId, event: &ClickEvent) -> Prevented {
  trigger(|| dispatch_click::<Headless>(node, event))
}

/// Dispatches the second click of a double click at `node`, bubbling through the scene:
/// the click, counting two, then the double click, whose verdict is returned.
pub fn trigger_double_click(node: NodeId) -> Prevented {
  trigger(|| {
    let event = ClickEvent::new(2);
    let _ = dispatch_click::<Headless>(node, &event);

    dispatch_double_click::<Headless>(node, &event)
  })
}

/// Dispatches a key press at `node`, bubbling through the scene; focus shows from
/// here on.
///
/// What the handlers leave to the platform follows as a browser's default does: a `Tab`
/// moves focus to the next tab stop of the tree `node` stands in, or to the previous one
/// with `Shift`; an `Escape` dismisses the topmost open overlay; and the menu key or
/// `Shift-F10` asks `node` for its context menu, the request naming no pointer.
pub fn trigger_key_down(node: NodeId, event: &KeyEvent) -> Prevented {
  trigger(|| {
    if event.shows_focus() {
      Scene::<Headless>::with(|scene| scene.set_focus_visible(true));
    }

    let prevented = dispatch_key_down::<Headless>(node, event);

    if prevented.platform {
      return prevented;
    }

    match Scene::<Headless>::read(|scene| scene.key_default(event)) {
      Some(KeyDefault::Traverse { back }) => traverse(node, if back { -1 } else { 1 }),
      Some(KeyDefault::Dismiss(overlay)) => notify_dismiss::<Headless>(overlay),
      Some(KeyDefault::ContextMenu) => {
        let mut request = PointerEvent::mouse(0., 0.);

        request.pointer.pointer_id = None;
        request.modifiers = event.modifiers;

        let _ = dispatch_context_menu::<Headless>(node, &request);
      }
      None => {}
    }

    prevented
  })
}

/// Moves focus `step` tab stops from where it is along the tab order of the tree `node`
/// stands in.
fn traverse(node: NodeId, step: isize) {
  let (root, order) = tab_order(node);
  let next = Scene::<Headless>::read(|scene| scene.tab_from(root, &order, scene.focused(), step));

  if let Some(next) = next {
    notify_focus_moved::<Headless>(Some(next));
  }
}

/// The root of the tree `node` stands in, with its tab order; a hidden box and its
/// content take no stop.
fn tab_order(node: NodeId) -> (NodeId, Vec<NodeId>) {
  let (root, nodes) = Scene::<Headless>::with(|scene| {
    let root = scene.ancestry(node).last().copied().unwrap_or(node);
    let mut nodes = Vec::new();
    let mut stack = vec![root];

    while let Some(current) = stack.pop() {
      nodes.push(current);
      stack.extend(scene.children(current));
    }

    (root, nodes)
  });
  let hidden: Vec<NodeId> = nodes
    .into_iter()
    .filter(|node| {
      matches!(
        resolve::resolve(*node, PropertyKey::Shared(Property::Visibility)),
        Some(SlotValue::Box(BoxValue::Visibility(Visibility::Hidden)))
      )
    })
    .collect();
  let order = Scene::<Headless>::read(|scene| {
    scene.tab_order(root, scene.open_overlays(), |node| hidden.contains(&node))
  });

  (root, order)
}

/// Dispatches a key release at `node`, bubbling through the scene.
pub fn trigger_key_up(node: NodeId, event: &KeyEvent) -> Prevented {
  trigger(|| dispatch_key_up::<Headless>(node, event))
}

/// Dispatches a pointer press at `node`, bubbling through the scene.
///
/// The open overlays the press lands outside of are dismissed first. Focus stops showing
/// from here on, and a primary press holds `node` and its ancestors pressed until its
/// release.
pub fn trigger_pointer_down(node: NodeId, event: &PointerEvent) -> Prevented {
  trigger(|| {
    Scene::<Headless>::with(|scene| {
      scene.set_focus_visible(false);

      if event.button == Some(PointerButton::Primary) {
        scene.set_pressed(Some(node));
      }
    });

    let dismissed = Scene::<Headless>::with(|scene| {
      let overlays = scene.open_overlays().to_vec();

      scene.dismissed_by_press(&overlays, Some(node))
    });

    for overlay in dismissed {
      notify_dismiss::<Headless>(overlay);
    }

    dispatch_pointer_down::<Headless>(node, event)
  })
}

/// Dispatches a pointer release at `node`, bubbling through the scene; a primary
/// release ends the press.
pub fn trigger_pointer_up(node: NodeId, event: &PointerEvent) -> Prevented {
  trigger(|| {
    if event.button == Some(PointerButton::Primary) {
      Scene::<Headless>::with(|scene| scene.set_pressed(None));
    }

    dispatch_pointer_up::<Headless>(node, event)
  })
}

/// Dispatches the cancel of a pointer pressed at `node`, bubbling through the scene: the
/// platform took the pointer for itself, so the press ends without a release.
pub fn trigger_pointer_cancel(node: NodeId, event: &PointerEvent) -> Prevented {
  trigger(|| {
    Scene::<Headless>::with(|scene| scene.set_pressed(None));

    dispatch_pointer_cancel::<Headless>(node, event)
  })
}

/// Dispatches a wheel's turn at `node`, bubbling through the scene; a verdict that
/// suppresses the platform's response keeps the scroll from happening.
pub fn trigger_wheel(node: NodeId, event: &WheelEvent) -> Prevented {
  trigger(|| dispatch_wheel::<Headless>(node, event))
}

/// Starts a drag from `node`, as a pointer pressing it and moving off would: the
/// nearest node at or above it offering drag data builds what the drag carries;
/// whether one did.
#[must_use]
pub fn trigger_drag_start(node: NodeId) -> bool {
  trigger(|| start_drag::<Headless>(node).is_some())
}

/// Moves the drag in progress over `node`; whether a node at or above it takes the
/// drag.
#[must_use]
pub fn trigger_drag_over(node: NodeId) -> bool {
  trigger(|| move_drag::<Headless>(Some(node)).is_some())
}

/// Drops the drag in progress where it is; whether its target took it.
#[must_use]
pub fn trigger_drop() -> bool {
  trigger(drop_drag::<Headless>)
}

/// Ends the drag in progress without a drop, as `Escape` or a drop outside the app
/// does.
pub fn trigger_drag_end() {
  trigger(end_drag::<Headless>);
}

/// Dispatches a pointer move at `node`, bubbling through the scene.
pub fn trigger_pointer_move(node: NodeId, event: &PointerEvent) -> Prevented {
  trigger(|| dispatch_pointer_move::<Headless>(node, event))
}

/// Moves the pointer over `node`, which holds `Hovered` with its ancestors, notifying
/// each node the pointer left, innermost first, and each it entered, outermost first.
pub fn trigger_pointer_enter(node: NodeId) {
  trigger(|| notify_pointer_moved::<Headless>(Some(node)));
}

/// Moves the pointer off `node` onto the node around it, notifying `node` and each node
/// beneath it the pointer was over.
pub fn trigger_pointer_leave(node: NodeId) {
  trigger(|| notify_pointer_moved::<Headless>(Scene::<Headless>::with(|scene| scene.parent(node))));
}

/// Moves focus to `node`, notifying each node whose subtree focus left and each whose
/// subtree it reached.
pub fn trigger_focus(node: NodeId) {
  trigger(|| notify_focus_moved::<Headless>(Some(node)));
}

/// Takes focus off `node`, if it holds it, notifying `node` and each ancestor whose
/// subtree held it.
pub fn trigger_blur(node: NodeId) {
  trigger(|| {
    if Scene::<Headless>::with(|scene| scene.focused()) == Some(node) {
      notify_focus_moved::<Headless>(None);
    }
  });
}

/// Records `value` as the node's textual value and notifies its change handlers.
pub fn trigger_change(node: NodeId, value: &str) {
  trigger(|| {
    Headless::set_value(node, value);

    notify_change::<Headless>(node, value);
  });
}

/// Records `selection` as the node's selection and notifies its selection handlers.
pub fn trigger_selection_change(node: NodeId, selection: TextSelection) {
  trigger(|| {
    Headless::set_selection(node, selection);

    notify_selection_change::<Headless>(node, selection);
  });
}

/// Records `checked` as the node's checked state and notifies its toggle handlers.
pub fn trigger_toggle(node: NodeId, checked: bool) {
  trigger(|| {
    Headless::set_checked(node, checked);

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

  with_records(|records| std::mem::take(&mut records.clipboard))
}

/// Sets what `action` answers when a view asks whether it can be offered; enabled
/// unless set.
pub fn set_clipboard_availability(action: ClipboardAction, availability: Availability) {
  with_records(|records| {
    records.availability.insert(action, availability);
  });
}

/// Reports `size` as the measured size of `node` to every observer bound to it; a box
/// named for container queries also publishes it as that container's size.
pub fn trigger_size(node: NodeId, size: Size<f32>) {
  trigger(|| {
    let name = Scene::<Headless>::with(|scene| scene.record(node).and_then(NodeRecord::container));

    if let Some(name) = name {
      record_container_size(name, size);
    }

    let observers = with_record(node, |record| {
      record.size = Some(size);
      record.size_observers.clone()
    })
    .unwrap_or_default();

    for observer in observers {
      observer(size);
    }
  });
}

/// Simulates the viewport resizing to `size`.
pub fn trigger_resize(size: Size<f32>) {
  trigger(|| {
    let write = with_records(|records| {
      records.viewport = Some(size);

      records.media().set_viewport
    });

    write.set(size);
  });
}

/// Simulates the platform's color scheme changing to `scheme`; the scheme in force
/// follows it unless the app chose one.
pub fn trigger_color_scheme(scheme: ColorScheme) {
  trigger(|| {
    let (write, in_force) = with_records(|records| {
      records.scheme = Some(scheme);

      (records.media().set_scheme, records.scheme_in_force())
    });

    write.set(in_force);
  });
}

/// Records the scheme the app chose, the scheme in force following.
fn record_chosen_scheme(scheme: Option<ColorScheme>) {
  let (write, in_force) = with_records(|records| {
    records.chosen_scheme = scheme;

    (records.media().set_scheme, records.scheme_in_force())
  });

  write.set(in_force);
}

/// The scheme the app chose, if any.
#[must_use]
pub fn chosen_scheme() -> Option<ColorScheme> {
  end_turn();

  with_records(|records| records.chosen_scheme)
}

/// Simulates whether the render stands hydrated, as the web leaves answer it: false
/// stands in for a server render or a hydration still adopting, true for a live page;
/// hydrated until triggered.
pub fn trigger_hydrated(hydrated: bool) {
  trigger(|| {
    let write = with_records(|records| {
      records.hydrated = Some(hydrated);

      records.media().set_hydrated
    });

    write.set(hydrated);
  });
}

/// Simulates whether a painted box shows as pixels; shown until triggered.
pub fn trigger_graphics(shown: bool) {
  trigger(|| {
    let write = with_records(|records| {
      records.graphics = Some(shown);

      records.media().set_graphics
    });

    write.set(shown);
  });
}

/// Simulates the reader's reduced-motion preference.
pub fn trigger_reduced_motion(reduced: bool) {
  trigger(|| {
    let write = with_records(|records| {
      records.reduced_motion = Some(reduced);

      records.media().set_reduced_motion
    });

    write.set(reduced);
  });
}

/// Simulates the reader's reduced-transparency preference.
pub fn trigger_reduced_transparency(reduced: bool) {
  trigger(|| {
    let write = with_records(|records| {
      records.reduced_transparency = Some(reduced);

      records.media().set_reduced_transparency
    });

    write.set(reduced);
  });
}

/// Simulates the reader's preference for more contrast.
pub fn trigger_high_contrast(high: bool) {
  trigger(|| {
    let write = with_records(|records| {
      records.high_contrast = Some(high);

      records.media().set_high_contrast
    });

    write.set(high);
  });
}

/// Simulates a change of the reader's primary pointer to one pointing as precisely as
/// `precision` says.
pub fn trigger_pointer_precision(precision: PointerPrecision) {
  trigger(|| {
    let write = with_records(|records| {
      records.pointer = Some(precision);

      records.media().set_pointer
    });

    write.set(precision);
  });
}

/// Simulates the display keeping `insets` clear of the viewport's edges.
pub fn trigger_safe_area(insets: Inset<f32>) {
  trigger(|| {
    let write = with_records(|records| {
      records.safe_area = Some(insets);

      records.media().set_safe_area
    });

    write.set(insets);
  });
}

/// Simulates a change of the reader's primary pointer to one that hovers, or not.
pub fn trigger_pointer_hovers(hovers: bool) {
  trigger(|| {
    let write = with_records(|records| {
      records.hover = Some(hovers);

      records.media().set_hover
    });

    write.set(hovers);
  });
}

/// How many scheduled actions are still waiting for the clock, for a test asserting that
/// a view cancels its own.
#[must_use]
pub fn pending_timers() -> usize {
  end_turn();

  with_records(|records| records.timers.len())
}

/// Simulates a frame passing, stepping the clock forward by `elapsed`.
///
/// Every property a resolution has observed is judged again at the clock's reading, so
/// a value that moved since starts its transition there, before the clock steps; the
/// next resolution samples every transition and animation at the new reading.
pub fn trigger_frame(elapsed: Duration) {
  trigger(|| {
    let tracked = with_records(|records| records.tracked());

    for (node, keys) in tracked {
      for key in keys {
        let _ = resolve::resolve(node, key);
      }
    }

    let batch = with_records(|records| {
      records.clock += elapsed.as_secs_f64();

      records.timers.next_timer()
    });

    while let Some(action) = with_records(|records| records.timers.pop_due(records.clock, batch)) {
      action();
    }
  });
}

/// Simulates the query container `name` measuring `size`.
pub fn trigger_container_size(name: ContainerName, size: Size<f32>) {
  trigger(|| record_container_size(name, size));
}

/// Records the measured size of the query container `name`.
fn record_container_size(name: ContainerName, size: Size<f32>) {
  let sizes = with_records(|records| records.media().signals.container_sizes);

  sizes.set(name, size);
}

/// Whether the pointer is over `node` or a node beneath it.
#[must_use]
pub fn is_hovered(node: NodeId) -> bool {
  end_turn();

  Scene::<Headless>::with(|scene| scene.is_hovered(node))
}

/// Whether `node` holds focus.
#[must_use]
pub fn is_focused(node: NodeId) -> bool {
  focused_node() == Some(node)
}

/// The node holding focus.
#[must_use]
pub fn focused_node() -> Option<NodeId> {
  end_turn();

  Scene::<Headless>::with(|scene| scene.focused())
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

  outline_into(node, 0, full, &mut out);

  out
}

fn outline_into(node: NodeId, depth: usize, full: bool, out: &mut String) {
  let children: Vec<NodeId> = Scene::<Headless>::with(|scene| scene.children(node).collect());
  let Some(line) = with_records(|records| {
    let record = records.record(node)?;
    let line = match &record.kind {
      Kind::Box(_) => Some("Box".to_owned()),
      Kind::Control(props) => Some(format!("Control {:?}", props.native)),
      Kind::Overlay(props) => Some(format!(
        "Overlay {}",
        if props.modal { "modal" } else { "popup" }
      )),
      Kind::Run(_) => Some(format!("Run {:?}", record.text)),
      Kind::Anchor => full.then(|| "Anchor".to_owned()),
      Kind::Scope => full.then(|| "Scope".to_owned()),
    };

    Some(line)
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

  with_records(|records| records.record(node).is_some())
}

/// The props the box, control or overlay `node` was created with, carrying the focus
/// order and accessibility properties the scene holds for it now.
#[must_use]
pub fn box_props(node: NodeId) -> Option<BoxProps<Headless>> {
  end_turn();

  let mut props = with_record(node, |record| record.kind.box_props().cloned())??;

  current_facts(node, &mut props);

  Some(props)
}

/// The props the control `node` was created with, carrying the focus order and
/// accessibility properties the scene holds for it now.
#[must_use]
pub fn control_props(node: NodeId) -> Option<ControlProps<Headless>> {
  end_turn();

  let mut props = with_record(node, |record| match &record.kind {
    Kind::Control(props) => Some((**props).clone()),
    Kind::Box(_) | Kind::Overlay(_) | Kind::Run(_) | Kind::Anchor | Kind::Scope => None,
  })??;

  current_facts(node, &mut props.box_props);

  Some(props)
}

/// The props the overlay `node` was created with, carrying the focus order and
/// accessibility properties the scene holds for it now.
#[must_use]
pub fn overlay_props(node: NodeId) -> Option<OverlayProps<Headless>> {
  end_turn();

  let mut props = with_record(node, |record| match &record.kind {
    Kind::Overlay(props) => Some((**props).clone()),
    Kind::Box(_) | Kind::Control(_) | Kind::Run(_) | Kind::Anchor | Kind::Scope => None,
  })??;

  current_facts(node, &mut props.box_props);

  Some(props)
}

/// Writes the focus order and accessibility properties the scene holds for `node` into
/// `props`.
fn current_facts(node: NodeId, props: &mut BoxProps<Headless>) {
  Scene::<Headless>::read(|scene| {
    props.focus_order = scene.focus_order(node);
    props.access = scene.access(node);
  });
}

/// Whether the overlay `node` is open.
#[must_use]
pub fn is_open(node: NodeId) -> bool {
  end_turn();

  with_record(node, |record| record.open) == Some(true)
}

/// The node the overlay `node` is placed against: the first box of the anchor it was
/// last opened or closed against.
#[must_use]
pub fn overlay_anchor(node: NodeId) -> Option<NodeId> {
  end_turn();

  Scene::<Headless>::read(|scene| {
    scene
      .overlay_anchor(node)
      .and_then(|anchor| scene.placement(anchor))
  })
}

/// Where the overlay `node` is pinned, if it is.
#[must_use]
pub fn overlay_position(node: NodeId) -> Option<Point<f32>> {
  end_turn();

  with_record(node, |record| record.position)?
}

/// The state of the control `node`.
#[must_use]
pub fn control_state(node: NodeId) -> Option<ControlState> {
  end_turn();

  Scene::<Headless>::read(|scene| scene.control_state(node))
}

/// The part of its enclosing control `node` is recorded as; a control node is its own
/// [`Part::Root`].
#[must_use]
pub fn part(node: NodeId) -> Option<Part> {
  end_turn();

  Scene::<Headless>::with(|scene| scene.part(node))
}

/// The place in focus traversal of `node`, as the scene holds it; `None` when it takes
/// no focus.
#[must_use]
pub fn focus_order(node: NodeId) -> Option<FocusOrder> {
  end_turn();

  Scene::<Headless>::read(|scene| scene.focus_order(node))
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

skene_core::access_queries!(Headless, name_facts);

/// What the accessible-name computation reads of `node`.
fn name_facts(node: NodeId) -> NameFacts {
  let hidden = matches!(
    resolve::resolve(node, PropertyKey::Shared(Property::Visibility)),
    Some(SlotValue::Box(BoxValue::Visibility(Visibility::Hidden)))
  );

  let access = Scene::<Headless>::read(|scene| scene.access(node));

  with_record(node, |record| {
    let facts = match &record.kind {
      Kind::Run(props) => NameFacts::of_run(props, record.text.clone()),
      Kind::Box(props) => NameFacts::of_box(props, access),
      Kind::Overlay(props) => {
        NameFacts::of_box(&props.box_props, access).with_overlay(props.role(), record.open)
      }
      Kind::Control(props) => {
        let value = match props.native {
          Some(NativeElement::TextField | NativeElement::TextEditor) => Some(record.value.clone()),
          Some(NativeElement::Picker) => record
            .options
            .iter()
            .find(|option| option.value == record.value)
            .map(|option| option.name.clone()),
          Some(
            NativeElement::Button
            | NativeElement::Checkbox
            | NativeElement::Radio
            | NativeElement::Slider,
          )
          | None => None,
        };

        NameFacts::of_box(&props.box_props, access).with_control(
          props.native,
          props.label.clone(),
          props.placeholder.clone(),
          value,
        )
      }
      Kind::Anchor | Kind::Scope => NameFacts::default(),
    };

    NameFacts {
      hidden: facts.hidden || hidden,
      ..facts
    }
  })
  .unwrap_or_default()
}

/// The props the run `node` was created with.
#[must_use]
pub fn run_props(node: NodeId) -> Option<RunProps<Headless>> {
  end_turn();

  with_record(node, |record| match &record.kind {
    Kind::Run(props) => Some((**props).clone()),
    Kind::Box(_) | Kind::Control(_) | Kind::Overlay(_) | Kind::Anchor | Kind::Scope => None,
  })?
}

/// The text of the run `node`.
#[must_use]
pub fn text(node: NodeId) -> Option<String> {
  end_turn();

  with_record(node, |record| match record.kind {
    Kind::Run(_) => Some(record.text.clone()),
    Kind::Box(_) | Kind::Control(_) | Kind::Overlay(_) | Kind::Anchor | Kind::Scope => None,
  })?
}

/// The textual value last written to `node`.
#[must_use]
pub fn value(node: NodeId) -> Option<String> {
  end_turn();

  with_record(node, |record| record.value.clone())
}

/// The options last written to `node`.
#[must_use]
pub fn options(node: NodeId) -> Option<Vec<PickerOption>> {
  end_turn();

  with_record(node, |record| record.options.clone())
}

/// The selection last written to `node`.
#[must_use]
pub fn selection(node: NodeId) -> Option<TextSelection> {
  end_turn();

  with_record(node, |record| record.selection)
}

/// The checked state last written to `node`.
#[must_use]
pub fn checked(node: NodeId) -> Option<bool> {
  end_turn();

  with_record(node, |record| record.checked)
}

/// The latest leaf a render effect wrote into `slot` on `node`: a value, or a token or
/// expression resolved where the slot is read.
#[must_use]
pub fn slot_leaf(node: NodeId, slot: Slot) -> Option<SlotLeaf> {
  end_turn();

  with_record(node, |record| record.slots.get(&slot).cloned())?
}

/// The event kinds `node` has handlers for, in [`EventKind`] order.
#[must_use]
pub fn listened(node: NodeId) -> Vec<EventKind> {
  end_turn();

  with_record(node, |record| record.listened.clone()).unwrap_or_default()
}

/// The token definitions placed on the scope `node`.
#[must_use]
pub fn definitions(node: NodeId) -> Option<Definitions<TokenValue>> {
  end_turn();

  with_record(node, |record| record.definitions.clone())?
}

/// The token definitions every node falls back to.
pub fn root_definitions() -> Definitions<TokenValue> {
  end_turn();

  with_records(|records| records.root_definitions.clone())
}

/// How many times the root definitions have been written since the last reset.
#[must_use]
pub fn root_definition_writes() -> usize {
  end_turn();

  with_records(|records| records.root_definition_writes)
}

/// The children of `node` as the scene holds them, in order.
#[must_use]
pub fn children(node: NodeId) -> Vec<NodeId> {
  end_turn();

  Scene::<Headless>::with(|scene| scene.children(node).collect())
}

/// The color the property `key` of `node` resolves to.
///
/// The variant in force under the environment and the node's hover and focus state,
/// then its leaf through the scene's tokens, the expression evaluator or the recorded
/// slot value.
#[must_use]
pub fn resolve_color(node: NodeId, key: PropertyKey<GuiProperty>) -> Option<Color> {
  end_turn();

  match resolve::resolve(node, key)? {
    SlotValue::Box(BoxValue::Color(color)) | SlotValue::Run(RunValue::Color(color)) => Some(color),
    SlotValue::Box(_) | SlotValue::Run(_) => None,
  }
}

/// The inset the property `key` of `node` resolves to, by the same resolution as
/// [`resolve_color`]: a padding, a border's width, or a single length on every edge.
#[must_use]
pub fn resolve_inset(node: NodeId, key: PropertyKey<GuiProperty>) -> Option<Inset<f32>> {
  end_turn();

  match resolve::resolve(node, key)? {
    SlotValue::Box(BoxValue::Inset(inset)) => Some(inset),
    SlotValue::Box(BoxValue::Length(length)) => Some(Inset::all(length.0)),
    SlotValue::Box(BoxValue::Dimension(Dimension::Length(length))) => Some(Inset::all(length)),
    SlotValue::Box(_) | SlotValue::Run(_) => None,
  }
}

/// The length the property `key` of `node` resolves to, by the same resolution as
/// [`resolve_color`]: a box's length, fixed-length dimension, number, or uniform inset or
/// spacing, or a run's length.
#[must_use]
pub fn resolve_length(node: NodeId, key: PropertyKey<GuiProperty>) -> Option<f32> {
  end_turn();

  match resolve::resolve(node, key)? {
    SlotValue::Box(BoxValue::Length(length)) | SlotValue::Run(RunValue::Length(length)) => {
      Some(length.0)
    }
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

/// The visibility of the box `node`, by the same resolution as [`resolve_color`];
/// `None` where none is set, which draws the box where its ancestors are drawn.
#[must_use]
pub fn resolve_visibility(node: NodeId) -> Option<Visibility> {
  end_turn();

  match resolve::resolve(node, PropertyKey::Shared(Property::Visibility))? {
    SlotValue::Box(BoxValue::Visibility(visibility)) => Some(visibility),
    SlotValue::Box(_) | SlotValue::Run(_) => None,
  }
}

/// The aspect ratio of the box `node`, by the same resolution as [`resolve_color`];
/// `None` where none is set.
#[must_use]
pub fn resolve_aspect_ratio(node: NodeId) -> Option<AspectRatio> {
  end_turn();

  match resolve::resolve(node, PropertyKey::Shared(Property::AspectRatio))? {
    SlotValue::Box(BoxValue::AspectRatio(ratio)) => Some(ratio),
    SlotValue::Box(_) | SlotValue::Run(_) => None,
  }
}

/// The overflow of the box `node` on each axis, by the same resolution as
/// [`resolve_color`] and as CSS computes it; `None` where none is set.
#[must_use]
pub fn resolve_overflow(node: NodeId) -> Option<Axes<Overflow>> {
  end_turn();

  match resolve::resolve(node, PropertyKey::Shared(Property::Overflow))? {
    SlotValue::Box(BoxValue::Overflow(overflow)) => Some(overflow.computed()),
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
