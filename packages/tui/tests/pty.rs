//! The real loop end to end in a pseudo-terminal: the app draws into the terminal, a
//! typed `Tab` and `Enter` reach its button, a resize reflows it, and `Ctrl-Q` ends it,
//! the terminal handed back.

#![cfg(unix)]

use std::{thread, time::Duration};

use skene_tui::{
  Activate,
  Launch,
  WindowSize,
  prelude::*,
  testing::{Pty, PtyHost},
};

/// The screen a terminal emulator shows for what the app drew, fed as it arrives.
struct Screen {
  parser: vt100::Parser,
}

impl Screen {
  /// An empty screen of `size`.
  fn new(size: WindowSize) -> Self {
    Self {
      parser: vt100::Parser::new(size.rows, size.columns, 0),
    }
  }

  /// Reads what the app draws until the screen shows `text`, or panics after `patience`.
  fn wait_for(&mut self, host: &mut PtyHost, text: &str, patience: Duration) -> String {
    let deadline = std::time::Instant::now() + patience;

    loop {
      let drawn = host
        .drawn(Duration::from_millis(100))
        .expect("read the terminal");

      self.parser.process(&drawn);

      let contents = self.parser.screen().contents();

      if contents.contains(text) {
        return contents;
      }

      assert!(
        std::time::Instant::now() < deadline,
        "the screen never showed {text:?}:\n{contents}"
      );
    }
  }
}

#[test]
fn the_loop_draws_takes_keys_reflows_and_quits_in_a_terminal() {
  let size = WindowSize::cells(40, 8);
  let (pty, mut host) = Pty::open(size).expect("open a pseudo-terminal");
  let app = thread::spawn(move || {
    Launch::new().run_on(pty, || {
      let clicks = Signal::new(0u32);

      Column::new().set_children((
        RawButton::new(Activate::new())
          .set_label("Press")
          .handle_click(move || clicks.update(|count| *count += 1)),
        Text::derive(move || format!("pressed {} time(s)", clicks.get())),
      ))
    })
  });
  let mut screen = Screen::new(size);

  screen.wait_for(&mut host, "pressed 0 time(s)", Duration::from_secs(10));

  host.type_bytes(b"\t").expect("type Tab");
  host.type_bytes(b"\r").expect("type Enter");

  screen.wait_for(&mut host, "pressed 1 time(s)", Duration::from_secs(10));

  assert!(
    screen.parser.screen().alternate_screen(),
    "the app draws on the alternate screen"
  );

  let narrow = WindowSize::cells(12, 8);

  host.resize(narrow).expect("resize the terminal");
  screen
    .parser
    .screen_mut()
    .set_size(narrow.rows, narrow.columns);

  let reflowed = screen.wait_for(&mut host, "time(s)", Duration::from_secs(10));

  assert!(
    !reflowed.contains("pressed 1 time(s)"),
    "the text wraps to the narrow window:\n{reflowed}"
  );

  host.type_bytes(b"\x11").expect("type Ctrl-Q");

  let pty = app.join().expect("the app ends");
  let _ = host
    .drawn(Duration::from_millis(100))
    .map(|drawn| screen.parser.process(&drawn));

  assert!(
    !screen.parser.screen().alternate_screen(),
    "quitting hands the terminal back"
  );

  drop(pty);
}
