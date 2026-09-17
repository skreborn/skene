//! The transport on a host: a plain client on a thread of its own reading messages
//! into the receiver, and sends written from the caller's thread between its reads.

use std::{
  future::Future,
  net::TcpStream,
  pin::Pin,
  sync::{Arc, Mutex},
  task::{Context, Poll, Waker},
  thread,
  time::Duration,
};

use tungstenite::{Message as Frame, WebSocket as Socket, stream::MaybeTlsStream};

use super::{Error, Message, Receiver};

/// How long one read waits before yielding the socket to a send.
const READ_SLICE: Duration = Duration::from_millis(50);

/// The socket shared between the reading thread and the sender.
type Shared = Arc<Mutex<Option<Socket<MaybeTlsStream<TcpStream>>>>>;

/// The sending half: the socket, taken between the reader's slices.
pub(crate) struct Link {
  socket: Shared,
}

impl Link {
  pub(crate) fn send(&self, message: Message) -> Result<(), Error> {
    let mut socket = self
      .socket
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(socket) = socket.as_mut()
    else {
      return Err(Error::Closed);
    };
    let frame = match message {
      Message::Text(text) => Frame::text(text),
      Message::Binary(bytes) => Frame::binary(bytes),
    };

    socket
      .send(frame)
      .map_err(|error| Error::Transport(error.to_string()))
  }

  pub(crate) fn close(&self) {
    let mut socket = self
      .socket
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);

    if let Some(socket) = socket.as_mut() {
      let _ = socket.close(None);
      let _ = socket.flush();
    }
  }
}

impl Drop for Link {
  fn drop(&mut self) {
    self.close();
  }
}

/// What the connecting thread and the future share.
struct Opening {
  outcome: Option<Result<Link, Error>>,
  waker: Option<Waker>,
}

/// A connection being opened on its thread.
struct Connect(Arc<Mutex<Opening>>);

impl Future for Connect {
  type Output = Result<Link, Error>;

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
    let mut opening = self
      .0
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);

    if let Some(outcome) = opening.outcome.take() {
      return Poll::Ready(outcome);
    }

    opening.waker = Some(context.waker().clone());

    Poll::Pending
  }
}

/// Opens `url` on a thread that then reads every message into `receiver` until the
/// socket closes.
pub(crate) fn connect(
  url: String,
  receiver: Receiver,
) -> impl Future<Output = Result<Link, Error>> {
  let opening = Arc::new(Mutex::new(Opening {
    outcome: None,
    waker: None,
  }));
  let worker = Arc::clone(&opening);

  thread::spawn(move || {
    let outcome = open(&url).map(|socket| {
      let shared: Shared = Arc::new(Mutex::new(Some(socket)));

      read_all(Arc::clone(&shared), receiver);

      Link { socket: shared }
    });
    let waker = {
      let mut opening = worker
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

      opening.outcome = Some(outcome);
      opening.waker.take()
    };

    if let Some(waker) = waker {
      waker.wake();
    }
  });

  Connect(opening)
}

/// The handshake, with the stream set to yield each read after a slice.
fn open(url: &str) -> Result<Socket<MaybeTlsStream<TcpStream>>, Error> {
  let (socket, _response) =
    tungstenite::connect(url).map_err(|error| Error::Transport(error.to_string()))?;
  let stream = match socket.get_ref() {
    MaybeTlsStream::Plain(stream) => stream,
    MaybeTlsStream::Rustls(stream) => stream.get_ref(),
    _ => return Err(Error::Transport("an unknown stream".to_owned())),
  };

  stream
    .set_read_timeout(Some(READ_SLICE))
    .map_err(|error| Error::Transport(error.to_string()))?;

  Ok(socket)
}

/// Reads on a thread until the socket closes, delivering each message and releasing
/// the socket between slices so a send gets its turn.
fn read_all(shared: Shared, receiver: Receiver) {
  thread::spawn(move || {
    loop {
      let outcome = {
        let mut socket = shared
          .lock()
          .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(socket) = socket.as_mut()
        else {
          break;
        };

        socket.read()
      };

      match outcome {
        Ok(Frame::Text(text)) => receiver.deliver(Message::Text(text.to_string())),
        Ok(Frame::Binary(bytes)) => receiver.deliver(Message::Binary(bytes.to_vec())),
        Ok(Frame::Ping(_) | Frame::Pong(_) | Frame::Frame(_)) => {}
        Err(tungstenite::Error::Io(error))
          if matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
          ) =>
        {
          thread::yield_now();
        }
        Ok(Frame::Close(_)) | Err(_) => break,
      }
    }

    shared
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .take();
    receiver.close();
  });
}
