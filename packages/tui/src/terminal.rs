//! The interactive terminal loop, and the seams that drive it without a terminal.
//!
//! [`launch`] takes over the terminal (raw mode, the alternate screen), builds the app,
//! and runs the event loop: keyboard and mouse events dispatch inside a reactive batch
//! through core's dispatch, and every change repaints. `Ctrl-C` quits.

use std::{io::Write, time::Duration};

use crossterm::{
  cursor,
  event::{
    self,
    DisableMouseCapture,
    EnableMouseCapture,
    Event,
    KeyCode,
    KeyEventKind,
    KeyModifiers,
    MouseButton,
    MouseEventKind,
  },
  execute,
  queue,
  style::{
    Attribute,
    Color as TerminalColor,
    ResetColor,
    SetAttribute,
    SetBackgroundColor,
    SetForegroundColor,
  },
  terminal,
};
use skene_core::{ColorScheme, IntoView, KeyEvent, Modifiers, MouseCursor, PointerButton};

/// How long the loop waits for input while nothing moves.
const IDLE_POLL: Duration = Duration::from_millis(100);

/// How long the loop waits for input between frames while a tween or an animation
/// runs: the frame rate's cap.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

use crate::{
  Tui,
  View as _,
  detect,
  frame,
  interaction::{self, WHEEL_STEP},
  paint,
  with_store,
};

/// Restores the terminal when the loop unwinds.
struct TerminalGuard;

impl Drop for TerminalGuard {
  fn drop(&mut self) {
    restore_terminal();
  }
}

/// Returns the terminal to its ordinary state: cooked mode, the main screen, mouse
/// capture off, the keyboard back to legacy encoding, the cursor shown. Safe to run
/// more than once, on any thread.
fn restore_terminal() {
  let mut out = std::io::stdout();
  let _ = execute!(
    out,
    // The stackless form of the kitty keyboard protocol (`CSI = flags ; 1 u`):
    // idempotent, so the reset holds however many app instances set it before.
    crossterm::style::Print("\x1b[=0;1u"),
    DisableMouseCapture,
    cursor::SetCursorStyle::DefaultUserShape,
    cursor::Show,
    terminal::LeaveAlternateScreen
  );

  let _ = terminal::disable_raw_mode();
}

/// Mounts and runs a skene application in the terminal.
///
/// Installs the terminal driver, builds `app`, and runs the event loop until `Ctrl-C`.
/// Terminal detection runs beside the loop: the first frame paints for a dark
/// background and the answers arrive as signal writes. A panic anywhere in the process
/// restores the terminal before its message prints.
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
  let previous_hook = std::panic::take_hook();

  std::panic::set_hook(Box::new(move |info| {
    restore_terminal();
    previous_hook(info);
  }));

  crate::install();
  with_store(|store| store.clock = crate::store::Clock::Real(std::time::Instant::now()));

  crate::dev::install();

  let _ = crate::mount(|| app().into_view().build());

  terminal::enable_raw_mode().expect("enable raw mode");

  let _guard = TerminalGuard;
  let depth = detect::ColorDepth::detect();
  let mut detection = Some(detect::start());

  {
    crate::dev::adopt_scroll();

    if let Some(notice) = crate::dev::notice_signal() {
      let _ = crate::mount(|| crate::dev::notice_view(notice).into_view().build());
    }
  }

  execute!(
    std::io::stdout(),
    terminal::EnterAlternateScreen,
    EnableMouseCapture,
    cursor::Hide
  )
  .expect("enter the alternate screen");

  // A terminal speaking the kitty keyboard protocol disambiguates modified keys—
  // `Ctrl+Backspace` above all; the rest keep the legacy encoding, where `Ctrl+W`
  // stands in for it.
  if terminal::supports_keyboard_enhancement().unwrap_or(false) {
    let _ = execute!(std::io::stdout(), crossterm::style::Print("\x1b[=1;1u"));
  }

  let mut last_frame: Option<paint::Buffer> = None;

  loop {
    crate::task::pump();

    if let Some(receiver) = &detection {
      match receiver.try_recv() {
        Ok(detected) => {
          apply_detection(detected);
          detection = None;
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => detection = None,
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
      }
    }

    let title = with_store(|store| {
      store
        .title_dirty
        .then(|| store.title.clone().unwrap_or_default())
    });

    if let Some(title) = title {
      with_store(|store| store.title_dirty = false);
      emit_title(&title);
    }

    let dirty = with_store(|store| store.needs_frame());

    if dirty {
      // A degenerate reported size (a fresh pseudo-terminal) falls back to a classic
      // 80x24 rather than painting nothing.
      let (width, height) = match terminal::size() {
        Ok((width, height)) if width > 0 && height > 0 => (width, height),
        _ => crate::store::DEFAULT_SIZE,
      };
      let frame = frame::render(width, height);

      draw(&frame, depth, last_frame.as_ref());
      last_frame = Some(frame);
    }

    let (quit, copy, open) = with_store(|store| {
      (
        store.quit,
        store.pending_copy.take(),
        store.pending_open.take(),
      )
    });

    if let Some(text) = copy {
      emit_copy(&text);
    }

    if let Some(url) = open {
      open_url(&url);
    }

    if quit {
      break;
    }

    crate::dev::tick();

    // While detection reads its replies from stdin the loop leaves the input alone.
    if detection.is_some() {
      std::thread::sleep(Duration::from_millis(10));

      continue;
    }

    let wait = if with_store(|store| store.motion_active) {
      FRAME_INTERVAL
    }
    else {
      IDLE_POLL
    };

    if event::poll(wait).unwrap_or(false) {
      match event::read() {
        Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
          dispatch_key(&key);

          crate::dev::mark_changed();
        }
        Ok(Event::Mouse(mouse)) => {
          dispatch_mouse(mouse);

          crate::dev::mark_changed();
        }
        Ok(Event::Resize(..)) => {
          // A full rewrite: the previous frame no longer matches what the terminal
          // shows.
          last_frame = None;

          with_store(|store| store.paint_dirty = true);
        }
        _ => {}
      }
    }
  }

  crate::dev::flush();
}

/// Applies what detection found: the background as the platform's scheme, and the
/// clipboard support.
fn apply_detection(detected: detect::Detected) {
  set_detected_scheme(detected.dark);
  with_store(|store| store.clipboard = detected.clipboard);
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
fn dispatch_mouse(mouse: event::MouseEvent) {
  let modifiers = portable_modifiers(mouse.modifiers);

  skene_reactive::batch(|| match mouse.kind {
    MouseEventKind::Moved | MouseEventKind::Drag(_) => {
      let button = match mouse.kind {
        MouseEventKind::Drag(button) => Some(portable_button(button)),
        _ => None,
      };

      interaction::pointer_move(mouse.column, mouse.row, button, modifiers);
      set_pointer_shape(with_store(|store| store.hover_cursor()));
    }
    MouseEventKind::Down(button) => {
      with_store(|store| match button {
        MouseButton::Left => store.left_held = true,
        MouseButton::Right => store.right_held = true,
        MouseButton::Middle => {}
      });
      interaction::pointer_press(mouse.column, mouse.row, portable_button(button), modifiers);
      set_pointer_shape(with_store(|store| store.hover_cursor()));
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
      set_pointer_shape(with_store(|store| store.hover_cursor()));
    }
    MouseEventKind::ScrollDown => interaction::wheel(mouse.column, mouse.row, 0, WHEEL_STEP),
    MouseEventKind::ScrollUp => interaction::wheel(mouse.column, mouse.row, 0, -WHEEL_STEP),
    MouseEventKind::ScrollRight => interaction::wheel(mouse.column, mouse.row, WHEEL_STEP, 0),
    MouseEventKind::ScrollLeft => interaction::wheel(mouse.column, mouse.row, -WHEEL_STEP, 0),
  });
}

/// A crossterm mouse button as the portable vocabulary's.
fn portable_button(button: MouseButton) -> PointerButton {
  match button {
    MouseButton::Left => PointerButton::Primary,
    MouseButton::Right => PointerButton::Secondary,
    MouseButton::Middle => PointerButton::Auxiliary,
  }
}

/// Crossterm modifier state as the portable vocabulary's.
fn portable_modifiers(modifiers: KeyModifiers) -> Modifiers {
  Modifiers {
    ctrl: modifiers.contains(KeyModifiers::CONTROL),
    shift: modifiers.contains(KeyModifiers::SHIFT),
    alt: modifiers.contains(KeyModifiers::ALT),
    meta: modifiers.contains(KeyModifiers::SUPER),
  }
}

/// Emits the terminal title via OSC 2; terminals without support ignore it.
fn emit_title(title: &str) {
  let mut out = std::io::stdout();
  let _ = write!(out, "\x1b]2;{title}\x1b\\");
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
fn emit_copy(text: &str) {
  let mut out = std::io::stdout();
  let _ = write!(out, "\x1b]52;c;{}\x1b\\", base64(text.as_bytes()));
  let _ = out.flush();
}

/// Standard base64 with padding, for the OSC 52 payload.
fn base64(bytes: &[u8]) -> String {
  const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

  let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

  for chunk in bytes.chunks(3) {
    let block = (u32::from(chunk[0]) << 16)
      | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
      | u32::from(*chunk.get(2).unwrap_or(&0));

    for position in 0..4 {
      if position <= chunk.len() {
        out.push(ALPHABET[(block >> (18 - 6 * position)) as usize & 63] as char);
      }
      else {
        out.push('=');
      }
    }
  }

  out
}

/// Sets the terminal's pointer shape via OSC 22; terminals without support ignore it.
fn set_pointer_shape(cursor: Option<MouseCursor>) {
  let shape = match cursor {
    Some(MouseCursor::Click) => "pointer",
    Some(MouseCursor::Text) => "text",
    Some(MouseCursor::Grab) => "grab",
    Some(MouseCursor::Grabbing) => "grabbing",
    Some(MouseCursor::Default) | None => "default",
  };

  let mut out = std::io::stdout();
  let _ = write!(out, "\x1b]22;{shape}\x1b\\");
  let _ = out.flush();
}

/// Writes a painted frame to the terminal, quantizing colors to `depth`; rows
/// unchanged since the previous frame are skipped.
fn draw(buffer: &paint::Buffer, depth: detect::ColorDepth, previous: Option<&paint::Buffer>) {
  let mut out = std::io::stdout();
  let _ = queue!(out, cursor::MoveTo(0, 0), ResetColor);

  for row in 0..buffer.height {
    if !paint::row_changed(previous, buffer, row) {
      continue;
    }

    let _ = queue!(
      out,
      cursor::MoveTo(0, u16::try_from(row).unwrap_or(u16::MAX)),
      ResetColor,
      SetAttribute(Attribute::Reset),
    );

    let mut current = paint::TerminalCell::EMPTY;

    for cell in &buffer.cells[row * buffer.width..(row + 1) * buffer.width] {
      if cell.foreground != current.foreground {
        let color = cell
          .foreground
          .map_or(TerminalColor::Reset, |rgb| depth.terminal_color(rgb));

        let _ = queue!(out, SetForegroundColor(color));
      }

      if cell.background != current.background {
        let color = cell
          .background
          .map_or(TerminalColor::Reset, |rgb| depth.terminal_color(rgb));

        let _ = queue!(out, SetBackgroundColor(color));
      }

      if cell.bold != current.bold {
        let _ = queue!(
          out,
          SetAttribute(if cell.bold {
            Attribute::Bold
          }
          else {
            Attribute::NormalIntensity
          })
        );
      }

      if cell.underline != current.underline {
        let _ = queue!(
          out,
          SetAttribute(if cell.underline {
            Attribute::Underlined
          }
          else {
            Attribute::NoUnderline
          })
        );
      }

      if cell.reverse != current.reverse {
        let _ = queue!(
          out,
          SetAttribute(if cell.reverse {
            Attribute::Reverse
          }
          else {
            Attribute::NoReverse
          })
        );
      }

      // OSC 8 anchors: close the running link on any change, open the next.
      if cell.link != current.link {
        if current.link.is_some() {
          let _ = queue!(out, crossterm::style::Print("\x1b]8;;\x1b\\"));
        }

        if let Some(url) = cell
          .link
          .and_then(|index| buffer.links.get(usize::from(index)))
        {
          let _ = queue!(out, crossterm::style::Print(format!("\x1b]8;;{url}\x1b\\")));
        }
      }

      let _ = queue!(out, crossterm::style::Print(cell.character));
      current = *cell;
    }

    if current.link.is_some() {
      let _ = queue!(out, crossterm::style::Print("\x1b]8;;\x1b\\"));
    }
  }

  // The focused entry's caret is the terminal's own bar cursor, blinking as the
  // terminal does.
  match buffer.caret {
    Some((column, row)) => {
      let _ = queue!(
        out,
        cursor::MoveTo(column, row),
        cursor::SetCursorStyle::BlinkingBar,
        cursor::Show
      );
    }
    None => {
      let _ = queue!(out, cursor::Hide);
    }
  }

  let _ = out.flush();
}

/// Dispatches one keyboard event through core's dispatch, then the terminal's own
/// response unless a handler prevented it; `Ctrl-C` stays unconditional.
fn dispatch_key(key: &event::KeyEvent) {
  if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
    with_store(|store| store.quit = true);

    return;
  }

  let event = portable_key(key.code, key.modifiers);

  skene_reactive::batch(|| interaction::key(&event));
}

/// Maps a terminal key to the portable W3C key value.
fn portable_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
  let key = match code {
    KeyCode::Char(character) => character.to_string(),
    KeyCode::Enter => "Enter".to_owned(),
    KeyCode::Backspace => "Backspace".to_owned(),
    KeyCode::Tab | KeyCode::BackTab => "Tab".to_owned(),
    KeyCode::Esc => "Escape".to_owned(),
    KeyCode::Delete => "Delete".to_owned(),
    KeyCode::Left => "ArrowLeft".to_owned(),
    KeyCode::Right => "ArrowRight".to_owned(),
    KeyCode::Up => "ArrowUp".to_owned(),
    KeyCode::Down => "ArrowDown".to_owned(),
    KeyCode::Home => "Home".to_owned(),
    KeyCode::End => "End".to_owned(),
    KeyCode::PageUp => "PageUp".to_owned(),
    KeyCode::PageDown => "PageDown".to_owned(),
    KeyCode::F(number) => format!("F{number}"),
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

/// Simulates the clipboard-support detection, for a headless test driving the copy
/// menu's disabled state.
pub fn trigger_clipboard_support(supported: bool) {
  with_store(|store| {
    store.clipboard = supported;
    store.paint_dirty = true;
  });
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

/// The rows the last render rewrote, for a test asserting that a change repainted only
/// the rows it touched.
#[must_use]
pub fn repainted_rows() -> Vec<usize> {
  with_store(|store| store.repainted.clone())
}
