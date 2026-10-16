//! An input method in a test: a stand-in for Fcitx5 on the test's private session bus, served
//! from a thread of its own, which a window on X11 finds and talks to through its real path
//! (`org.fcitx.Fcitx.InputMethod1` and `InputContext1`), as it talks to Fcitx5 on a desktop.
//! It hands out one input context, answers every key as not its own, and keeps what the
//! window told it: whether a field has focus, the caret's rectangle (on the screen, in device
//! pixels) and the text around the caret.

use std::sync::{Arc, Mutex, PoisonError};

use zbus::blocking::Connection;

use self::served::{Context, Method};

/// The context the stand-in hands out.
const CONTEXT: &str = "/org/freedesktop/portal/inputcontext/1";

/// What a window told the input method.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Told {
  /// Whether a field has focus: `FocusIn` came after the last `FocusOut`.
  pub focused: bool,
  /// The caret's rectangle, last told: `x, y, width, height` on the screen, in device pixels.
  pub cursor: Option<(i32, i32, i32, i32)>,
  /// The text around the caret, last told: the text, the caret and the anchor in characters.
  pub surrounding: Option<(String, u32, u32)>,
  /// How many contexts were asked for.
  pub contexts: u32,
}

type Shared = Arc<Mutex<Told>>;

/// The served interfaces, in a module of their own.
mod served {
  use std::sync::PoisonError;

  use zbus::{interface, zvariant::OwnedObjectPath};

  use super::{CONTEXT, Shared};

  pub(super) struct Method {
    pub(super) told: Shared,
  }

  #[interface(name = "org.fcitx.Fcitx.InputMethod1")]
  impl Method {
    /// A new input context for a client with `hints`: the one context, and its id.
    fn create_input_context(
      &self,
      _hints: Vec<(String, String)>,
    ) -> zbus::fdo::Result<(OwnedObjectPath, Vec<u8>)> {
      self
        .told
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contexts += 1;

      let path = OwnedObjectPath::try_from(CONTEXT)
        .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;

      Ok((path, vec![1; 16]))
    }
  }

  pub(super) struct Context {
    pub(super) told: Shared,
  }

  impl Context {
    fn tell(&self, change: impl FnOnce(&mut super::Told)) {
      change(&mut self.told.lock().unwrap_or_else(PoisonError::into_inner));
    }
  }

  #[interface(name = "org.fcitx.Fcitx.InputContext1")]
  impl Context {
    /// What the client can show.
    fn set_capability(&self, _capability: u64) {}

    /// A field took focus.
    fn focus_in(&self) {
      self.tell(|told| told.focused = true);
    }

    /// The field lost focus.
    fn focus_out(&self) {
      self.tell(|told| told.focused = false);
    }

    /// The client dropped the composition.
    fn reset(&self) {}

    /// The caret's rectangle on the screen.
    fn set_cursor_rect(&self, x: i32, y: i32, width: i32, height: i32) {
      self.tell(|told| told.cursor = Some((x, y, width, height)));
    }

    /// The text around the caret.
    fn set_surrounding_text(&self, text: String, cursor: u32, anchor: u32) {
      self.tell(|told| told.surrounding = Some((text, cursor, anchor)));
    }

    /// A key: never the input method's, so the client handles it.
    fn process_key_event(
      &self,
      _keysym: u32,
      _keycode: u32,
      _state: u32,
      _release: bool,
      _time: u32,
    ) -> bool {
      false
    }
  }
}

/// The stand-in, served while it lives.
pub struct InputMethod {
  _connection: Connection,
  told: Shared,
}

impl InputMethod {
  /// Serves the stand-in as `org.fcitx.Fcitx5` on the session bus. A window opened after
  /// finds it and talks to it (on X11; on Wayland the window's input method is the
  /// compositor's, over text-input-v3).
  ///
  /// # Errors
  ///
  /// Where there is no session bus, or another input method holds the name.
  pub fn serve() -> zbus::Result<Self> {
    let told = Shared::default();
    let connection = zbus::blocking::connection::Builder::session()?
      .name("org.fcitx.Fcitx5")?
      .serve_at(
        "/org/freedesktop/portal/inputmethod",
        Method {
          told: Arc::clone(&told),
        },
      )?
      .serve_at(
        CONTEXT,
        Context {
          told: Arc::clone(&told),
        },
      )?
      .build()?;

    Ok(Self {
      _connection: connection,
      told,
    })
  }

  /// What the window told it so far.
  #[must_use]
  pub fn told(&self) -> Told {
    self
      .told
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .clone()
  }
}
