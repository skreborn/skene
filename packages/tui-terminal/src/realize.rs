//! The terminal's realization of the platform: the drawn runtime's adapter for everything
//! that is no cell's, beside each node's cell state, the layout tree its records lay out
//! in, and the terminal's own focus, layout, clock, clipboard and title.

use std::{any::TypeId, cell::RefCell, rc::Rc, thread::LocalKey};

use skene_foundation::{
  Availability,
  BoxStyle,
  ClipboardAction,
  ColorScheme,
  CompiledStyle,
  Content,
  ContentValue,
  ControlState,
  Definitions,
  Inset,
  Keep,
  Media,
  MediaSignals,
  NodeId,
  NodeKind,
  NodeSpec,
  Observation,
  Painting,
  Plain,
  PointerEvent,
  Realization,
  Resolver,
  RunStyle,
  Scene,
  Size,
  Slot,
  SlotLeaf,
  StaticLeaf,
  SystemColors,
  Timer,
  TokenScope,
  terminal::{BOLD_WEIGHT, BorderGlyphs, TerminalStyle, Typography},
  text::{ParagraphStyle, TextStyle, editing::Entry},
  typography,
};
use skene_layout::{
  Borders,
  runtime::{Drawn, OverlayFocus, Runtime},
};
use skene_reactive::{Memo, ReadSignal};

use crate::{
  CellEngine,
  Terminal,
  Unadoptable,
  cells::{CellPainter, Painter},
  interaction,
  paint::ShadowPaint,
  records::{Field, PictureRecord, Records},
};

thread_local! {
  static SCENE: RefCell<Scene<Terminal>> = RefCell::new(Scene::new());
  static STORE: RefCell<Option<Records>> = const { RefCell::new(None) };
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

impl Drawn for Terminal {
  type Engine = CellEngine;

  const BORDERS: Borders = Borders::Unit;
  const ROUNDED: bool = true;

  fn with_runtime<R>(operation: impl FnOnce(&mut Runtime<Self>) -> R) -> Option<R> {
    try_with_records(|records| operation(&mut records.runtime))
  }

  fn sizes_text() -> bool {
    crate::text::sizing()
  }

  /// A corner radius above zero draws rounded border glyphs where the box sets none.
  fn finish_box(style: &mut BoxStyle<i32, Plain, TerminalStyle>) {
    if style.ext.border_glyphs.is_none()
      && style.ext.corner_radius.is_some_and(|radius| radius.0 > 0.)
    {
      style.ext.border_glyphs = Some(BorderGlyphs::ROUNDED);
    }
  }

  /// A size or weight the run leaves unset is the typography's in force at the run, and
  /// a run that says nothing of bold is bold from its weight.
  fn finish_run(
    style: &mut RunStyle<Plain, TerminalStyle>,
    resolver: &Resolver<'_, i32, TerminalStyle>,
  ) {
    if style.ext.size.is_none() {
      style.ext.size = resolver.keyframe_run_value(&StaticLeaf::from(typography::SIZE));
    }

    if style.ext.weight.is_none() {
      style.ext.weight = resolver.keyframe_run_value(&StaticLeaf::from(typography::WEIGHT));
    }

    if style.ext.bold.is_none() {
      style.ext.bold = style.ext.weight.map(|weight| weight >= BOLD_WEIGHT);
    }
  }

  /// A shadow reaches past the box by its band of cells, down and to the end.
  fn reach(style: &BoxStyle<i32, Plain, TerminalStyle>) -> Inset<f32> {
    let band = f32::from(ShadowPaint::band(style));

    Inset {
      top: 0.,
      end: band,
      bottom: band,
      start: 0.,
    }
  }

  fn engine() -> CellEngine {
    crate::engine::engine()
  }

  fn paragraph_style() -> ParagraphStyle {
    crate::engine::paragraph_style()
  }

  /// A run lays out at the scale its size draws at, one cell where the terminal does not
  /// size text, a line holding no run at one cell.
  fn text_style(run: Option<&RunStyle<Plain, TerminalStyle>>) -> TextStyle {
    crate::text::scale_of(run.and_then(|run| run.ext.size)).style()
  }
}

impl Realization for Terminal {
  const FONT_SIZE: f32 = 1.;
  const KEEP: Keep = Keep::Nothing;

  type Scalar = i32;
  type StyleExt = TerminalStyle;
  type TokenValue = skene_foundation::TokenValue<i32>;
  type Native = Unadoptable;
  type Entry = Entry<CellEngine>;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create(node: NodeId, spec: &NodeSpec) -> bool {
    with_records(|records| records.create(node, spec))
  }

  fn apply_style(node: NodeId, style: CompiledStyle<i32, TerminalStyle>) {
    with_records(|records| records.runtime.apply_style(node, style));
  }

  fn apply_slot(node: NodeId, slot: Slot, leaf: &SlotLeaf<i32, TerminalStyle>) {
    with_records(|records| records.runtime.apply_slot(node, slot, leaf));
  }

  fn set_control_state(node: NodeId, _state: &ControlState) {
    with_records(|records| records.runtime.set_control_state(node));
  }

  fn set_overlay_open(node: NodeId, open: bool, _anchor: Option<NodeId>) {
    let focus = with_records(|records| records.runtime.set_overlay_open(node, open));

    if let OverlayFocus::Moved(next) = focus {
      interaction::set_focus(Some(next));
    }
  }

  fn entry(node: NodeId, multiline: bool) -> Option<Entry<CellEngine>> {
    Some(Field::entry(node, multiline))
  }

  fn adopt(_node: NodeId, native: Unadoptable, _spec: &NodeSpec) -> bool {
    match native {}
  }

  fn write(node: NodeId, value: ContentValue<'_>) {
    match value {
      ContentValue::Edit(edit) => {
        with_records(|records| records.show_entry(node, edit));
        crate::field::report_selections();
      }
      ContentValue::Placeholder(hint) => with_records(|records| {
        if let Some(field) = records.field_mut(node) {
          field.placeholder = Some(hint.to_owned());
          records.runtime.layout_dirty = true;
          records.runtime.damage(node);
        }
      }),
      ContentValue::Secure(secure) => {
        with_records(|records| records.set_entry_secure(node, secure));
      }
      ContentValue::Keyboard(_) | ContentValue::Door { .. } => {}
      value => with_records(|records| {
        if records.runtime.write(node, value) {
          records.invalidate_selection(node);
        }
      }),
    }
  }

  fn read<R>(
    node: NodeId,
    content: Content,
    read: impl FnOnce(ContentValue<'_>) -> R,
  ) -> Option<R> {
    if content != Content::Edit {
      return None;
    }

    let shown = try_with_records(|records| records.entry(node)?.shown()).flatten()?;

    Some(read(ContentValue::Edit(&shown)))
  }

  fn attach(_parent: NodeId, _child: NodeId, _before: Option<NodeId>) {
    with_records(|records| records.runtime.note_structure());
  }

  fn relocate(_parent: NodeId, _child: NodeId, _before: Option<NodeId>) {
    with_records(|records| records.runtime.note_structure());
  }

  fn hide(_parent: NodeId, _child: NodeId) {
    with_records(|records| records.runtime.note_structure());
  }

  fn remove(_parent: NodeId, _child: NodeId) {
    with_records(|records| records.runtime.note_structure());
  }

  fn destroy(node: NodeId) {
    with_records(|records| records.destroy(node));
  }

  fn define_tokens(_scope: TokenScope, _definitions: &Definitions<Self::TokenValue>) {
    with_records(|records| records.runtime.invalidate_styles());
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

  fn lay_out() {
    let (width, height) = with_records(|records| records.screen_size());

    crate::frame::settle(width, height);
  }

  fn observe_size(node: NodeId, report: Rc<dyn Fn(Size<i32>)>) -> Observation {
    Runtime::<Self>::observe_size(node, report)
  }

  fn media() -> MediaSignals<i32> {
    with_records(|records| records.runtime.media())
  }

  fn watch_media(media: &Media<i32>) -> Memo<bool> {
    Self::media().watch(media)
  }

  fn set_scheme(scheme: Option<ColorScheme>) {
    let pending = Runtime::<Self>::report(|facts| facts.choice = scheme);

    define_system_colors();
    pending.write();
  }

  fn schedule(delay: std::time::Duration, action: Box<dyn FnOnce()>) -> Timer {
    with_records(|records| {
      let at = records.runtime.now() + delay.as_secs_f64();

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
      // cutting and pasting where it takes edits, a secure entry neither copying nor
      // cutting.
      let entry = records.clipboard_entry();
      let editable = entry.is_some_and(|entry| records.is_editable(entry));
      let secure = entry.is_some_and(|entry| records.is_secure_entry(entry));
      let applies = match action {
        ClipboardAction::Copy => !secure && records.has_selection(),
        ClipboardAction::Cut => {
          !secure && editable && entry.is_some_and(|entry| records.entry_has_selection(entry))
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
    with_records(|records| records.runtime.chosen_scheme())
  }

  fn set_window_title(title: Option<&str>) {
    with_records(|records| {
      records.title = title.map(str::to_owned);
      records.title_dirty = true;
    });
  }

  /// The terminal has no view transitions: the update runs at once.
  fn commit_navigation(update: Box<dyn FnOnce()>) {
    update();
  }
}

impl Painting for Terminal {
  fn available() -> ReadSignal<bool> {
    with_records(|records| records.runtime.painting_available())
  }

  fn paint(node: NodeId, painter: Box<skene_foundation::Painter>) {
    with_records(|records| {
      if let Some(cell) = records.cell_mut(node) {
        cell.picture = Some(PictureRecord {
          painter: Rc::from(painter),
          image: None,
        });
        records.runtime.damage(node);
      }
    });
  }
}

impl CellPainter for Terminal {
  fn paint(node: NodeId, painter: Box<Painter>) {
    with_records(|records| {
      if let Some(cell) = records.cell_mut(node) {
        cell.painter = Some(painter);
        records.runtime.damage(node);
      }
    });
  }
}

/// Installs the terminal platform for this thread: the reactive runtime, flushing at
/// the end of each turn, the host executor, the scene and the records, the app taking
/// the mouse where `mouse` holds.
///
/// # Panics
///
/// Panics if it is already installed.
pub(crate) fn install(mouse: bool) {
  skene_foundation::hold_flushes(|| {});
  skene_foundation::task::install();
  crate::text::reset();

  STORE.with(|records| {
    let mut records = records.borrow_mut();

    assert!(
      records.is_none(),
      "the terminal platform is already installed for this thread",
    );

    *records = Some(Records::new(
      Scene::<Terminal>::with(|scene| scene.create(NodeKind::Box)),
      mouse,
    ));
  });

  // Every run inherits its text properties through the typography tokens, so the
  // terminal's defaults are defined at the root before any theme redefines them.
  <Typography as skene_foundation::ThemedIn<Terminal>>::define_root();
  define_system_colors();

  crate::dev::provide_restoration();
}

/// Defines the platform's colors ([`SystemColors`]) at the root from what the terminal
/// reports, redefining them where that moved; definitions equal to those in force change
/// nothing.
pub(crate) fn define_system_colors() {
  let definitions = Definitions::of(&with_records(|records| records.system_colors()));

  Scene::<Terminal>::with(|scene| {
    if scene.define_root_set(TypeId::of::<SystemColors<i32>>(), definitions.clone()) {
      return;
    }

    let mut root = scene.root_definitions().clone();

    root.overlay(definitions);
    scene.define_tokens(TokenScope::Root, root);
  });
}

/// Uninstalls the terminal platform: empties the scene and the records, and resets the
/// reactive runtime and root scope, so one thread can run many independent mounts.
pub fn uninstall() {
  let owner = try_with_records(|records| records.runtime.take_media_owner()).flatten();

  if let Some(owner) = owner {
    owner.dispose();
  }

  skene_foundation::reset_thread::<Terminal>();
  STORE.with(|records| *records.borrow_mut() = None);
}

/// Builds `view` under the installed root as one turn, returning its node.
///
/// The first mount opens the window: once its turn ends, the control set to autofocus
/// that comes first in the scene's child order takes focus, without showing it; nothing
/// is focused where no control asked.
///
/// # Panics
///
/// Panics if the platform is not installed for this thread.
pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
  Runtime::<Terminal>::mount(view)
}
