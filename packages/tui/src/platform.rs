//! The terminal's realization of the platform: one record per node beside the scene,
//! the taffy tree those records lay out in, and the environment as signals.

use std::{cell::RefCell, rc::Rc, thread::LocalKey};

use skene_core::{
  AccessProp,
  Availability,
  BoxProps,
  BoxValue,
  ClipboardAction,
  ColorScheme,
  ControlKind,
  ControlProps,
  ControlState,
  Definitions,
  EventKind,
  FocusOrder,
  MediaSignals,
  NodeId,
  NodeKind,
  OverlayProps,
  Placement,
  Platform,
  Point,
  PointerEvent,
  RunProps,
  RunValue,
  Scene,
  Size,
  Slot,
  StaticLeaf,
  TextSelection,
  Timer,
};
use skene_reactive::{ReadSignal, Set};
use skene_views::Painting;

use crate::{
  TokenValue,
  TuiExt,
  Typography,
  cells::{CellPainter, Painter},
  interaction,
  motion::RunMotion,
  store::{BoxRecord, Field, Handle, Kind, PictureRecord, SlotValue, Store},
};

/// The terminal platform: a zero-sized [`Platform`] whose sink is a [`Handle`] into this
/// crate's record store.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Tui;

thread_local! {
  static SCENE: RefCell<Scene<Tui>> = RefCell::new(Scene::new());
  static STORE: RefCell<Option<Store>> = const { RefCell::new(None) };
  static ROOT: RefCell<Option<NodeId>> = const { RefCell::new(None) };
}

/// Runs `operation` on the installed store; nothing of the user's runs inside it.
///
/// # Panics
///
/// Panics if the terminal platform is not installed for this thread.
pub(crate) fn with_store<R>(operation: impl FnOnce(&mut Store) -> R) -> R {
  STORE.with(|store| {
    operation(
      store
        .borrow_mut()
        .as_mut()
        .expect("the terminal platform to be installed for this thread"),
    )
  })
}

/// Runs `operation` on the store, or `None` when none is installed.
pub(crate) fn try_with_store<R>(operation: impl FnOnce(&mut Store) -> R) -> Option<R> {
  STORE.with(|store| store.borrow_mut().as_mut().map(operation))
}

/// The handle of `node`, if it is live.
#[must_use]
pub fn handle(node: NodeId) -> Option<Handle> {
  Scene::<Tui>::with(|scene| scene.record(node).map(|record| *record.sink()))
}

/// The scene node every mounted view attaches under.
pub(crate) fn root_node() -> Option<NodeId> {
  ROOT.with(|root| *root.borrow())
}

impl Platform for Tui {
  type Scalar = u32;
  type Sink = Handle;
  type StyleExt = TuiExt;
  type TokenValue = TokenValue;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn bind_node(&self, sink: &Handle, node: NodeId) {
    with_store(|store| {
      if let Some(record) = store.record_mut(*sink) {
        record.node = Some(node);
      }
    });
  }

  fn create_box(&self, props: &BoxProps<Self>) -> Handle {
    with_store(|store| store.create_laid_out(Kind::Box(BoxRecord::new(props.clone()))))
  }

  fn update_box(&self, sink: &Handle, slot: Slot, leaf: &StaticLeaf<BoxValue<u32, TuiExt>>) {
    with_store(|store| {
      if let Some(record) = store.record_mut(*sink) {
        record.slots.insert(slot, leaf.clone().map(SlotValue::Box));

        if let Some(inner) = record.kind.box_record_mut() {
          inner.mark_stale();
        }

        store.layout_dirty = true;
        store.damage(*sink);
      }
    });
  }

  fn update_focus_order(&self, sink: &Handle, focus_order: Option<FocusOrder>) {
    with_store(|store| {
      if let Some(inner) = store
        .record_mut(*sink)
        .and_then(|record| record.kind.box_record_mut())
      {
        inner.props.focus_order = focus_order;
      }
    });
  }

  fn update_access(&self, sink: &Handle, access: &[AccessProp]) {
    with_store(|store| {
      if let Some(inner) = store
        .record_mut(*sink)
        .and_then(|record| record.kind.box_record_mut())
      {
        access.clone_into(&mut inner.props.access);
      }
    });
  }

  fn create_control(&self, props: &ControlProps<Self>) -> Handle {
    let field = matches!(props.kind, ControlKind::TextField | ControlKind::TextEditor)
      .then(|| Field::new(props.placeholder.clone(), props.lines));

    with_store(|store| {
      store.create_laid_out(Kind::Control {
        kind: props.kind,
        inner: BoxRecord::new(props.box_props.clone()),
        state: ControlState::DEFAULT,
        field,
        checked: false,
      })
    })
  }

  fn set_control_state(&self, sink: &Handle, state: &ControlState) {
    with_store(|store| {
      if let Some(Kind::Control { state: held, .. }) =
        store.record_mut(*sink).map(|record| &mut record.kind)
        && *held != *state
      {
        *held = *state;
        store.layout_dirty = true;
        store.damage(*sink);
      }
    });
  }

  fn create_overlay(&self, props: &OverlayProps<Self>) -> Handle {
    with_store(|store| {
      let handle = store.create_laid_out(Kind::Overlay {
        inner: BoxRecord::new(props.box_props.clone()),
        modal: props.modal,
        placement: props.placement,
        backdrop: props.backdrop,
        open: false,
        anchor: None,
        position: None,
        return_focus: None,
      });

      store.overlays.push(handle);

      handle
    })
  }

  fn set_overlay_open(&self, sink: &Handle, open: bool, anchor: Option<&Handle>) {
    let focus = with_store(|store| store.set_overlay_open(*sink, open, anchor.copied()));

    if let interaction::OverlayFocus::Moved(next) = focus {
      if open {
        interaction::set_focus(next);
      }
      else {
        interaction::set_focus_restored(next);
      }
    }
  }

  fn set_overlay_position(&self, sink: &Handle, position: Option<Point<f32>>) {
    with_store(|store| {
      if let Some(Kind::Overlay { position: held, .. }) =
        store.record_mut(*sink).map(|record| &mut record.kind)
      {
        #[expect(clippy::cast_possible_truncation)]
        let cell = position.map(|point| (point.x.floor() as i64, point.y.floor() as i64));

        *held = cell;
        store.paint_dirty = true;
      }
    });
  }

  fn set_overlay_placement(&self, sink: &Handle, placement: Placement) {
    with_store(|store| {
      if let Some(Kind::Overlay {
        placement: held, ..
      }) = store.record_mut(*sink).map(|record| &mut record.kind)
      {
        *held = placement;
        store.paint_dirty = true;
      }
    });
  }

  fn create_run(&self, props: &RunProps<Self>) -> Handle {
    let inline_box = props.display == skene_core::RunDisplay::InlineBox;

    with_store(|store| {
      let handle = store.create_laid_out(Kind::Run {
        props: props.clone(),
        cache: None,
        motion: RunMotion::default(),
      });

      // An inline box starts one cell wide; its own layout resizes the placeholder.
      if inline_box && let Some(record) = store.record_mut(handle) {
        record.text = crate::layout::INLINE_BOX.to_string();
      }

      handle
    })
  }

  fn update_run(&self, sink: &Handle, slot: Slot, leaf: &StaticLeaf<RunValue<TuiExt>>) {
    with_store(|store| {
      if let Some(record) = store.record_mut(*sink) {
        record.slots.insert(slot, leaf.clone().map(SlotValue::Run));

        if let Kind::Run {
          cache: Some(cache), ..
        } = &mut record.kind
        {
          cache.generation = 0;
        }

        store.layout_dirty = true;
        store.damage(*sink);
      }
    });
  }

  fn set_text(&self, sink: &Handle, text: &str) {
    with_store(|store| {
      let Some(record) = store.record_mut(*sink)
      else {
        return;
      };

      if record.text == text {
        return;
      }

      text.clone_into(&mut record.text);
      store.invalidate_selection(*sink);
      store.layout_dirty = true;
      store.damage(*sink);
    });
  }

  fn create_anchor(&self) -> Handle {
    with_store(|store| store.create(Kind::Anchor, None))
  }

  fn create_scope(&self) -> Handle {
    with_store(|store| store.create(Kind::Scope, None))
  }

  fn attach(&self, parent: &Handle, child: &Handle, before: Option<&Handle>) {
    with_store(|store| {
      let previous = store.parent(*child);

      if let Some(previous) = previous
        && let Some(record) = store.record_mut(previous)
      {
        record.children.retain(|existing| existing != child);
      }

      let Some(record) = store.record_mut(*parent)
      else {
        return;
      };

      record.children.retain(|existing| existing != child);

      let index = before
        .and_then(|before| record.children.iter().position(|node| node == before))
        .unwrap_or(record.children.len());

      record.children.insert(index, *child);

      if let Some(record) = store.record_mut(*child) {
        record.parent = Some(*parent);
      }

      store.structure_dirty = true;
      store.paint_dirty = true;
    });
  }

  fn detach(&self, parent: &Handle, child: &Handle) {
    with_store(|store| {
      if let Some(record) = store.record_mut(*parent) {
        record.children.retain(|existing| existing != child);
      }

      if let Some(record) = store.record_mut(*child)
        && record.parent == Some(*parent)
      {
        record.parent = None;
      }

      store.structure_dirty = true;
      store.paint_dirty = true;
    });
  }

  fn destroy(&self, sink: Handle) {
    with_store(|store| store.destroy(sink));
  }

  fn define_tokens(&self, _scope: &Handle, _definitions: &Definitions<TokenValue>) {
    with_store(Store::invalidate_styles);
  }

  fn define_root_tokens(&self, _definitions: &Definitions<TokenValue>) {
    with_store(Store::invalidate_styles);
  }

  fn listen(&self, sink: &Handle, kind: EventKind) {
    with_store(|store| {
      if let Some(record) = store.record_mut(*sink) {
        record.listened |= 1 << (kind as u16);
      }
    });
  }

  fn localize_pointer(&self, sink: &Handle, event: &PointerEvent) -> PointerEvent {
    let (x, y) = with_store(|store| store.screen_origin(*sink));

    #[expect(clippy::cast_precision_loss)]
    PointerEvent {
      local_x: event.x - x as f32,
      local_y: event.y - y as f32,
      ..*event
    }
  }

  fn focus(&self, sink: &Handle) {
    interaction::set_focus(Some(*sink));
  }

  fn blur(&self, sink: &Handle) {
    let holds = with_store(|store| store.focused == Some(*sink));

    if holds {
      interaction::set_focus(None);
    }
  }

  fn set_value(&self, sink: &Handle, value: &str) {
    with_store(|store| {
      let Some(record) = store.record_mut(*sink)
      else {
        return;
      };
      let Some(field) = record.kind.field_mut()
      else {
        return;
      };

      if !field.set_value(value) {
        return;
      }

      store.settle_entry(*sink);
      store.layout_dirty = true;
      store.damage(*sink);
    });

    crate::field::report_selections();
  }

  fn set_selection(&self, sink: &Handle, selection: TextSelection) {
    with_store(|store| {
      let Some(field) = store
        .record_mut(*sink)
        .and_then(|record| record.kind.field_mut())
      else {
        return;
      };

      if !field.set_selection(selection) {
        return;
      }

      store.settle_entry(*sink);
      store.damage(*sink);
    });
  }

  fn set_checked(&self, sink: &Handle, checked: bool) {
    with_store(|store| {
      if let Some(Kind::Control { checked: held, .. }) =
        store.record_mut(*sink).map(|record| &mut record.kind)
      {
        *held = checked;
      }
    });
  }

  fn observe_size(&self, sink: &Handle, report: Rc<dyn Fn(Size<u32>)>) {
    with_store(|store| {
      if let Some(record) = store.record_mut(*sink) {
        record.observers.push(report);
        record.reported = None;
        store.layout_dirty = true;
      }
    });
  }

  fn media(&self) -> MediaSignals<u32> {
    with_store(|store| store.media().signals)
  }

  fn set_scheme(&self, scheme: Option<ColorScheme>) {
    let write = with_store(|store| {
      let before = store.scheme();

      store.choice = scheme;

      let after = store.scheme();

      if before == after {
        return None;
      }

      store.invalidate_styles();

      Some((store.media().set_scheme, after))
    });

    if let Some((write, scheme)) = write {
      write.set(scheme);
    }
  }

  fn schedule(&self, delay: std::time::Duration, action: Box<dyn FnOnce()>) -> Timer {
    with_store(|store| {
      let timer = Timer(store.next_timer);
      let due = store.now() + delay.as_secs_f64();

      store.next_timer += 1;
      store.timers.push((due, timer, action));

      timer
    })
  }

  fn cancel(&self, timer: Timer) {
    with_store(|store| store.timers.retain(|(_, held, _)| *held != timer));
  }

  fn clipboard_availability(&self, action: ClipboardAction) -> Availability {
    with_store(|store| {
      // A clipboard that answers offers every operation; the launch choice stands
      // for one that does not.
      let offered = store.clipboard_answers
        || match action {
          ClipboardAction::Copy | ClipboardAction::Cut => store.clipboard.copy,
          ClipboardAction::Paste => store.clipboard.paste,
        };

      if !offered {
        return Availability::Absent;
      }

      // Outside an entry, only the document's selection is copyable, and nothing is
      // cut or pasted; inside one, every action is listed and applies to its selection.
      let entry = store.clipboard_entry();
      let applies = match action {
        ClipboardAction::Copy => store.has_selection(),
        ClipboardAction::Cut => entry.is_some_and(|entry| store.entry_has_selection(entry)),
        ClipboardAction::Paste => entry.is_some(),
      };

      match (entry, action) {
        (None, ClipboardAction::Copy) if !applies => Availability::Absent,
        (None, ClipboardAction::Cut | ClipboardAction::Paste) => Availability::Absent,
        _ if applies => Availability::Enabled,
        _ => Availability::Disabled,
      }
    })
  }

  fn clipboard(&self, action: ClipboardAction) {
    match action {
      ClipboardAction::Copy => with_store(Store::copy_selection),
      ClipboardAction::Cut => crate::field::cut_focused(),
      ClipboardAction::Paste => with_store(Store::request_paste),
    }
  }

  fn stored_scheme(&self) -> Option<ColorScheme> {
    with_store(|store| store.choice)
  }
}

impl Painting for Tui {
  fn available(&self) -> ReadSignal<bool> {
    with_store(|store| store.media().graphics)
  }

  fn paint(&self, sink: &Handle, painter: Box<skene_views::Painter>) {
    with_store(|store| {
      if let Some(record) = store.record_mut(*sink) {
        record.picture = Some(PictureRecord {
          painter: Rc::from(painter),
          image: None,
        });
        store.damage(*sink);
      }
    });
  }
}

impl CellPainter for Tui {
  fn paint(&self, sink: &Handle, painter: Box<Painter>) {
    with_store(|store| {
      if let Some(record) = store.record_mut(*sink) {
        record.painter = Some(painter);
        store.damage(*sink);
      }
    });
  }
}

/// Installs the terminal platform for this thread: the reactive runtime, the host
/// executor, the scene and the store.
///
/// # Panics
///
/// Panics if it is already installed.
pub(crate) fn install() {
  skene_reactive::Runtime::init_on_ui_thread();
  skene_core::executor::install();
  crate::text::reset();

  STORE.with(|store| {
    let mut store = store.borrow_mut();

    assert!(
      store.is_none(),
      "the terminal platform is already installed for this thread",
    );

    *store = Some(Store::new());
  });

  let root = with_store(|store| store.root);
  let node = Scene::<Tui>::with(|scene| {
    // Every run inherits its text properties through the typography tokens, so the
    // terminal's defaults are defined at the root before any theme redefines them.
    scene.define_root_tokens_once(std::any::TypeId::of::<Typography>(), || {
      Definitions::of(&Typography::default())
    });

    scene.create(NodeKind::Box, root)
  });

  ROOT.with(|held| *held.borrow_mut() = Some(node));

  crate::dev::provide_restoration();
}

/// Uninstalls the terminal platform: empties the scene and the store, and resets the
/// reactive runtime and root scope, so one thread can run many independent mounts.
pub fn uninstall() {
  let owner = STORE.with(|store| {
    store
      .borrow_mut()
      .as_mut()
      .and_then(Store::take_media_owner)
  });

  if let Some(owner) = owner {
    owner.dispose();
  }

  skene_reactive::Runtime::reset();
  skene_core::reset_root_owner();
  skene_core::executor::clear();
  Scene::<Tui>::with(|scene| *scene = Scene::new());
  ROOT.with(|root| *root.borrow_mut() = None);
  STORE.with(|store| *store.borrow_mut() = None);
}

/// Builds `view` under the installed root, returning its node.
///
/// # Panics
///
/// Panics if the platform is not installed for this thread.
pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
  skene_core::root_owner().enter(|| {
    let node = view();
    let root = root_node().expect("the root node exists once installed");

    Scene::<Tui>::with(|scene| scene.attach(root, node, None));

    node
  })
}
