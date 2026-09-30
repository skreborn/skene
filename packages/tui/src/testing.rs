//! The terminal's test seams, under the naming rule every platform's seams share.
//!
//! `trigger_<event>` simulates what reaches the app from outside, `take_<output>` drains
//! what the app emitted since the last take, `render_to_<form>` paints the frame into a
//! form a test reads, `is_<state>` answers a yes-or-no question, and any other read is
//! named for what it returns; [`user`] composes events into a user's gestures.

mod pty;
pub mod user;

use std::time::Duration;

use skene_core::{
  ColorScheme,
  KeyEvent,
  Modifiers,
  MouseCursor,
  NameFacts,
  NativeElement,
  NodeId,
  PointerButton,
  Role,
  RunDisplay,
  Scene,
  TreeUpdate,
};

pub use self::pty::{Pty, PtyHost};
use crate::{
  CellDecoration,
  CellScale,
  Picture,
  Tui,
  frame,
  interaction,
  platform,
  platform::end_turn,
  store::Kind,
  terminal::{
    apply_clustering,
    apply_graphics,
    apply_sizing,
    key_event,
    press,
    run_due_timers,
    set_detected_background,
    set_detected_scheme,
  },
  with_store,
};

/// Simulates the terminal's report that it reports key releases (the kitty keyboard
/// protocol's event types), or that it does not, for a test driving the loop headlessly.
///
/// Until it does, a release follows every press at once.
pub fn trigger_key_releases(confirmed: bool) {
  trigger(|| with_store(|store| store.key_releases = confirmed));
}

/// Simulates the terminal's report that its clipboard answers (mode 5522), or that it
/// does not, for a test driving the loop headlessly.
///
/// An answering clipboard offers every operation, whatever the launch chose.
pub fn trigger_answering_clipboard(confirmed: bool) {
  trigger(|| with_store(|store| store.clipboard_answers = confirmed));
}

/// Simulates the terminal's report that grapheme clustering (mode 2027) is set, or
/// that it is not, for a test driving the loop headlessly.
pub fn trigger_grapheme_clustering(confirmed: bool) {
  trigger(|| {
    apply_clustering(confirmed);
  });
}

/// Simulates the terminal's reply that proves text sizing (OSC 66), or one that does
/// not, for a test driving the loop headlessly.
pub fn trigger_text_sizing(confirmed: bool) {
  trigger(|| {
    apply_sizing(confirmed);
  });
}

/// Simulates the terminal's reply that proves the graphics protocol, or its absence,
/// for a test driving the loop headlessly.
pub fn trigger_graphics(confirmed: bool) {
  trigger(|| {
    apply_graphics(confirmed);
  });
}

/// Runs `event` as one turn of the loop, for a test driving it headlessly: what earlier
/// writes scheduled takes effect first, and what the event's own writes schedule once
/// it is handled.
fn trigger(event: impl FnOnce()) {
  end_turn();
  event();
  end_turn();
}

/// Simulates one key press by its portable W3C key value, for a test driving the loop
/// headlessly.
///
/// The value is a key's name (`"a"`, `"Enter"`, `"Tab"`, `"Backspace"`, …), and
/// `"Ctrl+"`, `"Alt+"`, and `"Shift+"` prefixes add the modifier (`"Ctrl+ArrowLeft"`,
/// `"Shift+Tab"`). Until [`trigger_key_releases`] confirms releases, the key's release
/// follows at once.
pub fn trigger_key(key: &str) {
  trigger(|| press(&key_event(key)));
}

/// Simulates one key press as `event` describes it—its key value, its physical code
/// and its modifiers—for a test driving a layout the key value alone does not place.
pub fn trigger_key_event(event: &KeyEvent) {
  trigger(|| press(event));
}

/// Whether a key asked the app to quit, for a headless test.
#[must_use]
pub fn is_quit_requested() -> bool {
  end_turn();
  with_store(|store| store.quit)
}

/// Simulates one key release by its portable W3C key value, with the prefixes
/// [`trigger_key`] reads, for a test driving the loop headlessly.
pub fn trigger_key_up(key: &str) {
  trigger(|| interaction::key_up(&key_event(key)));
}

/// The node holding focus, for a test asserting focus movement.
#[must_use]
pub fn focused_node() -> Option<skene_core::NodeId> {
  end_turn();
  with_store(|store| {
    store
      .focused()
      .and_then(|focused| store.record(focused).and_then(|record| record.node))
  })
}

/// The name `node` reports to assistive technology, by the accessible-name computation
/// ([`Scene::accessible_name`]).
#[must_use]
pub fn accessible_name(node: NodeId) -> Option<String> {
  end_turn();

  Scene::<Tui>::accessible_name(node, name_facts)
}

/// The accessibility tree, as AccessKit models it ([`Scene::access_tree`]).
#[must_use]
pub fn access_tree() -> TreeUpdate {
  end_turn();

  Scene::<Tui>::access_tree(name_facts)
}

/// The accessibility tree as indented text, one node a line ([`Scene::access_outline`]).
#[must_use]
pub fn access_outline() -> String {
  end_turn();

  Scene::<Tui>::access_outline(name_facts)
}

/// The first node in tree order whose role is `role` and whose accessible name is
/// `name`, outside hidden boxes and closed overlays.
#[must_use]
pub fn find_by_role(role: Role, name: &str) -> Option<NodeId> {
  end_turn();

  Scene::<Tui>::find_by_role(role, name, name_facts)
}

/// Every node whose role is `role`, in tree order, outside hidden boxes and closed
/// overlays.
#[must_use]
pub fn find_all_by_role(role: Role) -> Vec<NodeId> {
  end_turn();

  Scene::<Tui>::find_all_by_role(role, name_facts)
}

/// Every node that must be named and is not, in tree order, outside hidden boxes and
/// closed overlays ([`Scene::find_all_unnamed`]).
#[must_use]
pub fn find_all_unnamed() -> Vec<NodeId> {
  end_turn();

  Scene::<Tui>::find_all_unnamed(name_facts)
}

/// The first run in tree order whose own text is `text`, whitespace collapsed on both,
/// outside hidden boxes and closed overlays.
#[must_use]
pub fn find_by_text(text: &str) -> Option<NodeId> {
  end_turn();

  Scene::<Tui>::find_by_text(text, name_facts)
}

/// What the accessible-name computation reads of `node`.
fn name_facts(node: NodeId) -> NameFacts {
  with_store(|store| {
    let Some(record) = store.record(node)
    else {
      return NameFacts::default();
    };
    let mut facts = NameFacts {
      hidden: store.is_hidden(node),
      ..NameFacts::default()
    };

    if let Kind::Run { props, .. } = &record.kind {
      facts.text = Some(record.text.clone());
      facts.link = props.link.is_some();
      facts.role = facts.link.then_some(Role::Link);
      facts.block = props.display == RunDisplay::Block;

      return facts;
    }

    if let Some(inner) = record.kind.box_record() {
      facts.link = inner.props.destination.is_some();
      facts.role = inner.props.role.or(facts.link.then_some(Role::Link));
      facts.access.clone_from(&inner.props.access);
      facts.block = true;
    }

    match &record.kind {
      Kind::Overlay { role, open, .. } => {
        facts.hidden |= !open;
        facts.role = *role;
      }
      Kind::Control {
        native,
        label,
        field,
        ..
      } => {
        facts.role = facts.role.or(native.map(NativeElement::role));
        facts.label.clone_from(label);
        facts.value = field.as_ref().map(|field| field.value.clone());
        facts.placeholder = field.as_ref().and_then(|field| field.placeholder.clone());
      }
      Kind::Box(_) | Kind::Run { .. } | Kind::Anchor | Kind::Scope => {}
    }

    facts
  })
}

/// Simulates the mouse moving to the cell (`x`, `y`), for a test driving hover
/// headlessly. Hit-testing resolves against the last render's layout.
pub fn trigger_mouse_move(x: u16, y: u16) {
  trigger(|| {
    with_store(|store| store.pointer_within = crate::store::CELL_CENTER);
    interaction::pointer_move(x, y, None, Modifiers::default());
  });
}

/// Simulates a left click at the cell (`x`, `y`), for a test driving mouse interaction
/// headlessly. Hit-testing resolves against the last render's layout.
pub fn trigger_mouse_click(x: u16, y: u16) {
  trigger_mouse_press(x, y);
  trigger_mouse_release(x, y);
}

/// Simulates pressing the left button at (`x`, `y`) without releasing—the start of a
/// drag—for a test driving drags headlessly.
pub fn trigger_mouse_press(x: u16, y: u16) {
  trigger(|| {
    with_store(|store| {
      store.pointer_within = crate::store::CELL_CENTER;
      store.buttons.insert(PointerButton::Primary);
    });
    interaction::pointer_press(x, y, PointerButton::Primary, Modifiers::default());
  });
}

/// Simulates dragging to the center of the cell (`x`, `y`) with the left button held,
/// after [`trigger_mouse_press`].
pub fn trigger_mouse_drag(x: u16, y: u16) {
  trigger_mouse_drag_within(x, y, crate::store::CELL_CENTER);
}

/// Simulates dragging to the cell (`x`, `y`) with the left button held, the pointer
/// `within` the cell—`0..1` along each axis, as a terminal reporting pixels places
/// it—after [`trigger_mouse_press`].
pub fn trigger_mouse_drag_within(x: u16, y: u16, within: (f32, f32)) {
  trigger(|| {
    with_store(|store| store.pointer_within = within);
    interaction::pointer_move(x, y, Some(PointerButton::Primary), Modifiers::default());
  });
}

/// Simulates releasing the left button at (`x`, `y`), ending a drag. A release with no
/// press delivered stands in for the whole click, as the interactive loop's does.
pub fn trigger_mouse_release(x: u16, y: u16) {
  trigger(|| {
    with_store(|store| store.pointer_within = crate::store::CELL_CENTER);

    if !with_store(|store| store.buttons.contains(PointerButton::Primary)) {
      with_store(|store| store.buttons.insert(PointerButton::Primary));
      interaction::pointer_press(x, y, PointerButton::Primary, Modifiers::default());
    }

    with_store(|store| store.buttons.remove(PointerButton::Primary));
    interaction::pointer_release(x, y, PointerButton::Primary, Modifiers::default());
  });
}

/// Simulates a right click at the cell (`x`, `y`)—the copy-menu gesture—for a test
/// driving selection headlessly.
pub fn trigger_mouse_right_click(x: u16, y: u16) {
  trigger(|| {
    with_store(|store| {
      store.pointer_within = crate::store::CELL_CENTER;
      store.buttons.insert(PointerButton::Secondary);
    });
    interaction::pointer_press(x, y, PointerButton::Secondary, Modifiers::default());
    with_store(|store| store.buttons.remove(PointerButton::Secondary));
    interaction::pointer_release(x, y, PointerButton::Secondary, Modifiers::default());
  });
}

/// Simulates the mouse wheel at the cell (`x`, `y`), for a headless test driving
/// scrolling.
///
/// Positive deltas scroll right and down, one cell per unit; hit-testing resolves
/// against the last render's layout.
pub fn trigger_mouse_scroll(x: u16, y: u16, delta_columns: i64, delta_rows: i64) {
  trigger(|| interaction::wheel(x, y, delta_columns, delta_rows, Modifiers::NONE));
}

/// Takes the text the last `Copy` staged for the clipboard, for a headless test.
#[must_use]
pub fn take_copied() -> Option<String> {
  end_turn();
  with_store(|store| store.pending_copy.take())
}

/// Takes whether the last `Paste` staged a clipboard read, for a headless test.
#[must_use]
pub fn take_paste_requested() -> bool {
  end_turn();
  with_store(|store| std::mem::take(&mut store.pending_paste))
}

/// Simulates the terminal pasting `text` by its own key—a bracketed paste—into the
/// focused entry, for a headless test.
pub fn trigger_paste(text: &str) {
  trigger(|| interaction::paste(text));
}

/// Simulates the terminal answering the last clipboard read with `text`, for a headless
/// test: it lands in the entry that asked, wherever focus has moved since.
pub fn trigger_clipboard_reply(text: &str) {
  trigger(|| interaction::paste_reply(text));
}

/// Takes the link the last click staged for the system handler, for a headless test.
#[must_use]
pub fn take_opened() -> Option<String> {
  end_turn();
  with_store(|store| store.pending_open.take())
}

/// The current terminal title, for a headless test.
#[must_use]
pub fn window_title() -> Option<String> {
  end_turn();
  with_store(|store| store.title.clone())
}

/// Simulates the terminal reporting `scheme` as its background, for a headless test
/// driving the scheme conditions; the app's own choice still wins.
pub fn trigger_color_scheme(scheme: ColorScheme) {
  trigger(|| set_detected_scheme(scheme == ColorScheme::Dark));
}

/// Simulates the terminal reporting its own `foreground` and `background` (OSC 10 and
/// 11), for a headless test asserting what a paint mixes against; the scheme follows
/// the background's luminance.
pub fn trigger_terminal_colors(foreground: (u8, u8, u8), background: (u8, u8, u8)) {
  trigger(|| {
    with_store(|store| store.detected_foreground = Some(foreground));
    set_detected_background(background);
  });
}

/// Steps the motion clock forward by `elapsed`, for a headless test driving transitions,
/// animations and a caret drag's autoscroll frame by frame; the next render samples
/// them at the new reading.
///
/// A headless platform's clock stands still otherwise, so a render shows a transition
/// at the point the last step left it.
pub fn trigger_frame(elapsed: Duration) {
  trigger(|| {
    with_store(|store| {
      store.clock.advance(elapsed.as_secs_f64());
      store.motion_active = true;
    });
    run_due_timers();
  });
}

/// Simulates the reader's reduced-motion preference, for a headless test driving the
/// reduced-motion condition and the motion that respects it.
pub fn trigger_reduced_motion(reduced: bool) {
  trigger(|| {
    let write = with_store(|store| {
      if store.reduced_motion == reduced {
        return None;
      }

      store.reduced_motion = reduced;
      store.invalidate_styles();

      Some(store.media().set_reduced_motion)
    });

    if let Some(write) = write {
      skene_reactive::Set::set(&write, reduced);
    }
  });
}

/// Simulates the terminal resizing to `columns` x `rows`, for a headless test driving
/// the size conditions; the next render lays out at the size it is given.
pub fn trigger_resize(columns: u16, rows: u16) {
  trigger(|| frame::resize(columns, rows));
}

/// The pointer shape the current hover resolves to, for a headless test.
#[must_use]
pub fn mouse_cursor() -> Option<MouseCursor> {
  end_turn();
  with_store(|store| store.hover_cursor())
}

/// Whether the loop would draw a frame now, something having changed since the last
/// render, for a test asserting that a change schedules one.
#[must_use]
pub fn is_frame_pending() -> bool {
  end_turn();
  with_store(|store| store.needs_frame())
}

/// How many layouts have been computed since the platform was installed, for a test
/// asserting that a change repainted without a relayout.
#[must_use]
pub fn layout_generation() -> u64 {
  end_turn();
  with_store(|store| store.layout_generation)
}

/// How many taffy styles have been written since the platform was installed, for a test
/// counting the restyles a change caused.
#[must_use]
pub fn style_writes() -> u64 {
  end_turn();
  with_store(|store| store.set_style_calls)
}

/// How many node styles have been resolved from their declarations since the platform
/// was installed, for a test counting what a frame or a change resolved.
#[must_use]
pub fn style_resolutions() -> u64 {
  end_turn();
  with_store(|store| store.style_resolutions)
}

/// The rows the last render rewrote, for a test asserting that a change repainted only
/// the rows it touched.
#[must_use]
pub fn repainted_rows() -> Vec<usize> {
  end_turn();
  with_store(|store| store.repainted.clone())
}

/// Lays the mounted tree out at `width` x `height` cells and paints it to plain text:
/// the headless render a test asserts against.
#[must_use]
pub fn render_to_text(width: u16, height: u16) -> String {
  frame::render(width, height).to_text()
}

/// The painted frame at `width` x `height` as text with its styles, for a snapshot file
/// a test compares.
///
/// The text as [`render_to_text`] draws it, then each cell's style as a character, `.`
/// for a cell painting as the terminal's own, then the colors and attributes each
/// character stands for.
#[must_use]
pub fn render_to_styled(width: u16, height: u16) -> String {
  frame::render(width, height).to_styled()
}

/// The painted frame's caret cell at `width` x `height`—where the terminal's bar
/// cursor shows—for a headless test asserting caret placement.
#[must_use]
pub fn render_to_caret(width: u16, height: u16) -> Option<(u16, u16)> {
  frame::render(width, height).caret
}

/// The painted frame's colors at `width` x `height`: each cell's foreground and
/// background, row-major, for a headless test asserting paint.
#[must_use]
#[expect(
  clippy::type_complexity,
  reason = "a cell's two colors, plainer spelled out than behind an alias"
)]
pub fn render_to_colors(
  width: u16,
  height: u16,
) -> Vec<(Option<(u8, u8, u8)>, Option<(u8, u8, u8)>)> {
  frame::render(width, height)
    .cells
    .iter()
    .map(|cell| (cell.foreground, cell.background))
    .collect()
}

/// The painted frame's bold cells at `width` x `height`, row-major, for a headless test
/// asserting a run's or an entry's weight.
#[must_use]
pub fn render_to_bold(width: u16, height: u16) -> Vec<bool> {
  frame::render(width, height)
    .cells
    .iter()
    .map(|cell| cell.bold)
    .collect()
}

/// The painted frame's reverse-video cells at `width` x `height`, row-major, for a
/// headless test asserting the emphasis a colorless selection or a colorless focused
/// link paints in.
#[must_use]
pub fn render_to_reverse(width: u16, height: u16) -> Vec<bool> {
  frame::render(width, height)
    .cells
    .iter()
    .map(|cell| cell.reverse)
    .collect()
}

/// The painted frame's decoration lines at `width` x `height`, row-major, for a
/// headless test asserting an underline's style and color, an overline or a line
/// through.
#[must_use]
pub fn render_to_decorations(width: u16, height: u16) -> Vec<CellDecoration> {
  frame::render(width, height)
    .cells
    .iter()
    .map(|cell| cell.decoration)
    .collect()
}

/// The pictures the painted frame at `width` x `height` places, for a headless test
/// asserting what a terminal with graphics shows; none until `trigger_graphics`.
#[must_use]
pub fn render_to_pictures(width: u16, height: u16) -> Vec<Picture> {
  frame::render(width, height)
    .pictures
    .iter()
    .map(Picture::from)
    .collect()
}

/// The painted frame's text scales at `width` x `height`, row-major, for a headless
/// test asserting a run's size: a glyph's cell carries its scale, and the other cells
/// of its block carry one.
#[must_use]
pub fn render_to_scales(width: u16, height: u16) -> Vec<CellScale> {
  frame::render(width, height)
    .cells
    .iter()
    .map(|cell| cell.scale)
    .collect()
}

/// Polls every ready task as one turn, as the interactive loop does between events;
/// `true` when any ran. For a headless test driving async work.
#[must_use]
pub fn pump_tasks() -> bool {
  platform::end_turn();

  let ran = skene_core::executor::pump();

  platform::end_turn();

  ran
}
