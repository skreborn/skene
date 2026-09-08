//! The interactive terminal loop, and the seams that drive it without a terminal.
//!
//! [`launch`] takes over the terminal (raw mode, the alternate screen, bracketed
//! paste), builds the app, and runs the event loop: keyboard and mouse events dispatch
//! inside a reactive batch through core's dispatch, a paste the terminal brackets
//! lands in the focused entry, and every change repaints. `Ctrl-Q` quits.
//!
//! The terminal's replies arrive as events in the stream the keys come through. Launch
//! writes its queries and paints at once under its assumptions—a dark background, 16
//! colors, the legacy key encoding—and each reply upgrades one whenever it lands. A
//! clipboard read fires on demand and inserts when its report arrives.

use std::{
  io::{self, Write},
  time::Duration,
};

use skene_core::{ColorScheme, IntoView, KeyEvent, Modifiers, MouseCursor, PointerButton};
use termina::{
  Event,
  OneBased,
  PlatformTerminal,
  Terminal as _,
  escape::{
    csi::{
      self,
      Csi,
      DecPrivateMode,
      DecPrivateModeCode,
      Keyboard,
      KittyKeyboardFlags,
      Mode,
      SetKeyboardFlagsMode,
      Sgr,
      ThemeMode,
    },
    dcs::{Dcs, DcsRequest, DcsResponse},
    osc::{ColorOrQuery, DynamicColorNumber, Osc, Selection},
  },
  event::{KeyCode, KeyEventKind, Modifiers as TerminalModifiers, MouseButton, MouseEventKind},
  style::{ColorSpec, CursorStyle, Intensity, Underline},
};

use crate::{
  Tui,
  View as _,
  detect::{self, ColorDepth},
  frame,
  interaction::{self, WHEEL_STEP},
  paint,
  with_store,
};

/// How long the loop waits for input while nothing moves.
const IDLE_POLL: Duration = Duration::from_millis(100);

/// How long the loop waits for input between frames while a tween or an animation
/// runs: the frame rate's cap.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// A DEC private mode set or reset, as one control.
fn dec_mode(code: DecPrivateModeCode, on: bool) -> Csi {
  let mode = DecPrivateMode::Code(code);

  Csi::Mode(if on {
    Mode::SetDecPrivateMode(mode)
  }
  else {
    Mode::ResetDecPrivateMode(mode)
  })
}

/// Writes the controls that take the terminal over: the alternate screen, mouse
/// reporting, bracketed paste, scheme notifications, and the cursor hidden.
fn write_setup(out: &mut impl Write) -> io::Result<()> {
  write!(
    out,
    "{}{}{}{}{}{}{}{}",
    dec_mode(DecPrivateModeCode::ClearAndEnableAlternateScreen, true),
    dec_mode(DecPrivateModeCode::MouseTracking, true),
    dec_mode(DecPrivateModeCode::ButtonEventMouse, true),
    dec_mode(DecPrivateModeCode::AnyEventMouse, true),
    dec_mode(DecPrivateModeCode::SGRMouse, true),
    dec_mode(DecPrivateModeCode::BracketedPaste, true),
    dec_mode(DecPrivateModeCode::Theme, true),
    dec_mode(DecPrivateModeCode::ShowCursor, false),
  )
}

/// Writes the queries whose replies upgrade the launch assumptions: the scheme (a 2031
/// report, the OSC 11 background), the kitty keyboard flags, and the color depth (a
/// DECRQSS report after a true-color and after a palette background).
fn write_queries(out: &mut impl Write) -> io::Result<()> {
  write!(
    out,
    "{}{}{}{}{}{}{}{}",
    Csi::Mode(Mode::QueryTheme),
    Osc::ChangeDynamicColors(
      DynamicColorNumber::TextBackgroundColor,
      vec![ColorOrQuery::Query]
    ),
    Csi::Keyboard(Keyboard::QueryFlags),
    Csi::Sgr(Sgr::Background(detect::TRUE_COLOR_PROBE.into())),
    Dcs::Request(DcsRequest::GraphicRendition),
    Csi::Sgr(Sgr::Background(ColorSpec::PaletteIndex(
      detect::PALETTE_PROBE
    ))),
    Dcs::Request(DcsRequest::GraphicRendition),
    Csi::Sgr(Sgr::Reset),
  )
}

/// Writes the controls that hand the terminal back: the legacy key encoding, mouse
/// reporting, bracketed paste and scheme notifications off, the cursor shown in its
/// default shape, the main screen. Idempotent, so it holds however many app instances
/// set the terminal up.
fn write_restore(out: &mut impl Write) -> io::Result<()> {
  write!(
    out,
    "{}{}{}{}{}{}{}{}{}{}",
    // The stackless form of the kitty keyboard protocol (`CSI = flags ; 1 u`).
    Csi::Keyboard(Keyboard::SetFlags {
      flags: KittyKeyboardFlags::empty(),
      mode: SetKeyboardFlagsMode::AssignAll,
    }),
    dec_mode(DecPrivateModeCode::Theme, false),
    dec_mode(DecPrivateModeCode::BracketedPaste, false),
    dec_mode(DecPrivateModeCode::SGRMouse, false),
    dec_mode(DecPrivateModeCode::AnyEventMouse, false),
    dec_mode(DecPrivateModeCode::ButtonEventMouse, false),
    dec_mode(DecPrivateModeCode::MouseTracking, false),
    Csi::Cursor(csi::Cursor::CursorStyle(CursorStyle::Default)),
    dec_mode(DecPrivateModeCode::ShowCursor, true),
    dec_mode(DecPrivateModeCode::ClearAndEnableAlternateScreen, false),
  )
}

/// Mounts and runs a skene application in the terminal.
///
/// Installs the terminal driver, builds `app`, and runs the event loop until `Ctrl-Q`.
/// The first frame paints for a dark background at 16 colors; the terminal's replies
/// arrive as events and upgrade the scheme, the depth and the key encoding as they
/// land. A panic anywhere in the process restores the terminal before its message
/// prints.
///
/// # Panics
///
/// Panics if the driver is already installed, or when the terminal cannot be set up.
// The loop body in one place; length is the event count, not complexity.
#[expect(clippy::too_many_lines)]
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Tui>,
{
  crate::install();
  with_store(|store| store.clock = crate::store::Clock::Real(std::time::Instant::now()));

  crate::dev::install();

  let _ = crate::mount(|| app().into_view().build());

  let mut terminal = PlatformTerminal::new().expect("open the terminal");

  terminal.enter_raw_mode().expect("enable raw mode");
  terminal.set_panic_hook(|handle| {
    let _ = write_restore(handle);
    let _ = handle.flush();
  });

  {
    crate::dev::adopt_restoration();

    if let Some(notice) = crate::dev::notice_signal() {
      let _ = crate::mount(|| crate::dev::notice_view(notice).into_view().build());
    }
  }

  let _ = write_setup(&mut terminal);
  let _ = write_queries(&mut terminal);
  let _ = terminal.flush();

  let mut depth = ColorDepth::Ansi16;
  let mut last_frame: Option<paint::Buffer> = None;

  loop {
    let _ = skene_core::executor::pump();

    let title = with_store(|store| {
      store
        .title_dirty
        .then(|| store.title.clone().unwrap_or_default())
    });

    if let Some(title) = title {
      with_store(|store| store.title_dirty = false);
      emit_title(&mut terminal, &title);
    }

    let dirty = with_store(|store| store.needs_frame());

    if dirty {
      // A degenerate reported size (a fresh pseudo-terminal) falls back to a classic
      // 80x24 rather than painting nothing.
      let (width, height) = match terminal.get_dimensions() {
        Ok(size) if size.cols > 0 && size.rows > 0 => (size.cols, size.rows),
        _ => crate::store::DEFAULT_SIZE,
      };
      let frame = frame::render(width, height);

      draw(&mut terminal, &frame, depth, last_frame.as_ref());
      last_frame = Some(frame);
    }

    let (quit, copy, read, open) = with_store(|store| {
      (
        store.quit,
        store.pending_copy.take(),
        std::mem::take(&mut store.pending_paste),
        store.pending_open.take(),
      )
    });

    if let Some(text) = copy {
      emit_copy(&mut terminal, &text);
    }

    if read {
      let _ = write!(terminal, "{}", Osc::QuerySelection(Selection::CLIPBOARD));
      let _ = terminal.flush();
    }

    if let Some(url) = open {
      open_url(&url);
    }

    if quit {
      break;
    }

    crate::dev::tick();

    let wait = if with_store(|store| store.motion_active) {
      FRAME_INTERVAL
    }
    else {
      IDLE_POLL
    };

    match terminal.poll(|_| true, Some(wait)) {
      Ok(true) => {}
      Ok(false) => continue,
      // The terminal is gone.
      Err(_) => break,
    }

    let Ok(event) = terminal.read(|_| true)
    else {
      break;
    };

    match event {
      Event::Key(key) if key.kind != KeyEventKind::Release => {
        dispatch_key(&key);

        crate::dev::mark_changed();
      }
      Event::Mouse(mouse) => {
        dispatch_mouse(&mut terminal, mouse);

        crate::dev::mark_changed();
      }
      Event::Paste(text) => {
        paste(&text);

        crate::dev::mark_changed();
      }
      Event::WindowResized(..) => {
        // A full rewrite: the previous frame no longer matches what the terminal
        // shows.
        last_frame = None;

        with_store(|store| store.paint_dirty = true);
      }
      Event::Csi(Csi::Mode(Mode::ReportTheme(mode))) => {
        set_detected_scheme(mode == ThemeMode::Dark);
      }
      Event::Osc(Osc::ChangeDynamicColors(DynamicColorNumber::TextBackgroundColor, colors)) => {
        if let Some(ColorOrQuery::Color(color)) = colors.first() {
          set_detected_scheme(detect::is_dark(*color));
        }
      }
      Event::Osc(Osc::SetSelection(_, content)) => {
        paste_reply(&content);

        crate::dev::mark_changed();
      }
      // A terminal speaking the kitty keyboard protocol disambiguates modified
      // keys—`Ctrl+Backspace` above all; the rest keep the legacy encoding, where
      // `Ctrl+W` stands in for it.
      Event::Csi(Csi::Keyboard(Keyboard::ReportFlags(_))) => {
        let _ = write!(
          terminal,
          "{}",
          Csi::Keyboard(Keyboard::SetFlags {
            flags: KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES,
            mode: SetKeyboardFlagsMode::AssignAll,
          })
        );
        let _ = terminal.flush();
      }
      Event::Dcs(Dcs::Response {
        value: DcsResponse::GraphicRendition(attributes),
        ..
      }) => {
        if let Some(reported) = ColorDepth::reported(&attributes)
          && reported > depth
        {
          depth = reported;
          last_frame = None;

          with_store(|store| store.paint_dirty = true);
        }
      }
      _ => {}
    }
  }

  crate::dev::flush();

  let _ = write_restore(&mut terminal);
  let _ = terminal.flush();
}

/// Inserts text the terminal pasted—bracketed, by its own key—into the focused entry,
/// inside a reactive batch.
fn paste(text: &str) {
  skene_reactive::batch(|| interaction::paste(text));
}

/// Inserts the clipboard the terminal reported into the entry that asked for it,
/// inside a reactive batch; nothing happens when no entry asked or it is gone.
fn paste_reply(text: &str) {
  skene_reactive::batch(|| interaction::paste_reply(text));
}

/// Records the detected background, moving the scheme in force where it changes.
fn set_detected_scheme(dark: bool) {
  let write = with_store(|store| {
    let before = store.scheme();

    store.detected_dark = dark;

    let after = store.scheme();

    if before == after {
      return None;
    }

    store.invalidate_styles();

    Some((store.media().set_scheme, after))
  });

  if let Some((write, scheme)) = write {
    skene_reactive::Set::set(&write, scheme);
  }
}

/// Dispatches one mouse event: hover tracking (with the pointer shape), the pointer
/// handlers through core's dispatch, and left-button clicks—all inside a reactive
/// batch.
fn dispatch_mouse(out: &mut impl Write, mouse: termina::event::MouseEvent) {
  let modifiers = portable_modifiers(mouse.modifiers);

  skene_reactive::batch(|| match mouse.kind {
    MouseEventKind::Moved | MouseEventKind::Drag(_) => {
      let button = match mouse.kind {
        MouseEventKind::Drag(button) => Some(portable_button(button)),
        _ => None,
      };

      interaction::pointer_move(mouse.column, mouse.row, button, modifiers);
      set_pointer_shape(out, with_store(|store| store.hover_cursor()));
    }
    MouseEventKind::Down(button) => {
      with_store(|store| match button {
        MouseButton::Left => store.left_held = true,
        MouseButton::Right => store.right_held = true,
        MouseButton::Middle => {}
      });
      interaction::pointer_press(mouse.column, mouse.row, portable_button(button), modifiers);
      set_pointer_shape(out, with_store(|store| store.hover_cursor()));
    }
    MouseEventKind::Up(button) => {
      // A release whose press was never delivered stands in for the whole click: VTE
      // terminals report only the first three presses of a rapid sequence to the app,
      // yet forward every release.
      let orphan = with_store(|store| match button {
        MouseButton::Left => !std::mem::replace(&mut store.left_held, false),
        MouseButton::Right => !std::mem::replace(&mut store.right_held, false),
        MouseButton::Middle => false,
      });

      if orphan {
        with_store(|store| {
          if button == MouseButton::Left {
            store.left_held = true;
          }
        });
        interaction::pointer_press(mouse.column, mouse.row, portable_button(button), modifiers);
        with_store(|store| store.left_held = false);
      }

      interaction::pointer_release(mouse.column, mouse.row, portable_button(button), modifiers);
      set_pointer_shape(out, with_store(|store| store.hover_cursor()));
    }
    MouseEventKind::ScrollDown => interaction::wheel(mouse.column, mouse.row, 0, WHEEL_STEP),
    MouseEventKind::ScrollUp => interaction::wheel(mouse.column, mouse.row, 0, -WHEEL_STEP),
    MouseEventKind::ScrollRight => interaction::wheel(mouse.column, mouse.row, WHEEL_STEP, 0),
    MouseEventKind::ScrollLeft => interaction::wheel(mouse.column, mouse.row, -WHEEL_STEP, 0),
  });
}

/// A terminal mouse button as the portable vocabulary's.
fn portable_button(button: MouseButton) -> PointerButton {
  match button {
    MouseButton::Left => PointerButton::Primary,
    MouseButton::Right => PointerButton::Secondary,
    MouseButton::Middle => PointerButton::Auxiliary,
  }
}

/// Terminal modifier state as the portable vocabulary's.
fn portable_modifiers(modifiers: TerminalModifiers) -> Modifiers {
  Modifiers {
    ctrl: modifiers.contains(TerminalModifiers::CONTROL),
    shift: modifiers.contains(TerminalModifiers::SHIFT),
    alt: modifiers.contains(TerminalModifiers::ALT),
    meta: modifiers.contains(TerminalModifiers::SUPER),
  }
}

/// Emits the terminal title via OSC 2; terminals without support ignore it.
fn emit_title(out: &mut impl Write, title: &str) {
  let _ = write!(out, "{}", Osc::SetWindowTitle(title));
  let _ = out.flush();
}

/// Opens `url` with the system handler, detached and best-effort.
fn open_url(url: &str) {
  #[cfg(target_os = "macos")]
  let command = "open";
  #[cfg(all(unix, not(target_os = "macos")))]
  let command = "xdg-open";
  #[cfg(windows)]
  let command = "explorer";

  let _ = std::process::Command::new(command)
    .arg(url)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn();
}

/// Copies text to the system clipboard via OSC 52; terminals without support ignore
/// it, and terminals gating clipboard writes ask the user.
fn emit_copy(out: &mut impl Write, text: &str) {
  let _ = write!(
    out,
    "{}",
    Osc::SetSelection(Selection::CLIPBOARD, text.into())
  );
  let _ = out.flush();
}

/// Sets the terminal's pointer shape via OSC 22; terminals without support ignore it.
fn set_pointer_shape(out: &mut impl Write, cursor: Option<MouseCursor>) {
  let shape = match cursor {
    Some(MouseCursor::Click) => "pointer",
    Some(MouseCursor::Text) => "text",
    Some(MouseCursor::Grab) => "grab",
    Some(MouseCursor::Grabbing) => "grabbing",
    Some(MouseCursor::Default) | None => "default",
  };

  let _ = write!(out, "\x1b]22;{shape}\x1b\\");
  let _ = out.flush();
}

/// The control moving the cursor to the cell (`column`, `row`).
fn move_to(column: u16, row: u16) -> Csi {
  Csi::Cursor(csi::Cursor::Position {
    line: OneBased::from_zero_based(row),
    col: OneBased::from_zero_based(column),
  })
}

/// Writes a painted frame to the terminal, quantizing colors to `depth`; rows
/// unchanged since the previous frame are skipped.
fn draw(
  out: &mut impl Write,
  buffer: &paint::Buffer,
  depth: ColorDepth,
  previous: Option<&paint::Buffer>,
) {
  let _ = write!(out, "{}{}", move_to(0, 0), Csi::Sgr(Sgr::Reset));

  for row in 0..buffer.height {
    if !paint::row_changed(previous, buffer, row) {
      continue;
    }

    let _ = write!(
      out,
      "{}{}",
      move_to(0, u16::try_from(row).unwrap_or(u16::MAX)),
      Csi::Sgr(Sgr::Reset)
    );

    let mut current = paint::TerminalCell::EMPTY;

    for cell in &buffer.cells[row * buffer.width..(row + 1) * buffer.width] {
      if cell.foreground != current.foreground {
        let color = cell
          .foreground
          .map_or(ColorSpec::Reset, |rgb| depth.terminal_color(rgb));

        let _ = write!(out, "{}", Csi::Sgr(Sgr::Foreground(color)));
      }

      if cell.background != current.background {
        let color = cell
          .background
          .map_or(ColorSpec::Reset, |rgb| depth.terminal_color(rgb));

        let _ = write!(out, "{}", Csi::Sgr(Sgr::Background(color)));
      }

      if cell.bold != current.bold {
        let intensity = if cell.bold {
          Intensity::Bold
        }
        else {
          Intensity::Normal
        };

        let _ = write!(out, "{}", Csi::Sgr(Sgr::Intensity(intensity)));
      }

      if cell.underline != current.underline {
        let underline = if cell.underline {
          Underline::Single
        }
        else {
          Underline::None
        };

        let _ = write!(out, "{}", Csi::Sgr(Sgr::Underline(underline)));
      }

      if cell.reverse != current.reverse {
        let _ = write!(out, "{}", Csi::Sgr(Sgr::Reverse(cell.reverse)));
      }

      // OSC 8 anchors: close the running link on any change, open the next.
      if cell.link != current.link {
        if current.link.is_some() {
          let _ = write!(out, "\x1b]8;;\x1b\\");
        }

        if let Some(url) = cell
          .link
          .and_then(|index| buffer.links.get(usize::from(index)))
        {
          let _ = write!(out, "\x1b]8;;{url}\x1b\\");
        }
      }

      let _ = write!(out, "{}", cell.character);
      current = *cell;
    }

    if current.link.is_some() {
      let _ = write!(out, "\x1b]8;;\x1b\\");
    }
  }

  // The focused entry's caret is the terminal's own bar cursor, blinking as the
  // terminal does.
  match buffer.caret {
    Some((column, row)) => {
      let _ = write!(
        out,
        "{}{}{}",
        move_to(column, row),
        Csi::Cursor(csi::Cursor::CursorStyle(CursorStyle::BlinkingBar)),
        dec_mode(DecPrivateModeCode::ShowCursor, true)
      );
    }
    None => {
      let _ = write!(out, "{}", dec_mode(DecPrivateModeCode::ShowCursor, false));
    }
  }

  let _ = out.flush();
}

/// Dispatches one keyboard event through core's dispatch, then the terminal's own
/// response unless a handler prevented it; `Ctrl-Q` stays unconditional.
fn dispatch_key(key: &termina::event::KeyEvent) {
  if key.code == KeyCode::Char('q') && key.modifiers.contains(TerminalModifiers::CONTROL) {
    with_store(|store| store.quit = true);

    return;
  }

  let event = portable_key(key.code, key.modifiers);

  skene_reactive::batch(|| interaction::key(&event));
}

/// Maps a terminal key to the portable W3C key value.
fn portable_key(code: KeyCode, modifiers: TerminalModifiers) -> KeyEvent {
  let key = match code {
    KeyCode::Char(character) => character.to_string(),
    KeyCode::Enter => "Enter".to_owned(),
    KeyCode::Backspace => "Backspace".to_owned(),
    KeyCode::Tab | KeyCode::BackTab => "Tab".to_owned(),
    KeyCode::Escape => "Escape".to_owned(),
    KeyCode::Delete => "Delete".to_owned(),
    KeyCode::Left => "ArrowLeft".to_owned(),
    KeyCode::Right => "ArrowRight".to_owned(),
    KeyCode::Up => "ArrowUp".to_owned(),
    KeyCode::Down => "ArrowDown".to_owned(),
    KeyCode::Home => "Home".to_owned(),
    KeyCode::End => "End".to_owned(),
    KeyCode::PageUp => "PageUp".to_owned(),
    KeyCode::PageDown => "PageDown".to_owned(),
    KeyCode::Function(number) => format!("F{number}"),
    _ => "Unidentified".to_owned(),
  };
  let mut modifiers = portable_modifiers(modifiers);

  if code == KeyCode::BackTab {
    modifiers.shift = true;
  }

  KeyEvent { key, modifiers }
}

/// Simulates one key press by its portable W3C key value (`"a"`, `"Enter"`, `"Tab"`,
/// `"Backspace"`, …), for a test driving the loop headlessly. `"Ctrl+"`, `"Alt+"`, and
/// `"Shift+"` prefixes add the modifier (`"Ctrl+ArrowLeft"`, `"Shift+Tab"`).
pub fn trigger_key(key: &str) {
  let mut modifiers = Modifiers::default();
  let mut key = key;

  while let Some((modifier, rest)) = key.split_once('+') {
    match modifier {
      "Ctrl" => modifiers.ctrl = true,
      "Alt" => modifiers.alt = true,
      "Shift" => modifiers.shift = true,
      _ => break,
    }

    key = rest;
  }

  let event = KeyEvent {
    key: key.to_owned(),
    modifiers,
  };

  skene_reactive::batch(|| interaction::key(&event));
}

/// The node holding focus, for a test asserting focus movement.
#[must_use]
pub fn focused_node() -> Option<skene_core::NodeId> {
  with_store(|store| {
    store
      .focused
      .and_then(|focused| store.record(focused).and_then(|record| record.node))
  })
}

/// Simulates the mouse moving to the cell (`x`, `y`), for a test driving hover
/// headlessly. Hit-testing resolves against the last render's layout.
pub fn trigger_mouse_move(x: u16, y: u16) {
  skene_reactive::batch(|| interaction::pointer_move(x, y, None, Modifiers::default()));
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
  skene_reactive::batch(|| {
    with_store(|store| store.left_held = true);
    interaction::pointer_press(x, y, PointerButton::Primary, Modifiers::default());
  });
}

/// Simulates dragging to (`x`, `y`) with the left button held, after
/// [`trigger_mouse_press`].
pub fn trigger_mouse_drag(x: u16, y: u16) {
  skene_reactive::batch(|| {
    interaction::pointer_move(x, y, Some(PointerButton::Primary), Modifiers::default());
  });
}

/// Simulates releasing the left button at (`x`, `y`), ending a drag. A release with no
/// press delivered stands in for the whole click, as the interactive loop's does.
pub fn trigger_mouse_release(x: u16, y: u16) {
  skene_reactive::batch(|| {
    if !with_store(|store| store.left_held) {
      with_store(|store| store.left_held = true);
      interaction::pointer_press(x, y, PointerButton::Primary, Modifiers::default());
    }

    with_store(|store| store.left_held = false);
    interaction::pointer_release(x, y, PointerButton::Primary, Modifiers::default());
  });
}

/// Simulates a right click at the cell (`x`, `y`)—the copy-menu gesture—for a test
/// driving selection headlessly.
pub fn trigger_mouse_right_click(x: u16, y: u16) {
  skene_reactive::batch(|| {
    with_store(|store| store.right_held = true);
    interaction::pointer_press(x, y, PointerButton::Secondary, Modifiers::default());
    with_store(|store| store.right_held = false);
    interaction::pointer_release(x, y, PointerButton::Secondary, Modifiers::default());
  });
}

/// Simulates the mouse wheel at the cell (`x`, `y`), for a headless test driving
/// scrolling. Positive deltas scroll right and down, one cell per unit; hit-testing
/// resolves against the last render's layout.
pub fn trigger_mouse_scroll(x: u16, y: u16, delta_columns: i64, delta_rows: i64) {
  skene_reactive::batch(|| interaction::wheel(x, y, delta_columns, delta_rows));
}

/// Takes the text the last `Copy` staged for the clipboard, for a headless test.
#[must_use]
pub fn take_copied() -> Option<String> {
  with_store(|store| store.pending_copy.take())
}

/// Takes whether the last `Paste` staged a clipboard read, for a headless test.
#[must_use]
pub fn take_paste_requested() -> bool {
  with_store(|store| std::mem::take(&mut store.pending_paste))
}

/// Simulates the terminal pasting `text` by its own key—a bracketed paste—into the
/// focused entry, for a headless test.
pub fn trigger_paste(text: &str) {
  paste(text);
}

/// Simulates the terminal answering the last clipboard read with `text`, for a headless
/// test: it lands in the entry that asked, wherever focus has moved since.
pub fn trigger_clipboard_reply(text: &str) {
  paste_reply(text);
}

/// Takes the link the last click staged for the system handler, for a headless test.
#[must_use]
pub fn take_opened() -> Option<String> {
  with_store(|store| store.pending_open.take())
}

/// The current terminal title, for a headless test.
#[must_use]
pub fn current_title() -> Option<String> {
  with_store(|store| store.title.clone())
}

/// Simulates the terminal reporting `scheme` as its background, for a headless test
/// driving the scheme conditions; the app's own choice still wins.
pub fn trigger_color_scheme(scheme: ColorScheme) {
  set_detected_scheme(scheme == ColorScheme::Dark);
}

/// Steps the motion clock forward by `elapsed`, for a headless test driving transitions
/// and animations frame by frame; the next render samples them at the new reading.
///
/// A headless driver's clock stands still otherwise, so a render shows a transition
/// at the point the last step left it.
pub fn trigger_frame(elapsed: Duration) {
  with_store(|store| {
    store.clock.advance(elapsed.as_secs_f64());
    store.motion_active = true;
  });
}

/// Simulates the reader's reduced-motion preference, for a headless test driving the
/// reduced-motion condition and the motion that respects it.
pub fn trigger_reduced_motion(reduced: bool) {
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
}

/// Simulates the terminal resizing to `columns` x `rows`, for a headless test driving
/// the size conditions; the next render lays out at the size it is given.
pub fn trigger_resize(columns: u16, rows: u16) {
  frame::resize(columns, rows);
}

/// The pointer shape the current hover resolves to, for a headless test.
#[must_use]
pub fn current_cursor() -> Option<MouseCursor> {
  with_store(|store| store.hover_cursor())
}

/// How many layouts have been computed since the driver was installed, for a test
/// asserting that a change repainted without a relayout.
#[must_use]
pub fn layout_generation() -> u64 {
  with_store(|store| store.layout_generation)
}

/// How many taffy styles have been written since the driver was installed, for a test
/// counting the restyles a change caused.
#[must_use]
pub fn set_style_calls() -> u64 {
  with_store(|store| store.set_style_calls)
}

/// How many node styles have been resolved from their declarations since the driver
/// was installed, for a test counting what a frame or a change resolved.
#[must_use]
pub fn style_resolutions() -> u64 {
  with_store(|store| store.style_resolutions)
}

/// The rows the last render rewrote, for a test asserting that a change repainted only
/// the rows it touched.
#[must_use]
pub fn repainted_rows() -> Vec<usize> {
  with_store(|store| store.repainted.clone())
}
