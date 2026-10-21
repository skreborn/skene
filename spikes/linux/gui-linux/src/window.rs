//! A window open on the display server in use, its loop turned a step at a time: what the
//! app's start turns until the app quits, and what a test turns between its actions.

use std::{error::Error, time::Duration};

use crate::{WindowSystem, shell::Shell, wayland, x11};

/// A window and its window system's connection.
pub struct Window {
  inner: Inner,
}

enum Inner {
  X11 {
    open: Box<x11::Open>,
    shell: Box<Shell>,
  },
  Wayland(Box<wayland::Open>),
}

impl Window {
  /// Opens `shell` as a window of `app_id` on `system`, reported to readers.
  ///
  /// # Errors
  ///
  /// Where the display server cannot be reached or refuses the window.
  pub fn open(system: WindowSystem, shell: Shell, app_id: &str) -> Result<Self, Box<dyn Error>> {
    crate::shell::reset();
    crate::access::start();

    let inner = match system {
      WindowSystem::X11 => {
        let mut shell = Box::new(shell);
        let open = Box::new(x11::open(&mut shell, app_id)?);

        Inner::X11 { open, shell }
      }
      WindowSystem::Wayland => Inner::Wayland(Box::new(wayland::open(shell, app_id)?)),
    };

    Ok(Self { inner })
  }

  /// The window's shell.
  #[must_use]
  pub fn shell(&self) -> &Shell {
    match &self.inner {
      Inner::X11 { shell, .. } => shell,
      Inner::Wayland(open) => open.shell(),
    }
  }

  /// Whether a popup it asked for awaits the display server's placement (a Wayland
  /// compositor's configure); an X11 popup is placed where it is asked for.
  #[must_use]
  pub fn popup_pending(&self) -> bool {
    match &self.inner {
      Inner::X11 { .. } => false,
      Inner::Wayland(open) => open.popup_pending(),
    }
  }

  /// The window's shell, to change.
  pub fn shell_mut(&mut self) -> &mut Shell {
    match &mut self.inner {
      Inner::X11 { shell, .. } => shell,
      Inner::Wayland(open) => open.shell_mut(),
    }
  }

  /// Turns the loop once: handles what arrived, paints what changed, then waits for more,
  /// `wait` at most where given.
  ///
  /// # Errors
  ///
  /// Where the connection to the display server fails.
  pub fn step(&mut self, wait: Option<Duration>) -> Result<(), Box<dyn Error>> {
    match &mut self.inner {
      Inner::X11 { open, shell } => open.step(shell, wait),
      Inner::Wayland(open) => open.step(wait),
    }
  }

  /// Waits until the display server has handled every request the window sent: what it asked
  /// for (a popup's grab, among them) is granted or refused before what follows reaches it.
  ///
  /// # Errors
  ///
  /// Where the connection to the display server fails.
  pub fn round_trip(&mut self) -> Result<(), Box<dyn Error>> {
    match &mut self.inner {
      Inner::X11 { open, .. } => open.round_trip(),
      Inner::Wayland(open) => open.round_trip(),
    }
  }

  /// Asks for the window's geometry to be `width` by `height` pixels: the window manager is
  /// asked on X11, and on Wayland the window takes the size as a floating window may.
  ///
  /// # Errors
  ///
  /// Where the connection to the display server fails.
  pub fn resize(&mut self, width: u32, height: u32) -> Result<(), Box<dyn Error>> {
    match &mut self.inner {
      Inner::X11 { open, shell } => open.resize(shell, width, height),
      Inner::Wayland(open) => {
        open.resize(width, height);

        Ok(())
      }
    }
  }
}

impl Drop for Window {
  fn drop(&mut self) {
    self.shell_mut().release_paths();
    crate::access::stop();
  }
}
