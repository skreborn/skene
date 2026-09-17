//! The server transport against an echo server in the same component: a message sent
//! during a render comes back before the page serializes, a refused host is an error,
//! and a URL of another scheme is refused.
//!
//! Run under a WASI runtime with the network granted (the workspace's runner):
//! `cargo test -p skene-transport --target wasm32-wasip2`.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use std::{
  future::Future,
  io::{self, Read, Write},
  pin::Pin,
  task::{Context, Poll},
};

use skene_gui::{Resource, Text, View, derived, spawn, web::render_to_string};
use skene_transport::ws::{Error, Message};
use wasi::{
  io::{
    poll::Pollable,
    streams::{InputStream, OutputStream, StreamError},
  },
  sockets::{
    instance_network::instance_network,
    network::{ErrorCode, IpAddressFamily, IpSocketAddress, Ipv4SocketAddress},
    tcp::TcpSocket,
    tcp_create_socket::create_tcp_socket,
  },
};

/// A host operation tried on every poll until it is ready, each wait registered with
/// the reactor.
struct Retry<F>(F);

impl<F, T> Future for Retry<F>
where
  F: FnMut() -> Result<T, Pollable> + Unpin,
{
  type Output = T;

  fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
    match (self.0)() {
      Ok(value) => Poll::Ready(value),
      Err(pollable) => {
        skene_gui::web::register(pollable, context.waker().clone());

        Poll::Pending
      }
    }
  }
}

/// An accepted connection as a blocking writer and a non-blocking reader, its streams
/// dropped before the socket they belong to.
struct Accepted {
  input: InputStream,
  output: OutputStream,
  _socket: TcpSocket,
}

impl Read for Accepted {
  fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
    match self.input.read(buffer.len() as u64) {
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

impl Write for Accepted {
  fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
    for chunk in buffer.chunks(4096) {
      self
        .output
        .blocking_write_and_flush(chunk)
        .map_err(|error| io::Error::other(format!("{error:?}")))?;
    }

    Ok(buffer.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

/// A listening socket on a port of the loopback's choosing.
fn listen() -> (TcpSocket, u16) {
  let network = instance_network();
  let socket = create_tcp_socket(IpAddressFamily::Ipv4).expect("a socket");

  socket
    .start_bind(
      &network,
      IpSocketAddress::Ipv4(Ipv4SocketAddress {
        port: 0,
        address: (127, 0, 0, 1),
      }),
    )
    .expect("bind");
  socket.finish_bind().expect("bound");
  socket.start_listen().expect("listen");
  socket.finish_listen().expect("listening");

  let port = match socket.local_address().expect("an address") {
    IpSocketAddress::Ipv4(address) => address.port,
    IpSocketAddress::Ipv6(address) => address.port,
  };

  (socket, port)
}

/// Serves one connection on `listener`: every message echoed, `"bye"` closing from the
/// server's side.
async fn echo(listener: TcpSocket) {
  let accepted = Retry(|| match listener.accept() {
    Ok((socket, input, output)) => Ok(Accepted {
      input,
      output,
      _socket: socket,
    }),
    Err(ErrorCode::WouldBlock) => Err(listener.subscribe()),
    Err(error) => panic!("accept: {error:?}"),
  })
  .await;
  let mut fresh = Some(accepted);
  let mut paused = None;
  let mut socket = Retry(|| {
    let outcome = match (fresh.take(), paused.take()) {
      (Some(stream), _) => tungstenite::accept(stream),
      (None, Some(mid)) => tungstenite::handshake::MidHandshake::handshake(mid),
      (None, None) => unreachable!(),
    };

    match outcome {
      Ok(socket) => Ok(socket),
      Err(tungstenite::HandshakeError::Interrupted(mid)) => {
        let readable = mid.get_ref().get_ref().input.subscribe();

        paused = Some(mid);

        Err(readable)
      }
      Err(tungstenite::HandshakeError::Failure(error)) => panic!("handshake: {error}"),
    }
  })
  .await;

  loop {
    let frame = Retry(|| match socket.read() {
      Ok(frame) => Ok(Some(frame)),
      Err(tungstenite::Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {
        Err(socket.get_ref().input.subscribe())
      }
      Err(_) => Ok(None),
    })
    .await;

    match frame {
      Some(tungstenite::Message::Text(text)) if text.as_str() == "bye" => {
        let _ = socket.close(None);
        let _ = socket.flush();

        return;
      }
      Some(frame @ (tungstenite::Message::Text(_) | tungstenite::Message::Binary(_))) => {
        socket.send(frame).expect("echo");
      }
      Some(_) => {}
      None => return,
    }
  }
}

#[test]
fn a_message_sent_during_a_render_is_echoed_before_the_page_serializes() {
  let page = render_to_string(|| {
    let (listener, port) = listen();

    spawn(echo(listener));

    let outcome: Resource<Result<Vec<String>, String>> = Resource::new(move || async move {
      let socket = skene_transport::ws::connect(format!("ws://127.0.0.1:{port}/"))
        .await
        .map_err(|error| error.to_string())?;
      let mut lines = Vec::new();

      socket.send("hello").map_err(|error| error.to_string())?;
      socket
        .send(vec![1, 2, 3])
        .map_err(|error| error.to_string())?;
      socket.send("bye").map_err(|error| error.to_string())?;

      while let Some(message) = socket.receive().await {
        lines.push(match message {
          Message::Text(text) => text,
          Message::Binary(bytes) => format!("{bytes:?}"),
        });
      }

      lines.push(format!("closed: {}", socket.is_closed()));
      lines.push(format!("send after close: {:?}", socket.send("late")));

      Ok(lines)
    });

    derived(move || {
      outcome.with(|value| match value {
        None => Text::new("Connecting…").build(),
        Some(Ok(lines)) => Text::new(lines.join(" | ")).build(),
        Some(Err(error)) => Text::new(format!("Failed: {error}")).build(),
      })
    })
  });

  assert!(
    page
      .body()
      .contains("hello | [1, 2, 3] | closed: true | send after close: Err(Closed)"),
    "{}",
    page.body()
  );
}

#[test]
fn a_refused_host_is_an_error() {
  let page = render_to_string(|| {
    let outcome: Resource<Result<(), String>> = Resource::new(|| async {
      skene_transport::ws::connect("ws://127.0.0.1:1/")
        .await
        .map(drop)
        .map_err(|error| error.to_string())
    });

    derived(move || {
      outcome.with(|value| match value {
        None => Text::new("Connecting…").build(),
        Some(Ok(())) => Text::new("Connected").build(),
        Some(Err(error)) => Text::new(format!("Failed: {error}")).build(),
      })
    })
  });

  assert!(
    page.body().contains("Failed: the connection failed"),
    "{}",
    page.body()
  );
}

#[test]
fn a_url_of_another_scheme_is_refused() {
  let page = render_to_string(|| {
    let outcome: Resource<Result<(), String>> = Resource::new(|| async {
      skene_transport::ws::connect("http://not-a-socket/")
        .await
        .map(drop)
        .map_err(|error| match error {
          Error::Url(_) => "refused".to_owned(),
          other => other.to_string(),
        })
    });

    derived(move || {
      outcome.with(|value| match value {
        None => Text::new("Connecting…").build(),
        Some(Ok(())) => Text::new("Connected").build(),
        Some(Err(error)) => Text::new(error.clone()).build(),
      })
    })
  });

  assert!(page.body().contains(">refused<"), "{}", page.body());
}

/// A public echo service over TLS; needs the network, so it runs on request alone:
/// `cargo test -p skene-transport --target wasm32-wasip2 --test wasi_ws -- --ignored`.
#[test]
#[ignore = "reaches a public echo service over the network"]
fn a_public_echo_service_answers_over_tls() {
  let page = render_to_string(|| {
    let outcome: Resource<Result<String, String>> = Resource::new(|| async {
      let socket = skene_transport::ws::connect("wss://echo.websocket.org/")
        .await
        .map_err(|error| error.to_string())?;

      socket
        .send("hello over tls")
        .map_err(|error| error.to_string())?;

      // The service greets first, then echoes.
      while let Some(message) = socket.receive().await {
        if let Message::Text(text) = message
          && text == "hello over tls"
        {
          return Ok(text);
        }
      }

      Err("closed before the echo".to_owned())
    });

    derived(move || {
      outcome.with(|value| match value {
        None => Text::new("Connecting…").build(),
        Some(Ok(text)) => Text::new(format!("Echoed: {text}")).build(),
        Some(Err(error)) => Text::new(format!("Failed: {error}")).build(),
      })
    })
  });

  assert!(
    page.body().contains("Echoed: hello over tls"),
    "{}",
    page.body()
  );
}
