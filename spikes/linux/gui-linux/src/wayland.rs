//! The Wayland backend over smithay-client-toolkit:
//!
//! - an xdg-shell toplevel asking for server-side decorations through xdg-decoration, its
//!   title bar, shadow and resize band drawn by Skene where the compositor answers
//!   client-side or offers no xdg-decoration (mutter, weston);
//! - frames into `wl_shm` buffers (premultiplied ARGB, the shadow translucent), paced by frame
//!   callbacks, only the damaged rectangle reported;
//! - fractional-scale-v1 with viewporter where offered (the buffer at the fractional size, the
//!   surface at the logical one), the integer buffer scale otherwise;
//! - the cursor through cursor-shape-v1 where offered and the reader's xcursor theme
//!   otherwise;
//! - the pointer through sctk (high-resolution `value120` wheels, pixel-precise touchpads whose
//!   scroll flings on once the compositor marks the fingers lifted, `axis_stop`),
//!   the keyboard raw (the compositor's keymap into xkbcommon, so compose, layouts and repeat
//!   are Skene's);
//! - the touchscreen (`wl_touch`), the first finger down acting: a tap clicks, a drag scrolls or
//!   selects, a long press asks for a context menu;
//! - the clipboard (`wl_data_device`) and the primary selection;
//! - an open overlay on an `xdg_popup`, placed by the compositor beside the anchor it hangs
//!   from and grabbing the pointer and the keyboard while it is open;
//! - text-input-v3, through [`crate::ime::wayland`].

use std::{
  collections::HashMap,
  error::Error,
  io::{Read as _, Write as _},
  os::fd::AsRawFd as _,
  time::{Duration, Instant},
};

use sctk::{
  compositor::{CompositorHandler, CompositorState, FrameCallbackData, Region},
  data_device_manager::{
    DataDeviceManagerState, WritePipe,
    data_device::{DataDevice, DataDeviceHandler},
    data_offer::{DataOfferHandler, DragOffer},
    data_source::{CopyPasteSource, DataSourceHandler, DragSource},
  },
  delegate_registry,
  dispatch2::Dispatch2,
  output::{OutputHandler, OutputState},
  primary_selection::{
    PrimarySelectionManagerState,
    device::{PrimarySelectionDevice, PrimarySelectionDeviceHandler},
    selection::{PrimarySelectionSource, PrimarySelectionSourceHandler},
  },
  reexports::{
    csd_frame::{WindowManagerCapabilities, WindowState},
    protocols::{
      wp::{
        fractional_scale::v1::client::{
          wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
          wp_fractional_scale_v1::{self, WpFractionalScaleV1},
        },
        primary_selection::zv1::client::{
          zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1,
          zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1,
        },
        viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
      },
      xdg::shell::client::{
        xdg_positioner::{Anchor, ConstraintAdjustment, Gravity},
        xdg_toplevel::ResizeEdge,
      },
    },
  },
  registry::{ProvidesRegistryState, RegistryState},
  registry_handlers,
  seat::{
    Capability, SeatHandler, SeatState,
    pointer::{
      CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer,
    },
    touch::TouchHandler,
  },
  shell::{
    WaylandSurface,
    xdg::{
      XdgPositioner, XdgShell, XdgSurface as _,
      popup::{Popup, PopupConfigure, PopupHandler},
      window::{DecorationMode, Window, WindowConfigure, WindowDecorations, WindowHandler},
    },
  },
  shm::{
    Shm, ShmHandler,
    slot::{Buffer, SlotPool},
  },
};
use wayland_client::{
  Connection, EventQueue, QueueHandle, WEnum,
  globals::registry_queue_init,
  protocol::{
    wl_data_device::WlDataDevice, wl_data_device_manager::DndAction, wl_data_source::WlDataSource,
    wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface, wl_touch,
  },
};
use xkbcommon::xkb;

use crate::{
  chrome::Edge,
  clipboard::{self, Op, Selection, Then},
  keys::Keyboard,
  runtime::Message,
  shell::{Cursor, DropAction, Offered, Request, Shell, Wanted, Wheel},
  trace,
};

/// The MIME types a text selection is offered and read under, preferred first.
const TEXT_TYPES: [&str; 4] = [
  "text/plain;charset=utf-8",
  "UTF8_STRING",
  "text/plain",
  "STRING",
];

/// User data of the realization's own objects.
struct Raw;

/// The open overlay on its own surface.
struct OpenPopup {
  popup: Popup,
  viewport: Option<WpViewport>,
  configured: bool,
  buffers: Vec<Buffer>,
}

/// A held key repeating: its code and when it repeats next.
struct Repeat {
  keycode: u32,
  next: Instant,
}

struct State {
  /// The window shell the protocol events are handed to.
  shell: Shell,
  /// The compositor's globals.
  registry: RegistryState,
  /// The seats and their capabilities.
  seats: SeatState,
  /// The outputs.
  outputs: OutputState,
  /// The compositor global.
  compositor: CompositorState,
  /// Shared memory for the buffers.
  shm: Shm,
  /// The buffers' pool.
  pool: SlotPool,
  /// The buffers drawn into, each with the damage it still owes.
  buffers: Vec<(Buffer, (u32, u32, u32, u32))>,
  /// The toplevel.
  window: Window,
  /// Whether the toplevel was configured once.
  configured: bool,
  /// The size the toplevel had when it last floated, which it returns to where the
  /// compositor leaves the size to it as it floats again.
  floating_size: (u32, u32),
  /// Whether a frame callback is awaited.
  frame_pending: bool,
  /// The seat in use.
  seat: Option<wl_seat::WlSeat>,
  /// The seat's keyboard.
  keyboard_object: Option<wl_keyboard::WlKeyboard>,
  /// The keymap and its state.
  keyboard: Keyboard,
  /// The seat's pointer, with its cursor theme.
  pointer: Option<ThemedPointer>,
  /// The seat's touchscreen.
  touch: Option<wl_touch::WlTouch>,
  /// The touch points down on the open popup, and where each is in its coordinates.
  popup_touches: HashMap<i32, (f32, f32)>,
  /// The cursor shown.
  cursor: Cursor,
  /// The serial of the last input event.
  last_serial: u32,
  /// The serial of the last button press.
  last_press_serial: u32,
  /// The serial of the last button press or key event: what a popup's grab names (a
  /// compositor honours a key's press while it is held and the last release).
  grab_serial: u32,
  /// The surface's fractional scale.
  fractional: Option<WpFractionalScaleV1>,
  /// The surface's viewport, where it is scaled fractionally.
  viewport: Option<WpViewport>,
  /// The surface's integer buffer scale.
  integer_scale: i32,
  /// The input region last set, in surface pixels.
  input_rect: Option<(i32, i32, i32, i32)>,
  /// The key repeating, where one is held.
  repeat: Option<Repeat>,
  /// The data device manager.
  devices: Option<DataDeviceManagerState>,
  /// The seat's data device.
  device: Option<DataDevice>,
  /// The clipboard selection offered.
  source: Option<CopyPasteSource>,
  /// The primary selection manager.
  primaries: Option<PrimarySelectionManagerState>,
  /// The seat's primary selection device.
  primary_device: Option<PrimarySelectionDevice>,
  /// The primary selection offered.
  primary_source: Option<PrimarySelectionSource>,
  /// The text of each selection this window offers.
  owned: HashMap<Selection, String>,
  /// The drag of text out of the window, its text and the action the compositor chose.
  dragging: Option<(DragSource, String, Option<DndAction>)>,
  /// The drops whose data is being read, by read id: where each landed.
  drops: HashMap<u64, (DragOffer, (f32, f32), Wanted)>,
  /// The selection reads in flight, by id: what to do with the text.
  reads: HashMap<u64, Then>,
  /// The next read's id.
  next_read: u64,
  /// The connection.
  conn: Connection,
  /// The xdg-shell global.
  xdg: XdgShell,
  /// The viewporter global.
  viewporter: Option<WpViewporter>,
  /// The open popup surface.
  popup: Option<OpenPopup>,
  /// The text-input-v3 client.
  text_input: crate::ime::wayland::TextInput,
  /// The queue handle new objects are made on.
  qh: QueueHandle<Self>,
}

/// A window open on `WAYLAND_DISPLAY`, its loop turned a step at a time.
pub struct Open {
  state: State,
  queue: EventQueue<State>,
}

impl Open {
  /// The window's shell.
  pub const fn shell(&self) -> &Shell {
    &self.state.shell
  }

  /// The window's shell, to change.
  pub const fn shell_mut(&mut self) -> &mut Shell {
    &mut self.state.shell
  }

  /// Takes `width` by `height` pixels as the window's geometry, as a floating window may: the
  /// compositor learns it from the next frame.
  pub fn resize(&mut self, width: u32, height: u32) {
    self.state.shell.configure(width, height);

    let (width, height) = self.state.shell.size;
    let rect = skene_foundation::layout::Rect {
      x: 0.,
      y: 0.,
      width: width as f32,
      height: height as f32,
    };

    crate::access::set_window_bounds(rect, rect);
  }

  /// Turns the loop once: handles what arrived, paints what changed, then waits for more,
  /// `wait` at most where given. A turn that paints a frame ends with it: what arrived
  /// meanwhile (read by the GPU's swapchain, which reads the connection too) is the next
  /// turn's.
  pub fn step(&mut self, wait: Option<Duration>) -> Result<(), Box<dyn Error>> {
    let deadline = wait.map(|wait| Instant::now() + wait);
    let state = &mut self.state;
    let queue = &mut self.queue;
    let qh = state.qh.clone();
    let inbox = crate::runtime::inbox();
    let frames = state.shell.frames;

    loop {
      queue.dispatch_pending(state)?;
      state.repeat_keys();
      crate::turn(&mut state.shell);
      state.clipboard(&qh);

      let pending = state.shell.take_requests();

      state.requests(pending);
      state.sync_text_input();
      state.draw_popup(&qh);

      if state.shell.quit {
        return Ok(());
      }

      if state.configured && !state.frame_pending && state.shell.wants_frame() {
        state.draw(&qh);
      }

      queue.flush()?;

      let Some(guard) = queue.prepare_read() else {
        if state.shell.frames > frames {
          return Ok(());
        }

        continue;
      };
      let mut fds = [
        libc::pollfd {
          fd: guard.connection_fd().as_raw_fd(),
          events: libc::POLLIN,
          revents: 0,
        },
        libc::pollfd {
          fd: inbox.read,
          events: libc::POLLIN,
          revents: 0,
        },
      ];
      let left = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
      let timeout = [
        crate::runtime::timeout(),
        state
          .repeat
          .as_ref()
          .map(|repeat| repeat.next.saturating_duration_since(Instant::now())),
      ]
      .into_iter()
      .flatten()
      .min()
      .map(|timeout| timeout + Duration::from_millis(1))
      .into_iter()
      .chain(left)
      .min()
      .map_or(-1, |timeout| {
        i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX)
      });

      // SAFETY: polls two live descriptors.
      unsafe {
        libc::poll(fds.as_mut_ptr(), 2, timeout);
      }

      if fds[0].revents & libc::POLLIN != 0 {
        guard.read()?;
      }

      return Ok(());
    }
  }
}

/// Opens the window on `WAYLAND_DISPLAY`.
pub fn open(shell: Shell, app_id: &str) -> Result<Open, Box<dyn Error>> {
  let conn = Connection::connect_to_env()?;
  let (globals, queue) = registry_queue_init::<State>(&conn)?;
  let qh = queue.handle();
  let compositor = CompositorState::bind(&globals, &qh)?;
  let xdg = XdgShell::bind(&globals, &qh)?;
  let shm = Shm::bind(&globals, &qh)?;
  let fractional_manager = globals
    .bind::<WpFractionalScaleManagerV1, _, _>(&qh, 1..=1, Raw)
    .ok();
  let viewporter: Option<WpViewporter> = globals.bind::<WpViewporter, _, _>(&qh, 1..=1, Raw).ok();
  let surface = compositor.create_surface(&qh);
  let (fractional, viewport) = match (&fractional_manager, &viewporter) {
    (Some(manager), Some(viewporter)) => (
      Some(manager.get_fractional_scale(&surface, &qh, Raw)),
      Some(viewporter.get_viewport(&surface, &qh, Raw)),
    ),
    _ => (None, None),
  };
  let window = xdg.create_window(surface, WindowDecorations::RequestServer, &qh);

  window.set_title(shell.title().to_owned());
  window.set_app_id(app_id.to_owned());
  window.set_min_size(Some((200, 120)));
  window.commit();

  trace::log(|| {
    let globals: Vec<String> = globals
      .contents()
      .clone_list()
      .into_iter()
      .map(|global| format!("{}v{}", global.interface, global.version))
      .collect();

    format!("[wayland] globals {}", globals.join(" "))
  });

  let pool = SlotPool::new(480 * 640 * 4, &shm)?;
  let mut state = State {
    registry: RegistryState::new(&globals),
    seats: SeatState::new(&globals, &qh),
    outputs: OutputState::new(&globals, &qh),
    compositor,
    shm,
    pool,
    buffers: Vec::new(),
    window,
    configured: false,
    floating_size: (0, 0),
    frame_pending: false,
    seat: None,
    keyboard_object: None,
    keyboard: Keyboard::new(),
    pointer: None,
    touch: None,
    popup_touches: HashMap::new(),
    cursor: Cursor::Default,
    last_serial: 0,
    last_press_serial: 0,
    grab_serial: 0,
    fractional,
    viewport,
    integer_scale: 1,
    input_rect: None,
    repeat: None,
    devices: DataDeviceManagerState::bind(&globals, &qh).ok(),
    device: None,
    source: None,
    primaries: PrimarySelectionManagerState::bind(&globals, &qh).ok(),
    primary_device: None,
    primary_source: None,
    owned: HashMap::new(),
    dragging: None,
    drops: HashMap::new(),
    reads: HashMap::new(),
    next_read: 0,
    conn: conn.clone(),
    xdg,
    viewporter,
    popup: None,
    text_input: crate::ime::wayland::TextInput::bind(&globals, &qh),
    qh: qh.clone(),
    shell,
  };

  // sctk announces only the seats that appear after startup.
  for seat in state.seats.seats().collect::<Vec<_>>() {
    state.new_seat(&conn, &qh, seat);
  }

  #[cfg(feature = "gpu")]
  {
    use wayland_client::Proxy as _;

    let display = std::ptr::NonNull::new(conn.backend().display_ptr().cast());
    let surface = std::ptr::NonNull::new(state.window.wl_surface().id().as_ptr().cast());

    if let (Some(display), Some(surface)) = (display, surface) {
      // SAFETY: the connection and the window's surface live as long as the window.
      unsafe {
        state
          .shell
          .choose_path(crate::gpu::Target::Wayland { display, surface });
      }
    }
  }

  Ok(Open { state, queue })
}

impl State {
  /// Paints what changed and attaches it.
  fn draw(&mut self, qh: &QueueHandle<Self>) {
    let started = Instant::now();
    let scale = self.shell.scale;
    let (geometry_width, geometry_height) = self.shell.size;
    let (surface_width, surface_height) = self
      .shell
      .chrome
      .surface_size(geometry_width, geometry_height);
    let (ox, oy) = self.shell.chrome.origin();
    let input = self
      .shell
      .chrome
      .input_rect(geometry_width, geometry_height);
    let gpu = self.shell.presents();

    if gpu {
      // The swapchain commits the frame: the surface's state and the frame callback go first.
      self.prepare_surface((ox, oy), input, (surface_width, surface_height));
      self
        .window
        .wl_surface()
        .frame(qh, FrameCallbackData(self.window.wl_surface().clone()));
    }

    let Some(frame) = self.shell.frame() else {
      return;
    };

    if frame.presented {
      self.frame_pending = true;
      trace::log(|| {
        format!(
          "[wayland] present gpu in {:.2} ms",
          started.elapsed().as_secs_f64() * 1000.
        )
      });

      if trace::shots_on()
        && let Some(frame) = self.shell.last_frame()
      {
        trace::shot(frame);
      }

      return;
    }

    let pixmap = frame.pixmap;
    let (dx, dy, dw, dh) = frame.damage;
    let (width, height) = (i32::from(pixmap.width()), i32::from(pixmap.height()));
    let convert = |source: &[vello_cpu::peniko::color::PremulRgba8], target: &mut [u8]| {
      for (pixel, target) in source.iter().zip(target.as_chunks_mut::<4>().0) {
        *target = [pixel.b, pixel.g, pixel.r, pixel.a];
      }
    };
    let pixels = pixmap.data();

    // Two buffers take turns. Each frame's damage is added to what each buffer lacks; a
    // buffer the compositor released gets only what it lacks, and a fresh one the whole frame.
    for (_, missing) in &mut self.buffers {
      *missing = union_damage(*missing, (dx, dy, dw, dh));
    }

    self
      .buffers
      .retain(|(buffer, _)| buffer.height() == height && buffer.stride() == width * 4);

    let mut reused = None;

    for (index, (buffer, missing)) in self.buffers.iter_mut().enumerate() {
      if let Some(canvas) = buffer.canvas(&mut self.pool) {
        let (mx, my, mw, mh) = *missing;

        for row in my..my + mh {
          let start = (row * width as u32 + mx) as usize;
          let end = start + mw as usize;

          convert(&pixels[start..end], &mut canvas[start * 4..end * 4]);
        }

        *missing = (0, 0, 0, 0);
        reused = Some(index);

        break;
      }
    }

    let current = match reused {
      Some(index) => index,
      None => {
        let Ok((buffer, canvas)) =
          self
            .pool
            .create_buffer(width, height, width * 4, wl_shm::Format::Argb8888)
        else {
          return;
        };

        convert(pixels, canvas);

        if self.buffers.len() >= 2 {
          self.buffers.remove(0);
        }

        self.buffers.push((buffer, (0, 0, 0, 0)));
        self.buffers.len() - 1
      }
    };

    if !gpu {
      self.prepare_surface((ox, oy), input, (surface_width, surface_height));
    }

    let surface = self.window.wl_surface();

    surface.damage_buffer(dx as i32, dy as i32, dw as i32, dh as i32);

    if !gpu {
      surface.frame(qh, FrameCallbackData(surface.clone()));
    }

    if let Some((buffer, _)) = self.buffers.get(current)
      && buffer.attach_to(surface).is_ok()
    {
      self.window.commit();
      self.frame_pending = true;
    }

    trace::log(|| {
      format!(
        "[wayland] present {dw}x{dh}+{dx}+{dy} of {width}x{height} at {scale} {} in {:.2} ms",
        if reused.is_some() {
          "into a released buffer"
        } else {
          "into a new buffer"
        },
        started.elapsed().as_secs_f64() * 1000.
      )
    });

    if trace::shots_on()
      && let Some(frame) = self.shell.last_frame()
    {
      trace::shot(frame);
    }
  }

  /// Gives the surface the window's geometry within it (`origin`, past the shadow's margin),
  /// the input region and its scale for `size` surface pixels: state the next commit applies.
  fn prepare_surface(
    &mut self,
    origin: (f32, f32),
    input: skene_foundation::layout::Rect,
    size: (u32, u32),
  ) {
    let (geometry_width, geometry_height) = self.shell.size;

    self.window.set_window_geometry(
      origin.0 as u32,
      origin.1 as u32,
      geometry_width,
      geometry_height,
    );

    let input = (
      input.x as i32,
      input.y as i32,
      input.width as i32,
      input.height as i32,
    );

    if self.input_rect != Some(input)
      && let Ok(region) = Region::new(&self.compositor)
    {
      region.add(input.0, input.1, input.2, input.3);
      self
        .window
        .wl_surface()
        .set_input_region(Some(region.wl_region()));
      self.input_rect = Some(input);
    }

    let surface = self.window.wl_surface();

    match &self.viewport {
      Some(viewport) => {
        surface.set_buffer_scale(1);
        viewport.set_destination(size.0 as i32, size.1 as i32);
      }
      None => surface.set_buffer_scale(self.shell.scale.round() as i32),
    }
  }

  fn requests(&mut self, requests: Vec<Request>) {
    for request in requests {
      trace::log(|| format!("[wayland] request {request:?}"));

      match request {
        Request::Popup(placement) => self.place_popup(placement),
        Request::Close => self.shell.quit = true,
        Request::ToggleMaximize => {
          if self.shell.chrome.states.maximized {
            self.window.unset_maximized();
          } else {
            self.window.set_maximized();
          }
        }
        Request::Minimize => self.window.set_minimized(),
        Request::Move => {
          if let Some(seat) = &self.seat {
            self.window.move_(seat, self.last_press_serial);
          }
        }
        Request::Resize(edge) => {
          if let Some(seat) = &self.seat {
            self
              .window
              .resize(seat, self.last_press_serial, resize_edge(edge));
          }
        }
        Request::ShowMenu(x, y) => {
          if let Some(seat) = &self.seat {
            self
              .window
              .show_window_menu(seat, self.last_press_serial, (x as i32, y as i32));
          }
        }
        Request::Cursor(cursor) => {
          self.cursor = cursor;
          self.set_cursor();
        }
        Request::StartDrag => self.start_drag(),
      }
    }
  }

  /// Starts the drag the shell asked for, from the last press: the compositor carries it,
  /// showing what a target under it answers.
  fn start_drag(&mut self) {
    let (Some(devices), Some(device)) = (&self.devices, &self.device) else {
      self.shell.drag_ended(None);

      return;
    };
    let Some((text, moves)) = self
      .shell
      .dragged()
      .map(|(text, moves)| (text.to_owned(), moves))
    else {
      return;
    };
    let actions = if moves {
      DndAction::Copy | DndAction::Move
    } else {
      DndAction::Copy
    };
    let source = devices.create_drag_and_drop_source(&self.qh, TEXT_TYPES, actions);

    source.start_drag(
      device,
      self.window.wl_surface(),
      None,
      self.last_press_serial,
    );
    trace::log(|| format!("[dnd] start a drag, serial {}", self.last_press_serial));
    self.dragging = Some((source, text, None));
  }

  /// What `offer` offers.
  fn offered(offer: &DragOffer) -> Offered {
    offer.with_mime_types(|types| Offered {
      text: TEXT_TYPES
        .iter()
        .any(|wanted| types.iter().any(|offered| offered == *wanted)),
      uris: types.iter().any(|offered| offered == "text/uri-list"),
    })
  }

  /// Answers `offer` at `x`, `y` (surface pixels) as the shell answers a drag there: the type
  /// it would take and the action, or a refusal.
  fn answer_offer(&mut self, offer: &DragOffer, x: f64, y: f64) {
    let offered = Self::offered(offer);
    let action = self.shell.drag_over(x as f32, y as f32, offered);
    let mime = match self.shell.drop_wants(x as f32, y as f32, offered) {
      Some(Wanted::Uris) => Some("text/uri-list".to_owned()),
      Some(Wanted::Text) => offer.with_mime_types(pick_mime),
      None => None,
    };

    match action.zip(mime) {
      Some((action, mime)) => {
        let action = match action {
          DropAction::Copy => DndAction::Copy,
          DropAction::Move => DndAction::Move,
        };

        offer.accept_mime_type(offer.serial, Some(mime));
        offer.set_actions(action, action);
      }
      None => {
        offer.accept_mime_type(offer.serial, None);
        offer.set_actions(DndAction::empty(), DndAction::empty());
      }
    }
  }

  /// Tells the compositor's input method about the focused entry.
  fn sync_text_input(&mut self) {
    let origin = self.shell.chrome.origin();

    self.text_input.sync(&self.shell, origin);
  }

  /// Opens the open overlay's surface at `placement`, or closes it.
  fn place_popup(&mut self, placement: Option<crate::overlay::Placement>) {
    self.popup = None;

    let Some(placement) = placement else {
      return;
    };
    let qh = self.qh.clone();
    let Ok(positioner) = XdgPositioner::new(&self.xdg) else {
      return;
    };
    let anchor = placement.anchor;

    positioner.set_size(placement.size.0 as i32, placement.size.1 as i32);
    positioner.set_anchor_rect(
      anchor.x.round() as i32,
      anchor.y.round() as i32,
      (anchor.width.round() as i32).max(1),
      (anchor.height.round() as i32).max(1),
    );
    positioner.set_anchor(Anchor::BottomLeft);
    positioner.set_gravity(Gravity::BottomRight);
    positioner.set_offset(0, 2);
    positioner.set_constraint_adjustment(
      ConstraintAdjustment::FlipY | ConstraintAdjustment::SlideX | ConstraintAdjustment::SlideY,
    );

    let surface = self.compositor.create_surface(&qh);
    let Ok(popup) = Popup::from_surface(
      Some(self.window.xdg_surface()),
      &positioner,
      &qh,
      surface,
      &self.xdg,
    ) else {
      return;
    };

    if let Some(seat) = &self.seat {
      popup.xdg_popup().grab(seat, self.grab_serial);
    }

    let viewport = self
      .viewporter
      .as_ref()
      .map(|viewporter| viewporter.get_viewport(popup.wl_surface(), &qh, Raw));

    popup.wl_surface().commit();
    trace::log(|| {
      format!(
        "[wayland] popup at {anchor:?} grab serial {}",
        self.grab_serial
      )
    });
    self.popup = Some(OpenPopup {
      popup,
      viewport,
      configured: false,
      buffers: Vec::new(),
    });
  }

  /// Paints the open overlay where it changed and attaches it.
  fn draw_popup(&mut self, _: &QueueHandle<Self>) {
    let Some(open) = &mut self.popup else {
      return;
    };

    if !open.configured || !self.shell.popup_wants_frame() {
      return;
    }

    let scale = self.shell.scale;
    let Some((width, height)) = self.shell.popup().map(|list| list.placement.size) else {
      return;
    };
    let Some(frame) = self.shell.popup_frame() else {
      return;
    };
    let pixmap = frame.pixmap;
    let (device_width, device_height) = (i32::from(pixmap.width()), i32::from(pixmap.height()));
    let Ok((buffer, canvas)) = self.pool.create_buffer(
      device_width,
      device_height,
      device_width * 4,
      wl_shm::Format::Argb8888,
    ) else {
      return;
    };

    for (pixel, target) in pixmap.data().iter().zip(canvas.as_chunks_mut::<4>().0) {
      *target = [pixel.b, pixel.g, pixel.r, pixel.a];
    }

    let surface = open.popup.wl_surface();

    match &open.viewport {
      Some(viewport) => {
        surface.set_buffer_scale(1);
        viewport.set_destination(width as i32, height as i32);
      }
      None => surface.set_buffer_scale(scale.round() as i32),
    }

    if buffer.attach_to(surface).is_ok() {
      surface.damage_buffer(0, 0, device_width, device_height);
      surface.commit();
    }

    open.buffers.push(buffer);

    if open.buffers.len() > 2 {
      open.buffers.remove(0);
    }

    if let Some(list) = self.shell.popup() {
      trace::shot_popup(list.pixmap());
    }
  }

  fn set_cursor(&self) {
    if let Some(pointer) = &self.pointer {
      let result = pointer.set_cursor(&self.conn, cursor_icon(self.cursor));

      trace::log(|| format!("[wayland] cursor {} -> {result:?}", self.cursor.name()));
    }
  }

  fn repeat_keys(&mut self) {
    let Some(repeat) = &mut self.repeat else {
      return;
    };
    let now = Instant::now();

    if repeat.next > now {
      return;
    }

    let keycode = repeat.keycode;
    let (rate, _) = self.keyboard.repeat;

    repeat.next = now + Duration::from_millis(1000 / u64::from(rate.max(1)));

    if let Some(input) = self.keyboard.key(keycode, true, true) {
      self.shell.key(&input);
    }
  }

  fn selection_text(&mut self, selection: Selection) -> Option<String> {
    self.owned.get(&selection).cloned()
  }

  /// Carries out the clipboard operations queued.
  fn clipboard(&mut self, qh: &QueueHandle<Self>) {
    for op in clipboard::take() {
      match op {
        Op::Write(selection, text) => {
          trace::log(|| format!("[clipboard] offer {selection:?} ({} bytes)", text.len()));

          match selection {
            Selection::Clipboard => {
              if let (Some(devices), Some(device)) = (&self.devices, &self.device) {
                let source = devices.create_copy_paste_source(qh, TEXT_TYPES);

                source.set_selection(device, self.last_serial);
                self.source = Some(source);
                self.owned.insert(selection, text);
              }
            }
            Selection::Primary => {
              if let (Some(primaries), Some(device)) = (&self.primaries, &self.primary_device) {
                let source = primaries.create_selection_source(qh, TEXT_TYPES);

                source.set_selection(device, self.last_serial);
                self.primary_source = Some(source);
                self.owned.insert(selection, text);
              }
            }
          }
        }
        Op::Read(selection, then) => {
          if let Some(text) = self.selection_text(selection) {
            then(Some(text));

            continue;
          }

          let pipe = match selection {
            Selection::Clipboard => self
              .device
              .as_ref()
              .and_then(|device| device.data().selection_offer())
              .and_then(|offer| {
                let mime = offer.with_mime_types(pick_mime)?;

                offer.receive(mime).ok()
              }),
            Selection::Primary => self
              .primary_device
              .as_ref()
              .and_then(|device| device.data().selection_offer())
              .and_then(|offer| {
                let mime = offer.with_mime_types(pick_mime)?;

                offer.receive(mime).ok()
              }),
          };
          let Some(mut pipe) = pipe else {
            then(None);

            continue;
          };
          let id = self.next_read;
          let inbox = crate::runtime::inbox();

          self.next_read += 1;
          self.reads.insert(id, then);

          std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let text = pipe
              .read_to_end(&mut bytes)
              .ok()
              .and_then(|_| String::from_utf8(bytes).ok());

            PIPE_RESULTS
              .lock()
              .unwrap_or_else(std::sync::PoisonError::into_inner)
              .push((id, text));
            inbox.send(Message::Wake);
          });
        }
      }
    }

    let arrived: Vec<(u64, Option<String>)> = std::mem::take(
      &mut *PIPE_RESULTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner),
    );

    for (id, text) in arrived {
      if let Some(then) = self.reads.remove(&id) {
        then(text);
      } else if let Some((offer, at, wanted)) = self.drops.remove(&id) {
        let text = text.unwrap_or_default();
        let dropped = match wanted {
          Wanted::Uris => skene_foundation::seam::report::Dropped {
            text: None,
            uris: text
              .lines()
              .map(str::trim)
              .filter(|line| !line.is_empty() && !line.starts_with('#'))
              .map(std::rc::Rc::from)
              .collect(),
          },
          Wanted::Text => skene_foundation::seam::report::Dropped {
            text: Some(std::rc::Rc::from(text.as_str())),
            uris: Vec::new(),
          },
        };
        let action = self.shell.drop_data(at.0, at.1, dropped);

        trace::log(|| format!("[dnd] the drop did {action:?}"));

        if action.is_some() {
          offer.finish();
        }

        offer.destroy();
      }
    }
  }

  fn answer(&self, selection: Selection, mut pipe: WritePipe) {
    let text = self.owned.get(&selection).cloned().unwrap_or_default();

    std::thread::spawn(move || {
      let _ = pipe.write_all(text.as_bytes());
    });
  }
}

/// What the reading threads delivered: a read's id and its text.
static PIPE_RESULTS: std::sync::Mutex<Vec<(u64, Option<String>)>> =
  std::sync::Mutex::new(Vec::new());

/// The smallest rectangle holding both (x, y, width, height), an empty one adding nothing.
const fn union_damage(a: (u32, u32, u32, u32), b: (u32, u32, u32, u32)) -> (u32, u32, u32, u32) {
  if a.2 == 0 || a.3 == 0 {
    return b;
  }

  if b.2 == 0 || b.3 == 0 {
    return a;
  }

  let x = if a.0 < b.0 { a.0 } else { b.0 };
  let y = if a.1 < b.1 { a.1 } else { b.1 };
  let right = if a.0 + a.2 > b.0 + b.2 {
    a.0 + a.2
  } else {
    b.0 + b.2
  };
  let bottom = if a.1 + a.3 > b.1 + b.3 {
    a.1 + a.3
  } else {
    b.1 + b.3
  };

  (x, y, right - x, bottom - y)
}

fn pick_mime(types: &[String]) -> Option<String> {
  TEXT_TYPES
    .iter()
    .find(|wanted| types.iter().any(|offered| offered == *wanted))
    .map(|mime| (*mime).to_owned())
}

const fn resize_edge(edge: Edge) -> ResizeEdge {
  match edge {
    Edge::Top => ResizeEdge::Top,
    Edge::Bottom => ResizeEdge::Bottom,
    Edge::Left => ResizeEdge::Left,
    Edge::Right => ResizeEdge::Right,
    Edge::TopLeft => ResizeEdge::TopLeft,
    Edge::TopRight => ResizeEdge::TopRight,
    Edge::BottomLeft => ResizeEdge::BottomLeft,
    Edge::BottomRight => ResizeEdge::BottomRight,
  }
}

const fn cursor_icon(cursor: Cursor) -> CursorIcon {
  match cursor {
    Cursor::Default => CursorIcon::Default,
    Cursor::Pointer => CursorIcon::Pointer,
    Cursor::Text => CursorIcon::Text,
    Cursor::Resize(edge) => match edge {
      Edge::Top => CursorIcon::NResize,
      Edge::Bottom => CursorIcon::SResize,
      Edge::Left => CursorIcon::WResize,
      Edge::Right => CursorIcon::EResize,
      Edge::TopLeft => CursorIcon::NwResize,
      Edge::TopRight => CursorIcon::NeResize,
      Edge::BottomLeft => CursorIcon::SwResize,
      Edge::BottomRight => CursorIcon::SeResize,
    },
  }
}

impl CompositorHandler for State {
  fn scale_factor_changed(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_surface::WlSurface,
    factor: i32,
  ) {
    trace::log(|| format!("[wayland] integer scale {factor}"));
    self.integer_scale = factor;

    if self.fractional.is_none() {
      self.shell.set_scale(f64::from(factor));
    }
  }

  fn transform_changed(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_surface::WlSurface,
    _: wl_output::Transform,
  ) {
  }

  fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
    // The turn that dispatched it paints the next frame.
    self.frame_pending = false;
  }

  fn surface_enter(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_surface::WlSurface,
    _: &wl_output::WlOutput,
  ) {
  }

  fn surface_leave(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_surface::WlSurface,
    _: &wl_output::WlOutput,
  ) {
  }
}

impl OutputHandler for State {
  fn output_state(&mut self) -> &mut OutputState {
    &mut self.outputs
  }

  fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

  fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

  fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl WindowHandler for State {
  fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
    self.shell.quit = true;
  }

  fn configure(
    &mut self,
    _: &Connection,
    qh: &QueueHandle<Self>,
    _: &Window,
    configure: WindowConfigure,
    _: u32,
  ) {
    let state = configure.state;
    let capabilities = configure.capabilities;
    let chrome = &mut self.shell.chrome;
    let floated = chrome.floating();

    chrome.csd = configure.decoration_mode == DecorationMode::Client;
    chrome.states = crate::chrome::States {
      maximized: state.contains(WindowState::MAXIMIZED),
      fullscreen: state.contains(WindowState::FULLSCREEN),
      activated: state.contains(WindowState::ACTIVATED),
      resizing: state.contains(WindowState::RESIZING),
      suspended: state.contains(WindowState::SUSPENDED),
      tiled: [
        state.contains(WindowState::TILED_LEFT),
        state.contains(WindowState::TILED_TOP),
        state.contains(WindowState::TILED_RIGHT),
        state.contains(WindowState::TILED_BOTTOM),
      ],
    };
    chrome.capabilities = crate::chrome::Capabilities {
      window_menu: capabilities.contains(WindowManagerCapabilities::WINDOW_MENU),
      maximize: capabilities.contains(WindowManagerCapabilities::MAXIMIZE),
      minimize: capabilities.contains(WindowManagerCapabilities::MINIMIZE),
      fullscreen: capabilities.contains(WindowManagerCapabilities::FULLSCREEN),
    };

    let floats = self.shell.chrome.floating();

    if floated && !floats {
      self.floating_size = self.shell.size;
    }

    // A dimension the compositor leaves to the window: the size it floated at, as it floats
    // again, else the size it has.
    let (width, height) = if floats && !floated && self.floating_size.0 > 0 {
      self.floating_size
    } else {
      self.shell.size
    };
    let width = configure.new_size.0.map_or(width, |width| width.get());
    let height = configure.new_size.1.map_or(height, |height| height.get());

    trace::log(|| {
      format!(
        "[wayland] configure {width}x{height} decorations {:?} states {:?} capabilities {:?}",
        configure.decoration_mode, state, capabilities
      )
    });
    self.shell.configure(width, height);
    crate::shell::invalidate_layout();
    // Wayland keeps a window's place on the screen from it: the reader learns its size.
    let (width, height) = self.shell.size;
    let rect = skene_foundation::layout::Rect {
      x: 0.,
      y: 0.,
      width: width as f32,
      height: height as f32,
    };

    crate::access::set_window_bounds(rect, rect);
    self.configured = true;

    if !self.frame_pending {
      self.draw(qh);
    }
  }
}

impl SeatHandler for State {
  fn seat_state(&mut self) -> &mut SeatState {
    &mut self.seats
  }

  fn new_seat(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
    if let Some(devices) = &self.devices
      && self.device.is_none()
    {
      self.device = Some(devices.get_data_device(qh, &seat));
    }

    if let Some(primaries) = &self.primaries
      && self.primary_device.is_none()
    {
      self.primary_device = Some(primaries.get_selection_device(qh, &seat));
    }

    self.text_input.new_seat(&seat, qh);
    self.seat = Some(seat);
  }

  fn new_capability(
    &mut self,
    _: &Connection,
    qh: &QueueHandle<Self>,
    seat: wl_seat::WlSeat,
    capability: Capability,
  ) {
    if capability == Capability::Keyboard && self.keyboard_object.is_none() {
      self.keyboard_object = Some(seat.get_keyboard(qh, Raw));
    }

    if capability == Capability::Touch && self.touch.is_none() {
      self.touch = self.seats.get_touch(qh, &seat).ok();
      trace::log(|| format!("[wayland] touch {}", self.touch.is_some()));
    }

    if capability == Capability::Pointer && self.pointer.is_none() {
      let surface = self.compositor.create_surface(qh);

      self.pointer = self
        .seats
        .get_pointer_with_theme::<Self, ()>(
          qh,
          &seat,
          self.shm.wl_shm(),
          surface,
          ThemeSpec::System,
        )
        .ok();
    }
  }

  fn remove_capability(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: wl_seat::WlSeat,
    capability: Capability,
  ) {
    if capability == Capability::Keyboard
      && let Some(keyboard) = self.keyboard_object.take()
    {
      keyboard.release();
    }

    if capability == Capability::Pointer {
      self.pointer = None;
    }

    if capability == Capability::Touch
      && let Some(touch) = self.touch.take()
    {
      touch.release();
      self.shell.touch_cancel();
    }
  }

  fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for State {
  fn pointer_frame(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_pointer::WlPointer,
    events: &[PointerEvent],
  ) {
    for event in events {
      let (x, y) = (event.position.0 as f32, event.position.1 as f32);

      if self
        .popup
        .as_ref()
        .is_some_and(|open| open.popup.wl_surface() == &event.surface)
      {
        match event.kind {
          PointerEventKind::Motion { .. } | PointerEventKind::Enter { .. } => {
            self.shell.popup_motion(x, y);
          }
          PointerEventKind::Press { serial, .. } => {
            self.last_serial = serial;
            self.grab_serial = serial;
            self.shell.popup_button(true, x, y);
          }
          PointerEventKind::Release { serial, .. } => {
            self.last_serial = serial;
            self.shell.popup_button(false, x, y);
          }
          PointerEventKind::Axis { vertical, .. } => {
            self
              .shell
              .popup_wheel(Wheel::Pixels(0., vertical.absolute as f32));
          }
          PointerEventKind::Leave { .. } => {}
        }

        continue;
      }

      if &event.surface != self.window.wl_surface() {
        continue;
      }

      match event.kind {
        PointerEventKind::Enter { serial } => {
          let (ox, oy) = self.shell.chrome.origin();

          self.last_serial = serial;
          trace::pointer(x - ox, y - oy);

          let requests = self.shell.pointer_motion(x, y);

          self.set_cursor();
          self.requests(requests);
        }
        PointerEventKind::Leave { .. } => self.shell.pointer_leave(),
        PointerEventKind::Motion { .. } => {
          let (ox, oy) = self.shell.chrome.origin();

          trace::pointer(x - ox, y - oy);

          let requests = self.shell.pointer_motion(x, y);

          self.requests(requests);
        }
        PointerEventKind::Press { button, serial, .. }
        | PointerEventKind::Release { button, serial, .. } => {
          let down = matches!(event.kind, PointerEventKind::Press { .. });
          let button = match button {
            0x110 => 0,
            0x112 => 1,
            0x111 => 2,
            _ => continue,
          };

          self.last_serial = serial;

          if down {
            self.last_press_serial = serial;
            self.grab_serial = serial;
          }

          let requests = self.shell.pointer_button(button, down, (x, y));

          self.requests(requests);
        }
        PointerEventKind::Axis {
          time,
          horizontal,
          vertical,
          source,
        } => {
          let lines = horizontal.value120 != 0
            || vertical.value120 != 0
            || horizontal.discrete != 0
            || vertical.discrete != 0
            || matches!(
              source,
              Some(wl_pointer::AxisSource::Wheel | wl_pointer::AxisSource::WheelTilt)
            );

          if lines {
            // A wheel that reports no detents moves 15 pixels a detent, as libinput's do.
            let steps = |axis: sctk::seat::pointer::AxisScroll| {
              if axis.value120 != 0 {
                axis.value120 as f32 / 120.
              } else if axis.discrete != 0 {
                axis.discrete as f32
              } else {
                axis.absolute as f32 / 15.
              }
            };

            self
              .shell
              .wheel(Wheel::Lines(steps(horizontal), steps(vertical)));
          } else {
            // A touchpad's (or another continuous source's) scrolling, which flings once the
            // compositor says the fingers lifted.
            if horizontal.absolute != 0. || vertical.absolute != 0. {
              self
                .shell
                .finger_scroll(horizontal.absolute as f32, vertical.absolute as f32, time);
            }

            if horizontal.stop || vertical.stop {
              self.shell.finger_scroll_stop(time);
            }
          }
        }
      }
    }
  }
}

impl TouchHandler for State {
  fn down(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_touch::WlTouch,
    serial: u32,
    time: u32,
    surface: wl_surface::WlSurface,
    id: i32,
    position: (f64, f64),
  ) {
    let (x, y) = (position.0 as f32, position.1 as f32);

    self.last_serial = serial;
    self.last_press_serial = serial;
    self.grab_serial = serial;

    if self
      .popup
      .as_ref()
      .is_some_and(|open| open.popup.wl_surface() == &surface)
    {
      self.popup_touches.insert(id, (x, y));
      self.shell.popup_touch_down(x, y);

      return;
    }

    if &surface != self.window.wl_surface() {
      return;
    }

    let requests = self.shell.touch_down(id, x, y, time);

    self.requests(requests);
  }

  fn up(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_touch::WlTouch,
    serial: u32,
    time: u32,
    id: i32,
  ) {
    self.last_serial = serial;

    if let Some((x, y)) = self.popup_touches.remove(&id) {
      self.shell.popup_touch_up(x, y);

      return;
    }

    let requests = self.shell.touch_up(id, time);

    self.requests(requests);
  }

  fn motion(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_touch::WlTouch,
    time: u32,
    id: i32,
    position: (f64, f64),
  ) {
    let (x, y) = (position.0 as f32, position.1 as f32);

    if let Some(at) = self.popup_touches.get_mut(&id) {
      *at = (x, y);

      return;
    }

    let requests = self.shell.touch_motion(id, x, y, time);

    self.requests(requests);
  }

  fn shape(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_touch::WlTouch,
    _: i32,
    _: f64,
    _: f64,
  ) {
  }

  fn orientation(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_touch::WlTouch,
    _: i32,
    _: f64,
  ) {
  }

  fn cancel(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_touch::WlTouch) {
    self.popup_touches.clear();
    self.shell.touch_cancel();
  }
}

impl ShmHandler for State {
  fn shm_state(&mut self) -> &mut Shm {
    &mut self.shm
  }
}

impl DataDeviceHandler for State {
  fn enter(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    device: &WlDataDevice,
    x: f64,
    y: f64,
    surface: &wl_surface::WlSurface,
  ) {
    if surface != self.window.wl_surface() {
      return;
    }

    let offer = self
      .device
      .as_ref()
      .filter(|ours| ours.inner() == device)
      .and_then(|ours| ours.data().drag_offer());

    trace::log(|| format!("[dnd] enter at ({x}, {y}): {}", offer.is_some()));

    if let Some(offer) = offer {
      self.answer_offer(&offer, x, y);
    }
  }

  fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
    trace::log(|| "[dnd] leave".to_owned());
    self.shell.drag_left();
  }

  fn motion(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    device: &WlDataDevice,
    x: f64,
    y: f64,
  ) {
    let offer = self
      .device
      .as_ref()
      .filter(|ours| ours.inner() == device)
      .and_then(|ours| ours.data().drag_offer());

    if let Some(offer) = offer {
      self.answer_offer(&offer, x, y);
    }
  }

  fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
    trace::log(|| "[clipboard] the clipboard has a new offer".to_owned());
  }

  fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice) {
    let Some(offer) = self
      .device
      .as_ref()
      .filter(|ours| ours.inner() == device)
      .and_then(|ours| ours.data().drag_offer())
    else {
      return;
    };
    let at = (offer.x as f32, offer.y as f32);
    let offered = Self::offered(&offer);
    let wanted = self.shell.drop_wants(at.0, at.1, offered);
    let mime = match wanted {
      Some(Wanted::Uris) => Some("text/uri-list".to_owned()),
      Some(Wanted::Text) => offer.with_mime_types(pick_mime),
      None => None,
    };

    trace::log(|| format!("[dnd] drop at {at:?}: {wanted:?} as {mime:?}"));

    let (Some(wanted), Some(mime)) = (wanted, mime) else {
      self.shell.drag_left();
      offer.destroy();

      return;
    };
    let Ok(mut pipe) = offer.receive(mime) else {
      self.shell.drag_left();
      offer.destroy();

      return;
    };
    let id = self.next_read;
    let inbox = crate::runtime::inbox();

    self.next_read += 1;
    self.drops.insert(id, (offer, at, wanted));

    std::thread::spawn(move || {
      let mut bytes = Vec::new();
      let text = pipe
        .read_to_end(&mut bytes)
        .ok()
        .map(|_| String::from_utf8_lossy(&bytes).into_owned());

      PIPE_RESULTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push((id, text));
      inbox.send(Message::Wake);
    });
  }
}

impl DataOfferHandler for State {
  fn source_actions(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &mut DragOffer,
    _: DndAction,
  ) {
  }

  fn selected_action(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &mut DragOffer,
    _: DndAction,
  ) {
  }
}

impl DataSourceHandler for State {
  fn accept_mime(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &WlDataSource,
    _: Option<String>,
  ) {
  }

  fn send_request(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    source: &WlDataSource,
    mime: String,
    mut pipe: WritePipe,
  ) {
    if let Some((ours, text, _)) = &self.dragging
      && ours.inner() == source
    {
      trace::log(|| format!("[dnd] send the dragged text as {mime}"));

      let text = text.clone();

      std::thread::spawn(move || {
        let _ = pipe.write_all(text.as_bytes());
      });

      return;
    }

    trace::log(|| format!("[clipboard] send the clipboard as {mime}"));
    self.answer(Selection::Clipboard, pipe);
  }

  fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
    if self
      .dragging
      .as_ref()
      .is_some_and(|(ours, ..)| ours.inner() == source)
    {
      trace::log(|| "[dnd] the drag was cancelled".to_owned());
      self.dragging = None;
      self.shell.drag_ended(None);

      return;
    }

    if self
      .source
      .as_ref()
      .is_some_and(|ours| ours.inner() == source)
    {
      self.source = None;
      self.owned.remove(&Selection::Clipboard);
      trace::log(|| "[clipboard] another client took the clipboard".to_owned());
    }
  }

  fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {
    trace::log(|| "[dnd] dropped".to_owned());
  }

  fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
    let Some((ours, _, action)) = self.dragging.take() else {
      return;
    };

    if ours.inner() != source {
      self.dragging = Some((ours, String::new(), action));

      return;
    }

    let action = action.and_then(|action| {
      if action.contains(DndAction::Move) {
        Some(DropAction::Move)
      } else if action.contains(DndAction::Copy) {
        Some(DropAction::Copy)
      } else {
        None
      }
    });

    trace::log(|| format!("[dnd] finished: {action:?}"));
    self.shell.drag_ended(action);
  }

  fn action(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    source: &WlDataSource,
    action: DndAction,
  ) {
    if let Some((ours, _, chosen)) = &mut self.dragging
      && ours.inner() == source
    {
      trace::log(|| format!("[dnd] the compositor chose {action:?}"));
      *chosen = Some(action);
    }
  }
}

impl PrimarySelectionDeviceHandler for State {
  fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ZwpPrimarySelectionDeviceV1) {
    trace::log(|| "[clipboard] the primary selection has a new offer".to_owned());
  }
}

impl PrimarySelectionSourceHandler for State {
  fn send_request(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &ZwpPrimarySelectionSourceV1,
    mime: String,
    pipe: WritePipe,
  ) {
    trace::log(|| format!("[clipboard] send the primary selection as {mime}"));
    self.answer(Selection::Primary, pipe);
  }

  fn cancelled(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    source: &ZwpPrimarySelectionSourceV1,
  ) {
    if self
      .primary_source
      .as_ref()
      .is_some_and(|ours| ours.inner() == source)
    {
      self.primary_source = None;
      self.owned.remove(&Selection::Primary);
    }
  }
}

impl Dispatch2<wl_keyboard::WlKeyboard, State> for Raw {
  fn event(
    &self,
    state: &mut State,
    _: &wl_keyboard::WlKeyboard,
    event: wl_keyboard::Event,
    _: &Connection,
    _: &QueueHandle<State>,
  ) {
    match event {
      wl_keyboard::Event::Keymap { format, fd, size } => {
        if format != WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) {
          return;
        }

        // SAFETY: the compositor's keymap fd, mapped read-only for `size` bytes.
        let keymap = unsafe {
          xkb::Keymap::new_from_fd(
            &state.keyboard.context,
            fd,
            size as usize,
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
          )
        };

        if let Ok(Some(keymap)) = keymap {
          let xkb_state = xkb::State::new(&keymap);

          state.keyboard.set_keymap(keymap, xkb_state);
          trace::log(|| format!("[wayland] keymap {:?}", state.keyboard.layouts()));
        }
      }
      wl_keyboard::Event::Enter { serial, .. } => {
        state.last_serial = serial;
        state.shell.keyboard_focus(true);
      }
      wl_keyboard::Event::Leave { surface, .. } => {
        state.repeat = None;

        // A popup's grab takes the keyboard from the window to the popup, and the popup's
        // surface gives it back as it closes (KWin says so after the window has it again): the
        // window loses it only when it leaves the window's own surface with no popup open.
        if state.popup.is_none() && &surface == state.window.wl_surface() {
          state.shell.keyboard_focus(false);
        }
      }
      wl_keyboard::Event::Modifiers {
        mods_depressed,
        mods_latched,
        mods_locked,
        group,
        ..
      } => {
        state
          .keyboard
          .update_mask([mods_depressed, mods_latched, mods_locked], [0, 0, group]);

        state.shell.set_modifiers(state.keyboard.modifiers());
      }
      wl_keyboard::Event::RepeatInfo { rate, delay } => {
        state.keyboard.repeat = (
          u32::try_from(rate).unwrap_or(0),
          Duration::from_millis(u64::try_from(delay).unwrap_or(600)),
        );
      }
      wl_keyboard::Event::Key {
        key,
        state: key_state,
        serial,
        ..
      } => {
        let keycode = key + 8;
        let down = key_state == WEnum::Value(wl_keyboard::KeyState::Pressed);

        state.last_serial = serial;
        state.grab_serial = serial;

        if down {
          let (rate, delay) = state.keyboard.repeat;

          state.repeat = (rate > 0 && state.keyboard.repeats(keycode)).then(|| Repeat {
            keycode,
            next: Instant::now() + delay,
          });
        } else if state
          .repeat
          .as_ref()
          .is_some_and(|repeat| repeat.keycode == keycode)
        {
          state.repeat = None;
        }

        if let Some(input) = state.keyboard.key(keycode, down, false) {
          state.shell.key(&input);
        }
      }
      _ => {}
    }
  }
}

impl Dispatch2<WpFractionalScaleManagerV1, State> for Raw {
  fn event(
    &self,
    _: &mut State,
    _: &WpFractionalScaleManagerV1,
    _: <WpFractionalScaleManagerV1 as wayland_client::Proxy>::Event,
    _: &Connection,
    _: &QueueHandle<State>,
  ) {
  }
}

impl Dispatch2<WpFractionalScaleV1, State> for Raw {
  fn event(
    &self,
    state: &mut State,
    _: &WpFractionalScaleV1,
    event: wp_fractional_scale_v1::Event,
    _: &Connection,
    _: &QueueHandle<State>,
  ) {
    if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
      trace::log(|| format!("[wayland] fractional scale {}/120", scale));
      state.shell.set_scale(f64::from(scale) / 120.);
      crate::shell::invalidate_all();
    }
  }
}

impl Dispatch2<WpViewporter, State> for Raw {
  fn event(
    &self,
    _: &mut State,
    _: &WpViewporter,
    _: <WpViewporter as wayland_client::Proxy>::Event,
    _: &Connection,
    _: &QueueHandle<State>,
  ) {
  }
}

impl Dispatch2<WpViewport, State> for Raw {
  fn event(
    &self,
    _: &mut State,
    _: &WpViewport,
    _: <WpViewport as wayland_client::Proxy>::Event,
    _: &Connection,
    _: &QueueHandle<State>,
  ) {
  }
}

impl crate::ime::wayland::HasTextInput for State {
  fn text_input(&mut self) -> (&mut crate::ime::wayland::TextInput, &mut Shell) {
    (&mut self.text_input, &mut self.shell)
  }
}

impl PopupHandler for State {
  fn configure(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    popup: &Popup,
    config: PopupConfigure,
  ) {
    if let Some(open) = &mut self.popup
      && open.popup.wl_surface() == popup.wl_surface()
    {
      trace::log(|| {
        format!(
          "[wayland] popup configured at {:?} {}x{}",
          config.position, config.width, config.height
        )
      });
      open.configured = true;
      self
        .shell
        .popup_placed(config.position.0 as f32, config.position.1 as f32);
    }
  }

  fn done(&mut self, _: &Connection, _: &QueueHandle<Self>, popup: &Popup) {
    if self
      .popup
      .as_ref()
      .is_some_and(|open| open.popup.wl_surface() == popup.wl_surface())
    {
      trace::log(|| "[wayland] popup dismissed".to_owned());
      self.popup = None;
      self.shell.dismiss_popup();
    }
  }
}

delegate_registry!(State);

impl ProvidesRegistryState for State {
  fn registry(&mut self) -> &mut RegistryState {
    &mut self.registry
  }

  registry_handlers![OutputState, SeatState];
}

sctk::delegate_dispatch2!(State);
