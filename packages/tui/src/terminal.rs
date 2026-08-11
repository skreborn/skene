//! The interactive terminal loop.
//!
//! [`launch`] takes over the terminal (raw mode, the alternate screen), builds the app,
//! and runs the event loop: keyboard events dispatch inside a reactive batch, and every
//! change repaints. `Ctrl-C` quits.

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
use skene_core::{IntoView, KeyEvent, MouseCursor, NodeId, Prevention, View};

use crate::{
  paint,
  state::{NodeKind, State},
  with_state,
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
    // idempotent, so the reset holds however many app instances set it before—a
    // dev-loop swap kills the previous instance without running its restore.
    crossterm::style::Print("\x1b[=0;1u"),
    DisableMouseCapture,
    cursor::Show,
    terminal::LeaveAlternateScreen
  );

  let _ = terminal::disable_raw_mode();
}

/// Mounts and runs a skene application in the terminal.
///
/// Installs the terminal driver, builds `app`, and runs the keyboard loop until
/// `Ctrl-C`. The reactive runtime, handlers and tree are kept alive for the run's
/// lifetime. A panic anywhere in the process restores the terminal before its message
/// prints.
///
/// # Panics
///
/// Panics if the driver is already installed, or when the terminal cannot be set up.
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  let previous_hook = std::panic::take_hook();

  std::panic::set_hook(Box::new(move |info| {
    restore_terminal();
    previous_hook(info);
  }));

  crate::install();

  #[cfg(feature = "snapshot")]
  crate::dev::install();

  crate::mount(|| app().into_view().build());

  terminal::enable_raw_mode().expect("enable raw mode");

  let _guard = TerminalGuard;
  let dark = crate::detect::background_is_dark();
  let depth = crate::detect::ColorDepth::detect();
  let clipboard = crate::detect::clipboard_writes_work();

  with_state(|state| {
    state.dark.set(dark);
    state.clipboard.set(clipboard);
  });

  #[cfg(feature = "snapshot")]
  {
    crate::dev::adopt_scroll();

    if let Some(notice) = crate::dev::notice_signal() {
      crate::mount(|| crate::dev::notice_view(notice).into_view().build());
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
  // stands in for it. The stackless set form (`CSI = 1 ; 1 u`), not a push: a
  // dev-loop swap kills an instance without popping, and a set stays balanced.
  if terminal::supports_keyboard_enhancement().unwrap_or(false) {
    let _ = execute!(std::io::stdout(), crossterm::style::Print("\x1b[=1;1u"));
  }

  let mut last_frame: Option<paint::Buffer> = None;

  loop {
    crate::task::pump();

    let (quit, copy) = with_state(|state| {
      if state.title_dirty.replace(false) {
        emit_title(state.title.borrow().as_deref().unwrap_or(""));
      }

      if state.dirty.replace(false) {
        // A degenerate reported size (a fresh pseudo-terminal) falls back to a classic
        // 80x24 rather than painting nothing.
        let (width, height) = match terminal::size() {
          Ok((width, height)) if width > 0 && height > 0 => (width, height),
          _ => (80, 24),
        };

        let frame = paint::render(state, width, height);

        draw(&frame, depth, last_frame.as_ref());
        last_frame = Some(frame);
      }

      (state.quit.get(), state.pending_copy.borrow_mut().take())
    });

    if let Some(text) = copy {
      emit_copy(&text);
    }

    if quit {
      break;
    }

    #[cfg(feature = "snapshot")]
    crate::dev::tick();

    if event::poll(Duration::from_millis(100)).unwrap_or(false) {
      match event::read() {
        Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
          with_state(|state| dispatch_key(state, &key));

          #[cfg(feature = "snapshot")]
          crate::dev::mark_changed();
        }
        Ok(Event::Mouse(mouse)) => {
          with_state(|state| dispatch_mouse(state, mouse));

          #[cfg(feature = "snapshot")]
          crate::dev::mark_changed();
        }
        Ok(Event::Resize(..)) => {
          // A full rewrite: the previous frame no longer matches what the terminal
          // shows (a resize reflows it, outside prints may have overwritten it).
          last_frame = None;

          with_state(|state| state.dirty.set(true));
        }
        _ => {}
      }
    }
  }

  #[cfg(feature = "snapshot")]
  crate::dev::flush();
}

/// Dispatches one mouse event: hover tracking (with the pointer shape), the pointer
/// down/up/move handlers, and left-button clicks—all inside a reactive batch.
fn dispatch_mouse(state: &State, mouse: event::MouseEvent) {
  let modifiers = portable_modifiers(mouse.modifiers);

  skene_reactive::batch(|| match mouse.kind {
    MouseEventKind::Moved | MouseEventKind::Drag(_) => {
      let button = match mouse.kind {
        MouseEventKind::Drag(button) => Some(portable_button(button)),
        _ => None,
      };

      state.set_hover(mouse.column, mouse.row);

      let _ = state.run_pointer(
        &state.pointer_move,
        mouse.column,
        mouse.row,
        button,
        modifiers,
      );

      if matches!(mouse.kind, MouseEventKind::Drag(MouseButton::Left)) {
        state.drag_to(mouse.column, mouse.row);
      }

      set_pointer_shape(state.hover_cursor());
    }
    MouseEventKind::Down(button) => {
      match button {
        MouseButton::Left => state.left_held.set(true),
        MouseButton::Right => state.right_held.set(true),
        MouseButton::Middle => {}
      }

      dispatch_press(state, mouse.column, mouse.row, button, modifiers);
    }
    MouseEventKind::Up(button) => {
      // A release whose press was never delivered stands in for the whole click: VTE
      // terminals report only the first three presses of a rapid sequence to the app,
      // yet forward every release.
      let orphan = match button {
        MouseButton::Left => !state.left_held.replace(false),
        MouseButton::Right => !state.right_held.replace(false),
        MouseButton::Middle => false,
      };

      if orphan {
        dispatch_press(state, mouse.column, mouse.row, button, modifiers);
      }

      let _ = state.run_pointer(
        &state.pointer_up,
        mouse.column,
        mouse.row,
        Some(portable_button(button)),
        modifiers,
      );

      if button == MouseButton::Left {
        state.mouse_release(mouse.column, mouse.row);
        state.end_drag();
      }
    }
    MouseEventKind::ScrollDown => state.mouse_scroll(mouse.column, mouse.row, 0, 1),
    MouseEventKind::ScrollUp => state.mouse_scroll(mouse.column, mouse.row, 0, -1),
    MouseEventKind::ScrollRight => state.mouse_scroll(mouse.column, mouse.row, 1, 0),
    MouseEventKind::ScrollLeft => state.mouse_scroll(mouse.column, mouse.row, -1, 0),
  });
}

/// Dispatches one press: hover, the pointer-down handlers, then—unless a handler
/// prevented the default—the left click or the right press's copy menu.
fn dispatch_press(
  state: &State,
  column: u16,
  row: u16,
  button: MouseButton,
  modifiers: skene_core::Modifiers,
) {
  state.set_hover(column, row);

  let prevention = state.run_pointer(
    &state.pointer_down,
    column,
    row,
    Some(portable_button(button)),
    modifiers,
  );

  if prevention.default {
    return;
  }

  if button == MouseButton::Left {
    state.mouse_press(column, row);
  }

  // A right press opens the copy menu at the pressed cell while a selection exists.
  if button == MouseButton::Right {
    state.right_press(column, row);
  }
}

/// A crossterm mouse button as the portable vocabulary's.
fn portable_button(button: MouseButton) -> skene_core::PointerButton {
  match button {
    MouseButton::Left => skene_core::PointerButton::Primary,
    MouseButton::Right => skene_core::PointerButton::Secondary,
    MouseButton::Middle => skene_core::PointerButton::Auxiliary,
  }
}

/// Crossterm modifier state as the portable vocabulary's.
fn portable_modifiers(modifiers: KeyModifiers) -> skene_core::Modifiers {
  skene_core::Modifiers {
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
    Some(MouseCursor::Default) | None => "default",
  };

  let mut out = std::io::stdout();
  let _ = write!(out, "\x1b]22;{shape}\x1b\\");
  let _ = out.flush();
}

/// Writes a painted frame to the terminal, quantizing colors to `depth`; rows
/// unchanged since the previous frame are skipped.
fn draw(
  buffer: &paint::Buffer,
  depth: crate::detect::ColorDepth,
  previous: Option<&paint::Buffer>,
) {
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

    let mut current = paint::TerminalCell {
      character: ' ',
      foreground: None,
      background: None,
      bold: false,
      reverse: false,
    };

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

      let _ = queue!(out, crossterm::style::Print(cell.character));
      current = *cell;
    }
  }

  let _ = out.flush();
}

/// Dispatches one keyboard event per the shared contract: the key-down handlers run
/// first (the focused node then its ancestors), then the key's built-in default—focus
/// movement, activation, control arrows, dismissal, editing—unless a handler prevented
/// it. All inside a reactive batch; `Ctrl-C` stays unconditional.
fn dispatch_key(state: &State, key: &event::KeyEvent) {
  if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
    state.quit.set(true);

    return;
  }

  skene_reactive::batch(|| {
    let code = key.code;
    let key_event = portable_key(code, key.modifiers);
    let targets = match state.focused.get() {
      Some(focused) => state.ancestors(focused),
      None => state.key_down.borrow().keys().copied().collect(),
    };
    let mut prevention = Prevention::default();

    for target in &targets {
      prevention.merge(State::run_preventable(
        &state.key_down,
        *target,
        &key_event,
        |run, event| run(event),
      ));

      if prevention.bubbling {
        break;
      }
    }

    if prevention.default {
      return;
    }

    key_default(state, code, key.modifiers);
  });
}

/// Runs a key's built-in default: focus movement, dismissal, activation, control
/// arrows, page scrolling, and input editing.
fn key_default(state: &State, code: KeyCode, modifiers: KeyModifiers) {
  match code {
    KeyCode::Tab => {
      state.move_focus(1);

      return;
    }
    KeyCode::BackTab => {
      state.move_focus(-1);

      return;
    }
    KeyCode::Esc => {
      // The most transient layer goes first: the copy menu, then the selection, then
      // an open overlay.
      if state.menu.get().is_some() {
        state.close_menu();
      }
      else if state.selection.get().is_some() {
        state.clear_selection();
      }
      else if let Some((top, _)) = state.topmost_open_overlay() {
        State::run_handlers(&state.dismiss, top, (), |run, ()| run());
      }

      return;
    }
    _ => {}
  }

  let space_activates = code == KeyCode::Char(' ')
    && state.focused.get().is_some_and(|focused| {
      !matches!(
        state
          .nodes
          .borrow()
          .get(&focused)
          .map(|record| &record.kind),
        Some(crate::state::NodeKind::TextField { .. })
      )
    });

  if (code == KeyCode::Enter || space_activates)
    && let Some(focused) = state.focused.get()
  {
    state.activate_dispatch(focused);
  }

  if matches!(
    code,
    KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
  ) && let Some(focused) = state.focused.get()
  {
    control_arrow(state, focused, code);
  }

  if code == KeyCode::PageDown {
    state.page_scroll(1);
  }

  if code == KeyCode::PageUp {
    state.page_scroll(-1);
  }

  edit_focused_input(state, code, modifiers);
}

/// Applies an editing or caret key to the focused input: characters insert at the
/// caret, `Backspace`/`Delete` remove around it, arrows and `Home`/`End` move it, and
/// `Ctrl` makes the arrows jump and the removals delete whole words. Input handlers
/// fire on every value change.
fn edit_focused_input(state: &State, code: KeyCode, modifiers: KeyModifiers) {
  enum Edit {
    Value(String),
    Caret,
  }

  let Some(focused) = state.focused.get()
  else {
    return;
  };

  let byte_of = |value: &str, position: usize| {
    value
      .char_indices()
      .nth(position)
      .map_or(value.len(), |(index, _)| index)
  };

  let edit: Option<Edit> = state
    .nodes
    .borrow_mut()
    .get_mut(&focused)
    .and_then(|record| {
      let NodeKind::TextField { value, caret, .. } = &mut record.kind
      else {
        return None;
      };

      let count = value.chars().count();

      match code {
        KeyCode::Char(character)
          if !modifiers.contains(KeyModifiers::CONTROL)
            && !modifiers.contains(KeyModifiers::ALT) =>
        {
          let at = byte_of(value, *caret);

          value.insert(at, character);
          *caret += 1;

          Some(Edit::Value(value.clone()))
        }
        // `Ctrl+W` matches `Ctrl+Backspace`: legacy keyboard encodings cannot express
        // the latter, and the readline binding is muscle memory anyway.
        KeyCode::Backspace | KeyCode::Char('w')
          if modifiers.contains(KeyModifiers::CONTROL) && *caret > 0 =>
        {
          let target = word_before(value, *caret);

          value.replace_range(byte_of(value, target)..byte_of(value, *caret), "");
          *caret = target;

          Some(Edit::Value(value.clone()))
        }
        KeyCode::Delete if modifiers.contains(KeyModifiers::CONTROL) && *caret < count => {
          value.replace_range(
            byte_of(value, *caret)..byte_of(value, word_after(value, *caret)),
            "",
          );

          Some(Edit::Value(value.clone()))
        }
        // Legacy encodings send `Ctrl+Backspace` as `0x08`—the byte of `Ctrl+H`—with
        // no way to tell the two apart; deleting one character honors both readings.
        KeyCode::Backspace | KeyCode::Char('h')
          if !modifiers.contains(KeyModifiers::ALT) && *caret > 0 =>
        {
          *caret -= 1;
          value.remove(byte_of(value, *caret));

          Some(Edit::Value(value.clone()))
        }
        KeyCode::Delete if *caret < count => {
          value.remove(byte_of(value, *caret));

          Some(Edit::Value(value.clone()))
        }
        KeyCode::Left if modifiers.contains(KeyModifiers::CONTROL) && *caret > 0 => {
          *caret = word_before(value, *caret);

          Some(Edit::Caret)
        }
        KeyCode::Right if modifiers.contains(KeyModifiers::CONTROL) && *caret < count => {
          *caret = word_after(value, *caret);

          Some(Edit::Caret)
        }
        KeyCode::Left if *caret > 0 => {
          *caret -= 1;

          Some(Edit::Caret)
        }
        KeyCode::Right if *caret < count => {
          *caret += 1;

          Some(Edit::Caret)
        }
        KeyCode::Home if *caret > 0 => {
          *caret = 0;

          Some(Edit::Caret)
        }
        KeyCode::End if *caret < count => {
          *caret = count;

          Some(Edit::Caret)
        }
        _ => None,
      }
    });

  match edit {
    Some(Edit::Value(value)) => {
      state.dirty.set(true);

      for target in state.ancestors(focused) {
        State::run_handlers(&state.input, target, value.as_str(), |run, value| {
          run(value);
        });
      }
    }
    Some(Edit::Caret) => state.dirty.set(true),
    None => {}
  }
}

/// The caret position at the start of the word before `caret`: spaces skipped, then
/// the word.
fn word_before(value: &str, caret: usize) -> usize {
  let characters: Vec<char> = value.chars().collect();
  let mut position = caret;

  while position > 0 && characters[position - 1] == ' ' {
    position -= 1;
  }

  while position > 0 && characters[position - 1] != ' ' {
    position -= 1;
  }

  position
}

/// The caret position at the end of the word after `caret`: spaces skipped, then the
/// word.
fn word_after(value: &str, caret: usize) -> usize {
  let characters: Vec<char> = value.chars().collect();
  let mut position = caret;

  while position < characters.len() && characters[position] == ' ' {
    position += 1;
  }

  while position < characters.len() && characters[position] != ' ' {
    position += 1;
  }

  position
}

/// Applies an arrow key to the focused control: radios move the check through their
/// group, selects cycle, sliders nudge by one step.
fn control_arrow(state: &State, focused: NodeId, code: KeyCode) {
  enum Move {
    Radio,
    Picker,
    Slider,
  }

  let target = {
    let nodes = state.nodes.borrow();

    match nodes.get(&focused).map(|record| &record.kind) {
      Some(NodeKind::Radio { .. }) => Move::Radio,
      Some(NodeKind::Picker { .. }) => Move::Picker,
      Some(NodeKind::Slider { .. }) => Move::Slider,
      _ => return,
    }
  };
  let forward = matches!(code, KeyCode::Right | KeyCode::Down);

  match target {
    Move::Radio => {
      let Some((members, position)) = state.radio_group(focused)
      else {
        return;
      };

      let Ok(count) = isize::try_from(members.len())
      else {
        return;
      };

      let Ok(position) = isize::try_from(position)
      else {
        return;
      };

      let step: isize = if forward { 1 } else { -1 };
      let Ok(next) = usize::try_from((position + step).rem_euclid(count.max(1)))
      else {
        return;
      };

      let next = members[next];

      state.set_focused(Some(next));
      state.check_radio(next);
    }
    Move::Picker => state.cycle_picker(focused, if forward { 1 } else { -1 }),
    Move::Slider => {
      // A slider rises on `Up`/`Right`, unlike a group's next-member order.
      let increase = matches!(code, KeyCode::Right | KeyCode::Up);

      state.nudge_slider(focused, if increase { 1.0 } else { -1.0 });
    }
  }
}

/// Maps a terminal key to the portable W3C key value.
fn portable_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
  let key = match code {
    KeyCode::Char(character) => character.to_string(),
    KeyCode::Enter => "Enter".to_owned(),
    KeyCode::Backspace => "Backspace".to_owned(),
    KeyCode::Tab => "Tab".to_owned(),
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

  KeyEvent {
    key,
    modifiers: portable_modifiers(modifiers),
  }
}

/// Simulates one key press by its portable W3C key value (`"a"`, `"Enter"`, `"Tab"`,
/// `"Backspace"`, …), for a test driving the loop headlessly. `"Ctrl+"`, `"Alt+"`, and
/// `"Shift+"` prefixes add the modifier (`"Ctrl+ArrowLeft"`).
///
/// # Panics
///
/// Panics if `key` is not a supported key value.
#[cfg(feature = "test")]
pub fn trigger_key(key: &str) {
  let mut modifiers = KeyModifiers::NONE;
  let mut key = key;

  while let Some((modifier, rest)) = key.split_once('+') {
    match modifier {
      "Ctrl" => modifiers |= KeyModifiers::CONTROL,
      "Alt" => modifiers |= KeyModifiers::ALT,
      "Shift" => modifiers |= KeyModifiers::SHIFT,
      _ => break,
    }

    key = rest;
  }

  let code = match key {
    "Enter" => KeyCode::Enter,
    "Backspace" => KeyCode::Backspace,
    "Tab" => KeyCode::Tab,
    "Escape" => KeyCode::Esc,
    "Delete" => KeyCode::Delete,
    "ArrowLeft" => KeyCode::Left,
    "ArrowRight" => KeyCode::Right,
    "ArrowUp" => KeyCode::Up,
    "ArrowDown" => KeyCode::Down,
    "Home" => KeyCode::Home,
    "End" => KeyCode::End,
    other => {
      let mut characters = other.chars();
      let character = characters.next().expect("a key value");

      assert!(
        characters.next().is_none(),
        "unsupported key value: {other}"
      );

      KeyCode::Char(character)
    }
  };

  with_state(|state| {
    dispatch_key(state, &event::KeyEvent::new(code, modifiers));
  });
}

/// The id of the currently-focused node, for a test asserting focus movement.
#[cfg(feature = "test")]
#[must_use]
pub fn focused_node() -> Option<skene_core::NodeId> {
  with_state(|state| state.focused.get())
}

/// Simulates the mouse moving to the cell (`x`, `y`), for a test driving hover
/// headlessly. Hit-testing resolves against the last render's layout.
#[cfg(feature = "test")]
pub fn trigger_mouse_move(x: u16, y: u16) {
  with_state(|state| {
    skene_reactive::batch(|| {
      state.set_hover(x, y);
      let _ = state.run_pointer(
        &state.pointer_move,
        x,
        y,
        None,
        skene_core::Modifiers::default(),
      );
    });
  });
}

/// Simulates a left click at the cell (`x`, `y`), for a test driving mouse interaction
/// headlessly. Hit-testing resolves against the last render's layout.
#[cfg(feature = "test")]
pub fn trigger_mouse_click(x: u16, y: u16) {
  trigger_mouse_press(x, y);
  trigger_mouse_release(x, y);
}

/// Simulates pressing the left button at (`x`, `y`) without releasing—the start of a
/// drag—for a test driving drags headlessly.
#[cfg(feature = "test")]
pub fn trigger_mouse_press(x: u16, y: u16) {
  with_state(|state| {
    skene_reactive::batch(|| {
      state.left_held.set(true);
      state.set_hover(x, y);
      let _ = state.run_pointer(
        &state.pointer_down,
        x,
        y,
        Some(skene_core::PointerButton::Primary),
        skene_core::Modifiers::default(),
      );
      state.mouse_press(x, y);
    });
  });
}

/// Simulates dragging to (`x`, `y`) with the left button held, after
/// [`trigger_mouse_press`].
#[cfg(feature = "test")]
pub fn trigger_mouse_drag(x: u16, y: u16) {
  with_state(|state| {
    skene_reactive::batch(|| {
      state.set_hover(x, y);
      let _ = state.run_pointer(
        &state.pointer_move,
        x,
        y,
        Some(skene_core::PointerButton::Primary),
        skene_core::Modifiers::default(),
      );
      state.drag_to(x, y);
    });
  });
}

/// Simulates releasing the left button at (`x`, `y`), ending a drag. A release with no
/// press delivered stands in for the whole click, as the interactive loop's does.
#[cfg(feature = "test")]
pub fn trigger_mouse_release(x: u16, y: u16) {
  with_state(|state| {
    skene_reactive::batch(|| {
      if !state.left_held.replace(false) {
        state.set_hover(x, y);
        let _ = state.run_pointer(
          &state.pointer_down,
          x,
          y,
          Some(skene_core::PointerButton::Primary),
          skene_core::Modifiers::default(),
        );
        state.mouse_press(x, y);
      }

      let _ = state.run_pointer(
        &state.pointer_up,
        x,
        y,
        Some(skene_core::PointerButton::Primary),
        skene_core::Modifiers::default(),
      );
      state.mouse_release(x, y);
      state.end_drag();
    });
  });
}

/// Simulates a right click at the cell (`x`, `y`)—the copy-menu gesture—for a test
/// driving selection headlessly.
#[cfg(feature = "test")]
pub fn trigger_mouse_right_click(x: u16, y: u16) {
  with_state(|state| {
    skene_reactive::batch(|| {
      state.right_held.set(true);
      state.set_hover(x, y);
      let _ = state.run_pointer(
        &state.pointer_down,
        x,
        y,
        Some(skene_core::PointerButton::Secondary),
        skene_core::Modifiers::default(),
      );
      state.right_press(x, y);
      state.right_held.set(false);
      let _ = state.run_pointer(
        &state.pointer_up,
        x,
        y,
        Some(skene_core::PointerButton::Secondary),
        skene_core::Modifiers::default(),
      );
    });
  });
}

/// Takes the text the last `Copy` staged for the clipboard, for a headless test.
#[cfg(feature = "test")]
#[must_use]
pub fn take_copied() -> Option<String> {
  with_state(|state| state.pending_copy.borrow_mut().take())
}

/// The current terminal title, for a headless test.
#[cfg(feature = "test")]
#[must_use]
pub fn current_title() -> Option<String> {
  with_state(|state| state.title.borrow().clone())
}

/// Simulates the terminal reporting a dark or light background, for a headless test
/// driving the `DarkMode`/`LightMode` conditions.
#[cfg(feature = "test")]
pub fn trigger_color_scheme(dark: bool) {
  with_state(|state| {
    state.dark.set(dark);
    state.dirty.set(true);
  });
}

/// Simulates the clipboard-support detection, for a headless test driving the copy
/// menu's disabled state.
#[cfg(feature = "test")]
pub fn trigger_clipboard_support(supported: bool) {
  with_state(|state| {
    state.clipboard.set(supported);
    state.dirty.set(true);
  });
}

/// The pointer shape the current hover resolves to, for a headless test.
#[cfg(feature = "test")]
#[must_use]
pub fn current_cursor() -> Option<MouseCursor> {
  with_state(State::hover_cursor)
}

/// Simulates the mouse wheel at the cell (`x`, `y`), for a headless test driving
/// scrolling. Positive deltas scroll right and down, one cell per unit; hit-testing
/// resolves against the last render's layout.
#[cfg(feature = "test")]
pub fn trigger_mouse_scroll(x: u16, y: u16, delta_columns: i64, delta_rows: i64) {
  with_state(|state| {
    skene_reactive::batch(|| state.mouse_scroll(x, y, delta_columns, delta_rows));
  });
}
