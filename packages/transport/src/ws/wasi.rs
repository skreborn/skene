//! The transport on a server render: a TCP connection through `wasi:sockets`, TLS
//! through rustls for `wss://`, the handshake and the frames through a plain client
//! (`tungstenite`) over it, each wait on the host registered with the server leaf's
//! reactor, and the frames read as the receiving future is polled.

use std::{
  cell::RefCell,
  future::Future,
  io::{self, Read, Write},
  net::IpAddr,
  pin::Pin,
  rc::Rc,
  sync::Arc,
  task::{Context, Poll},
};

use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned, pki_types::ServerName};
use tungstenite::{
  HandshakeError,
  Message as Frame,
  WebSocket as Socket,
  handshake::{MidHandshake, client::ClientHandshake},
};
use wasi::{
  io::{
    poll::Pollable,
    streams::{InputStream, OutputStream, StreamError},
  },
  sockets::{
    instance_network::instance_network,
    ip_name_lookup,
    network::{
      IpAddress,
      IpAddressFamily,
      IpSocketAddress,
      Ipv4SocketAddress,
      Ipv6SocketAddress,
      Network,
    },
    tcp::{ShutdownType, TcpSocket},
    tcp_create_socket::create_tcp_socket,
  },
};

use super::{Error, Message, Receiver, url};

/// How many bytes one read asks the host for.
const READ: u64 = 64 * 1024;

/// The most one blocking write hands the host at once.
const WRITE: usize = 4096;

/// A connection's streams as a blocking writer and a non-blocking reader: a read with
/// nothing arrived fails with `WouldBlock`, and the input's pollable says when to
/// try again. The streams are dropped before the socket they belong to.
struct Tcp {
  input: InputStream,
  output: OutputStream,
  socket: TcpSocket,
}

impl Tcp {
  /// The pollable ready once a read would return bytes or the end.
  fn readable(&self) -> Pollable {
    self.input.subscribe()
  }
}

impl Read for Tcp {
  fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
    let wanted = u64::try_from(buffer.len()).unwrap_or(READ).min(READ);

    match self.input.read(wanted) {
      Ok(chunk) if chunk.is_empty() => Err(io::ErrorKind::WouldBlock.into()),
      Ok(chunk) => {
        buffer[..chunk.len()].copy_from_slice(&chunk);

        Ok(chunk.len())
      }
      Err(StreamError::Closed) => Ok(0),
      Err(StreamError::LastOperationFailed(error)) => {
        Err(io::Error::other(error.to_debug_string()))
      }
    }
  }
}

impl Write for Tcp {
  fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
    for chunk in buffer.chunks(WRITE) {
      self
        .output
        .blocking_write_and_flush(chunk)
        .map_err(|error| match error {
          StreamError::Closed => io::ErrorKind::BrokenPipe.into(),
          StreamError::LastOperationFailed(error) => io::Error::other(error.to_debug_string()),
        })?;
    }

    Ok(buffer.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

/// The connection, plain or under TLS.
enum Stream {
  Plain(Tcp),
  Tls(Box<StreamOwned<ClientConnection, Tcp>>),
}

impl Stream {
  fn tcp(&self) -> &Tcp {
    match self {
      Self::Plain(tcp) => tcp,
      Self::Tls(stream) => &stream.sock,
    }
  }
}

impl Read for Stream {
  fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
    match self {
      Self::Plain(tcp) => tcp.read(buffer),
      Self::Tls(stream) => stream.read(buffer),
    }
  }
}

impl Write for Stream {
  fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
    match self {
      Self::Plain(tcp) => tcp.write(buffer),
      Self::Tls(stream) => stream.write(buffer),
    }
  }

  fn flush(&mut self) -> io::Result<()> {
    match self {
      Self::Plain(tcp) => tcp.flush(),
      Self::Tls(stream) => stream.flush(),
    }
  }
}

/// The socket shared between the sender and the receiving future.
type Shared = Rc<RefCell<Option<Socket<Stream>>>>;

/// The sending half, and the reader the receiving future pumps.
pub(crate) struct Link {
  socket: Shared,
  receiver: Receiver,
}

impl Link {
  pub(crate) fn send(&self, message: Message) -> Result<(), Error> {
    let mut socket = self.socket.borrow_mut();
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
    if let Some(mut socket) = self.socket.borrow_mut().take() {
      let _ = socket.close(None);
      let _ = socket.flush();
      let _ = socket.get_ref().tcp().socket.shutdown(ShutdownType::Both);
    }

    self.receiver.close();
  }

  /// Reads every frame that has arrived into the receiver, registering the next
  /// arrival with the reactor when nothing was there; a close or a failure ends the
  /// receiving.
  pub(crate) fn pump(&self, context: &mut Context<'_>) {
    let mut delivered = false;

    loop {
      let outcome = {
        let mut socket = self.socket.borrow_mut();
        let Some(socket) = socket.as_mut()
        else {
          return;
        };

        socket.read()
      };

      match outcome {
        Ok(Frame::Text(text)) => {
          self.receiver.deliver(Message::Text(text.to_string()));
          delivered = true;
        }
        Ok(Frame::Binary(bytes)) => {
          self.receiver.deliver(Message::Binary(bytes.to_vec()));
          delivered = true;
        }
        Ok(Frame::Ping(_) | Frame::Pong(_) | Frame::Frame(_)) => {}
        Err(tungstenite::Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {
          if !delivered {
            let socket = self.socket.borrow();

            if let Some(socket) = socket.as_ref() {
              skene_web::register(socket.get_ref().tcp().readable(), context.waker().clone());
            }
          }

          return;
        }
        Ok(Frame::Close(_)) | Err(_) => {
          self.close();

          return;
        }
      }
    }
  }
}

impl Drop for Link {
  fn drop(&mut self) {
    self.close();
  }
}

/// One step of a host operation: its result, or the pollable to wait on before trying
/// again.
enum Step<T> {
  Ready(T),
  Wait(Pollable),
}

/// A host operation tried on every poll until it is ready, each wait registered with
/// the reactor.
struct Retry<F>(F);

impl<F, T> Future for Retry<F>
where
  F: FnMut() -> Result<Step<T>, Error> + Unpin,
{
  type Output = Result<T, Error>;

  fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
    match (self.0)() {
      Ok(Step::Ready(value)) => Poll::Ready(Ok(value)),
      Ok(Step::Wait(pollable)) => {
        skene_web::register(pollable, context.waker().clone());

        Poll::Pending
      }
      Err(error) => Poll::Ready(Err(error)),
    }
  }
}

/// Opens `url`: the host resolved, the connection made, TLS on it for `wss://`, and
/// the handshake done, each wait registered with the reactor.
pub(crate) async fn connect(url: String, receiver: Receiver) -> Result<Link, Error> {
  let parts = url::parts(&url)?;
  let network = instance_network();
  let address = resolve(&network, &parts.host).await?;
  let tcp = open(&network, address, parts.port).await?;
  let stream = if parts.secure {
    Stream::Tls(Box::new(secure(tcp, &parts.host)?))
  }
  else {
    Stream::Plain(tcp)
  };
  let socket = handshake(&url, stream).await?;

  Ok(Link {
    socket: Rc::new(RefCell::new(Some(socket))),
    receiver,
  })
}

/// The first address `host` resolves to, or the literal it is.
async fn resolve(network: &Network, host: &str) -> Result<IpAddress, Error> {
  if let Ok(literal) = host.parse::<IpAddr>() {
    return Ok(match literal {
      IpAddr::V4(address) => {
        let octets = address.octets();

        IpAddress::Ipv4((octets[0], octets[1], octets[2], octets[3]))
      }
      IpAddr::V6(address) => {
        let segments = address.segments();

        IpAddress::Ipv6((
          segments[0],
          segments[1],
          segments[2],
          segments[3],
          segments[4],
          segments[5],
          segments[6],
          segments[7],
        ))
      }
    });
  }

  let addresses = ip_name_lookup::resolve_addresses(network, host)
    .map_err(|error| Error::Transport(format!("{host}: {error:?}")))?;

  Retry(|| match addresses.resolve_next_address() {
    Ok(Some(address)) => Ok(Step::Ready(address)),
    Ok(None) => Err(Error::Transport(format!("{host} has no address"))),
    Err(wasi::sockets::network::ErrorCode::WouldBlock) => Ok(Step::Wait(addresses.subscribe())),
    Err(error) => Err(Error::Transport(format!("{host}: {error:?}"))),
  })
  .await
}

/// A connection to `address` at `port`.
async fn open(network: &Network, address: IpAddress, port: u16) -> Result<Tcp, Error> {
  let (family, remote) = match address {
    IpAddress::Ipv4(address) => (
      IpAddressFamily::Ipv4,
      IpSocketAddress::Ipv4(Ipv4SocketAddress { port, address }),
    ),
    IpAddress::Ipv6(address) => (
      IpAddressFamily::Ipv6,
      IpSocketAddress::Ipv6(Ipv6SocketAddress {
        port,
        flow_info: 0,
        address,
        scope_id: 0,
      }),
    ),
  };
  let socket = create_tcp_socket(family).map_err(|error| Error::Transport(format!("{error:?}")))?;

  socket
    .start_connect(network, remote)
    .map_err(|error| Error::Transport(format!("{error:?}")))?;

  let (input, output) = Retry(|| match socket.finish_connect() {
    Ok(streams) => Ok(Step::Ready(streams)),
    Err(wasi::sockets::network::ErrorCode::WouldBlock) => Ok(Step::Wait(socket.subscribe())),
    Err(error) => Err(Error::Transport(format!("{error:?}"))),
  })
  .await?;

  Ok(Tcp {
    input,
    output,
    socket,
  })
}

/// TLS over `tcp`, verifying `host` against the bundled roots.
fn secure(tcp: Tcp, host: &str) -> Result<StreamOwned<ClientConnection, Tcp>, Error> {
  let roots = RootCertStore {
    roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
  };
  let config = ClientConfig::builder()
    .with_root_certificates(roots)
    .with_no_client_auth();
  let name = ServerName::try_from(host.to_owned())
    .map_err(|error| Error::Url(format!("{host:?} is no server name: {error}")))?;
  let connection = ClientConnection::new(Arc::new(config), name)
    .map_err(|error| Error::Transport(error.to_string()))?;

  Ok(StreamOwned::new(connection, tcp))
}

/// The client handshake over `stream`, resumed after each wait on the host.
async fn handshake(url: &str, stream: Stream) -> Result<Socket<Stream>, Error> {
  let mut fresh = Some(stream);
  let mut paused: Option<MidHandshake<ClientHandshake<Stream>>> = None;

  Retry(|| {
    let outcome = match (fresh.take(), paused.take()) {
      (Some(stream), _) => tungstenite::client(url, stream),
      (None, Some(mid)) => mid.handshake(),
      (None, None) => return Err(Error::Transport("the handshake was lost".to_owned())),
    };

    match outcome {
      Ok((socket, _response)) => Ok(Step::Ready(socket)),
      Err(HandshakeError::Interrupted(mid)) => {
        let readable = mid.get_ref().get_ref().tcp().readable();

        paused = Some(mid);

        Ok(Step::Wait(readable))
      }
      Err(HandshakeError::Failure(error)) => Err(Error::Transport(error.to_string())),
    }
  })
  .await
}
