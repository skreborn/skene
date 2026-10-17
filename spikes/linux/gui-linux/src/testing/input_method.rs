//! Input methods in a test: stand-ins a window finds and talks to through its real path, as it
//! talks to an input method on a desktop, each served from a thread of the test's own.
//!
//! - [`InputMethod::serve`]: Fcitx5 (`org.fcitx.Fcitx5` on the test's private session bus,
//!   `org.fcitx.Fcitx.InputMethod1` and `InputContext1`), which a window on X11 finds;
//! - [`InputMethod::serve_ibus`]: IBus (`org.freedesktop.IBus` on the same bus, its address in
//!   the file ibus-daemon writes, in the test's private configuration directory), which a
//!   window on X11 finds where no Fcitx5 runs;
//! - [`InputMethod::serve_wayland`]: an input method of the compositor's seat
//!   (`zwp_input_method_v2`, which sway offers), to which the compositor relays what a window
//!   says over text-input-v3.
//!
//! Each hands out an input context per window, answers every key as not its own (the D-Bus ones are
//! handed every key; the Wayland one takes no keyboard), keeps what the window told it, and
//! sends what an input method sends ([`InputMethod::trigger_commit`],
//! [`InputMethod::trigger_delete_before`]).

use std::{
  os::fd::AsRawFd as _,
  path::PathBuf,
  sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
  },
  thread::JoinHandle,
};

use sctk::reexports::protocols::wp::text_input::zv3::client::zwp_text_input_v3::ContentPurpose;
use wayland_protocols_misc::zwp_input_method_v2::client::{
  zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
  zwp_input_method_v2::{self, ZwpInputMethodV2},
};
use zbus::blocking::Connection;

use self::served::{IBusContext, IBusMethod, Method};

/// Where the Fcitx5 contexts the stand-in hands out live, each at a number of its own.
const CONTEXTS: &str = "/org/freedesktop/portal/inputcontext";

/// The IBus context the stand-in hands out.
const IBUS_CONTEXT: &str = "/org/freedesktop/IBus/InputContext_1";

/// What a field is for, as a window told an input method: the purpose its protocol carries.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Purpose {
  /// Any text, or nothing told.
  #[default]
  Normal,
  /// An email address.
  Email,
  /// A number.
  Number,
  /// A phone number.
  Phone,
  /// A URL.
  Url,
  /// A secret.
  Password,
}

impl Purpose {
  /// The purpose Fcitx5's capability bits state: password 1 << 3, email 1 << 7, URL 1 << 12,
  /// dialable 1 << 13, number 1 << 14.
  #[must_use]
  pub const fn of_fcitx(capability: u64) -> Self {
    if capability & 1 << 3 != 0 {
      Self::Password
    } else if capability & 1 << 7 != 0 {
      Self::Email
    } else if capability & 1 << 12 != 0 {
      Self::Url
    } else if capability & 1 << 13 != 0 {
      Self::Phone
    } else if capability & 1 << 14 != 0 {
      Self::Number
    } else {
      Self::Normal
    }
  }

  /// The purpose IBus's input purpose states: number 3, phone 4, URL 5, email 6, password 8.
  #[must_use]
  pub const fn of_ibus(purpose: u32) -> Self {
    match purpose {
      3 => Self::Number,
      4 => Self::Phone,
      5 => Self::Url,
      6 => Self::Email,
      8 => Self::Password,
      _ => Self::Normal,
    }
  }

  /// The purpose text-input-v3's content purpose states.
  #[must_use]
  pub const fn of_wayland(purpose: ContentPurpose) -> Self {
    match purpose {
      ContentPurpose::Number | ContentPurpose::Digits => Self::Number,
      ContentPurpose::Phone => Self::Phone,
      ContentPurpose::Url => Self::Url,
      ContentPurpose::Email => Self::Email,
      ContentPurpose::Password | ContentPurpose::Pin => Self::Password,
      _ => Self::Normal,
    }
  }
}

/// What a window told the input method.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Told {
  /// Whether a field has focus: `FocusIn` came after the last `FocusOut` (on Wayland: the
  /// compositor activated the input method).
  pub focused: bool,
  /// The caret's rectangle, last told: `x, y, width, height` on the screen, in device pixels
  /// (never on Wayland, where the compositor keeps it).
  pub cursor: Option<(i32, i32, i32, i32)>,
  /// The text around the caret, last told: the text, the caret and the anchor in characters.
  pub surrounding: Option<(String, u32, u32)>,
  /// Every text around the caret told, in order.
  pub surroundings: Vec<String>,
  /// What the focused field was last told to be for: Fcitx5's capability, IBus's input
  /// purpose, text-input-v3's content purpose.
  pub purpose: Purpose,
  /// How many key events the window handed the input method.
  pub keys: u32,
  /// How many contexts were asked for.
  pub contexts: u32,
}

impl Told {
  /// Records a text around the caret, its caret and anchor in characters.
  fn surround(&mut self, text: String, cursor: u32, anchor: u32) {
    self.surroundings.push(text.clone());
    self.surrounding = Some((text, cursor, anchor));
  }
}

type Shared = Arc<Mutex<Told>>;

/// Changes what was told, a poisoned lock taken as it is.
fn tell(told: &Shared, change: impl FnOnce(&mut Told)) {
  change(&mut told.lock().unwrap_or_else(PoisonError::into_inner));
}

/// The served interfaces, in a module of their own.
mod served {
  use zbus::{
    interface,
    zvariant::{OwnedObjectPath, OwnedValue},
  };

  use std::sync::{Arc, Mutex, PoisonError};

  use super::{CONTEXTS, IBUS_CONTEXT, Purpose, Shared, tell};

  pub(super) struct Method {
    pub(super) told: Shared,
    pub(super) latest: Arc<Mutex<String>>,
  }

  #[interface(name = "org.fcitx.Fcitx.InputMethod1")]
  impl Method {
    /// A new input context for a client with `hints`, at a path of its own, and its id: the
    /// one the stand-in's signals go to from then on.
    async fn create_input_context(
      &self,
      #[zbus(object_server)] server: &zbus::ObjectServer,
      _hints: Vec<(String, String)>,
    ) -> zbus::fdo::Result<(OwnedObjectPath, Vec<u8>)> {
      let mut number = 0;

      tell(&self.told, |told| {
        told.contexts += 1;
        number = told.contexts;
      });

      let path = format!("{CONTEXTS}/{number}");

      server
        .at(
          path.as_str(),
          Context {
            told: Arc::clone(&self.told),
          },
        )
        .await
        .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
      path.clone_into(&mut self.latest.lock().unwrap_or_else(PoisonError::into_inner));

      let path = OwnedObjectPath::try_from(path)
        .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;

      Ok((path, vec![1; 16]))
    }
  }

  pub(super) struct Context {
    pub(super) told: Shared,
  }

  #[interface(name = "org.fcitx.Fcitx.InputContext1")]
  impl Context {
    /// What the client can show, and what the focused field is for.
    fn set_capability(&self, capability: u64) {
      tell(&self.told, |told| {
        told.purpose = Purpose::of_fcitx(capability)
      });
    }

    /// A field took focus.
    fn focus_in(&self) {
      tell(&self.told, |told| told.focused = true);
    }

    /// The field lost focus.
    fn focus_out(&self) {
      tell(&self.told, |told| told.focused = false);
    }

    /// The client dropped the composition.
    fn reset(&self) {}

    /// The caret's rectangle on the screen.
    fn set_cursor_rect(&self, x: i32, y: i32, width: i32, height: i32) {
      tell(&self.told, |told| told.cursor = Some((x, y, width, height)));
    }

    /// The text around the caret.
    fn set_surrounding_text(&self, text: String, cursor: u32, anchor: u32) {
      tell(&self.told, |told| told.surround(text, cursor, anchor));
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
      tell(&self.told, |told| told.keys += 1);

      false
    }
  }

  pub(super) struct IBusMethod {
    pub(super) told: Shared,
    pub(super) client: Arc<Mutex<Option<String>>>,
  }

  #[interface(name = "org.freedesktop.IBus")]
  impl IBusMethod {
    /// A new input context for the client `name`: the one context, whose signals go to the
    /// client that asked (IBus sends them to it rather than to every listener).
    fn create_input_context(
      &self,
      #[zbus(header)] header: zbus::message::Header<'_>,
      _name: String,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
      tell(&self.told, |told| told.contexts += 1);
      *self.client.lock().unwrap_or_else(PoisonError::into_inner) =
        header.sender().map(ToString::to_string);

      OwnedObjectPath::try_from(IBUS_CONTEXT)
        .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))
    }
  }

  pub(super) struct IBusContext {
    pub(super) told: Shared,
    pub(super) content_type: (u32, u32),
  }

  #[interface(name = "org.freedesktop.IBus.InputContext")]
  impl IBusContext {
    /// What the client can show.
    fn set_capabilities(&self, _capabilities: u32) {}

    /// A field took focus.
    fn focus_in(&self) {
      tell(&self.told, |told| told.focused = true);
    }

    /// The field lost focus.
    fn focus_out(&self) {
      tell(&self.told, |told| told.focused = false);
    }

    /// The client dropped the composition.
    fn reset(&self) {}

    /// The caret's rectangle on the screen.
    fn set_cursor_location(&self, x: i32, y: i32, width: i32, height: i32) {
      tell(&self.told, |told| told.cursor = Some((x, y, width, height)));
    }

    /// The text around the caret, an `IBusText`.
    fn set_surrounding_text(&self, text: OwnedValue, cursor: u32, anchor: u32) {
      let text = crate::ime::ibus::ibus_text(&text).unwrap_or_default();

      tell(&self.told, |told| told.surround(text, cursor, anchor));
    }

    /// A key: never the input method's, so the client handles it.
    fn process_key_event(&self, _keysym: u32, _keycode: u32, _state: u32) -> bool {
      tell(&self.told, |told| told.keys += 1);

      false
    }

    /// What the focused field is for: its purpose and hints.
    #[zbus(property)]
    fn content_type(&self) -> (u32, u32) {
      self.content_type
    }

    /// The client says what the focused field is for.
    #[zbus(property)]
    fn set_content_type(&mut self, content_type: (u32, u32)) {
      self.content_type = content_type;
      tell(&self.told, |told| {
        told.purpose = Purpose::of_ibus(content_type.0);
      });
    }
  }
}

/// The Wayland stand-in's connection state.
struct Seat {
  told: Shared,
  pending: Pending,
  done: Arc<Mutex<u32>>,
}

/// What the compositor said since its last `done`.
#[derive(Default)]
struct Pending {
  active: Option<bool>,
  surrounding: Option<(String, u32, u32)>,
  purpose: Option<zwp_input_method_v2::Event>,
}

impl
  wayland_client::Dispatch<
    wayland_client::protocol::wl_registry::WlRegistry,
    wayland_client::globals::GlobalListContents,
  > for Seat
{
  fn event(
    _: &mut Self,
    _: &wayland_client::protocol::wl_registry::WlRegistry,
    _: wayland_client::protocol::wl_registry::Event,
    _: &wayland_client::globals::GlobalListContents,
    _: &wayland_client::Connection,
    _: &wayland_client::QueueHandle<Self>,
  ) {
  }
}

wayland_client::delegate_noop!(Seat: ignore wayland_client::protocol::wl_seat::WlSeat);
wayland_client::delegate_noop!(Seat: ignore ZwpInputMethodManagerV2);

impl wayland_client::Dispatch<ZwpInputMethodV2, ()> for Seat {
  fn event(
    state: &mut Self,
    _: &ZwpInputMethodV2,
    event: zwp_input_method_v2::Event,
    _: &(),
    _: &wayland_client::Connection,
    _: &wayland_client::QueueHandle<Self>,
  ) {
    match event {
      zwp_input_method_v2::Event::Activate => {
        state.pending = Pending {
          active: Some(true),
          ..Pending::default()
        };
      }
      zwp_input_method_v2::Event::Deactivate => state.pending.active = Some(false),
      zwp_input_method_v2::Event::SurroundingText {
        text,
        cursor,
        anchor,
      } => {
        let chars = |byte: u32| crate::ime::byte_to_char(&text, byte as usize) as u32;
        let (cursor, anchor) = (chars(cursor), chars(anchor));

        state.pending.surrounding = Some((text, cursor, anchor));
      }
      purpose @ zwp_input_method_v2::Event::ContentType { .. } => {
        state.pending.purpose = Some(purpose);
      }
      zwp_input_method_v2::Event::Done => {
        let pending = std::mem::take(&mut state.pending);

        *state.done.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        tell(&state.told, |told| {
          if let Some(active) = pending.active {
            told.focused = active;
          }

          if let Some((text, cursor, anchor)) = pending.surrounding {
            told.surround(text, cursor, anchor);
          }

          if let Some(zwp_input_method_v2::Event::ContentType {
            purpose: wayland_client::WEnum::Value(purpose),
            ..
          }) = pending.purpose
          {
            told.purpose = Purpose::of_wayland(purpose);
          }
        });
      }
      _ => {}
    }
  }
}

/// The Wayland stand-in: its connection, its input method, the thread reading its events.
struct Wayland {
  conn: wayland_client::Connection,
  method: ZwpInputMethodV2,
  done: Arc<Mutex<u32>>,
  stop: Arc<AtomicBool>,
  reader: Option<JoinHandle<()>>,
}

/// The IBus stand-in: its connection, the address file it wrote, the client its context
/// signals go to.
struct IBus {
  connection: Connection,
  file: PathBuf,
  client: Arc<Mutex<Option<String>>>,
}

impl IBus {
  /// Sends the context's signal `member` with `body` to its client.
  fn signal<B>(&self, member: &str, body: &B) -> zbus::Result<()>
  where
    B: serde::Serialize + zbus::zvariant::DynamicType,
  {
    let client = self
      .client
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .clone();

    self.connection.emit_signal(
      client.as_deref(),
      IBUS_CONTEXT,
      "org.freedesktop.IBus.InputContext",
      member,
      body,
    )
  }
}

/// The Fcitx5 stand-in: its connection, and the path of the context it handed out last.
struct Fcitx {
  connection: Connection,
  latest: Arc<Mutex<String>>,
}

impl Fcitx {
  /// Sends the latest context's signal `member` with `body`.
  fn signal<B>(&self, member: &str, body: &B) -> zbus::Result<()>
  where
    B: serde::Serialize + zbus::zvariant::DynamicType,
  {
    let path = self
      .latest
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .clone();

    self.connection.emit_signal(
      None::<&str>,
      path.as_str(),
      "org.fcitx.Fcitx.InputContext1",
      member,
      body,
    )
  }
}

/// Which input method the stand-in is.
enum Kind {
  Fcitx(Fcitx),
  IBus(IBus),
  Wayland(Wayland),
}

/// A stand-in, served while it lives.
pub struct InputMethod {
  kind: Kind,
  told: Shared,
}

impl InputMethod {
  /// Serves the Fcitx5 stand-in as `org.fcitx.Fcitx5` on the session bus. A window opened
  /// after finds it and talks to it (on X11; on Wayland the window's input method is the
  /// compositor's, over text-input-v3).
  ///
  /// # Errors
  ///
  /// Where there is no session bus, or another input method holds the name.
  pub fn serve() -> zbus::Result<Self> {
    let told = Shared::default();
    let latest = Arc::default();
    let connection = zbus::blocking::connection::Builder::session()?
      .name("org.fcitx.Fcitx5")?
      .serve_at(
        "/org/freedesktop/portal/inputmethod",
        Method {
          told: Arc::clone(&told),
          latest: Arc::clone(&latest),
        },
      )?
      .build()?;

    Ok(Self {
      kind: Kind::Fcitx(Fcitx { connection, latest }),
      told,
    })
  }

  /// Serves the IBus stand-in as `org.freedesktop.IBus` on the session bus, its address in the
  /// file ibus-daemon writes for the display, under `XDG_CONFIG_HOME` (the test's private
  /// one). A window opened after finds it where no Fcitx5 runs and talks to it, on X11.
  ///
  /// # Errors
  ///
  /// Where there is no session bus, another input method holds the name, or the address file
  /// cannot be written.
  pub fn serve_ibus() -> Result<Self, Box<dyn std::error::Error>> {
    let told = Shared::default();
    let client = Arc::default();
    let connection = zbus::blocking::connection::Builder::session()?
      .name("org.freedesktop.IBus")?
      .serve_at(
        "/org/freedesktop/IBus",
        IBusMethod {
          told: Arc::clone(&told),
          client: Arc::clone(&client),
        },
      )?
      .serve_at(
        IBUS_CONTEXT,
        IBusContext {
          told: Arc::clone(&told),
          content_type: (0, 0),
        },
      )?
      .build()?;
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS")?;
    let display = std::env::var("DISPLAY")?;
    let number = display
      .split_once(':')
      .and_then(|(_, rest)| rest.split('.').next())
      .ok_or("DISPLAY names no display")?;
    let machine = std::fs::read_to_string("/var/lib/dbus/machine-id")
      .or_else(|_| std::fs::read_to_string("/etc/machine-id"))?;
    let directory = PathBuf::from(std::env::var("XDG_CONFIG_HOME")?).join("ibus/bus");
    let file = directory.join(format!("{}-unix-{number}", machine.trim()));

    std::fs::create_dir_all(&directory)?;
    std::fs::write(
      &file,
      format!(
        "IBUS_ADDRESS={address}\nIBUS_DAEMON_PID={}\n",
        std::process::id()
      ),
    )?;

    Ok(Self {
      kind: Kind::IBus(IBus {
        connection,
        file,
        client,
      }),
      told,
    })
  }

  /// Serves the Wayland stand-in: an input method of `WAYLAND_DISPLAY`'s seat, which the
  /// compositor activates while a window's text-input-v3 is enabled and tells what the window
  /// says.
  ///
  /// # Errors
  ///
  /// Where the compositor offers no input method manager (only sway does among the legs), or
  /// the seat has one already.
  pub fn serve_wayland() -> Result<Self, Box<dyn std::error::Error>> {
    let told = Shared::default();
    let done = Arc::new(Mutex::new(0));
    let conn = wayland_client::Connection::connect_to_env()?;
    let (globals, mut queue) = wayland_client::globals::registry_queue_init::<Seat>(&conn)?;
    let qh = queue.handle();
    let seat: wayland_client::protocol::wl_seat::WlSeat = globals.bind(&qh, 1..=1, ())?;
    let manager: ZwpInputMethodManagerV2 = globals.bind(&qh, 1..=1, ())?;
    let method = manager.get_input_method(&seat, &qh, ());
    let mut state = Seat {
      told: Arc::clone(&told),
      pending: Pending::default(),
      done: Arc::clone(&done),
    };

    queue.roundtrip(&mut state)?;

    let stop = Arc::new(AtomicBool::new(false));
    let reader = std::thread::Builder::new()
      .name("input-method".into())
      .spawn({
        let stop = Arc::clone(&stop);

        move || {
          while !stop.load(Ordering::Relaxed) {
            if queue.dispatch_pending(&mut state).is_err() || queue.flush().is_err() {
              return;
            }

            let Some(guard) = queue.prepare_read() else {
              continue;
            };
            let mut poll = [libc::pollfd {
              fd: guard.connection_fd().as_raw_fd(),
              events: libc::POLLIN,
              revents: 0,
            }];

            // SAFETY: one pollfd of a descriptor the guard keeps open, for 50 ms.
            if unsafe { libc::poll(poll.as_mut_ptr(), 1, 50) } > 0 && guard.read().is_err() {
              return;
            }
          }
        }
      })?;

    Ok(Self {
      kind: Kind::Wayland(Wayland {
        conn,
        method,
        done,
        stop,
        reader: Some(reader),
      }),
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

  /// Commits `text` to the focused field, as an input method does when a composition ends.
  ///
  /// # Errors
  ///
  /// Where the window's connection no longer takes it.
  pub fn trigger_commit(&self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    match &self.kind {
      Kind::Fcitx(fcitx) => fcitx.signal("CommitString", &(text,))?,
      Kind::IBus(ibus) => ibus.signal("CommitText", &crate::ime::ibus::to_ibus_text(text))?,
      Kind::Wayland(wayland) => {
        wayland.method.commit_string(text.to_owned());
        wayland.commit()?;
      }
    }

    Ok(())
  }

  /// Deletes the `characters` characters before the caret of the focused field, counted in the
  /// text it last told, as an input method replacing what it typed does.
  ///
  /// # Errors
  ///
  /// Where the window's connection no longer takes it.
  pub fn trigger_delete_before(&self, characters: u32) -> Result<(), Box<dyn std::error::Error>> {
    match &self.kind {
      Kind::Fcitx(fcitx) => fcitx.signal(
        "DeleteSurroundingText",
        &(-i32::try_from(characters)?, characters),
      )?,
      Kind::IBus(ibus) => ibus.signal(
        "DeleteSurroundingText",
        &(-i32::try_from(characters)?, characters),
      )?,
      Kind::Wayland(wayland) => {
        let (text, cursor, _) = self.told().surrounding.unwrap_or_default();
        let end = crate::ime::char_to_byte(&text, cursor as usize);
        let start = crate::ime::char_to_byte(&text, cursor.saturating_sub(characters) as usize);

        wayland
          .method
          .delete_surrounding_text(u32::try_from(end - start)?, 0);
        wayland.commit()?;
      }
    }

    Ok(())
  }
}

impl Wayland {
  /// Applies what was sent since the last commit, at the serial of the `done`s seen.
  fn commit(&self) -> Result<(), Box<dyn std::error::Error>> {
    let serial = *self.done.lock().unwrap_or_else(PoisonError::into_inner);

    self.method.commit(serial);
    self.conn.flush()?;

    Ok(())
  }
}

impl Drop for InputMethod {
  fn drop(&mut self) {
    match &mut self.kind {
      Kind::Fcitx(_) => {}
      Kind::IBus(ibus) => {
        let _ = std::fs::remove_file(&ibus.file);
      }
      Kind::Wayland(wayland) => {
        wayland.method.destroy();
        let _ = wayland.conn.flush();
        wayland.stop.store(true, Ordering::Relaxed);

        if let Some(reader) = wayland.reader.take() {
          let _ = reader.join();
        }
      }
    }
  }
}
