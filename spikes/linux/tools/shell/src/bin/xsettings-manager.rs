//! An XSETTINGS manager on `$DISPLAY`, for the realization's X11 settings checks: it owns
//! `_XSETTINGS_S0` and publishes the settings it is given, then changes them a step at a time.
//!
//! `xsettings-manager <name>=<value> ... [@ <name>=<value> ...]`: an integer value is an integer
//! setting, anything else a string (`Xft/DPI=147456`, `Net/ThemeName=Adwaita-dark`); each `@`
//! starts the next step, which each `SIGUSR1` it is sent publishes. It exits after 120 seconds.

use std::time::Duration;

use skene_shell_tools::Steps;

use x11rb::{
  connection::Connection as _,
  protocol::xproto::{AtomEnum, ConnectionExt as _, CreateWindowAux, PropMode, WindowClass},
  wrapper::ConnectionExt as _,
};

fn encode(settings: &[(String, String)], serial: u32) -> Vec<u8> {
  let mut data = vec![0u8, 0, 0, 0];
  let pad = |data: &mut Vec<u8>| {
    while !data.len().is_multiple_of(4) {
      data.push(0);
    }
  };

  data.extend_from_slice(&serial.to_le_bytes());
  data.extend_from_slice(&(settings.len() as u32).to_le_bytes());

  for (name, value) in settings {
    let integer = value.parse::<i32>().ok();

    data.push(if integer.is_some() { 0 } else { 1 });
    data.push(0);
    data.extend_from_slice(&(name.len() as u16).to_le_bytes());
    data.extend_from_slice(name.as_bytes());
    pad(&mut data);
    data.extend_from_slice(&0u32.to_le_bytes());

    match integer {
      Some(integer) => data.extend_from_slice(&integer.to_le_bytes()),
      None => {
        data.extend_from_slice(&(value.len() as u32).to_le_bytes());
        data.extend_from_slice(value.as_bytes());
        pad(&mut data);
      }
    }
  }

  data
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let steps = Steps::block();
  let started = std::time::Instant::now();
  let (conn, screen) = x11rb::connect(None)?;
  let root = conn.setup().roots[screen].root;
  let window = conn.generate_id()?;

  conn.create_window(
    0,
    window,
    root,
    0,
    0,
    1,
    1,
    0,
    WindowClass::INPUT_ONLY,
    0,
    &CreateWindowAux::new(),
  )?;

  let selection = conn
    .intern_atom(false, format!("_XSETTINGS_S{screen}").as_bytes())?
    .reply()?
    .atom;
  let property = conn
    .intern_atom(false, b"_XSETTINGS_SETTINGS")?
    .reply()?
    .atom;
  let mut phases: Vec<Vec<(String, String)>> = vec![Vec::new()];

  for argument in std::env::args().skip(1) {
    if argument == "@" {
      phases.push(Vec::new());
    } else if let Some((name, value)) = argument.split_once('=') {
      phases
        .last_mut()
        .expect("a phase")
        .push((name.to_owned(), value.to_owned()));
    }
  }

  let lifetime = Duration::from_secs(120);
  let mut settings: Vec<(String, String)> = Vec::new();

  for (serial, changes) in phases.into_iter().enumerate() {
    if serial > 0 && !steps.next(lifetime.saturating_sub(started.elapsed())) {
      return Ok(());
    }

    for (name, value) in changes {
      settings.retain(|(existing, _)| *existing != name);
      settings.push((name, value));
    }

    conn.change_property8(
      PropMode::REPLACE,
      window,
      property,
      property,
      &encode(&settings, serial as u32),
    )?;

    if serial == 0 {
      conn.set_selection_owner(window, selection, x11rb::CURRENT_TIME)?;
    }

    conn.flush()?;
    println!(
      "published {} settings at step {serial}, {} ms",
      settings.len(),
      started.elapsed().as_millis()
    );
  }

  let _ = AtomEnum::ANY;

  std::thread::sleep(lifetime.saturating_sub(started.elapsed()));

  Ok(())
}
