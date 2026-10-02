//! A WebSocket client on the framework's public seams alone, as [`http`](super::http)
//! is.
//!
//! [`connect`] is a future resolving to a [`WebSocket`], a message is queued for
//! sending at once, and [`receive`](WebSocket::receive) is a future a task awaits in a
//! loop, resolving with each message as it lands and with `None` once the socket has
//! closed. Nothing in the framework knows this crate exists.
//!
//! The transport is the platform's: the browser's `WebSocket` on the web, and on every
//! other host, a server render included, a plain client (`tungstenite`) on a thread of
//! its own that owns the socket, the future woken through its waker as messages arrive;
//! a socket opened during a render lives for that render. Dropping the socket closes
//! it.

use std::{
  collections::VecDeque,
  fmt,
  future::Future,
  pin::Pin,
  sync::{Arc, Mutex},
  task::{Context, Poll, Waker},
};

#[cfg(not(target_arch = "wasm32"))]
mod host;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod web;

#[cfg(not(target_arch = "wasm32"))]
use host as transport;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use web as transport;

/// One message either way.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub enum Message {
  /// Text.
  Text(String),
  /// Bytes.
  Binary(Vec<u8>),
}

impl From<&str> for Message {
  fn from(text: &str) -> Self {
    Self::Text(text.to_owned())
  }
}

impl From<String> for Message {
  #[inline(always)]
  fn from(text: String) -> Self {
    Self::Text(text)
  }
}

impl From<Vec<u8>> for Message {
  #[inline(always)]
  fn from(bytes: Vec<u8>) -> Self {
    Self::Binary(bytes)
  }
}

/// Why a socket could not be opened or used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
  /// The URL could not be used on this platform.
  Url(String),
  /// The connection failed on the way: refused, broken, or rejected by the server.
  Transport(String),
  /// The socket is closed, so nothing can be sent.
  Closed,
}

impl fmt::Display for Error {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Url(message) => write!(out, "the URL cannot be used: {message}"),
      Self::Transport(message) => write!(out, "the connection failed: {message}"),
      Self::Closed => out.write_str("the socket is closed"),
    }
  }
}

impl std::error::Error for Error {}

/// What has arrived and not yet been taken, and whether more can come.
struct Inbox {
  messages: VecDeque<Message>,
  closed: bool,
  waker: Option<Waker>,
}

/// The receiving end every transport delivers into: messages queued in order, the
/// waiting task woken as each lands, and a close after the last.
#[derive(Clone)]
pub(crate) struct Receiver(Arc<Mutex<Inbox>>);

impl Receiver {
  pub(crate) fn new() -> Self {
    Self(Arc::new(Mutex::new(Inbox {
      messages: VecDeque::new(),
      closed: false,
      waker: None,
    })))
  }

  /// Queues `message` and wakes the task waiting for it.
  pub(crate) fn deliver(&self, message: Message) {
    let waker = {
      let mut inbox = self
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

      inbox.messages.push_back(message);
      inbox.waker.take()
    };

    if let Some(waker) = waker {
      waker.wake();
    }
  }

  /// Marks the socket closed and wakes the task waiting, which then sees the end.
  pub(crate) fn close(&self) {
    let waker = {
      let mut inbox = self
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

      inbox.closed = true;
      inbox.waker.take()
    };

    if let Some(waker) = waker {
      waker.wake();
    }
  }

  /// Whether the socket has closed.
  pub(crate) fn is_closed(&self) -> bool {
    self
      .0
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .closed
  }
}

/// The next message, or `None` once the socket is closed and every message taken.
#[derive(Debug)]
#[must_use]
pub struct Receive<'a>(&'a WebSocket);

impl Future for Receive<'_> {
  type Output = Option<Message>;

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
    let mut inbox = self
      .0
      .receiver
      .0
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);

    if let Some(message) = inbox.messages.pop_front() {
      return Poll::Ready(Some(message));
    }

    if inbox.closed {
      return Poll::Ready(None);
    }

    inbox.waker = Some(context.waker().clone());

    Poll::Pending
  }
}

/// An open socket: messages sent at once, received one future at a time, closed on
/// drop.
#[must_use]
pub struct WebSocket {
  link: transport::Link,
  receiver: Receiver,
}

impl fmt::Debug for WebSocket {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    out
      .debug_struct("WebSocket")
      .field("closed", &self.is_closed())
      .finish_non_exhaustive()
  }
}

impl WebSocket {
  /// Queues `message` to be sent, as a browser's socket does; a failure on the way
  /// closes the socket.
  ///
  /// # Errors
  ///
  /// [`Error::Closed`] once the socket has closed, or a transport failure the platform
  /// reports at once.
  pub fn send(&self, message: impl Into<Message>) -> Result<(), Error> {
    if self.receiver.is_closed() {
      return Err(Error::Closed);
    }

    self.link.send(message.into())
  }

  /// The next message as it lands; `None` once the socket has closed and every
  /// message before the close has been taken.
  #[inline(always)]
  pub const fn receive(&self) -> Receive<'_> {
    Receive(self)
  }

  /// Whether the socket has closed, from either side.
  #[inline(always)]
  #[must_use]
  pub fn is_closed(&self) -> bool {
    self.receiver.is_closed()
  }

  /// Closes the socket; a message queued before the close is still received.
  #[inline(always)]
  pub fn close(&self) {
    self.link.close();
  }
}

/// Opens a socket to `url`, resolving once the connection is established.
///
/// `url` is a `ws://` or `wss://` URL, or an `http://` or `https://` one taken as the
/// socket URL a browser makes of it, and on the web a URL relative to the page.
///
/// # Errors
///
/// A URL the platform cannot open a socket to, or a connection that could not be made.
pub async fn connect(url: impl Into<String>) -> Result<WebSocket, Error> {
  let url = url.into();
  let receiver = Receiver::new();
  let link = transport::connect(url, receiver.clone()).await?;

  Ok(WebSocket { link, receiver })
}
