//! The launcher `attach` runs: the terminal app a `run terminal` loop builds, each rebuilt
//! binary swapped in with the state carried across.

#[cfg(unix)]
use std::{thread, time::Duration};

#[cfg(unix)]
use skene_dev::{CONTROL_VARIABLE, Control, STATE_VARIABLE, SWAP_EXIT};

#[cfg(unix)]
use crate::server::{Binary, fetch};

/// Runs the binary the `run terminal` loop with id `id` publishes, in this terminal, as its
/// launcher: the app hears each rebuilt binary and each notice on a control pipe,
/// records its state to an unnamed file it inherits and becomes the next binary in
/// place, so the terminal, the pipe and the state carry across every swap. Ends when
/// the app itself exits.
#[cfg(unix)]
pub(crate) fn attach(id: u32) -> Result<(), String> {
  use std::{io::Write as _, os::fd::AsRawFd as _};

  let published =
    || fetch(id, "/skene:binary").and_then(|body| Binary::parse(&String::from_utf8_lossy(&body)));
  let mut current = published().ok_or_else(|| {
    format!("no `run terminal` loop answers as {id}; start one and attach to the id it prints")
  })?;
  let (reader, mut writer) =
    std::io::pipe().map_err(|error| format!("open the control pipe: {error}"))?;

  // The app inherits the read end across every spawn and every swap in place, so it
  // must not close on exec.
  rustix::io::fcntl_setfd(&reader, rustix::io::FdFlags::empty())
    .map_err(|error| format!("share the control pipe: {error}"))?;

  let state = unnamed_file()?;

  rustix::io::fcntl_setfd(&state, rustix::io::FdFlags::empty())
    .map_err(|error| format!("share the state file: {error}"))?;

  let control = reader.as_raw_fd().to_string();
  let state_fd = state.as_raw_fd().to_string();
  let spawn = |binary: &Binary| {
    std::process::Command::new(&binary.path)
      .current_dir(&binary.dir)
      .env(CONTROL_VARIABLE, &control)
      .env(STATE_VARIABLE, &state_fd)
      .spawn()
      .map_err(|error| format!("{}: {error}", binary.path.display()))
  };
  // The terminal state before any app instance touches it, reapplied before a
  // spawn that follows an instance which could not restore its own.
  let sane_tty = crate::tty::Tty::capture();
  let mut child = spawn(&current)?;
  let mut notice: Option<Vec<u8>> = None;

  loop {
    if let Ok(Some(status)) = child.try_wait() {
      if status.code() == Some(SWAP_EXIT) {
        if let Some(tty) = &sane_tty {
          tty.restore();
        }

        child = spawn(&current)?;

        continue;
      }

      // The user quit the app; the attachment ends with it.
      return Ok(());
    }

    thread::sleep(Duration::from_millis(250));

    match published() {
      Some(next) if next.generation != current.generation => {
        current = next;

        let _ = writeln!(writer, "{}", Control::Swap(current.path.clone()).to_line());
      }
      Some(_) => {}
      None => {
        // The loop is gone; the app runs on until it quits.
        let _ = child.wait();

        return Ok(());
      }
    }

    if let Some(body) = fetch(id, "/skene:notice")
      && notice.as_ref() != Some(&body)
    {
      let text = std::str::from_utf8(&body)
        .ok()
        .and_then(|body| body.split_once('\n'))
        .map(|(_, text)| text.trim_end().to_owned())
        .filter(|text| !text.is_empty());

      let _ = writeln!(writer, "{}", Control::Notice(text).to_line());

      notice = Some(body);
    }
  }
}

/// A file only this process and its children reach: made where no other file is,
/// readable by its owner alone, and unlinked at once.
#[cfg(unix)]
fn unnamed_file() -> Result<std::fs::File, String> {
  use std::os::unix::fs::OpenOptionsExt as _;

  let dir = std::env::temp_dir();

  for attempt in 0..16_u32 {
    let path = dir.join(format!(
      "skene-state-{}-{attempt}-{}",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos())
    ));
    let opened = std::fs::OpenOptions::new()
      .read(true)
      .write(true)
      .create_new(true)
      .mode(0o600)
      .open(&path);

    match opened {
      Ok(file) => {
        std::fs::remove_file(&path)
          .map_err(|error| format!("unlink {}: {error}", path.display()))?;

        return Ok(file);
      }
      Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
      Err(error) => return Err(format!("make the state file: {error}")),
    }
  }

  Err("no free name for the state file".to_owned())
}

/// `attach` needs the terminal layer, which exists on Unix alone.
#[cfg(not(unix))]
pub(crate) fn attach(_id: u32) -> Result<(), String> {
  Err("`attach` runs on Unix; the terminal has no console layer yet".to_owned())
}
