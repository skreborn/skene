//! The transport on a server render: `wasi:http`'s outgoing handler, each wait on the
//! host registered with the server leaf's reactor.

use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use wasi::{
  http::{
    outgoing_handler,
    types::{
      Fields,
      FutureIncomingResponse,
      IncomingBody,
      Method,
      OutgoingBody,
      OutgoingRequest,
      Scheme,
    },
  },
  io::streams::{InputStream, StreamError},
};

use super::{Error, Request, Response, url};

/// How many bytes one read of the body asks for.
const READ: u64 = 64 * 1024;

/// Sends `request` through the outgoing handler and resolves with the whole response.
pub(crate) async fn send(request: Request) -> Result<Response, Error> {
  let parts = url::parts(request.url())?;
  let entries: Vec<(String, Vec<u8>)> = request
    .headers()
    .iter()
    .map(|(name, value)| (name.clone(), value.as_bytes().to_vec()))
    .collect();
  let fields = Fields::from_list(&entries).map_err(|error| Error::Url(error.to_string()))?;
  let outgoing = OutgoingRequest::new(fields);
  let method = match request.method() {
    super::Method::Get => Method::Get,
    super::Method::Head => Method::Head,
    super::Method::Post => Method::Post,
    super::Method::Put => Method::Put,
    super::Method::Patch => Method::Patch,
    super::Method::Delete => Method::Delete,
  };
  let scheme = if parts.secure {
    Scheme::Https
  }
  else {
    Scheme::Http
  };
  let refused = |()| Error::Url("the host refused the request's parts".to_owned());

  outgoing.set_method(&method).map_err(refused)?;
  outgoing.set_scheme(Some(&scheme)).map_err(refused)?;
  outgoing
    .set_authority(Some(&parts.authority))
    .map_err(refused)?;
  outgoing
    .set_path_with_query(Some(&parts.path))
    .map_err(refused)?;

  if !request.body().is_empty() {
    let body = outgoing.body().map_err(refused)?;

    {
      let stream = body.write().map_err(refused)?;

      for chunk in request.body().chunks(4096) {
        stream
          .blocking_write_and_flush(chunk)
          .map_err(|error| Error::Transport(error.to_string()))?;
      }
    }

    OutgoingBody::finish(body, None).map_err(|error| Error::Transport(error.to_string()))?;
  }

  let pending = outgoing_handler::handle(outgoing, None)
    .map_err(|error| Error::Transport(error.to_string()))?;
  let incoming = Arrival(pending).await?;
  let status = incoming.status();
  let headers = incoming
    .headers()
    .entries()
    .into_iter()
    .map(|(name, value)| (name, String::from_utf8_lossy(&value).into_owned()))
    .collect();
  let body = incoming
    .consume()
    .map_err(|()| Error::Transport("the body was consumed already".to_owned()))?;
  let stream = body
    .stream()
    .map_err(|()| Error::Transport("the body's stream was taken already".to_owned()))?;
  let bytes = ReadWhole {
    stream,
    read: Vec::new(),
  }
  .await?;

  let _trailers = IncomingBody::finish(body);

  Ok(Response::new(status, headers, bytes))
}

/// The response's arrival, waited on through the reactor.
struct Arrival(FutureIncomingResponse);

impl Future for Arrival {
  type Output = Result<wasi::http::types::IncomingResponse, Error>;

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
    match self.0.get() {
      Some(Ok(Ok(response))) => Poll::Ready(Ok(response)),
      Some(Ok(Err(error))) => Poll::Ready(Err(Error::Transport(error.to_string()))),
      Some(Err(())) => Poll::Ready(Err(Error::Transport(
        "the response was taken already".to_owned(),
      ))),
      None => {
        skene_web::register(self.0.subscribe(), context.waker().clone());

        Poll::Pending
      }
    }
  }
}

/// The body read to its end, each wait for more registered with the reactor.
struct ReadWhole {
  stream: InputStream,
  read: Vec<u8>,
}

impl Future for ReadWhole {
  type Output = Result<Vec<u8>, Error>;

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();

    loop {
      match this.stream.read(READ) {
        Ok(chunk) if chunk.is_empty() => {
          skene_web::register(this.stream.subscribe(), context.waker().clone());

          return Poll::Pending;
        }
        Ok(chunk) => this.read.extend(chunk),
        Err(StreamError::Closed) => return Poll::Ready(Ok(std::mem::take(&mut this.read))),
        Err(StreamError::LastOperationFailed(error)) => {
          return Poll::Ready(Err(Error::Transport(error.to_debug_string())));
        }
      }
    }
  }
}
