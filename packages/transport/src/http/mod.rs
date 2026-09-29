//! An HTTP client, on the framework's public seams alone: a request is
//! built, sent, and awaited inside any task the facade runs—a `Resource`'s load, a
//! `spawn`ed future—and the answer comes back as a [`Response`] with its status, its
//! headers and its body. Nothing in the framework knows this crate exists, so a
//! third-party client sits on exactly the same footing.
//!
//! The transport is the platform's: the browser's `fetch` on the web, and a plain HTTP
//! client on every other host, a server render included, one agent keeping connections
//! alive for every request and a bounded pool of threads running the exchanges, the
//! future waking the platform's executor when its exchange is done. A request outlives
//! nothing: dropping the future before it resolves abandons the exchange. A body is
//! read whole, however large, as `fetch` reads one.

use std::{fmt, string::FromUtf8Error};

#[cfg(not(target_arch = "wasm32"))]
mod host;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod web;

#[cfg(not(target_arch = "wasm32"))]
use host as transport;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use web as transport;

/// The method of a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Method {
  /// Reads a resource.
  Get,
  /// Reads a resource's headers alone.
  Head,
  /// Submits data to a resource.
  Post,
  /// Replaces a resource.
  Put,
  /// Changes part of a resource.
  Patch,
  /// Removes a resource.
  Delete,
}

impl Method {
  /// The method's name on the wire.
  #[inline(always)]
  #[must_use]
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Get => "GET",
      Self::Head => "HEAD",
      Self::Post => "POST",
      Self::Put => "PUT",
      Self::Patch => "PATCH",
      Self::Delete => "DELETE",
    }
  }
}

impl fmt::Display for Method {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    out.write_str(self.as_str())
  }
}

/// A request: a method, an absolute or page-relative URL, headers and a body.
///
/// A relative URL resolves against the page on the web and is an error elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Request {
  method: Method,
  url: String,
  headers: Vec<(String, String)>,
  body: Vec<u8>,
}

impl Request {
  /// A request of `method` to `url`, with no headers and an empty body.
  #[inline(always)]
  pub fn new(method: Method, url: impl Into<String>) -> Self {
    Self {
      method,
      url: url.into(),
      headers: Vec::new(),
      body: Vec::new(),
    }
  }

  /// A `GET` of `url`.
  #[inline(always)]
  pub fn get(url: impl Into<String>) -> Self {
    Self::new(Method::Get, url)
  }

  /// A `POST` to `url`.
  #[inline(always)]
  pub fn post(url: impl Into<String>) -> Self {
    Self::new(Method::Post, url)
  }

  /// A `PUT` to `url`.
  #[inline(always)]
  pub fn put(url: impl Into<String>) -> Self {
    Self::new(Method::Put, url)
  }

  /// A `PATCH` to `url`.
  #[inline(always)]
  pub fn patch(url: impl Into<String>) -> Self {
    Self::new(Method::Patch, url)
  }

  /// A `DELETE` of `url`.
  #[inline(always)]
  pub fn delete(url: impl Into<String>) -> Self {
    Self::new(Method::Delete, url)
  }

  /// Adds a header; a name given twice is sent twice.
  #[inline(always)]
  pub fn set_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
    self.headers.push((name.into(), value.into()));

    self
  }

  /// Sets the body.
  #[inline(always)]
  pub fn set_body(mut self, body: impl Into<Vec<u8>>) -> Self {
    self.body = body.into();

    self
  }

  /// The method.
  #[inline(always)]
  pub const fn method(&self) -> Method {
    self.method
  }

  /// The URL.
  #[inline(always)]
  #[must_use]
  pub fn url(&self) -> &str {
    &self.url
  }

  /// The headers, in the order added.
  #[inline(always)]
  #[must_use]
  pub fn headers(&self) -> &[(String, String)] {
    &self.headers
  }

  /// The body.
  #[inline(always)]
  #[must_use]
  pub fn body(&self) -> &[u8] {
    &self.body
  }

  /// Sends the request and awaits the whole response.
  ///
  /// A status outside the successes is not an error: the response carries it.
  ///
  /// # Errors
  ///
  /// A URL the platform cannot use, or an exchange that could not be made or
  /// completed.
  pub async fn send(self) -> Result<Response, Error> {
    transport::send(self).await
  }
}

/// A response: the status, the headers and the whole body.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
  status: u16,
  headers: Vec<(String, String)>,
  body: Vec<u8>,
}

impl Response {
  /// A response as received.
  #[inline(always)]
  pub const fn new(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Self {
    Self {
      status,
      headers,
      body,
    }
  }

  /// The status code.
  #[inline(always)]
  #[must_use]
  pub const fn status(&self) -> u16 {
    self.status
  }

  /// Whether the status is a success, `200` to `299`.
  #[inline(always)]
  #[must_use]
  pub const fn is_success(&self) -> bool {
    self.status >= 200 && self.status < 300
  }

  /// The first header named `name`, compared without regard to case.
  #[must_use]
  pub fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(header, _)| header.eq_ignore_ascii_case(name))
      .map(|(_, value)| value.as_str())
  }

  /// Every header, in the order received.
  #[inline(always)]
  #[must_use]
  pub fn headers(&self) -> &[(String, String)] {
    &self.headers
  }

  /// The body.
  #[inline(always)]
  #[must_use]
  pub fn body(&self) -> &[u8] {
    &self.body
  }

  /// The body, taken.
  #[inline(always)]
  #[must_use]
  pub fn into_body(self) -> Vec<u8> {
    self.body
  }

  /// The body as text.
  ///
  /// # Errors
  ///
  /// A body that is not UTF-8.
  pub fn text(&self) -> Result<String, Error> {
    String::from_utf8(self.body.clone()).map_err(Error::Text)
  }
}

/// Why a request could not be made or completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
  /// The URL could not be used on this platform.
  Url(String),
  /// The exchange failed on the way: no connection, a broken stream, a refusal.
  Transport(String),
  /// The body is not the text it was read as.
  Text(FromUtf8Error),
}

impl fmt::Display for Error {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Url(message) => write!(out, "the URL cannot be used: {message}"),
      Self::Transport(message) => write!(out, "the request failed: {message}"),
      Self::Text(error) => write!(out, "the body is not text: {error}"),
    }
  }
}

impl std::error::Error for Error {}

/// Sends a `GET` of `url` and awaits the whole response.
///
/// # Errors
///
/// As [`Request::send`].
pub async fn get(url: impl Into<String>) -> Result<Response, Error> {
  Request::get(url).send().await
}
