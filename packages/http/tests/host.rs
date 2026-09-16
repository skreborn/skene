//! The host transport against a server of its own: a status, headers and a body come
//! back whole, a failing status is a response, and a body is sent as given.

#![cfg(not(target_arch = "wasm32"))]

use std::{
  future::Future,
  io::{Read, Write},
  net::{TcpListener, TcpStream},
  pin::pin,
  sync::Arc,
  task::{Context, Poll, Wake, Waker},
  thread,
};

use skene_http::{Error, Request, Response};

/// Serves canned answers on a port of its own: `/hello` answers `201` with a header
/// and a body, `/echo` answers with the body it was sent, anything else `404`.
fn serve() -> String {
  let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
  let address = format!("http://{}", listener.local_addr().expect("an address"));

  thread::spawn(move || {
    for stream in listener.incoming().flatten() {
      answer(stream);
    }
  });

  address
}

/// Reads one request and writes its answer.
fn answer(mut stream: TcpStream) {
  let mut received = Vec::new();
  let mut chunk = [0; 1024];

  while !received.windows(4).any(|window| window == b"\r\n\r\n") {
    let read = stream.read(&mut chunk).unwrap_or(0);

    if read == 0 {
      return;
    }

    received.extend_from_slice(&chunk[..read]);
  }

  let head_end = received
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .expect("a head")
    + 4;
  let head = String::from_utf8_lossy(&received[..head_end]).into_owned();
  let length: usize = head
    .lines()
    .find_map(|line| {
      line
        .to_ascii_lowercase()
        .strip_prefix("content-length:")
        .map(|rest| rest.trim().parse().unwrap_or(0))
    })
    .unwrap_or(0);

  while received.len() < head_end + length {
    let read = stream.read(&mut chunk).unwrap_or(0);

    if read == 0 {
      break;
    }

    received.extend_from_slice(&chunk[..read]);
  }

  let body = received[head_end..].to_vec();
  let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
  let (status, extra, answer): (&str, &str, Vec<u8>) = match path.as_str() {
    "/hello" => ("201 Created", "X-Test: yes\r\n", b"hello".to_vec()),
    "/echo" => ("200 OK", "", body),
    _ => ("404 Not Found", "", b"nothing here".to_vec()),
  };

  let _ = write!(
    stream,
    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n",
    answer.len()
  );
  let _ = stream.write_all(&answer);
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
fn a_response_comes_back_whole() {
  let server = serve();
  let response: Response =
    block_on(skene_http::get(format!("{server}/hello"))).expect("a response");

  assert_eq!(response.status(), 201);
  assert!(response.is_success());
  assert_eq!(response.header("x-test"), Some("yes"));
  assert_eq!(response.text().as_deref(), Ok("hello"));
}

#[test]
fn a_failing_status_is_a_response_and_a_dead_host_an_error() {
  let server = serve();
  let missing = block_on(skene_http::get(format!("{server}/missing"))).expect("a response");

  assert_eq!(missing.status(), 404);
  assert!(!missing.is_success());
  assert_eq!(missing.body(), b"nothing here");

  let dead = block_on(skene_http::get("http://127.0.0.1:1/"));

  assert!(matches!(dead, Err(Error::Transport(_))), "{dead:?}");
}

#[test]
fn a_body_is_sent_as_given() {
  let server = serve();
  let echoed = block_on(
    Request::post(format!("{server}/echo"))
      .set_header("Content-Type", "text/plain")
      .set_body("ping")
      .send(),
  )
  .expect("a response");

  assert_eq!(echoed.body(), b"ping");
}
