//! Dev-loop state: restore from the CLI on launch, checkpoint back as state changes.
//!
//! The handshake pairs with the `skene` CLI's `run tui` loop: the CLI sets `SKENE_DEV`
//! to its state port, the app fetches `/skene:state` and stages it before mounting,
//! and checkpoints its snapshot back—debounced, off the UI thread—so the CLI can kill
//! and restart the process at any moment without losing state. The app also polls
//! `/skene:notice` for the CLI's rebuild failures, presenting each in a dismissable
//! modal. Without `SKENE_DEV` every call is a no-op.

use std::{
  cell::{Cell, RefCell},
  io::{Read, Write},
  net::TcpStream,
  sync::mpsc,
  time::{Duration, Instant},
};

use skene_reactive::{Get, Set, Signal};

/// The least time between two checkpoints.
const CHECKPOINT_EVERY: Duration = Duration::from_millis(200);

/// The time between two notice polls.
const NOTICE_EVERY: Duration = Duration::from_millis(500);

/// The snapshot tag carrying every scrolled node's offsets.
const SCROLL_TAG: &str = "skene-tui:scroll";

/// The dev channel, present when the CLI launched the app.
struct Channel {
  sender: mpsc::Sender<Vec<u8>>,
  notices: mpsc::Receiver<Option<String>>,
  notice: Signal<Option<String>>,
  changed: Cell<bool>,
  last: Cell<Instant>,
}

thread_local! {
  static CHANNEL: RefCell<Option<Channel>> = const { RefCell::new(None) };
}

/// Connects to the CLI when `SKENE_DEV` names its state port: restores the held
/// snapshot into the reactive runtime, and starts the checkpoint sender and the
/// notice poller.
pub(crate) fn install() {
  let Some(port) = std::env::var("SKENE_DEV")
    .ok()
    .and_then(|value| value.parse::<u16>().ok())
  else {
    return;
  };

  if let Some(bytes) = http_get(port, "/skene:state") {
    skene_reactive::restore(&bytes);
  }

  let (sender, receiver) = mpsc::channel::<Vec<u8>>();

  // Checkpoints post off the UI thread, so a slow write never stalls the terminal.
  std::thread::spawn(move || {
    while let Ok(bytes) = receiver.recv() {
      http_post(port, &bytes);
    }
  });

  let (notice_sender, notices) = mpsc::channel::<Option<String>>();

  // The notice poller: posts each change of the CLI's rebuild notice to the UI loop.
  std::thread::spawn(move || {
    let mut last: Option<Vec<u8>> = None;

    loop {
      std::thread::sleep(NOTICE_EVERY);

      let Some(body) = http_get(port, "/skene:notice")
      else {
        continue;
      };

      if last.as_ref() == Some(&body) {
        continue;
      }

      let text = std::str::from_utf8(&body)
        .ok()
        .and_then(|body| body.split_once('\n'))
        .map(|(_, text)| text.trim_end().to_owned())
        .filter(|text| !text.is_empty());

      if notice_sender.send(text).is_err() {
        return;
      }

      last = Some(body);
    }
  });

  CHANNEL.with(|channel| {
    *channel.borrow_mut() = Some(Channel {
      sender,
      notices,
      notice: Signal::new(None),
      changed: Cell::new(false),
      last: Cell::new(Instant::now()),
    });
  });
}

/// Marks the state changed, so the next [`tick`] checkpoints it.
pub(crate) fn mark_changed() {
  CHANNEL.with(|channel| {
    if let Some(channel) = channel.borrow().as_ref() {
      channel.changed.set(true);
    }
  });
}

/// Applies polled notices and checkpoints the current snapshot when the state changed
/// and the debounce elapsed.
pub(crate) fn tick() {
  CHANNEL.with(|channel| {
    if let Some(channel) = channel.borrow().as_ref() {
      while let Ok(update) = channel.notices.try_recv() {
        channel.notice.set(update);
      }

      if channel.changed.get() && channel.last.get().elapsed() >= CHECKPOINT_EVERY {
        channel.changed.set(false);
        channel.last.set(Instant::now());

        let _ = channel.sender.send(skene_reactive::snapshot());
      }
    }
  });
}

/// Checkpoints unconditionally and synchronously, for a clean exit.
pub(crate) fn flush() {
  let posted = CHANNEL.with(|channel| {
    channel.borrow().as_ref().map(|channel| {
      let _ = channel.sender.send(skene_reactive::snapshot());
    })
  });

  if posted.is_some() {
    // Give the sender thread a moment to drain; the process is about to exit.
    std::thread::sleep(Duration::from_millis(50));
  }
}

/// Registers the driver's scroll offsets as a snapshot provider; the entry rides
/// every snapshot alongside the tagged signals.
pub(crate) fn provide_scroll() {
  skene_reactive::provide(SCROLL_TAG, || {
    crate::try_with_state(|state| {
      let entries = state.scroll_snapshot();

      (!entries.is_empty()).then(|| postcard::to_allocvec(&entries).unwrap_or_default())
    })
    .flatten()
  });
}

/// Adopts the scroll offsets a restore staged, once the tree is mounted.
pub fn adopt_scroll() {
  if let Some(bytes) = skene_reactive::staged(SCROLL_TAG)
    && let Ok(entries) = postcard::from_bytes::<Vec<(u64, u32, u32)>>(&bytes)
  {
    crate::with_state(|state| state.restore_scroll(&entries));
  }
}

/// The rebuild-notice signal, present when the CLI launched the app.
pub(crate) fn notice_signal() -> Option<Signal<Option<String>>> {
  CHANNEL.with(|channel| channel.borrow().as_ref().map(|channel| channel.notice))
}

/// The dev overlay presenting the CLI's rebuild failures: a modal over the app while
/// `notice` holds one, dismissed back to `None` by Escape or a press outside.
#[must_use]
pub fn notice_view(notice: Signal<Option<String>>) -> impl crate::View {
  use skene_core::{Color, HasChildren};

  let dark = crate::with_state(|state| state.dark.get());
  let fill = if dark {
    Color::srgb(0., 0., 0.)
  }
  else {
    Color::srgb(1., 1., 1.)
  };

  crate::Overlay::modal()
    .derive_open(move || notice.get().is_some())
    .handle_dismiss(move || notice.set(None))
    .set_style(
      crate::props::BoxStyle::new()
        .set_color(fill)
        .set_border(Color::srgb(0.8, 0.2, 0.2))
        .set_padding(crate::Inset {
          top: 1,
          right: 2,
          bottom: 1,
          left: 2,
        })
        .set_max_width(76),
    )
    .set_child(
      crate::Column::new().set_spacing(1).set_children((
        crate::Text::new("Rebuild failed")
          .set_bold(true)
          .set_color(Color::srgb(0.8, 0.2, 0.2)),
        crate::Text::derive(move || notice.get().unwrap_or_default()).set_selectable(true),
        crate::Text::new("Esc to dismiss").set_color(Color::srgb(0.5, 0.5, 0.5)),
      )),
    )
}

/// The CLI's response body at `path`, or `None` when it is unreachable or not `200`.
fn http_get(port: u16, path: &str) -> Option<Vec<u8>> {
  let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;

  stream
    .write_all(
      format!(
        "GET {path} HTTP/1.1\r\n\
         Host: localhost\r\n\
         Connection: close\r\n\r\n"
      )
      .as_bytes(),
    )
    .ok()?;

  let mut response = Vec::new();

  stream.read_to_end(&mut response).ok()?;

  let split = response
    .windows(4)
    .position(|window| window == b"\r\n\r\n")?;
  let head = std::str::from_utf8(&response[..split]).ok()?;

  if !head.starts_with("HTTP/1.1 200") {
    return None;
  }

  Some(response[split + 4..].to_vec())
}

/// Posts `bytes` as the CLI-held snapshot.
fn http_post(port: u16, bytes: &[u8]) {
  let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port))
  else {
    return;
  };

  let header = format!(
    "POST /skene:state HTTP/1.1\r\n\
     Host: localhost\r\n\
     Content-Length: {}\r\n\
     Connection: close\r\n\r\n",
    bytes.len()
  );

  let _ = stream.write_all(header.as_bytes());
  let _ = stream.write_all(bytes);
  let _ = stream.flush();

  // Wait for the response so the CLI surely stored the bytes before a final exit.
  let mut response = Vec::new();
  let _ = stream.read_to_end(&mut response);
}
