//! The host transport against an echo server of its own: a message sent comes back,
//! bytes as bytes, a close from the far side ends the receiving, and a refused host
//! is an error, and a URL of another scheme is refused.

#![cfg(not(target_arch = "wasm32"))]

use std::{
  future::Future,
  net::TcpListener,
  pin::pin,
  sync::Arc,
  task::{Context, Poll, Wake, Waker},
  thread,
};

use skene_transport::ws::{Error, Message};

/// An echo server on a port of its own: every message comes back, and `"bye"` closes
/// the socket from the server's side.
fn serve() -> String {
  let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
  let address = format!("ws://{}", listener.local_addr().expect("an address"));

  thread::spawn(move || {
    for stream in listener.incoming().flatten() {
      thread::spawn(move || {
        let Ok(mut socket) = tungstenite::accept(stream)
        else {
          return;
        };

        while let Ok(message) = socket.read() {
          match message {
            tungstenite::Message::Text(text) if text.as_str() == "bye" => {
              let _ = socket.close(None);
              let _ = socket.flush();

              break;
            }
            tungstenite::Message::Text(_) | tungstenite::Message::Binary(_) => {
              let _ = socket.send(message);
            }
            tungstenite::Message::Close(_) => break,
            _ => {}
          }
        }
      });
    }
  });

  address
}

/// Drives `future` to its end on this thread, parking between polls.
fn block_on<F: Future>(future: F) -> F::Output {
  struct Unpark(thread::Thread);

  impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
      self.0.unpark();
    }
  }

  let waker = Waker::from(Arc::new(Unpark(thread::current())));
  let mut context = Context::from_waker(&waker);
  let mut future = pin!(future);

  loop {
    match future.as_mut().poll(&mut context) {
      Poll::Ready(output) => return output,
      Poll::Pending => thread::park(),
    }
  }
}

#[test]
fn messages_are_echoed_and_a_close_ends_the_receiving() {
  let server = serve();
  let socket = block_on(skene_transport::ws::connect(server)).expect("a socket");

  socket.send("hello").expect("sent");
  socket.send(vec![1, 2, 3]).expect("sent");

  assert_eq!(
    block_on(socket.receive()),
    Some(Message::Text("hello".to_owned()))
  );
  assert_eq!(
    block_on(socket.receive()),
    Some(Message::Binary(vec![1, 2, 3]))
  );
  assert!(!socket.is_closed());

  socket.send("bye").expect("sent");

  assert_eq!(block_on(socket.receive()), None, "the far side closed");
  assert!(socket.is_closed());
  assert_eq!(socket.send("more"), Err(Error::Closed));
}

#[test]
fn a_refused_host_is_an_error_and_closing_ends_the_receiving() {
  let refused = block_on(skene_transport::ws::connect("ws://127.0.0.1:1/"));

  assert!(matches!(refused, Err(Error::Transport(_))), "{refused:?}");

  let server = serve();
  let socket = block_on(skene_transport::ws::connect(server)).expect("a socket");

  socket.close();

  assert_eq!(block_on(socket.receive()), None);
}

#[test]
fn a_url_of_another_scheme_is_refused() {
  let outcome = block_on(skene_transport::ws::connect("http://not-a-socket/"));

  assert!(matches!(outcome, Err(Error::Url(_))), "{outcome:?}");
}
