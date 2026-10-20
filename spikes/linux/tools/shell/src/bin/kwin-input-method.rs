//! `kwin-input-method`: the input method KWin starts for a test (`kwin_wayland --inputmethod`,
//! which skene-runner passes on the KWin legs), a stand-in reading what KWin relays of a
//! window's text-input-v3 from outside the app.
//!
//! It speaks input-method-unstable-v1 on the connection KWin hands it (`WAYLAND_SOCKET`): it
//! writes every activation, deactivation, surrounding text and content type to the file
//! `$XDG_RUNTIME_DIR/skene-input-method`, one a line, and while active it shows an overlay
//! input panel, a block of [`COLOR`] [`SIZE`] pixels square, which KWin places at the focused
//! field's text-input cursor rectangle: where the block shows on the screen is where the window
//! told the compositor its caret is.
//!
//! ```text
//! activate
//! surrounding <cursor> <anchor> <text>
//! content <hint> <purpose>
//! deactivate
//! ```

use std::{fs::File, io::Write as _, os::fd::AsFd as _, path::PathBuf, process::ExitCode};

use wayland_client::{
  Connection, Dispatch, QueueHandle, delegate_noop,
  globals::{GlobalListContents, registry_queue_init},
  protocol::{wl_buffer, wl_compositor, wl_registry, wl_shm, wl_shm_pool, wl_surface},
};
use wayland_protocols::wp::input_method::zv1::client::{
  zwp_input_method_context_v1::{self, ZwpInputMethodContextV1},
  zwp_input_method_v1::{self, ZwpInputMethodV1},
  zwp_input_panel_surface_v1::ZwpInputPanelSurfaceV1,
  zwp_input_panel_v1::ZwpInputPanelV1,
};

/// The panel's color, as ARGB: a magenta no window of the tests draws.
const COLOR: u32 = 0xffff_00ff;

/// The panel's side, in surface pixels.
const SIZE: i32 = 12;

/// The stand-in's state.
struct State {
  /// Where what KWin relays is written.
  log: File,
  /// The compositor global.
  compositor: wl_compositor::WlCompositor,
  /// The input panel global.
  panel: ZwpInputPanelV1,
  /// The panel's buffer.
  buffer: wl_buffer::WlBuffer,
  /// The panel shown while active: its surface and its role.
  shown: Option<(wl_surface::WlSurface, ZwpInputPanelSurfaceV1)>,
}

impl State {
  /// Writes `line` to the log.
  fn note(&mut self, line: &str) {
    let _ = writeln!(self.log, "{line}");
    let _ = self.log.flush();
  }

  /// Shows the overlay panel, KWin placing it at the cursor rectangle.
  fn show(&mut self, qh: &QueueHandle<Self>) {
    if self.shown.is_some() {
      return;
    }

    let surface = self.compositor.create_surface(qh, ());
    let role = self.panel.get_input_panel_surface(&surface, qh, ());

    role.set_overlay_panel();
    surface.attach(Some(&self.buffer), 0, 0);
    surface.damage_buffer(0, 0, SIZE, SIZE);
    surface.commit();
    self.shown = Some((surface, role));
  }

  /// Takes the panel down.
  fn hide(&mut self) {
    if let Some((surface, _role)) = self.shown.take() {
      surface.destroy();
    }
  }
}

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("kwin-input-method: {error}");

      ExitCode::FAILURE
    }
  }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
  let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is not set")?;
  let log = File::options()
    .create(true)
    .append(true)
    .open(PathBuf::from(runtime).join("skene-input-method"))?;
  let conn = Connection::connect_to_env()?;
  let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
  let qh = queue.handle();
  let compositor: wl_compositor::WlCompositor = globals.bind(&qh, 1..=4, ())?;
  let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ())?;
  let panel: ZwpInputPanelV1 = globals.bind(&qh, 1..=1, ())?;
  let _method: ZwpInputMethodV1 = globals.bind(&qh, 1..=1, ())?;
  let buffer = solid(&shm, &qh)?;
  let mut state = State {
    log,
    compositor,
    panel,
    buffer,
    shown: None,
  };

  state.note("started");

  loop {
    queue.blocking_dispatch(&mut state)?;
  }
}

/// A buffer of [`SIZE`] pixels square, all [`COLOR`].
fn solid(
  shm: &wl_shm::WlShm,
  qh: &QueueHandle<State>,
) -> Result<wl_buffer::WlBuffer, Box<dyn std::error::Error>> {
  let stride = SIZE * 4;
  let length = stride * SIZE;
  let name = c"skene-input-panel";
  // SAFETY: creates an anonymous file from a constant, nul-terminated name.
  let fd = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC) };

  if fd < 0 {
    return Err(std::io::Error::last_os_error().into());
  }

  // SAFETY: `fd` was just opened and is owned here alone.
  let mut file = unsafe { <File as std::os::fd::FromRawFd>::from_raw_fd(fd) };
  let pixels: Vec<u8> = (0..SIZE * SIZE).flat_map(|_| COLOR.to_le_bytes()).collect();

  file.write_all(&pixels)?;

  let pool = shm.create_pool(file.as_fd(), length, qh, ());
  let buffer = pool.create_buffer(0, SIZE, SIZE, stride, wl_shm::Format::Argb8888, qh, ());

  pool.destroy();

  Ok(buffer)
}

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

impl Dispatch<ZwpInputMethodV1, ()> for State {
  fn event(
    state: &mut Self,
    _: &ZwpInputMethodV1,
    event: zwp_input_method_v1::Event,
    _: &(),
    _: &Connection,
    qh: &QueueHandle<Self>,
  ) {
    match event {
      zwp_input_method_v1::Event::Activate { .. } => {
        state.note("activate");
        state.show(qh);
      }
      zwp_input_method_v1::Event::Deactivate { context } => {
        state.note("deactivate");
        state.hide();
        context.destroy();
      }
      _ => {}
    }
  }

  wayland_client::event_created_child!(State, ZwpInputMethodV1, [
    zwp_input_method_v1::EVT_ACTIVATE_OPCODE => (ZwpInputMethodContextV1, ()),
  ]);
}

impl Dispatch<ZwpInputMethodContextV1, ()> for State {
  fn event(
    state: &mut Self,
    _: &ZwpInputMethodContextV1,
    event: zwp_input_method_context_v1::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match event {
      zwp_input_method_context_v1::Event::SurroundingText {
        text,
        cursor,
        anchor,
      } => state.note(&format!("surrounding {cursor} {anchor} {text}")),
      zwp_input_method_context_v1::Event::ContentType { hint, purpose } => {
        state.note(&format!("content {hint} {purpose}"));
      }
      _ => {}
    }
  }
}

delegate_noop!(State: ignore wl_compositor::WlCompositor);
delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
delegate_noop!(State: ignore wl_buffer::WlBuffer);
delegate_noop!(State: ignore wl_surface::WlSurface);
delegate_noop!(State: ignore ZwpInputPanelV1);
delegate_noop!(State: ignore ZwpInputPanelSurfaceV1);
