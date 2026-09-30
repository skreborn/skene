//! The terminal's realization of the platform: one record per node beside the scene,
//! the taffy tree those records lay out in, and the environment as signals.

use std::{cell::RefCell, rc::Rc, thread::LocalKey};

use skene_common::Painting;
use skene_core::{
  Availability,
  BoxProps,
  BoxValue,
  ClipboardAction,
  ColorScheme,
  ControlProps,
  ControlState,
  Definitions,
  Media,
  MediaSignals,
  NativeElement,
  NodeId,
  NodeKind,
  Observation,
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
use skene_reactive::{Memo, ReadSignal, Set};

use crate::{
  TokenValue,
  TuiExt,
  Typography,
  cells::{CellPainter, Painter},
  interaction,
  motion::RunMotion,
  store::{BoxRecord, Field, Kind, PictureRecord, Store},
};

/// The terminal platform: a zero-sized [`Platform`] keeping a record per node in this
/// crate's store.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Tui;

thread_local! {
  static SCENE: RefCell<Scene<Tui>> = RefCell::new(Scene::new());
  static STORE: RefCell<Option<Store>> = const { RefCell::new(None) };
  static ROOT: RefCell<Option<NodeId>> = const { RefCell::new(None) };
}

/// Ends the turn: runs the flush its writes scheduled, so their effects have taken hold
/// before the next event, frame or query.
pub(crate) fn end_turn() {
  skene_core::end_turn();
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

/// The scene node every mounted view attaches under.
pub(crate) fn root_node() -> Option<NodeId> {
  ROOT.with(|root| *root.borrow())
}

impl Platform for Tui {
  type Scalar = u32;
  type StyleExt = TuiExt;
  type TokenValue = TokenValue;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create_box(node: NodeId, props: &BoxProps<Self>) {
    with_store(|store| store.insert_laid_out(node, Kind::Box(BoxRecord::new(props.clone()))));
  }

  fn update_box(node: NodeId, slot: Slot, leaf: &StaticLeaf<BoxValue<u32, TuiExt>>) {
    with_store(|store| {
      if let Some(record) = store.record_mut(node) {
        record.slots.insert(slot, leaf.clone().into());

        if let Some(inner) = record.kind.box_record_mut() {
          inner.mark_stale();
        }

        store.layout_dirty = true;
        store.damage(node);
      }
    });
  }

  fn create_control(node: NodeId, props: &ControlProps<Self>) {
    let field = props
      .native
      .is_some_and(NativeElement::is_text_entry)
      .then(|| Box::new(Field::new(props.placeholder.clone(), props.lines)));

    with_store(|store| {
      store.insert_laid_out(
        node,
        Kind::Control {
          native: props.native,
          label: props.label.clone(),
          inner: BoxRecord::new(props.box_props.clone()),
          field,
        },
      );
    });
  }

  fn set_control_state(node: NodeId, _state: &ControlState) {
    with_store(|store| {
      store.layout_dirty = true;
      store.damage(node);
    });
  }

  fn create_overlay(node: NodeId, props: &OverlayProps<Self>) {
    with_store(|store| {
      store.insert_laid_out(
        node,
        Kind::Overlay {
          inner: BoxRecord::new(props.box_props.clone()),
          role: props.role(),
          placement: props.placement,
          backdrop: props.backdrop,
          open: false,
          position: None,
        },
      );
      store.overlays.push(node);
    });
  }

  fn set_overlay_open(node: NodeId, open: bool, _anchor: Option<NodeId>) {
    let focus = with_store(|store| store.set_overlay_open(node, open));

    if let interaction::OverlayFocus::Moved(next) = focus {
      interaction::set_focus(next);
    }
  }

  fn set_overlay_position(node: NodeId, position: Option<Point<f32>>) {
    with_store(|store| {
      if let Some(Kind::Overlay { position: held, .. }) =
        store.record_mut(node).map(|record| &mut record.kind)
      {
        #[expect(
          clippy::cast_possible_truncation,
          reason = "a pointer's cell lies within the terminal, far inside i64"
        )]
        let cell = position.map(|point| (point.x.floor() as i64, point.y.floor() as i64));

        *held = cell;
        store.paint_dirty = true;
      }
    });
  }

  fn set_overlay_placement(node: NodeId, placement: Placement) {
    with_store(|store| {
      if let Some(Kind::Overlay {
        placement: held, ..
      }) = store.record_mut(node).map(|record| &mut record.kind)
      {
        *held = placement;
        store.paint_dirty = true;
      }
    });
  }

  fn create_run(node: NodeId, props: &RunProps<Self>) {
    let inline_box = props.display == skene_core::RunDisplay::InlineBox;

    with_store(|store| {
      store.insert_laid_out(
        node,
        Kind::Run {
          props: props.clone(),
          cache: None,
          motion: RunMotion::default(),
          extent: None,
        },
      );

      // An inline box flows as one placeholder cluster, as large as its own layout.
      if inline_box && let Some(record) = store.record_mut(node) {
        record.text = crate::layout::INLINE_BOX.to_string();
      }
    });
  }

  fn update_run(node: NodeId, slot: Slot, leaf: &StaticLeaf<RunValue<TuiExt>>) {
    with_store(|store| {
      if let Some(record) = store.record_mut(node) {
        record.slots.insert(slot, leaf.clone().into());

        if let Kind::Run {
          cache: Some(cache), ..
        } = &mut record.kind
        {
          cache.generation = 0;
        }

        store.layout_dirty = true;
        store.damage(node);
      }
    });
  }

  fn set_text(node: NodeId, text: &str) {
    with_store(|store| {
      let Some(record) = store.record_mut(node)
      else {
        return;
      };

      if record.text == text {
        return;
      }

      text.clone_into(&mut record.text);
      store.invalidate_selection(node);
      store.layout_dirty = true;
      store.damage(node);
    });
  }

  fn create_anchor(node: NodeId) {
    with_store(|store| store.insert(node, Kind::Anchor, None));
  }

  fn create_scope(node: NodeId) {
    with_store(|store| store.insert(node, Kind::Scope, None));
  }

  fn attach(_parent: NodeId, _child: NodeId, _before: Option<NodeId>) {
    with_store(Store::note_structure);
  }

  fn relocate(_parent: NodeId, _child: NodeId, _before: Option<NodeId>) {
    with_store(Store::note_structure);
  }

  fn hide(_parent: NodeId, _child: NodeId) {
    with_store(Store::note_structure);
  }

  fn remove(_parent: NodeId, _child: NodeId) {
    with_store(Store::note_structure);
  }

  fn destroy(node: NodeId) {
    with_store(|store| store.destroy(node));
  }

  fn define_tokens(_scope: NodeId, _definitions: &Definitions<TokenValue>) {
    with_store(Store::invalidate_styles);
  }

  fn define_root_tokens(_definitions: &Definitions<TokenValue>) {
    with_store(Store::invalidate_styles);
  }

  fn localize_pointer(node: NodeId, event: &PointerEvent) -> PointerEvent {
    let (x, y) = with_store(|store| store.screen_origin(node));

    #[expect(
      clippy::cast_precision_loss,
      reason = "a screen origin in cells stays far below the integers f32 holds exactly"
    )]
    PointerEvent {
      local_x: event.x - x as f32,
      local_y: event.y - y as f32,
      ..*event
    }
  }

  fn focus(node: NodeId) {
    interaction::set_focus_restored(Some(node));
  }

  fn blur(node: NodeId) {
    let holds = with_store(|store| store.focused() == Some(node));

    if holds {
      interaction::set_focus(None);
    }
  }

  fn focus_visible() -> bool {
    with_store(|store| {
      store
        .focused()
        .is_some_and(|focused| store.focus_shows(focused))
    })
  }

  fn set_value(node: NodeId, value: &str) {
    with_store(|store| {
      let Some(record) = store.record_mut(node)
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

      store.settle_entry(node);
      store.layout_dirty = true;
      store.damage(node);
    });

    crate::field::report_selections();
  }

  fn set_selection(node: NodeId, selection: TextSelection) {
    with_store(|store| {
      let Some(field) = store
        .record_mut(node)
        .and_then(|record| record.kind.field_mut())
      else {
        return;
      };

      if !field.set_selection(selection) {
        return;
      }

      store.settle_entry(node);
      store.damage(node);
    });
  }

  fn set_checked(_: NodeId, _: bool) {}

  fn observe_size(node: NodeId, report: Rc<dyn Fn(Size<u32>)>) -> Observation {
    let kept = Rc::clone(&report);

    with_store(|store| {
      if let Some(record) = store.record_mut(node) {
        record.observers.push(report);
        record.reported = None;
        store.layout_dirty = true;
      }
    });

    Observation::new(move || {
      with_store(|store| {
        if let Some(record) = store.record_mut(node) {
          record
            .observers
            .retain(|observer| !Rc::ptr_eq(observer, &kept));
        }
      });
    })
  }

  fn media() -> MediaSignals<u32> {
    with_store(|store| store.media().signals)
  }

  fn watch_media(media: &Media<u32>) -> Memo<bool> {
    Self::media().watch(media)
  }

  fn set_scheme(scheme: Option<ColorScheme>) {
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

  fn schedule(delay: std::time::Duration, action: Box<dyn FnOnce()>) -> Timer {
    with_store(|store| {
      let at = store.now() + delay.as_secs_f64();

      store.timers.schedule(at, action)
    })
  }

  fn cancel(timer: Timer) {
    with_store(|store| store.timers.cancel(timer));
  }

  fn clipboard_availability(action: ClipboardAction) -> Availability {
    with_store(|store| {
      if !store.offers(action) {
        return Availability::Absent;
      }

      // Outside an entry, only the document's selection is copyable, and nothing is
      // cut or pasted; inside one, every action is listed, copying its selection, and
      // cutting and pasting where it takes edits.
      let entry = store.clipboard_entry();
      let editable = entry.is_some_and(|entry| store.is_editable(entry));
      let applies = match action {
        ClipboardAction::Copy => store.has_selection(),
        ClipboardAction::Cut => {
          editable && entry.is_some_and(|entry| store.entry_has_selection(entry))
        }
        ClipboardAction::Paste => editable,
      };

      match (entry, action) {
        (None, ClipboardAction::Copy) if !applies => Availability::Absent,
        (None, ClipboardAction::Cut | ClipboardAction::Paste) => Availability::Absent,
        _ if applies => Availability::Enabled,
        _ => Availability::Disabled,
      }
    })
  }

  fn clipboard(action: ClipboardAction) {
    match action {
      ClipboardAction::Copy => with_store(Store::copy_selection),
      ClipboardAction::Cut => crate::field::cut_focused(),
      ClipboardAction::Paste => with_store(Store::request_paste),
    }
  }

  fn stored_scheme() -> Option<ColorScheme> {
    with_store(|store| store.choice)
  }
}

impl Painting for Tui {
  fn available() -> ReadSignal<bool> {
    with_store(|store| store.media().graphics)
  }

  fn paint(node: NodeId, painter: Box<skene_common::Painter>) {
    with_store(|store| {
      if let Some(record) = store.record_mut(node) {
        record.picture = Some(PictureRecord {
          painter: Rc::from(painter),
          image: None,
        });
        store.damage(node);
      }
    });
  }
}

impl CellPainter for Tui {
  fn paint(node: NodeId, painter: Box<Painter>) {
    with_store(|store| {
      if let Some(record) = store.record_mut(node) {
        record.painter = Some(painter);
        store.damage(node);
      }
    });
  }
}

/// Installs the terminal platform for this thread: the reactive runtime, flushing at
/// the end of each turn, the host executor, the scene and the store.
///
/// # Panics
///
/// Panics if it is already installed.
pub(crate) fn install() {
  skene_core::hold_flushes(|| {});
  skene_core::executor::install();
  crate::text::reset();

  STORE.with(|store| {
    let mut store = store.borrow_mut();

    assert!(
      store.is_none(),
      "the terminal platform is already installed for this thread",
    );

    *store = Some(Store::new(Scene::<Tui>::with(|scene| {
      scene.create(NodeKind::Box)
    })));
  });

  let node = with_store(|store| store.root);
  // Every run inherits its text properties through the typography tokens, so the
  // terminal's defaults are defined at the root before any theme redefines them.
  let typography = Definitions::of(&Typography::default());

  Scene::<Tui>::with(|scene| {
    scene.define_root_tokens_once(std::any::TypeId::of::<Typography>(), typography);
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

  skene_core::reset_thread::<Tui>();
  ROOT.with(|root| *root.borrow_mut() = None);
  STORE.with(|store| *store.borrow_mut() = None);
}

/// Builds `view` under the installed root as one turn, returning its node.
///
/// # Panics
///
/// Panics if the platform is not installed for this thread.
pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
  end_turn();

  let node = skene_core::root_owner().enter(|| {
    let node = view();
    let root = root_node().expect("the root node exists once installed");

    Scene::<Tui>::with(|scene| scene.attach(root, node, None));

    node
  });

  end_turn();

  node
}
