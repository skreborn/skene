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
use skene_core::{IntoView, KeyEvent, MouseCursor, View};

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
/// capture off, the cursor shown. Safe to run more than once, on any thread.
fn restore_terminal() {
  let mut out = std::io::stdout();
  let _ = execute!(
    out,
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

  with_state(|state| state.dark.set(dark));

  execute!(
    std::io::stdout(),
    terminal::EnterAlternateScreen,
    EnableMouseCapture,
    cursor::Hide
  )
  .expect("enter the alternate screen");

  loop {
    let quit = with_state(|state| {
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

        draw(&paint::render(state, width, height), depth);
      }

      state.quit.get()
    });

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
        Ok(Event::Resize(..)) => with_state(|state| state.dirty.set(true)),
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

  skene_reactive::Effect::batch(|| match mouse.kind {
    MouseEventKind::Moved | MouseEventKind::Drag(_) => {
      let button = match mouse.kind {
        MouseEventKind::Drag(button) => Some(portable_button(button)),
        _ => None,
      };

      state.set_hover(mouse.column, mouse.row);
      state.run_pointer(
        &state.pointer_move,
        mouse.column,
        mouse.row,
        button,
        modifiers,
      );
      set_pointer_shape(state.hover_cursor());
    }
    MouseEventKind::Down(button) => {
      state.set_hover(mouse.column, mouse.row);
      state.run_pointer(
        &state.pointer_down,
        mouse.column,
        mouse.row,
        Some(portable_button(button)),
        modifiers,
      );

      if button == MouseButton::Left {
        state.mouse_click(mouse.column, mouse.row);
      }
    }
    MouseEventKind::Up(button) => {
      state.run_pointer(
        &state.pointer_up,
        mouse.column,
        mouse.row,
        Some(portable_button(button)),
        modifiers,
      );
    }
    MouseEventKind::ScrollDown => state.mouse_scroll(mouse.column, mouse.row, 0, 1),
    MouseEventKind::ScrollUp => state.mouse_scroll(mouse.column, mouse.row, 0, -1),
    MouseEventKind::ScrollRight => state.mouse_scroll(mouse.column, mouse.row, 1, 0),
    MouseEventKind::ScrollLeft => state.mouse_scroll(mouse.column, mouse.row, -1, 0),
  });
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

/// Writes a painted frame to the terminal, quantizing colors to `depth`.
fn draw(buffer: &paint::Buffer, depth: crate::detect::ColorDepth) {
  let mut out = std::io::stdout();
  let _ = queue!(out, cursor::MoveTo(0, 0), ResetColor);

  for row in 0..buffer.height {
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

/// Dispatches one keyboard event: focus movement, activation, input editing, and the
/// key-down handlers—all inside a reactive batch.
fn dispatch_key(state: &State, key: &event::KeyEvent) {
  if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
    state.quit.set(true);

    return;
  }

  skene_reactive::Effect::batch(|| match key.code {
    KeyCode::Tab => state.move_focus(1),
    KeyCode::BackTab => state.move_focus(-1),
    code => {
      if code == KeyCode::Esc
        && let Some((top, _)) = state.topmost_open_overlay()
      {
        State::run_handlers(&state.dismiss, top, (), |run, ()| run());

        return;
      }

      let key_event = portable_key(code, key.modifiers);
      let targets = match state.focused.get() {
        Some(focused) => state.ancestors(focused),
        None => state.key_down.borrow().keys().copied().collect(),
      };

      for target in &targets {
        State::run_handlers(&state.key_down, *target, &key_event, |run, event| {
          run(event);
        });
      }

      let space_activates = code == KeyCode::Char(' ')
        && state.focused.get().is_some_and(|focused| {
          !matches!(
            state
              .nodes
              .borrow()
              .get(&focused)
              .map(|record| &record.kind),
            Some(crate::state::NodeKind::Input { .. })
          )
        });

      if (code == KeyCode::Enter || space_activates)
        && let Some(focused) = state.focused.get()
      {
        for target in state.ancestors(focused) {
          State::run_handlers(&state.click, target, (), |run, ()| run());
        }
      }

      if code == KeyCode::PageDown {
        state.page_scroll(1);
      }

      if code == KeyCode::PageUp {
        state.page_scroll(-1);
      }

      edit_focused_input(state, code, key.modifiers);
    }
  });
}

/// Applies an editing key to the focused input, firing its input handlers.
fn edit_focused_input(state: &State, code: KeyCode, modifiers: KeyModifiers) {
  let Some(focused) = state.focused.get()
  else {
    return;
  };

  let edited: Option<String> = state
    .nodes
    .borrow_mut()
    .get_mut(&focused)
    .and_then(|record| {
      let NodeKind::Input { value, .. } = &mut record.kind
      else {
        return None;
      };

      match code {
        KeyCode::Char(character)
          if !modifiers.contains(KeyModifiers::CONTROL)
            && !modifiers.contains(KeyModifiers::ALT) =>
        {
          value.push(character);

          Some(value.clone())
        }
        KeyCode::Backspace => {
          value.pop();

          Some(value.clone())
        }
        _ => None,
      }
    });

  if let Some(value) = edited {
    state.dirty.set(true);

    for target in state.ancestors(focused) {
      State::run_handlers(&state.input, target, value.as_str(), |run, value| {
        run(value);
      });
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
/// `"Backspace"`, …), for a test driving the loop headlessly.
///
/// # Panics
///
/// Panics if `key` is not a supported key value.
#[cfg(feature = "test")]
pub fn trigger_key(key: &str) {
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
    dispatch_key(state, &event::KeyEvent::new(code, KeyModifiers::NONE));
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
    skene_reactive::Effect::batch(|| {
      state.set_hover(x, y);
      state.run_pointer(
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
  with_state(|state| {
    skene_reactive::Effect::batch(|| {
      state.set_hover(x, y);
      state.run_pointer(
        &state.pointer_down,
        x,
        y,
        Some(skene_core::PointerButton::Primary),
        skene_core::Modifiers::default(),
      );
      state.mouse_click(x, y);
      state.run_pointer(
        &state.pointer_up,
        x,
        y,
        Some(skene_core::PointerButton::Primary),
        skene_core::Modifiers::default(),
      );
    });
  });
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

/// Simulates the mouse wheel at the cell (`x`, `y`), for a headless test driving
/// scrolling. Positive deltas scroll right and down, one cell per unit; hit-testing
/// resolves against the last render's layout.
#[cfg(feature = "test")]
pub fn trigger_mouse_scroll(x: u16, y: u16, delta_columns: i32, delta_rows: i32) {
  with_state(|state| {
    skene_reactive::Effect::batch(|| state.mouse_scroll(x, y, delta_columns, delta_rows));
  });
}
