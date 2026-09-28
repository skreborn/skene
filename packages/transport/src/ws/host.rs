//! The transport on a host: a plain client on one thread of its own, which owns the
//! socket, reads its messages into the receiver and writes what the sender queues
//! between its reads.

use std::{
  net::TcpStream,
  sync::mpsc,
  thread,
  time::{Duration, Instant},
};

use tungstenite::{
  Connector,
  Message as Frame,
  WebSocket as Socket,
  client::IntoClientRequest as _,
  stream::MaybeTlsStream,
};

use super::{Error, Message, Receiver};
use crate::host;

/// How long one read waits before the thread writes what was queued meanwhile.
const READ_SLICE: Duration = Duration::from_millis(10);

/// How long a closing socket waits for its peer to finish the close.
const CLOSE_WAIT: Duration = Duration::from_secs(5);

/// What the sender asks of the socket's thread.
enum Command {
  Send(Frame),
  Close,
}

/// The sending half: the queue to the socket's thread.
pub(crate) struct Link {
  commands: mpsc::Sender<Command>,
}

impl Link {
  pub(crate) fn send(&self, message: Message) -> Result<(), Error> {
    let frame = match message {
      Message::Text(text) => Frame::text(text),
      Message::Binary(bytes) => Frame::binary(bytes),
    };

    self
      .commands
      .send(Command::Send(frame))
      .map_err(|_| Error::Closed)
  }

  pub(crate) fn close(&self) {
    let _ = self.commands.send(Command::Close);
  }
}

impl Drop for Link {
  fn drop(&mut self) {
    self.close();
  }
}

/// Opens `url` on a thread that then owns the socket, reading every message into
/// `receiver` and writing what the link queues until the socket closes.
pub(crate) async fn connect(url: String, receiver: Receiver) -> Result<Link, Error> {
  let (commands, queue) = mpsc::channel();
  let (settler, promise) = host::one_shot(Err(Error::Transport(
    "the socket's thread stopped before it opened".to_owned(),
  )));

  thread::spawn(move || {
    let socket = match open(&url) {
      Ok(socket) => socket,
      Err(error) => {
        settler.settle(Err(error));

        return;
      }
    };

    settler.settle(Ok(()));
    serve(socket, &queue, &receiver);
  });

  promise.await?;

  Ok(Link { commands })
}

/// The handshake with `url`, a `http` or `https` URL taken as the `ws` or `wss` one a
/// browser makes of it, over the shared TLS configuration; the stream then yields each
/// read after a slice.
fn open(url: &str) -> Result<Socket<MaybeTlsStream<TcpStream>>, Error> {
  let url = if let Some(rest) = url.strip_prefix("http://") {
    format!("ws://{rest}")
  }
  else if let Some(rest) = url.strip_prefix("https://") {
    format!("wss://{rest}")
  }
  else {
    url.to_owned()
  };
  let request = url
    .as_str()
    .into_client_request()
    .map_err(|error| Error::Url(error.to_string()))?;
  let secure = match request.uri().scheme_str() {
    Some("ws") => false,
    Some("wss") => true,
    _ => {
      return Err(Error::Url(format!(
        "{url:?} is not an absolute ws or wss URL"
      )));
    }
  };
  let host = request
    .uri()
    .host()
    .filter(|host| !host.is_empty())
    .ok_or_else(|| Error::Url(format!("{url:?} names no host")))?;
  let host = host
    .strip_prefix('[')
    .and_then(|host| host.strip_suffix(']'))
    .unwrap_or(host)
    .to_owned();
  let port = request
    .uri()
    .port_u16()
    .unwrap_or(if secure { 443 } else { 80 });
  let stream = TcpStream::connect((host.as_str(), port))
    .map_err(|error| Error::Transport(error.to_string()))?;
  let connector = if secure {
    Connector::Rustls(host::tls_config().map_err(|error| Error::Transport(error.to_string()))?)
  }
  else {
    Connector::Plain
  };
  let (socket, _response) =
    tungstenite::client_tls_with_config(request, stream, None, Some(connector))
      .map_err(|error| Error::Transport(error.to_string()))?;
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

/// Serves the socket until it closes: writes what was queued, then reads for a slice,
/// delivering each message. A close, or the link gone, closes the socket once, and a
/// peer that has not finished the close within [`CLOSE_WAIT`] is let go.
fn serve(
  mut socket: Socket<MaybeTlsStream<TcpStream>>,
  queue: &mpsc::Receiver<Command>,
  receiver: &Receiver,
) {
  let mut closing: Option<Instant> = None;

  loop {
    loop {
      match queue.try_recv() {
        Ok(Command::Send(frame)) => {
          if closing.is_none() && socket.send(frame).is_err() {
            receiver.close();

            return;
          }
        }
        Ok(Command::Close) | Err(mpsc::TryRecvError::Disconnected) if closing.is_none() => {
          closing = Some(Instant::now());

          let _ = socket.close(None);
          let _ = socket.flush();
        }
        Ok(Command::Close) => {}
        Err(_) => break,
      }
    }

    if closing.is_some_and(|since| since.elapsed() > CLOSE_WAIT) {
      break;
    }

    match socket.read() {
      Ok(Frame::Text(text)) => receiver.deliver(Message::Text(text.to_string())),
      Ok(Frame::Binary(bytes)) => receiver.deliver(Message::Binary(bytes.to_vec())),
      Ok(Frame::Ping(_) | Frame::Pong(_) | Frame::Frame(_)) => {}
      Err(tungstenite::Error::Io(error))
        if matches!(
          error.kind(),
          std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ) =>
      {
        let _ = socket.flush();
      }
      Ok(Frame::Close(_)) | Err(_) => break,
    }
  }

  receiver.close();
}
