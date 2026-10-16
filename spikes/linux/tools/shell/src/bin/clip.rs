//! A clipboard client beside the app, for the realization's clipboard checks: it reads or
//! offers the clipboard or the primary selection, as another application does.
//!
//! ```text
//! clip x11 read clipboard|primary
//! clip x11 write clipboard|primary TEXT [SECONDS]
//! clip wlr read clipboard|primary
//! clip wlr write clipboard|primary TEXT [SECONDS]
//! ```
//!
//! `x11` speaks ICCCM selections on `$DISPLAY`; `wlr` speaks wlr-data-control on
//! `$WAYLAND_DISPLAY` (sway), which needs no focus. A read prints the text, or `(none)`.

use std::{
  io::{Read as _, Write as _},
  os::fd::{AsFd as _, FromRawFd as _, OwnedFd},
  time::{Duration, Instant},
};

use wayland_client::{
  Connection, Dispatch, Proxy, QueueHandle, event_created_child,
  globals::{GlobalListContents, registry_queue_init},
  protocol::{wl_registry, wl_seat::WlSeat},
};
use wayland_protocols_wlr::data_control::v1::client::{
  zwlr_data_control_device_v1::{self, ZwlrDataControlDeviceV1},
  zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
  zwlr_data_control_offer_v1::{self, ZwlrDataControlOfferV1},
  zwlr_data_control_source_v1::{self, ZwlrDataControlSourceV1},
};
use x11rb::{
  connection::Connection as _,
  protocol::{
    Event,
    xproto::{
      AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, SelectionNotifyEvent,
      WindowClass,
    },
  },
  wrapper::ConnectionExt as _,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const MIME: &str = "text/plain;charset=utf-8";

fn x11(action: &str, which: &str, text: Option<&str>, seconds: u64) -> Result<()> {
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
    &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
  )?;

  let atom =
    |name: &str| -> Result<u32> { Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom) };
  let selection = atom(if which == "primary" {
    "PRIMARY"
  } else {
    "CLIPBOARD"
  })?;
  let utf8 = atom("UTF8_STRING")?;
  let targets = atom("TARGETS")?;
  let property = atom("CLIP_RESULT")?;

  if action == "read" {
    conn.convert_selection(window, selection, utf8, property, x11rb::CURRENT_TIME)?;
    conn.flush()?;

    let started = Instant::now();

    while started.elapsed() < Duration::from_secs(3) {
      if let Some(Event::SelectionNotify(event)) = conn.poll_for_event()? {
        if event.property == x11rb::NONE {
          println!("(none)");
        } else {
          let reply = conn
            .get_property(true, window, property, AtomEnum::ANY, 0, u32::MAX / 4)?
            .reply()?;

          println!("{}", String::from_utf8_lossy(&reply.value));
        }

        return Ok(());
      }

      std::thread::sleep(Duration::from_millis(5));
    }

    println!("(timeout)");

    return Ok(());
  }

  let text = text.unwrap_or_default();

  conn.set_selection_owner(window, selection, x11rb::CURRENT_TIME)?;
  conn.flush()?;
  println!("owning");

  let started = Instant::now();

  while started.elapsed() < Duration::from_secs(seconds) {
    match conn.poll_for_event()? {
      Some(Event::SelectionRequest(request)) => {
        let property = if request.property == x11rb::NONE {
          request.target
        } else {
          request.property
        };
        let answered = if request.target == targets {
          conn.change_property32(
            PropMode::REPLACE,
            request.requestor,
            property,
            AtomEnum::ATOM,
            &[targets, utf8],
          )?;
          true
        } else if request.target == utf8 {
          conn.change_property8(
            PropMode::REPLACE,
            request.requestor,
            property,
            utf8,
            text.as_bytes(),
          )?;
          true
        } else {
          false
        };

        conn.send_event(
          false,
          request.requestor,
          EventMask::NO_EVENT,
          SelectionNotifyEvent {
            response_type: x11rb::protocol::xproto::SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: if answered { property } else { x11rb::NONE },
          },
        )?;
        conn.flush()?;
        println!("served");
      }
      Some(Event::SelectionClear(_)) => {
        println!("lost");

        return Ok(());
      }
      _ => std::thread::sleep(Duration::from_millis(5)),
    }
  }

  Ok(())
}

#[derive(Default)]
struct Wlr {
  offers: Vec<(ZwlrDataControlOfferV1, Vec<String>)>,
  clipboard: Option<ZwlrDataControlOfferV1>,
  primary: Option<ZwlrDataControlOfferV1>,
  text: String,
  served: bool,
  cancelled: bool,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Wlr {
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

impl Dispatch<WlSeat, ()> for Wlr {
  fn event(
    _: &mut Self,
    _: &WlSeat,
    _: <WlSeat as Proxy>::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
  }
}

impl Dispatch<ZwlrDataControlManagerV1, ()> for Wlr {
  fn event(
    _: &mut Self,
    _: &ZwlrDataControlManagerV1,
    _: <ZwlrDataControlManagerV1 as Proxy>::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
  }
}

impl Dispatch<ZwlrDataControlDeviceV1, ()> for Wlr {
  fn event(
    state: &mut Self,
    _: &ZwlrDataControlDeviceV1,
    event: zwlr_data_control_device_v1::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match event {
      zwlr_data_control_device_v1::Event::DataOffer { id } => state.offers.push((id, Vec::new())),
      zwlr_data_control_device_v1::Event::Selection { id } => state.clipboard = id,
      zwlr_data_control_device_v1::Event::PrimarySelection { id } => state.primary = id,
      _ => {}
    }
  }

  event_created_child!(Wlr, ZwlrDataControlDeviceV1, [
    zwlr_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ZwlrDataControlOfferV1, ()),
  ]);
}

impl Dispatch<ZwlrDataControlOfferV1, ()> for Wlr {
  fn event(
    state: &mut Self,
    offer: &ZwlrDataControlOfferV1,
    event: zwlr_data_control_offer_v1::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    if let zwlr_data_control_offer_v1::Event::Offer { mime_type } = event
      && let Some((_, types)) = state.offers.iter_mut().find(|(known, _)| known == offer)
    {
      types.push(mime_type);
    }
  }
}

impl Dispatch<ZwlrDataControlSourceV1, ()> for Wlr {
  fn event(
    state: &mut Self,
    _: &ZwlrDataControlSourceV1,
    event: zwlr_data_control_source_v1::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match event {
      zwlr_data_control_source_v1::Event::Send { fd, .. } => {
        let mut file = std::fs::File::from(fd);
        let _ = file.write_all(state.text.as_bytes());

        state.served = true;
        println!("served");
      }
      zwlr_data_control_source_v1::Event::Cancelled => state.cancelled = true,
      _ => {}
    }
  }
}

fn wlr(action: &str, which: &str, text: Option<&str>, seconds: u64) -> Result<()> {
  let conn = Connection::connect_to_env()?;
  let (globals, mut queue) = registry_queue_init::<Wlr>(&conn)?;
  let qh = queue.handle();
  let seat: WlSeat = globals.bind(&qh, 1..=8, ())?;
  let manager: ZwlrDataControlManagerV1 = globals.bind(&qh, 1..=2, ())?;
  let device = manager.get_data_device(&seat, &qh, ());
  let mut state = Wlr::default();

  queue.roundtrip(&mut state)?;

  if action == "read" {
    let offer = if which == "primary" {
      state.primary.clone()
    } else {
      state.clipboard.clone()
    };
    let Some(offer) = offer else {
      println!("(none)");

      return Ok(());
    };
    let mut fds = [0; 2];

    // SAFETY: `fds` is two integers for pipe2 to fill.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
      return Err("pipe".into());
    }

    // SAFETY: both ends were just created and are owned here.
    let (read, write) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };

    offer.receive(MIME.to_owned(), write.as_fd());
    drop(write);
    queue.roundtrip(&mut state)?;

    let mut text = String::new();

    std::fs::File::from(read).read_to_string(&mut text)?;
    println!("{text}");

    return Ok(());
  }

  state.text = text.unwrap_or_default().to_owned();

  let source = manager.create_data_source(&qh, ());

  source.offer(MIME.to_owned());
  source.offer("text/plain".to_owned());

  if which == "primary" {
    device.set_primary_selection(Some(&source));
  } else {
    device.set_selection(Some(&source));
  }

  queue.roundtrip(&mut state)?;
  println!("owning");

  let started = Instant::now();

  while started.elapsed() < Duration::from_secs(seconds) && !state.cancelled {
    queue.flush()?;

    if let Some(guard) = queue.prepare_read() {
      let fd = std::os::fd::AsRawFd::as_raw_fd(&guard.connection_fd());
      let mut pollfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
      };

      // SAFETY: polls one live descriptor.
      unsafe {
        libc::poll(&mut pollfd, 1, 50);
      }

      if pollfd.revents & libc::POLLIN != 0 {
        guard.read()?;
      }
    }

    queue.dispatch_pending(&mut state)?;
  }

  if state.cancelled {
    println!("lost");
  }

  Ok(())
}

fn main() {
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  let get = |index: usize| arguments.get(index).map(String::as_str);
  let seconds = get(4).and_then(|value| value.parse().ok()).unwrap_or(10);
  let result = match (get(0), get(1), get(2)) {
    (Some("x11"), Some(action), Some(which)) => x11(action, which, get(3), seconds),
    (Some("wlr"), Some(action), Some(which)) => wlr(action, which, get(3), seconds),
    _ => Err("usage: clip x11|wlr read|write clipboard|primary [TEXT [SECONDS]]".into()),
  };

  if let Err(error) = result {
    println!("FAIL {error}");
    std::process::exit(1);
  }
}
