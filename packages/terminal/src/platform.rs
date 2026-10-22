//! The terminal's realization of the platform: one record per node beside the scene,
//! the layout tree those records lay out in, and the environment as signals.

use std::{cell::RefCell, rc::Rc, thread::LocalKey};

use skene_foundation::{
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
  Painting,
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
  TerminalStyle,
  TokenValue,
  Typography,
  cells::{CellPainter, Painter},
  interaction,
  motion::RunMotion,
  records::{BoxRecord, Field, Kind, PictureRecord, Records},
};

/// The terminal platform: a zero-sized [`Platform`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Terminal;

thread_local! {
  static SCENE: RefCell<Scene<Terminal>> = RefCell::new(Scene::new());
  static STORE: RefCell<Option<Records>> = const { RefCell::new(None) };
  static ROOT: RefCell<Option<NodeId>> = const { RefCell::new(None) };
}

/// Ends the turn: runs the flush its writes scheduled, so their effects have taken hold
/// before the next event, frame or query.
pub(crate) fn end_turn() {
  skene_foundation::end_turn();
}

/// Runs `operation` on the installed records; nothing of the user's runs inside it.
///
/// # Panics
///
/// Panics if the terminal platform is not installed for this thread.
pub(crate) fn with_records<R>(operation: impl FnOnce(&mut Records) -> R) -> R {
  STORE.with(|records| {
    operation(
      records
        .borrow_mut()
        .as_mut()
        .expect("the terminal platform to be installed for this thread"),
    )
  })
}

/// Runs `operation` on the records, or `None` when none is installed.
pub(crate) fn try_with_records<R>(operation: impl FnOnce(&mut Records) -> R) -> Option<R> {
  STORE.with(|records| records.borrow_mut().as_mut().map(operation))
}

/// The scene node every mounted view attaches under.
pub(crate) fn root_node() -> Option<NodeId> {
  ROOT.with(|root| *root.borrow())
}

impl Platform for Terminal {
  const FONT_SIZE: f32 = 1.;

  type Scalar = u32;
  type StyleExt = TerminalStyle;
  type TokenValue = skene_foundation::TokenValue<u32>;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create_box(node: NodeId, props: &BoxProps<Self>) {
    with_records(|records| records.insert_laid_out(node, Kind::Box(BoxRecord::new(props.clone()))));
  }

  fn update_box(node: NodeId, slot: Slot, leaf: &StaticLeaf<BoxValue<u32, TerminalStyle>>) {
    with_records(|records| {
      if let Some(record) = records.record_mut(node) {
        record.slots.insert(slot, leaf.clone().into());

        if let Some(inner) = record.kind.box_record_mut() {
          inner.mark_stale();
        }

        records.queue_restyle(node);
        records.layout_dirty = true;
        records.damage(node);
      }
    });
  }

  fn create_control(node: NodeId, props: &ControlProps<Self>) {
    let field = props
      .native
      .is_some_and(NativeElement::is_text_entry)
      .then(|| Box::new(Field::new(node, props.placeholder.clone(), props.lines)));

    let entry = field.is_some();

    with_records(|records| {
      records.insert_laid_out(
        node,
        Kind::Control {
          native: props.native,
          label: props.label.clone(),
          inner: BoxRecord::new(props.box_props.clone()),
          field,
        },
      );

      if entry {
        records.entries.push(node);
      }
    });
  }

  fn set_control_state(node: NodeId, _state: &ControlState) {
    with_records(|records| {
      records.layout_dirty = true;
      records.damage(node);
    });
  }

  fn create_overlay(node: NodeId, props: &OverlayProps<Self>) {
    with_records(|records| {
      records.insert_laid_out(
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
      records.overlays.push(node);
    });
  }

  fn set_overlay_open(node: NodeId, open: bool, _anchor: Option<NodeId>) {
    let focus = with_records(|records| records.set_overlay_open(node, open));

    if let interaction::OverlayFocus::Moved(next) = focus {
      interaction::set_focus(next);
    }
  }

  fn set_overlay_position(node: NodeId, position: Option<Point<f32>>) {
    with_records(|records| {
      if let Some(Kind::Overlay { position: held, .. }) =
        records.record_mut(node).map(|record| &mut record.kind)
      {
        #[expect(
          clippy::cast_possible_truncation,
          reason = "a pointer's cell lies within the terminal, far inside i64"
        )]
        let cell = position.map(|point| (point.x.floor() as i64, point.y.floor() as i64));

        *held = cell;
        records.paint_dirty = true;
      }
    });
  }

  fn set_overlay_placement(node: NodeId, placement: Placement) {
    with_records(|records| {
      if let Some(Kind::Overlay {
        placement: held, ..
      }) = records.record_mut(node).map(|record| &mut record.kind)
      {
        *held = placement;
        records.paint_dirty = true;
      }
    });
  }

  fn create_run(node: NodeId, props: &RunProps<Self>) {
    let inline_box = props.placement == skene_foundation::RunPlacement::Embedded;

    with_records(|records| {
      records.insert_laid_out(
        node,
        Kind::Run {
          props: props.clone(),
          cache: None,
          motion: RunMotion::default(),
          extent: None,
        },
      );

      // An inline box flows as one placeholder cluster, as large as its own layout.
      if inline_box && let Some(record) = records.record_mut(node) {
        record.text = crate::layout::INLINE_BOX.to_string();
      }
    });
  }

  fn update_run(node: NodeId, slot: Slot, leaf: &StaticLeaf<RunValue<TerminalStyle>>) {
    with_records(|records| {
      if let Some(record) = records.record_mut(node) {
        record.slots.insert(slot, leaf.clone().into());

        if let Kind::Run {
          cache: Some(cache), ..
        } = &mut record.kind
        {
          cache.generation = 0;
        }

        records.queue_restyle(node);
        records.layout_dirty = true;
        records.damage(node);
      }
    });
  }

  fn set_text(node: NodeId, text: &str) {
    with_records(|records| {
      let Some(record) = records.record_mut(node)
      else {
        return;
      };

      if record.text == text {
        return;
      }

      text.clone_into(&mut record.text);
      records.invalidate_selection(node);
      records.queue_restyle(node);
      records.layout_dirty = true;
      records.damage(node);
    });
  }

  fn create_anchor(node: NodeId) {
    with_records(|records| records.insert(node, Kind::Anchor));
  }

  fn create_scope(node: NodeId) {
    with_records(|records| records.insert(node, Kind::Scope));
  }

  fn attach(_parent: NodeId, _child: NodeId, _before: Option<NodeId>) {
    with_records(Records::note_structure);
  }

  fn relocate(_parent: NodeId, _child: NodeId, _before: Option<NodeId>) {
    with_records(Records::note_structure);
  }

  fn hide(_parent: NodeId, _child: NodeId) {
    with_records(Records::note_structure);
  }

  fn remove(_parent: NodeId, _child: NodeId) {
    with_records(Records::note_structure);
  }

  fn destroy(node: NodeId) {
    with_records(|records| records.destroy(node));
  }

  fn define_tokens(_scope: NodeId, _definitions: &Definitions<TokenValue>) {
    with_records(Records::invalidate_styles);
  }

  fn define_root_tokens(_definitions: &Definitions<TokenValue>) {
    with_records(Records::invalidate_styles);
  }

  fn localize_pointer(node: NodeId, event: &PointerEvent) -> PointerEvent {
    let (x, y) = with_records(|records| records.screen_origin(node));

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
    let holds = with_records(|records| records.focused() == Some(node));

    if holds {
      interaction::set_focus(None);
    }
  }

  fn focus_visible() -> bool {
    with_records(|records| {
      records
        .focused()
        .is_some_and(|focused| records.focus_shows(focused))
    })
  }

  fn set_value(node: NodeId, value: &str) {
    with_records(|records| records.write_entry(node, value));
    crate::field::report_selections();
  }

  fn set_selection(node: NodeId, selection: TextSelection) {
    with_records(|records| records.place_entry_selection(node, selection));
  }

  fn lay_out() {
    let (width, height) = with_records(|records| records.last_size);

    crate::frame::settle(width, height);
  }

  fn set_checked(_: NodeId, _: bool) {}

  fn observe_size(node: NodeId, report: Rc<dyn Fn(Size<u32>)>) -> Observation {
    let kept = Rc::clone(&report);

    with_records(|records| {
      if let Some(record) = records.record_mut(node) {
        record.observers.push(report);
        record.reported = None;
        records.layout_dirty = true;
      }
    });

    Observation::new(move || {
      with_records(|records| {
        if let Some(record) = records.record_mut(node) {
          record
            .observers
            .retain(|observer| !Rc::ptr_eq(observer, &kept));
        }
      });
    })
  }

  fn media() -> MediaSignals<u32> {
    with_records(|records| records.media().signals)
  }

  fn watch_media(media: &Media<u32>) -> Memo<bool> {
    Self::media().watch(media)
  }

  fn set_scheme(scheme: Option<ColorScheme>) {
    let write = with_records(|records| {
      let before = records.scheme();

      records.choice = scheme;

      let after = records.scheme();

      if before == after {
        return None;
      }

      records.invalidate_styles();

      Some((records.media().set_scheme, after))
    });

    if let Some((write, scheme)) = write {
      write.set(scheme);
    }
  }

  fn schedule(delay: std::time::Duration, action: Box<dyn FnOnce()>) -> Timer {
    with_records(|records| {
      let at = records.now() + delay.as_secs_f64();

      records.timers.schedule(at, action)
    })
  }

  fn cancel(timer: Timer) {
    with_records(|records| records.timers.cancel(timer));
  }

  fn clipboard_availability(action: ClipboardAction) -> Availability {
    with_records(|records| {
      if !records.offers(action) {
        return Availability::Absent;
      }

      // Outside an entry, only the document's selection is copyable, and nothing is
      // cut or pasted; inside one, every action is listed, copying its selection, and
      // cutting and pasting where it takes edits.
      let entry = records.clipboard_entry();
      let editable = entry.is_some_and(|entry| records.is_editable(entry));
      let applies = match action {
        ClipboardAction::Copy => records.has_selection(),
        ClipboardAction::Cut => {
          editable && entry.is_some_and(|entry| records.entry_has_selection(entry))
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
      ClipboardAction::Copy => with_records(Records::copy_selection),
      ClipboardAction::Cut => crate::field::cut_focused(),
      ClipboardAction::Paste => with_records(Records::request_paste),
    }
  }

  fn chosen_scheme() -> Option<ColorScheme> {
    with_records(|records| records.choice)
  }

  fn set_window_title(title: Option<&str>) {
    with_records(|records| {
      records.title = title.map(str::to_owned);
      records.title_dirty = true;
    });
  }
}

impl Painting for Terminal {
  fn available() -> ReadSignal<bool> {
    with_records(|records| records.media().graphics)
  }

  fn paint(node: NodeId, painter: Box<skene_foundation::Painter>) {
    with_records(|records| {
      if let Some(record) = records.record_mut(node) {
        record.picture = Some(PictureRecord {
          painter: Rc::from(painter),
          image: None,
        });
        records.damage(node);
      }
    });
  }
}

impl CellPainter for Terminal {
  fn paint(node: NodeId, painter: Box<Painter>) {
    with_records(|records| {
      if let Some(record) = records.record_mut(node) {
        record.painter = Some(painter);
        records.damage(node);
      }
    });
  }
}

/// Installs the terminal platform for this thread: the reactive runtime, flushing at
/// the end of each turn, the host executor, the scene and the records.
///
/// # Panics
///
/// Panics if it is already installed.
pub(crate) fn install() {
  skene_foundation::hold_flushes(|| {});
  skene_foundation::task::install();
  crate::text::reset();

  STORE.with(|records| {
    let mut records = records.borrow_mut();

    assert!(
      records.is_none(),
      "the terminal platform is already installed for this thread",
    );

    *records = Some(Records::new(Scene::<Terminal>::with(|scene| {
      scene.create(NodeKind::Box, None)
    })));
  });

  let node = with_records(|records| records.root);

  // Every run inherits its text properties through the typography tokens, so the
  // terminal's defaults are defined at the root before any theme redefines them.
  <Typography as skene_foundation::ThemedIn<Terminal>>::define_root();
  ROOT.with(|held| *held.borrow_mut() = Some(node));

  crate::dev::provide_restoration();
}

/// Uninstalls the terminal platform: empties the scene and the records, and resets the
/// reactive runtime and root scope, so one thread can run many independent mounts.
pub fn uninstall() {
  let owner = STORE.with(|records| {
    records
      .borrow_mut()
      .as_mut()
      .and_then(Records::take_media_owner)
  });

  if let Some(owner) = owner {
    owner.dispose();
  }

  skene_foundation::reset_thread::<Terminal>();
  ROOT.with(|root| *root.borrow_mut() = None);
  STORE.with(|records| *records.borrow_mut() = None);
}

/// Builds `view` under the installed root as one turn, returning its node.
///
/// # Panics
///
/// Panics if the platform is not installed for this thread.
pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
  end_turn();

  let node = skene_reactive::Owner::root().enter(|| {
    let node = view();
    let root = root_node().expect("the root node exists once installed");

    Scene::<Terminal>::with(|scene| scene.attach(root, node, None));

    node
  });

  end_turn();

  node
}
