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
    let mut out = std::io::stdout();
    let _ = execute!(
      out,
      DisableMouseCapture,
      cursor::Show,
      terminal::LeaveAlternateScreen
    );

    let _ = terminal::disable_raw_mode();
  }
}

/// Mounts and runs a skene application in the terminal.
///
/// Installs the terminal driver, builds `app`, and runs the keyboard loop until
/// `Ctrl-C`. The reactive runtime, handlers and tree are kept alive for the run's
/// lifetime.
///
/// # Panics
///
/// Panics if the driver is already installed, or when the terminal cannot be set up.
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView,
{
  crate::install();
  crate::mount(|| app().into_view().build());

  terminal::enable_raw_mode().expect("enable raw mode");

  let _guard = TerminalGuard;

  execute!(
    std::io::stdout(),
    terminal::EnterAlternateScreen,
    EnableMouseCapture,
    cursor::Hide
  )
  .expect("enter the alternate screen");

  loop {
    let quit = with_state(|state| {
      if state.dirty.replace(false) {
        // A degenerate reported size (a fresh pseudo-terminal) falls back to a classic
        // 80x24 rather than painting nothing.
        let (width, height) = match terminal::size() {
          Ok((width, height)) if width > 0 && height > 0 => (width, height),
          _ => (80, 24),
        };

        draw(&paint::render(state, width, height));
      }

      state.quit.get()
    });

    if quit {
      break;
    }

    if event::poll(Duration::from_millis(100)).unwrap_or(false) {
      match event::read() {
        Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
          with_state(|state| dispatch_key(state, &key));
        }
        Ok(Event::Mouse(mouse)) => {
          with_state(|state| dispatch_mouse(state, mouse));
        }
        Ok(Event::Resize(..)) => with_state(|state| state.dirty.set(true)),
        _ => {}
      }
    }
  }
}

/// Dispatches one mouse event: hover tracking (with the pointer shape), and left-button
/// clicks—all inside a reactive batch.
fn dispatch_mouse(state: &State, mouse: event::MouseEvent) {
  skene_reactive::Effect::batch(|| match mouse.kind {
    MouseEventKind::Moved | MouseEventKind::Drag(_) => {
      state.set_hover(mouse.column, mouse.row);
      set_pointer_shape(state.hover_cursor());
    }
    MouseEventKind::Down(MouseButton::Left) => {
      state.set_hover(mouse.column, mouse.row);
      state.mouse_click(mouse.column, mouse.row);
    }
    _ => {}
  });
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

/// Writes a painted frame to the terminal.
fn draw(buffer: &paint::Buffer) {
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
          .map_or(TerminalColor::Reset, |(red, green, blue)| {
            TerminalColor::Rgb {
              r: red,
              g: green,
              b: blue,
            }
          });

        let _ = queue!(out, SetForegroundColor(color));
      }

      if cell.background != current.background {
        let color = cell
          .background
          .map_or(TerminalColor::Reset, |(red, green, blue)| {
            TerminalColor::Rgb {
              r: red,
              g: green,
              b: blue,
            }
          });

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

      if code == KeyCode::Enter
        && let Some(focused) = state.focused.get()
      {
        for target in state.ancestors(focused) {
          State::run_handlers(&state.click, target, (), |run, ()| run());
        }
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
    ctrl: modifiers.contains(KeyModifiers::CONTROL),
    shift: modifiers.contains(KeyModifiers::SHIFT),
    alt: modifiers.contains(KeyModifiers::ALT),
    meta: modifiers.contains(KeyModifiers::SUPER),
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
    skene_reactive::Effect::batch(|| state.set_hover(x, y));
  });
}

/// Simulates a left click at the cell (`x`, `y`), for a test driving mouse interaction
/// headlessly. Hit-testing resolves against the last render's layout.
#[cfg(feature = "test")]
pub fn trigger_mouse_click(x: u16, y: u16) {
  with_state(|state| {
    skene_reactive::Effect::batch(|| {
      state.set_hover(x, y);
      state.mouse_click(x, y);
    });
  });
}
