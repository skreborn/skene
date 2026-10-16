//! A window open on the display server in use, its loop turned a step at a time: what the
//! app's start turns until the app quits, and what a test turns between its actions.

use std::{error::Error, time::Duration};

use crate::{Backend, shell::Shell, wayland, x11};

/// A window and its backend's connection.
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
  /// Opens `shell` as a window of `app_id` on `backend`'s display server, reported to readers.
  ///
  /// # Errors
  ///
  /// Where the display server cannot be reached or refuses the window.
  pub fn open(backend: Backend, shell: Shell, app_id: &str) -> Result<Self, Box<dyn Error>> {
    crate::shell::reset();
    crate::access::start();

    let inner = match backend {
      Backend::X11 => {
        let mut shell = Box::new(shell);
        let open = Box::new(x11::open(&mut shell, app_id)?);

        Inner::X11 { open, shell }
      }
      Backend::Wayland => Inner::Wayland(Box::new(wayland::open(shell, app_id)?)),
    };

    Ok(Self { inner })
  }

  /// The display server the window is on.
  #[must_use]
  pub const fn backend(&self) -> Backend {
    match self.inner {
      Inner::X11 { .. } => Backend::X11,
      Inner::Wayland(_) => Backend::Wayland,
    }
  }

  /// The window's shell.
  #[must_use]
  pub fn shell(&self) -> &Shell {
    match &self.inner {
      Inner::X11 { shell, .. } => shell,
      Inner::Wayland(open) => open.shell(),
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
    crate::access::stop();
  }
}
