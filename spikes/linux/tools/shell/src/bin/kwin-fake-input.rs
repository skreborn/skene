//! `kwin-fake-input`: KWin's fake input for a test on the KWin legs, in a process of its own.
//!
//! KWin offers `org_kde_kwin_fake_input` only to an executable a desktop file grants it to
//! (`X-KDE-Wayland-Interfaces`), and hashes that executable on every connection that asks.
//! skene-runner starts KWin with its permission checks on and grants the interface to this
//! helper alone: a test's own connections (one a window) are refused the restricted interfaces
//! at once, unhashed, and the test acts through this helper, which it starts and drives over
//! its standard input, one command a line (points in KWin's logical pixels):
//!
//! ```text
//! motion <x> <y>
//! button <code> <0|1>
//! key <code> <0|1>
//! axis <axis> <value>
//! touch-down <id> <x> <y>
//! touch-motion <id> <x> <y>
//! touch-up <id>
//! sync
//! ```
//!
//! It prints `ready` once KWin has taken its authentication, and answers each `sync` with `ok`
//! once KWin has handled every command before it, or with `error <reason>`. It ends at the end
//! of its input.

use std::{
  io::{BufRead as _, Write as _},
  process::ExitCode,
};

use wayland_client::{
  Connection, Dispatch, EventQueue, QueueHandle, delegate_noop,
  globals::{GlobalListContents, registry_queue_init},
  protocol::wl_registry,
};
use wayland_protocols_plasma::fake_input::client::org_kde_kwin_fake_input::OrgKdeKwinFakeInput;

/// The helper's state: it handles no event.
struct State;

delegate_noop!(State: ignore OrgKdeKwinFakeInput);

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
  fn event(
    _: &mut Self,
    _: &wl_registry::WlRegistry,
    _: wl_registry::Event,
    _: &GlobalListContents,
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
  }
}

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("kwin-fake-input: {error}");

      ExitCode::FAILURE
    }
  }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
  let conn = Connection::connect_to_env()?;
  let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
  let qh = queue.handle();
  let fake: OrgKdeKwinFakeInput = globals.bind(&qh, 4..=4, ()).map_err(|error| {
    format!("KWin offers no fake input here ({error}): is the desktop file granting it in place?")
  })?;

  fake.authenticate("skene-testing".to_owned(), "a test's real input".to_owned());
  queue.roundtrip(&mut State)?;

  let mut out = std::io::stdout().lock();

  writeln!(out, "ready")?;
  out.flush()?;

  for line in std::io::stdin().lock().lines() {
    let line = line?;
    let words: Vec<&str> = line.split_whitespace().collect();

    match send(&fake, &words) {
      Ok(true) => {
        match sync(&mut queue) {
          Ok(()) => writeln!(out, "ok")?,
          Err(error) => writeln!(out, "error {error}")?,
        }

        out.flush()?;
      }
      Ok(false) => {}
      Err(error) => {
        writeln!(out, "error {error}: {line}")?;
        out.flush()?;
      }
    }
  }

  Ok(())
}

/// Sends the request `words` name; whether they asked for a sync.
fn send(fake: &OrgKdeKwinFakeInput, words: &[&str]) -> Result<bool, Box<dyn std::error::Error>> {
  let number = |index: usize| -> Result<f64, Box<dyn std::error::Error>> {
    Ok(
      words
        .get(index)
        .ok_or("a number is missing")?
        .parse::<f64>()?,
    )
  };
  let whole = |index: usize| -> Result<u32, Box<dyn std::error::Error>> {
    Ok(
      words
        .get(index)
        .ok_or("a number is missing")?
        .parse::<u32>()?,
    )
  };

  match words.first().copied() {
    Some("sync") => return Ok(true),
    Some("motion") => fake.pointer_motion_absolute(number(1)?, number(2)?),
    Some("button") => fake.button(whole(1)?, whole(2)?),
    Some("key") => fake.keyboard_key(whole(1)?, whole(2)?),
    Some("axis") => fake.axis(whole(1)?, number(2)?),
    Some("touch-down") => {
      fake.touch_down(whole(1)?, number(2)?, number(3)?);
      fake.touch_frame();
    }
    Some("touch-motion") => {
      fake.touch_motion(whole(1)?, number(2)?, number(3)?);
      fake.touch_frame();
    }
    Some("touch-up") => {
      fake.touch_up(whole(1)?);
      fake.touch_frame();
    }
    _ => return Err("an unknown command".into()),
  }

  Ok(false)
}

/// Waits until KWin has handled every request sent.
fn sync(queue: &mut EventQueue<State>) -> Result<(), Box<dyn std::error::Error>> {
  queue.roundtrip(&mut State)?;

  Ok(())
}
