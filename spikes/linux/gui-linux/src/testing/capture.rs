//! What the screen shows: the pixels the display server or the compositor presents, read back
//! through the session's own capture, never from the app.
//!
//! - X11: the X screen (`GetImage` on its root window), where the server keeps what each window
//!   presented, the pointer left out;
//! - mutter: its screen cast of the monitor (`org.gnome.Mutter.ScreenCast`), the pointer left
//!   out, read from PipeWire through GStreamer's `pipewiresrc` (loaded when first needed);
//! - sway: wlroots' screencopy of the output;
//! - weston and KWin: the X screen their output's window is shown in (`SKENE_OUTPUT_DISPLAY`),
//!   read as on X11.
//!
//! Sway (its pixman renderer), weston and KWin draw the pointer into their output, which the
//! runner's transparent cursor theme keeps out of it.
//!
//! A rectangle is asked for in the pointer's space ([`super::input::Input`]): the X screen's
//! device pixels on X11, the compositor's logical pixels elsewhere, scaled by the leg's scale.

use std::{
  ffi::{CStr, CString, c_char, c_int, c_uint, c_void},
  os::fd::{AsFd as _, FromRawFd as _},
  sync::{OnceLock, mpsc},
  time::{Duration, Instant},
};

use wayland_client::{
  Connection, Dispatch, Proxy as _, QueueHandle, WEnum,
  globals::GlobalListContents,
  protocol::{wl_buffer, wl_output, wl_registry, wl_shm, wl_shm_pool},
};
use wayland_protocols_wlr::screencopy::v1::client::{
  zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
  zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};
use zbus::{
  blocking::{Connection as Bus, Proxy},
  zvariant::{OwnedObjectPath, Value},
};

use super::{Session, input::Result};

/// How long a screen cast may take to deliver its first frame.
const FIRST_FRAME: Duration = Duration::from_secs(3);

/// How long a screen cast must deliver no newer frame before its newest is what the screen
/// shows.
const CAST_QUIET: Duration = Duration::from_millis(200);

/// How long a screen cast may take to deliver a frame newer than the window's last paint, and
/// to stop delivering, before its newest is taken anyway.
const CAST_LIMIT: Duration = Duration::from_millis(1500);

/// Pixels the screen shows: `width` by `height` device pixels, straight RGBA row by row.
pub struct Pixels {
  /// The width in device pixels.
  pub width: u32,
  /// The height in device pixels.
  pub height: u32,
  /// The pixels, RGBA, row by row.
  pub rgba: Vec<u8>,
}

/// The session's screen, read back as it is presented.
pub struct Screen {
  /// Whether its device pixels per unit of the pointer's space are the output's scale, which
  /// can change while a window shows (a compositor's output), rather than one (an X screen).
  scaled: bool,
  /// How it is read.
  source: Source,
}

/// How a screen is read.
enum Source {
  /// An X screen's root window.
  X(Box<XScreen>),
  /// mutter's screen cast of the monitor.
  Cast(Cast),
  /// wlroots' screencopy of the output.
  Copy(Screencopy),
}

impl Screen {
  /// The screen of `session`, connected to its capture.
  ///
  /// # Errors
  ///
  /// Where the session's capture cannot be reached.
  pub fn open(session: Session) -> Result<Self> {
    let output = std::env::var("SKENE_OUTPUT_DISPLAY").ok();
    let (scaled, source) = match session {
      Session::X11 => (false, Source::X(Box::new(XScreen::connect(None)?))),
      Session::Mutter => (true, Source::Cast(Cast::start()?)),
      Session::Sway => (true, Source::Copy(Screencopy::connect()?)),
      Session::Weston | Session::Kwin | Session::KwinX11 => {
        let display = output.ok_or(
          "a nested compositor's leg names the X display its output shows in (SKENE_OUTPUT_DISPLAY)",
        )?;

        (true, Source::X(Box::new(XScreen::connect(Some(&display))?)))
      }
    };

    Ok(Self { scaled, source })
  }

  /// What the screen shows in `rect` (left, top, width, height in the pointer's space), in
  /// device pixels, once it shows what the window painted at `painted` (a screen cast's frame
  /// arrives after the compositor shows it), the output at `scale` (the window's, which is
  /// its output's now); transparent black where it falls off the screen.
  ///
  /// # Errors
  ///
  /// Where the capture fails.
  pub fn read(
    &mut self,
    rect: (f64, f64, f64, f64),
    painted: Option<Instant>,
    scale: f64,
  ) -> Result<Pixels> {
    let ratio = if self.scaled { scale } else { 1. };
    let device = |value: f64| (value * ratio).round() as i64;
    let (left, top) = (device(rect.0), device(rect.1));
    let (width, height) = (device(rect.2).max(0) as u32, device(rect.3).max(0) as u32);
    let area = Area {
      left,
      top,
      width,
      height,
    };

    match &mut self.source {
      Source::X(screen) => screen.read(area),
      Source::Cast(cast) => cast.read(area, painted),
      Source::Copy(copy) => copy.read(area),
    }
  }
}

/// A rectangle of the screen in device pixels.
#[derive(Clone, Copy, Debug)]
struct Area {
  left: i64,
  top: i64,
  width: u32,
  height: u32,
}

impl Area {
  /// The area's pixels out of a whole screen `width` by `height` whose pixel at `x`, `y` is
  /// `pixel(x, y)` as RGBA; transparent black off the screen.
  fn crop(self, width: u32, height: u32, pixel: impl Fn(usize, usize) -> [u8; 4]) -> Pixels {
    let mut rgba = Vec::with_capacity(self.width as usize * self.height as usize * 4);

    for row in 0..i64::from(self.height) {
      for column in 0..i64::from(self.width) {
        let (x, y) = (self.left + column, self.top + row);
        let on = x >= 0 && y >= 0 && x < i64::from(width) && y < i64::from(height);

        rgba.extend_from_slice(&if on {
          pixel(x as usize, y as usize)
        } else {
          [0; 4]
        });
      }
    }

    Pixels {
      width: self.width,
      height: self.height,
      rgba,
    }
  }
}

/// An X screen, read through a connection of the test's own.
struct XScreen {
  conn: x11rb::rust_connection::RustConnection,
  root: u32,
  size: (u16, u16),
}

impl XScreen {
  /// Connects to `display`, `DISPLAY` where `None`; through the server's abstract socket
  /// where its socket file is gone (an Xwayland that started beside it took the file of a
  /// display it found no lock file for).
  fn connect(display: Option<&str>) -> Result<Self> {
    use std::os::linux::net::SocketAddrExt as _;

    use x11rb::connection::Connection as _;

    let (conn, screen) = match x11rb::connect(display) {
      Ok(connected) => connected,
      Err(error) => {
        let number = display
          .and_then(|display| display.strip_prefix(':'))
          .and_then(|number| number.split('.').next()?.parse::<u32>().ok())
          .ok_or(error)?;
        let address =
          std::os::unix::net::SocketAddr::from_abstract_name(format!("/tmp/.X11-unix/X{number}"))?;
        let stream = std::os::unix::net::UnixStream::connect_addr(&address)?;
        let (stream, _) = x11rb::rust_connection::DefaultStream::from_unix_stream(stream)?;

        (
          x11rb::rust_connection::RustConnection::connect_to_stream(stream, 0)?,
          0,
        )
      }
    };
    let setup = &conn.setup().roots[screen];
    let (root, size) = (setup.root, (setup.width_in_pixels, setup.height_in_pixels));

    if setup.root_depth != 24 && setup.root_depth != 32 {
      return Err(
        format!(
          "the X screen is {} bits deep, not 24 or 32",
          setup.root_depth
        )
        .into(),
      );
    }

    Ok(Self { conn, root, size })
  }

  /// The screen's pixels in `area`: its part on the screen read in one `GetImage`.
  fn read(&mut self, area: Area) -> Result<Pixels> {
    use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat};

    let (screen_width, screen_height) = (i64::from(self.size.0), i64::from(self.size.1));
    let left = area.left.clamp(0, screen_width);
    let top = area.top.clamp(0, screen_height);
    let right = (area.left + i64::from(area.width)).clamp(0, screen_width);
    let bottom = (area.top + i64::from(area.height)).clamp(0, screen_height);
    let (width, height) = ((right - left) as u16, (bottom - top) as u16);

    if width == 0 || height == 0 {
      return Ok(area.crop(0, 0, |_, _| [0; 4]));
    }

    let image = self
      .conn
      .get_image(
        ImageFormat::Z_PIXMAP,
        self.root,
        left as i16,
        top as i16,
        width,
        height,
        !0,
      )?
      .reply()?;
    let stride = usize::from(width) * 4;

    if image.data.len() < stride * usize::from(height) {
      return Err(
        format!(
          "the X screen's image of {width}x{height} came back in {} bytes, not 32 bits a pixel",
          image.data.len()
        )
        .into(),
      );
    }

    // The server's pixels are 32 bits little-endian: blue, green, red, then padding.
    let shifted = Area {
      left: area.left - left,
      top: area.top - top,
      ..area
    };

    Ok(shifted.crop(u32::from(width), u32::from(height), |x, y| {
      let at = y * stride + x * 4;

      [image.data[at + 2], image.data[at + 1], image.data[at], 255]
    }))
  }
}

/// mutter's screen cast of its monitor, its stream read through GStreamer as it goes, by a
/// thread of its own that stamps each frame with when it arrived.
struct Cast {
  /// The session bus the cast's session lives on.
  _bus: Bus,
  /// The newest frame and when it arrived, shared with the reading thread.
  newest: std::sync::Arc<(std::sync::Mutex<Option<Arrived>>, std::sync::Condvar)>,
  /// Whether the reading thread is to stop.
  stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
  /// The reading thread, which owns the pipeline.
  reader: Option<std::thread::JoinHandle<()>>,
}

/// A frame of the cast as it arrived: when, its width and height, and its pixels (blue, green,
/// red, padding).
struct Arrived {
  at: Instant,
  width: u32,
  height: u32,
  pixels: Vec<u8>,
}

impl Drop for Cast {
  fn drop(&mut self) {
    self.stop.store(true, std::sync::atomic::Ordering::SeqCst);

    if let Some(reader) = self.reader.take() {
      let _ = reader.join();
    }
  }
}

const SCREEN_CAST: &str = "org.gnome.Mutter.ScreenCast";

impl Cast {
  /// Casts the monitor, the pointer left out, and starts reading the stream.
  fn start() -> Result<Self> {
    let bus = Bus::session()?;
    let session: OwnedObjectPath = bus
      .call_method(
        Some(SCREEN_CAST),
        "/org/gnome/Mutter/ScreenCast",
        Some(SCREEN_CAST),
        "CreateSession",
        &(std::collections::HashMap::<&str, Value<'_>>::new(),),
      )?
      .body()
      .deserialize()?;
    let mut options = std::collections::HashMap::<&str, Value<'_>>::new();

    // 0: the pointer is not drawn into the stream.
    options.insert("cursor-mode", Value::from(0u32));

    let stream: OwnedObjectPath = bus
      .call_method(
        Some(SCREEN_CAST),
        session.as_str(),
        Some(&*format!("{SCREEN_CAST}.Session")),
        "RecordMonitor",
        &("", options),
      )?
      .body()
      .deserialize()?;
    let proxy = Proxy::new(
      &bus,
      SCREEN_CAST,
      stream.as_str(),
      format!("{SCREEN_CAST}.Stream"),
    )?;
    let mut added = proxy.receive_signal("PipeWireStreamAdded")?;
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
      let node = added
        .next()
        .and_then(|message| message.body().deserialize::<u32>().ok());

      let _ = sender.send(node);
    });
    bus.call_method(
      Some(SCREEN_CAST),
      session.as_str(),
      Some(&*format!("{SCREEN_CAST}.Session")),
      "Start",
      &(),
    )?;

    let node = receiver
      .recv_timeout(FIRST_FRAME)
      .ok()
      .flatten()
      .ok_or("mutter's screen cast never said which PipeWire node its stream is")?;
    let pipeline = Pipeline::launch(&format!(
      "pipewiresrc path={node} always-copy=true ! video/x-raw,format=(string){{BGRx,BGRA}} ! appsink name=sink max-buffers=1 drop=true sync=false"
    ))?;
    let newest = std::sync::Arc::new((std::sync::Mutex::new(None), std::sync::Condvar::new()));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader = {
      let (newest, stop) = (std::sync::Arc::clone(&newest), std::sync::Arc::clone(&stop));

      std::thread::Builder::new()
        .name("skene-cast".to_owned())
        .spawn(move || {
          while !stop.load(std::sync::atomic::Ordering::SeqCst) {
            if let Ok(Some((width, height, pixels))) = pipeline.pull(Duration::from_millis(50)) {
              let (slot, arrived) = &*newest;

              *slot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arrived {
                at: Instant::now(),
                width,
                height,
                pixels,
              });
              arrived.notify_all();
            }
          }
        })?
    };

    Ok(Self {
      _bus: bus,
      newest,
      stop,
      reader: Some(reader),
    })
  }

  /// The screen's pixels in `area`, as the stream's newest frame shows them once one arrived
  /// after the window painted at `painted` and no newer one has come for [`CAST_QUIET`]
  /// (within [`CAST_LIMIT`], after which the newest is taken as it is).
  fn read(&mut self, area: Area, painted: Option<Instant>) -> Result<Pixels> {
    let (slot, arrived) = &*self.newest;
    let started = Instant::now();
    let mut newest = slot
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);

    loop {
      let fresh = newest.as_ref().is_some_and(|frame| {
        painted.is_none_or(|painted| frame.at > painted) && frame.at.elapsed() >= CAST_QUIET
      });
      let limit = if newest.is_none() {
        FIRST_FRAME
      } else {
        CAST_LIMIT
      };

      if fresh || started.elapsed() >= limit {
        break;
      }

      newest = arrived
        .wait_timeout(newest, Duration::from_millis(10))
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .0;
    }

    let frame = newest.as_ref().ok_or(
      "mutter's screen cast delivered no frame (does the session run WirePlumber, which links its stream?)",
    )?;
    let stride = frame.width as usize * 4;

    Ok(area.crop(frame.width, frame.height, |x, y| {
      let at = y * stride + x * 4;

      [
        frame.pixels[at + 2],
        frame.pixels[at + 1],
        frame.pixels[at],
        255,
      ]
    }))
  }
}

/// `gst_init_check`.
type InitCheck = unsafe extern "C" fn(*mut c_int, *mut c_void, *mut *mut GError) -> c_int;
/// `gst_parse_launch`.
type ParseLaunch = unsafe extern "C" fn(*const c_char, *mut *mut GError) -> *mut c_void;
/// `gst_bin_get_by_name`.
type BinGetByName = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;
/// `gst_element_set_state`.
type ElementSetState = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
/// `gst_object_unref`, `gst_mini_object_unref`.
type Unref = unsafe extern "C" fn(*mut c_void);
/// `gst_app_sink_try_pull_sample`.
type TryPullSample = unsafe extern "C" fn(*mut c_void, u64) -> *mut c_void;
/// `gst_sample_get_buffer`, `gst_sample_get_caps`.
type SampleGet = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
/// `gst_caps_get_structure`.
type CapsGetStructure = unsafe extern "C" fn(*const c_void, c_uint) -> *mut c_void;
/// `gst_structure_get_int`.
type StructureGetInt = unsafe extern "C" fn(*const c_void, *const c_char, *mut c_int) -> c_int;
/// `gst_buffer_map`.
type BufferMap = unsafe extern "C" fn(*mut c_void, *mut MapInfo, c_int) -> c_int;
/// `gst_buffer_unmap`.
type BufferUnmap = unsafe extern "C" fn(*mut c_void, *mut MapInfo);

/// The GStreamer calls the cast makes, loaded from the system's GStreamer when first needed.
struct Gst {
  parse_launch: ParseLaunch,
  bin_get_by_name: BinGetByName,
  element_set_state: ElementSetState,
  object_unref: Unref,
  try_pull_sample: TryPullSample,
  sample_get_buffer: SampleGet,
  sample_get_caps: SampleGet,
  caps_get_structure: CapsGetStructure,
  structure_get_int: StructureGetInt,
  buffer_map: BufferMap,
  buffer_unmap: BufferUnmap,
  mini_object_unref: Unref,
}

/// GLib's `GError`.
#[repr(C)]
struct GError {
  domain: u32,
  code: c_int,
  message: *const c_char,
}

/// GStreamer's `GstMapInfo`.
#[repr(C)]
struct MapInfo {
  memory: *mut c_void,
  flags: c_int,
  data: *const u8,
  size: usize,
  maxsize: usize,
  user_data: [*mut c_void; 4],
  reserved: [*mut c_void; 4],
}

/// `GST_STATE_NULL`.
const STATE_NULL: c_int = 1;
/// `GST_STATE_PLAYING`.
const STATE_PLAYING: c_int = 4;
/// `GST_STATE_CHANGE_FAILURE`.
const CHANGE_FAILURE: c_int = 0;
/// `GST_MAP_READ`.
const MAP_READ: c_int = 1;

impl Gst {
  /// GStreamer, loaded and initialized once.
  fn get() -> Result<&'static Self> {
    static GST: OnceLock<std::result::Result<Gst, String>> = OnceLock::new();

    GST
      .get_or_init(Self::load)
      .as_ref()
      .map_err(|error| error.clone().into())
  }

  fn load() -> std::result::Result<Self, String> {
    let open = |name: &CStr| {
      // SAFETY: loading a system library by name; its initializers are GStreamer's own.
      let handle = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) };

      if handle.is_null() {
        Err(format!(
          "{} cannot be loaded (install GStreamer and gstreamer1.0-pipewire)",
          name.to_string_lossy()
        ))
      } else {
        Ok(handle)
      }
    };
    let core = open(c"libgstreamer-1.0.so.0")?;
    let app = open(c"libgstapp-1.0.so.0")?;
    let symbol = |handle: *mut c_void, name: &CStr| {
      // SAFETY: `handle` is a library loaded above, `name` a nul-terminated symbol name.
      let address = unsafe { libc::dlsym(handle, name.as_ptr()) };

      if address.is_null() {
        Err(format!("GStreamer has no {}", name.to_string_lossy()))
      } else {
        Ok(address)
      }
    };

    macro_rules! function {
      ($handle:expr, $name:literal as $type:ty) => {
        // SAFETY: the symbol is the GStreamer function of that name, whose C signature the
        // type spells.
        unsafe { std::mem::transmute::<*mut c_void, $type>(symbol($handle, $name)?) }
      };
    }

    let init_check = function!(core, c"gst_init_check" as InitCheck);

    // SAFETY: GStreamer takes no arguments here; an error is left unread.
    if unsafe {
      init_check(
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
      )
    } == 0
    {
      return Err("GStreamer does not initialize".to_owned());
    }

    Ok(Self {
      parse_launch: function!(core, c"gst_parse_launch" as ParseLaunch),
      bin_get_by_name: function!(core, c"gst_bin_get_by_name" as BinGetByName),
      element_set_state: function!(core, c"gst_element_set_state" as ElementSetState),
      object_unref: function!(core, c"gst_object_unref" as Unref),
      try_pull_sample: function!(app, c"gst_app_sink_try_pull_sample" as TryPullSample),
      sample_get_buffer: function!(core, c"gst_sample_get_buffer" as SampleGet),
      sample_get_caps: function!(core, c"gst_sample_get_caps" as SampleGet),
      caps_get_structure: function!(core, c"gst_caps_get_structure" as CapsGetStructure),
      structure_get_int: function!(core, c"gst_structure_get_int" as StructureGetInt),
      buffer_map: function!(core, c"gst_buffer_map" as BufferMap),
      buffer_unmap: function!(core, c"gst_buffer_unmap" as BufferUnmap),
      mini_object_unref: function!(core, c"gst_mini_object_unref" as Unref),
    })
  }
}

/// A GStreamer pipeline ending in an app sink named `sink`, playing while it lives.
///
/// GStreamer's objects are thread-safe: the pipeline is pulled from and dropped on the thread
/// that reads the cast.
struct Pipeline {
  gst: &'static Gst,
  pipeline: *mut c_void,
  sink: *mut c_void,
}

impl Pipeline {
  /// Builds `description` (gst-launch's syntax) and plays it.
  fn launch(description: &str) -> Result<Self> {
    let gst = Gst::get()?;
    let description = CString::new(description)?;
    let mut error: *mut GError = std::ptr::null_mut();
    // SAFETY: a nul-terminated description and a slot for the error.
    let pipeline = unsafe { (gst.parse_launch)(description.as_ptr(), &raw mut error) };

    if pipeline.is_null() {
      let message = if error.is_null() {
        "an unknown error".to_owned()
      } else {
        // SAFETY: GStreamer filled `error` in, its message a nul-terminated string.
        unsafe { CStr::from_ptr((*error).message) }
          .to_string_lossy()
          .into_owned()
      };

      return Err(format!("GStreamer cannot build the cast's pipeline: {message}").into());
    }

    // SAFETY: the pipeline is a bin; the name is nul-terminated.
    let sink = unsafe { (gst.bin_get_by_name)(pipeline, c"sink".as_ptr()) };
    let pipeline = Self {
      gst,
      pipeline,
      sink,
    };

    if sink.is_null() {
      return Err("the cast's pipeline has no sink".into());
    }

    // SAFETY: the pipeline is an element.
    if unsafe { (gst.element_set_state)(pipeline.pipeline, STATE_PLAYING) } == CHANGE_FAILURE {
      return Err("the cast's pipeline does not play".into());
    }

    Ok(pipeline)
  }

  /// The next frame the sink holds, waiting `wait` at most: its width, height and pixels.
  fn pull(&self, wait: Duration) -> Result<Option<(u32, u32, Vec<u8>)>> {
    let gst = self.gst;
    let nanoseconds = u64::try_from(wait.as_nanos()).unwrap_or(u64::MAX);
    // SAFETY: the sink is an app sink of the playing pipeline.
    let sample = unsafe { (gst.try_pull_sample)(self.sink, nanoseconds) };

    if sample.is_null() {
      return Ok(None);
    }

    // SAFETY: a sample the sink handed over, owned here until unreferenced below; its caps and
    // buffer are borrowed from it, and the buffer is mapped and unmapped within.
    let frame = unsafe {
      let caps = (gst.sample_get_caps)(sample);
      let structure = (gst.caps_get_structure)(caps, 0);
      let (mut width, mut height) = (0, 0);

      (gst.structure_get_int)(structure, c"width".as_ptr(), &raw mut width);
      (gst.structure_get_int)(structure, c"height".as_ptr(), &raw mut height);

      let buffer = (gst.sample_get_buffer)(sample);
      let mut map: MapInfo = std::mem::zeroed();
      let frame = if (gst.buffer_map)(buffer, &raw mut map, MAP_READ) == 0 {
        None
      } else {
        let bytes = std::slice::from_raw_parts(map.data, map.size).to_vec();

        (gst.buffer_unmap)(buffer, &raw mut map);

        Some((width as u32, height as u32, bytes))
      };

      (gst.mini_object_unref)(sample);

      frame
    };
    let Some((width, height, bytes)) = frame else {
      return Err("a frame of the cast cannot be read".into());
    };

    if bytes.len() < width as usize * height as usize * 4 {
      return Err(
        format!(
          "a frame of {width}x{height} came in {} bytes, not 32 bits a pixel",
          bytes.len()
        )
        .into(),
      );
    }

    Ok(Some((width, height, bytes)))
  }
}

// SAFETY: GStreamer's elements are reference-counted thread-safe objects, used through its
// thread-safe calls alone.
unsafe impl Send for Pipeline {}

impl Drop for Pipeline {
  fn drop(&mut self) {
    // SAFETY: the pipeline and the sink were taken references of; each is released once.
    unsafe {
      (self.gst.element_set_state)(self.pipeline, STATE_NULL);
      (self.gst.object_unref)(self.sink);
      (self.gst.object_unref)(self.pipeline);
    }
  }
}

/// wlroots' screencopy of the output, on a connection of the test's own.
struct Screencopy {
  _conn: Connection,
  queue: wayland_client::EventQueue<Copying>,
  manager: ZwlrScreencopyManagerV1,
  shm: wl_shm::WlShm,
  output: wl_output::WlOutput,
}

/// What a screencopy frame said of itself.
#[derive(Default)]
struct Copying {
  /// The shared-memory buffer it asks for: format, width, height, stride.
  buffer: Option<(WEnum<wl_shm::Format>, u32, u32, u32)>,
  /// Whether it said every buffer type it takes.
  described: bool,
  /// Whether its rows come bottom first.
  inverted: bool,
  /// Whether it was copied, or failed.
  done: Option<bool>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Copying {
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

wayland_client::delegate_noop!(Copying: ignore wl_shm::WlShm);
wayland_client::delegate_noop!(Copying: ignore wl_output::WlOutput);
wayland_client::delegate_noop!(Copying: ignore wl_shm_pool::WlShmPool);
wayland_client::delegate_noop!(Copying: ignore wl_buffer::WlBuffer);
wayland_client::delegate_noop!(Copying: ignore ZwlrScreencopyManagerV1);

impl Dispatch<ZwlrScreencopyFrameV1, ()> for Copying {
  fn event(
    state: &mut Self,
    _: &ZwlrScreencopyFrameV1,
    event: zwlr_screencopy_frame_v1::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match event {
      zwlr_screencopy_frame_v1::Event::Buffer {
        format,
        width,
        height,
        stride,
      } => state.buffer = Some((format, width, height, stride)),
      zwlr_screencopy_frame_v1::Event::BufferDone => state.described = true,
      zwlr_screencopy_frame_v1::Event::Flags { flags } => {
        state.inverted = flags
          .into_result()
          .is_ok_and(|flags| flags.contains(zwlr_screencopy_frame_v1::Flags::YInvert));
      }
      zwlr_screencopy_frame_v1::Event::Ready { .. } => state.done = Some(true),
      zwlr_screencopy_frame_v1::Event::Failed => state.done = Some(false),
      _ => {}
    }
  }
}

impl Screencopy {
  /// Binds the compositor's screencopy, its shared memory and its output.
  fn connect() -> Result<Self> {
    let conn = Connection::connect_to_env()?;
    let (globals, queue) = wayland_client::globals::registry_queue_init::<Copying>(&conn)?;
    let qh = queue.handle();
    let manager: ZwlrScreencopyManagerV1 = globals.bind(&qh, 1..=3, ())?;
    let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ())?;
    let output: wl_output::WlOutput = globals.bind(&qh, 1..=4, ())?;

    Ok(Self {
      _conn: conn,
      queue,
      manager,
      shm,
      output,
    })
  }

  /// The output's pixels in `area`, copied whole now, the pointer left out.
  fn read(&mut self, area: Area) -> Result<Pixels> {
    let qh = self.queue.handle();
    let frame = self.manager.capture_output(0, &self.output, &qh, ());
    let mut state = Copying::default();
    let started = Instant::now();
    let version = self.manager.version();

    while state.buffer.is_none() || (version >= 3 && !state.described) {
      if state.done == Some(false) || started.elapsed() > FIRST_FRAME {
        frame.destroy();

        return Err("the compositor's screencopy offered no buffer".into());
      }

      self.queue.blocking_dispatch(&mut state)?;
    }

    let Some((format, width, height, stride)) = state.buffer else {
      return Err("the compositor's screencopy offered no buffer".into());
    };
    let size = stride as usize * height as usize;
    // SAFETY: an anonymous file for the copy, owned by `file` from here on.
    let file = unsafe {
      std::fs::File::from_raw_fd(libc::memfd_create(
        c"screencopy".as_ptr(),
        libc::MFD_CLOEXEC,
      ))
    };

    file.set_len(size as u64)?;

    let format = format
      .into_result()
      .map_err(|_| "the screencopy's buffer format is unknown")?;
    let pool = self
      .shm
      .create_pool(file.as_fd(), i32::try_from(size)?, &qh, ());
    let buffer = pool.create_buffer(
      0,
      i32::try_from(width)?,
      i32::try_from(height)?,
      i32::try_from(stride)?,
      format,
      &qh,
      (),
    );

    frame.copy(&buffer);

    while state.done.is_none() {
      if started.elapsed() > FIRST_FRAME {
        break;
      }

      self.queue.blocking_dispatch(&mut state)?;
    }

    frame.destroy();
    buffer.destroy();
    pool.destroy();

    if state.done != Some(true) {
      return Err("the compositor's screencopy failed".into());
    }

    let mut bytes = vec![0; size];

    std::os::unix::fs::FileExt::read_exact_at(&file, &mut bytes, 0)?;

    // Little-endian 32-bit pixels: blue, green, red, alpha for (A|X)RGB, red first for
    // (A|X)BGR.
    let red_first = matches!(format, wl_shm::Format::Abgr8888 | wl_shm::Format::Xbgr8888);

    if !red_first && !matches!(format, wl_shm::Format::Argb8888 | wl_shm::Format::Xrgb8888) {
      return Err(format!("the screencopy's buffer is {format:?}, not 32-bit RGB").into());
    }

    let inverted = state.inverted;

    Ok(area.crop(width, height, |x, y| {
      let row = if inverted { height as usize - 1 - y } else { y };
      let at = row * stride as usize + x * 4;
      let (red, blue) = if red_first {
        (bytes[at], bytes[at + 2])
      } else {
        (bytes[at + 2], bytes[at])
      };

      [red, bytes[at + 1], blue, 255]
    }))
  }
}
