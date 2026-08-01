//! Dev-loop state: restore from the CLI on launch, checkpoint back as state changes.
//!
//! The handshake pairs with the `skene` CLI's `run tui` loop: the CLI sets `SKENE_DEV`
//! to its state port, the app fetches `/skene:state` and stages it before mounting,
//! and checkpoints its snapshot back—debounced, off the UI thread—so the CLI can kill
//! and restart the process at any moment without losing state. Without `SKENE_DEV`
//! every call is a no-op.

use std::{
  cell::{Cell, RefCell},
  io::{Read, Write},
  net::TcpStream,
  sync::mpsc,
  time::{Duration, Instant},
};

/// The least time between two checkpoints.
const CHECKPOINT_EVERY: Duration = Duration::from_millis(200);

/// The dev channel, present when the CLI launched the app.
struct Channel {
  sender: mpsc::Sender<Vec<u8>>,
  changed: Cell<bool>,
  last: Cell<Instant>,
}

thread_local! {
  static CHANNEL: RefCell<Option<Channel>> = const { RefCell::new(None) };
}

/// Connects to the CLI when `SKENE_DEV` names its state port: restores the held
/// snapshot into the reactive runtime, and starts the checkpoint sender.
pub(crate) fn install() {
  let Some(port) = std::env::var("SKENE_DEV")
    .ok()
    .and_then(|value| value.parse::<u16>().ok())
  else {
    return;
  };

  if let Some(bytes) = http_get(port) {
    skene_reactive::restore(&bytes);
  }

  let (sender, receiver) = mpsc::channel::<Vec<u8>>();

  // Checkpoints post off the UI thread, so a slow write never stalls the terminal.
  std::thread::spawn(move || {
    while let Ok(bytes) = receiver.recv() {
      http_post(port, &bytes);
    }
  });

  CHANNEL.with(|channel| {
    *channel.borrow_mut() = Some(Channel {
      sender,
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

/// Checkpoints the current snapshot when the state changed and the debounce elapsed.
pub(crate) fn tick() {
  CHANNEL.with(|channel| {
    if let Some(channel) = channel.borrow().as_ref()
      && channel.changed.get()
      && channel.last.get().elapsed() >= CHECKPOINT_EVERY
    {
      channel.changed.set(false);
      channel.last.set(Instant::now());

      let _ = channel.sender.send(skene_reactive::snapshot());
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

/// The CLI-held snapshot, or `None` when there is none yet.
fn http_get(port: u16) -> Option<Vec<u8>> {
  let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;

  stream
    .write_all(
      b"GET /skene:state HTTP/1.1\r\n\
        Host: localhost\r\n\
        Connection: close\r\n\r\n",
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
