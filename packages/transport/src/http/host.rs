//! The transport on a host: a plain HTTP client, one agent shared by every request so
//! connections and TLS sessions stay alive, each exchange run on the shared pool and
//! the future woken when it is done.

use std::{future::Future, sync::OnceLock};

use ureq::{
  Agent,
  http::Uri,
  tls::{RootCerts, TlsConfig, TlsProvider},
};

use super::{Error, Request, Response};
use crate::host;

/// The agent every request goes through: any status a response, over the shared TLS
/// provider and the platform's trust store.
fn agent() -> &'static Agent {
  static AGENT: OnceLock<Agent> = OnceLock::new();

  AGENT.get_or_init(|| {
    Agent::config_builder()
      .http_status_as_error(false)
      .tls_config(
        TlsConfig::builder()
          .provider(TlsProvider::Rustls)
          .root_certs(RootCerts::PlatformVerifier)
          .unversioned_rustls_crypto_provider(host::provider())
          .build(),
      )
      .build()
      .into()
  })
}

/// Sends `request` on the pool and resolves with the whole response.
pub(crate) fn send(request: Request) -> impl Future<Output = Result<Response, Error>> {
  let (settler, promise) = host::one_shot(Err(Error::Transport(
    "the exchange stopped before it finished".to_owned(),
  )));

  host::run(move || settler.settle(exchange(&request)));

  promise
}

/// The whole exchange, blocking: any status is a response, and only a failure on the
/// way is an error.
fn exchange(request: &Request) -> Result<Response, Error> {
  let uri: Uri = request
    .url()
    .parse()
    .map_err(|error: ureq::http::uri::InvalidUri| Error::Url(error.to_string()))?;

  if uri.scheme().is_none() || uri.authority().is_none() {
    return Err(Error::Url(format!(
      "{:?} is relative, and a host has no page to resolve it against",
      request.url()
    )));
  }

  let mut builder = ureq::http::Request::builder()
    .method(request.method().as_str())
    .uri(uri);

  for (name, value) in request.headers() {
    builder = builder.header(name, value);
  }

  let outgoing = builder
    .body(request.body().to_vec())
    .map_err(|error| Error::Url(error.to_string()))?;
  let mut incoming = agent()
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
    .with_config()
    .limit(u64::MAX)
    .read_to_vec()
    .map_err(|error| Error::Transport(error.to_string()))?;

  Ok(Response::new(status, headers, body))
}
