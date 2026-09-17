//! The transport on a host: a plain HTTP client on a thread of its own, the future
//! woken when the thread is done.

use std::{
  future::Future,
  pin::Pin,
  sync::{Arc, Mutex},
  task::{Context, Poll, Waker},
  thread,
};

use super::{Error, Request, Response};

/// What the thread and the future share: the outcome once there is one, and the waker
/// to fire then.
struct Shared {
  outcome: Option<Result<Response, Error>>,
  waker: Option<Waker>,
}

/// A request in flight on its thread.
struct Exchange(Arc<Mutex<Shared>>);

impl Future for Exchange {
  type Output = Result<Response, Error>;

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
    let mut shared = self
      .0
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);

    if let Some(outcome) = shared.outcome.take() {
      return Poll::Ready(outcome);
    }

    shared.waker = Some(context.waker().clone());

    Poll::Pending
  }
}

/// Sends `request` on a thread and resolves with the whole response.
pub(crate) fn send(request: Request) -> impl Future<Output = Result<Response, Error>> {
  let shared = Arc::new(Mutex::new(Shared {
    outcome: None,
    waker: None,
  }));
  let worker = Arc::clone(&shared);

  thread::spawn(move || {
    let outcome = exchange(&request);
    let waker = {
      let mut shared = worker
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

      shared.outcome = Some(outcome);
      shared.waker.take()
    };

    if let Some(waker) = waker {
      waker.wake();
    }
  });

  Exchange(shared)
}

/// The whole exchange, blocking: any status is a response, and only a failure on the
/// way is an error.
fn exchange(request: &Request) -> Result<Response, Error> {
  let agent: ureq::Agent = ureq::Agent::config_builder()
    .http_status_as_error(false)
    .build()
    .into();
  let mut builder = ureq::http::Request::builder()
    .method(request.method().as_str())
    .uri(request.url());

  for (name, value) in request.headers() {
    builder = builder.header(name, value);
  }

  let outgoing = builder
    .body(request.body().to_vec())
    .map_err(|error| Error::Url(error.to_string()))?;
  let mut incoming = agent
    .run(outgoing)
    .map_err(|error| Error::Transport(error.to_string()))?;
  let status = incoming.status().as_u16();
  let headers = incoming
    .headers()
    .iter()
    .map(|(name, value)| {
      (
        name.as_str().to_owned(),
        value.to_str().unwrap_or_default().to_owned(),
      )
    })
    .collect();
  let body = incoming
    .body_mut()
    .read_to_vec()
    .map_err(|error| Error::Transport(error.to_string()))?;

  Ok(Response::new(status, headers, body))
}
