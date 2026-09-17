//! The dev server: static files, manifest assets, live-reload over SSE, the
//! authoritative state snapshot held across reloads, and—given a renderer—a route
//! rendered per request through the runner against the request's client hints.

use std::{
  ffi::OsStr,
  fs,
  io::{BufRead, BufReader, Read, Write},
  net::{TcpListener, TcpStream},
  path::{Path, PathBuf},
  sync::{Arc, Mutex},
  thread,
};

use percent_encoding::percent_decode_str;
use url::Url;

use crate::{
  manifest::App,
  render::{Renderer, hint_response_headers},
};

/// A request's headers, as `(name, value)` in arrival order.
type Headers = Vec<(String, String)>;

/// The dev channel: the open live-reload streams, the authoritative state snapshot
/// held across reloads, the rebuild-failure notice the running app polls, and the
/// terminal binary an attached terminal runs.
#[derive(Clone, Default)]
pub struct Dev {
  clients: Arc<Mutex<Vec<TcpStream>>>,
  state: Arc<Mutex<Option<Vec<u8>>>>,
  notice: Arc<Mutex<Notice>>,
  renderer: Arc<Mutex<Option<Renderer>>>,
  binary: Arc<Mutex<Option<Binary>>>,
}

/// The rebuild-failure notice: its text, and a version bumped on every change so the
/// app re-presents a repeated failure.
#[derive(Default)]
struct Notice {
  version: u64,
  text: Option<String>,
}

/// The terminal binary a `run tui` loop built: a generation bumped on every swap, the
/// binary's path, and the directory it runs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binary {
  pub generation: u64,
  pub path: PathBuf,
  pub dir: PathBuf,
}

impl Binary {
  /// The body `/skene:binary` serves: the generation, the path and the directory, a
  /// line each.
  fn body(&self) -> Vec<u8> {
    format!(
      "{}\n{}\n{}",
      self.generation,
      self.path.display(),
      self.dir.display()
    )
    .into_bytes()
  }

  /// The binary a `/skene:binary` body names.
  pub fn parse(body: &str) -> Option<Self> {
    let mut lines = body.lines();
    let generation = lines.next()?.parse().ok()?;
    let path = PathBuf::from(lines.next()?);
    let dir = PathBuf::from(lines.next()?);

    Some(Self {
      generation,
      path,
      dir,
    })
  }
}

impl Dev {
  /// Renders routes through `renderer` from now on, each request's client hints
  /// forwarded; `None` returns to serving the generated page.
  pub fn set_renderer(&self, renderer: Option<Renderer>) {
    *self
      .renderer
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner) = renderer;
  }

  /// Renders `route` against the client hints among `headers`, when a renderer is set.
  fn render(&self, route: &str, headers: &[(String, String)]) -> Option<Result<String, String>> {
    let mut renderer = self
      .renderer
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);
    let renderer = renderer.as_mut()?;
    let headers = headers
      .iter()
      .map(|(name, value)| (name.as_str(), value.as_str()));

    Some(renderer.render(route, headers))
  }

  /// Tells every connected page to reload, dropping streams that went away.
  pub fn trigger(&self) {
    let mut clients = self
      .clients
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);

    clients.retain_mut(|stream| stream.write_all(b"data: reload\n\n").is_ok());
  }

  fn adopt(&self, mut stream: TcpStream) {
    let header = "HTTP/1.1 200 OK\r\n\
                  Content-Type: text/event-stream\r\n\
                  Cache-Control: no-cache\r\n\r\n";

    if stream.write_all(header.as_bytes()).is_ok() {
      self
        .clients
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(stream);
    }
  }

  fn store(&self, bytes: Vec<u8>) {
    *self
      .state
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(bytes);
  }

  fn stored(&self) -> Option<Vec<u8>> {
    self
      .state
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .clone()
  }

  /// Posts a rebuild failure for the running app's dev modal, superseding the last.
  pub fn set_notice(&self, text: String) {
    let mut notice = self
      .notice
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);

    notice.version += 1;
    notice.text = Some(text);
  }

  /// Clears the failure notice, after a successful rebuild.
  pub fn clear_notice(&self) {
    let mut notice = self
      .notice
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);

    if notice.text.is_some() {
      notice.version += 1;
      notice.text = None;
    }
  }

  /// The notice body an app polls: the version line, then the failure text.
  fn notice_body(&self) -> Vec<u8> {
    let notice = self
      .notice
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);

    format!(
      "{}\n{}",
      notice.version,
      notice.text.as_deref().unwrap_or_default()
    )
    .into_bytes()
  }

  /// Publishes the terminal binary at `path`, run in `dir`, as the next generation an
  /// attached terminal swaps in.
  pub fn set_binary(&self, path: PathBuf, dir: PathBuf) {
    let mut binary = self
      .binary
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);
    let generation = binary.as_ref().map_or(1, |binary| binary.generation + 1);

    *binary = Some(Binary {
      generation,
      path,
      dir,
    });
  }

  /// The binary body an attached terminal polls, `None` before a build.
  fn binary_body(&self) -> Option<Vec<u8>> {
    self
      .binary
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .as_ref()
      .map(Binary::body)
  }
}

/// Fetches `path` from the state server on `port`: the body of a `200` reply, `None`
/// for any other or for no server.
pub fn fetch(port: u16, path: &str) -> Option<Vec<u8>> {
  let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;

  stream
    .write_all(
      format!(
        "GET {path} HTTP/1.1\r\n\
         Host: localhost\r\n\
         Connection: close\r\n\r\n"
      )
      .as_bytes(),
    )
    .ok()?;

  let mut response = Vec::new();

  stream.read_to_end(&mut response).ok()?;

  let split = response
    .windows(4)
    .position(|window| window == b"\r\n\r\n")?;
  let head = std::str::from_utf8(&response[..split]).ok()?;

  if !head.starts_with("HTTP/1.1 200") {
    return None;
  }

  Some(response[split + 4..].to_vec())
}

/// Serves only the state slot on an ephemeral port, for a dev loop around a target
/// with no page to serve (the terminal, native later); returns the port for
/// `SKENE_DEV`.
pub fn spawn_state_server(dev: Dev) -> Result<u16, String> {
  let listener =
    TcpListener::bind(("127.0.0.1", 0)).map_err(|error| format!("bind the state port: {error}"))?;
  let port = listener
    .local_addr()
    .map_err(|error| error.to_string())?
    .port();

  thread::spawn(move || {
    for stream in listener.incoming() {
      let Ok(stream) = stream
      else {
        continue;
      };

      let dev = dev.clone();

      thread::spawn(move || handle_state_request(stream, &dev));
    }
  });

  Ok(port)
}

/// Handles one request against the state slot alone.
fn handle_state_request(mut stream: TcpStream, dev: &Dev) {
  let Ok(clone) = stream.try_clone()
  else {
    return;
  };

  let mut reader = BufReader::new(clone);
  let mut line = String::new();

  if reader.read_line(&mut line).is_err() {
    return;
  }

  let mut parts = line.split_whitespace();

  match (parts.next(), parts.next()) {
    (Some("POST"), Some("/skene:state")) => match read_body(&mut reader) {
      Some(body) => {
        dev.store(body);

        let _ = write_response(&mut stream, "204 No Content", "text/plain", b"");
      }
      None => {
        let _ = write_response(&mut stream, "400 Bad Request", "text/plain", b"bad request");
      }
    },
    (Some("GET"), Some("/skene:state")) => match dev.stored() {
      Some(bytes) => {
        let _ = write_response(&mut stream, "200 OK", "application/octet-stream", &bytes);
      }
      None => {
        let _ = write_response(&mut stream, "404 Not Found", "text/plain", b"no state");
      }
    },
    (Some("GET"), Some("/skene:notice")) => {
      let _ = write_response(
        &mut stream,
        "200 OK",
        "text/plain; charset=utf-8",
        &dev.notice_body(),
      );
    }
    (Some("GET"), Some("/skene:binary")) => match dev.binary_body() {
      Some(body) => {
        let _ = write_response(&mut stream, "200 OK", "text/plain; charset=utf-8", &body);
      }
      None => {
        let _ = write_response(&mut stream, "404 Not Found", "text/plain", b"no binary");
      }
    },
    _ => {
      let _ = write_response(&mut stream, "404 Not Found", "text/plain", b"not found");
    }
  }
}

/// Serves the app on `port` until interrupted: `index.html` and the build output from
/// the app's `dist/web`, at its own path and as the site root, declared assets at
/// their URIs, and—with `dev`—the reload SSE endpoint, the state slot, and the reload
/// script injected into HTML.
pub fn serve(app: &Arc<App>, port: u16, dev: Option<&Dev>) -> Result<(), String> {
  let listener = TcpListener::bind(("127.0.0.1", port))
    .map_err(|error| format!("failed to bind 127.0.0.1:{port}: {error}"))?;

  println!("serving {} at http://localhost:{port}", app.dir.display());

  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        let app = Arc::clone(app);
        let dev = dev.cloned();

        thread::spawn(move || handle_request(stream, &app, dev.as_ref()));
      }
      Err(error) => eprintln!("connection failed: {error}"),
    }
  }

  Ok(())
}

fn handle_request(mut stream: TcpStream, app: &App, dev: Option<&Dev>) {
  let Ok(clone) = stream.try_clone()
  else {
    return;
  };

  let mut reader = BufReader::new(clone);
  let mut line = String::new();

  if reader.read_line(&mut line).is_err() {
    return;
  }

  let mut parts = line.split_whitespace();
  let (Some(method), Some(target)) = (parts.next(), parts.next())
  else {
    let _ = write_response(&mut stream, "400 Bad Request", "text/plain", b"bad request");

    return;
  };

  let Some(relative) = request_path(target)
  else {
    let _ = write_response(&mut stream, "400 Bad Request", "text/plain", b"bad request");

    return;
  };

  let headers = if method == "GET" {
    read_headers(&mut reader).unwrap_or_default()
  }
  else {
    Vec::new()
  };

  match (method, relative.as_str(), dev) {
    ("GET", "skene:reload", Some(dev)) => {
      drop(reader);
      dev.adopt(stream);
    }

    ("POST", "skene:state", Some(dev)) => match read_body(&mut reader) {
      Some(body) => {
        dev.store(body);

        let _ = write_response(&mut stream, "204 No Content", "text/plain", b"");
      }
      None => {
        let _ = write_response(&mut stream, "400 Bad Request", "text/plain", b"bad request");
      }
    },

    ("GET", "skene:state", Some(dev)) => match dev.stored() {
      Some(bytes) => {
        let _ = write_response(&mut stream, "200 OK", "application/octet-stream", &bytes);
      }
      None => {
        let _ = write_response(&mut stream, "404 Not Found", "text/plain", b"no state");
      }
    },

    ("GET", relative, Some(dev)) if is_route(relative) => {
      match dev.render(&format!("/{relative}"), &headers) {
        Some(Ok(page)) => {
          let _ = write_response_with(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            &hint_response_headers(),
            crate::pipeline::inject_client(&page, &app.stem).as_bytes(),
          );
        }
        Some(Err(error)) => {
          let _ = write_response(
            &mut stream,
            "500 Internal Server Error",
            "text/plain; charset=utf-8",
            error.as_bytes(),
          );
        }
        None => serve_file(&mut stream, app, relative),
      }
    }

    ("GET", relative, _) => serve_file(&mut stream, app, relative),

    _ => {
      let _ = write_response(&mut stream, "404 Not Found", "text/plain", b"not found");
    }
  }
}

/// Whether a request path names a route rather than a file: no extension in its last
/// segment.
fn is_route(relative: &str) -> bool {
  !relative
    .rsplit('/')
    .next()
    .unwrap_or_default()
    .contains('.')
}

/// Reads the request headers up to the blank line ending them.
fn read_headers(reader: &mut BufReader<TcpStream>) -> Option<Headers> {
  let mut headers = Vec::new();

  loop {
    let mut line = String::new();

    reader.read_line(&mut line).ok()?;

    let line = line.trim_end();

    if line.is_empty() {
      return Some(headers);
    }

    if let Some((name, value)) = line.split_once(':') {
      headers.push((name.trim().to_owned(), value.trim().to_owned()));
    }
  }
}

/// Reads the request body: the remaining headers for `Content-Length`, then exactly
/// that many bytes.
fn read_body(reader: &mut BufReader<TcpStream>) -> Option<Vec<u8>> {
  let length = read_headers(reader)?
    .into_iter()
    .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
    .and_then(|(_, value)| value.parse::<usize>().ok());

  let mut body = vec![0; length?];

  reader.read_exact(&mut body).ok()?;

  Some(body)
}

/// The request target as a decoded, root-relative path: parsed and normalized as a
/// proper URL (dot segments resolved, query and fragment separated), each segment
/// percent-decoded, and rejected when a decoded segment would climb or nest.
fn request_path(target: &str) -> Option<String> {
  let base = Url::parse("http://localhost/").ok()?;
  let url = base.join(target).ok()?;
  let mut segments = Vec::new();

  for segment in url.path_segments()? {
    if segment.is_empty() {
      continue;
    }

    let segment = percent_decode_str(segment).decode_utf8().ok()?;

    if segment == "." || segment == ".." || segment.contains(['/', '\\']) {
      return None;
    }

    segments.push(segment.into_owned());
  }

  Some(segments.join("/"))
}

/// Serves one static file: the generated page and bundle, or a declared asset—never
/// the app's sources or manifests.
fn serve_file(stream: &mut TcpStream, app: &App, relative: &str) {
  let relative = if relative.is_empty() || relative == "index.html" {
    "dist/web/index.html"
  }
  else {
    relative
  };

  // The build output is reachable at its own path and as the site root, which is how a
  // rendered page boots the bundle once `dist/web` is served as a static site.
  let built = relative.strip_prefix("dist/web/").map_or_else(
    || app.dir.join("dist/web").join(relative),
    |inside| app.dir.join("dist/web").join(inside),
  );
  let path = Some(built)
    .filter(|path| path.is_file())
    .or_else(|| app.resolve_asset(relative));

  let Some(path) = path
  else {
    let _ = write_response(stream, "404 Not Found", "text/plain", b"not found");

    return;
  };

  match fs::read(&path) {
    Ok(body) => {
      let _ = write_response(stream, "200 OK", content_type(&path), &body);
    }
    Err(_) => {
      let _ = write_response(
        stream,
        "500 Internal Server Error",
        "text/plain",
        b"read error",
      );
    }
  }
}

fn content_type(path: &Path) -> &'static str {
  match path.extension().and_then(OsStr::to_str) {
    Some("html") => "text/html; charset=utf-8",
    Some("js" | "mjs") => "text/javascript; charset=utf-8",
    Some("wasm") => "application/wasm",
    Some("css") => "text/css; charset=utf-8",
    Some("json" | "map") => "application/json",
    Some("svg") => "image/svg+xml",
    Some("png") => "image/png",
    Some("woff2") => "font/woff2",
    _ => "application/octet-stream",
  }
}

fn write_response(
  stream: &mut TcpStream,
  status: &str,
  content_type: &str,
  body: &[u8],
) -> std::io::Result<()> {
  write_response_with(stream, status, content_type, &[], body)
}

/// Writes a response carrying `extra` headers beside the content ones.
fn write_response_with(
  stream: &mut TcpStream,
  status: &str,
  content_type: &str,
  extra: &[(&str, &str)],
  body: &[u8],
) -> std::io::Result<()> {
  let mut header = format!(
    "HTTP/1.1 {status}\r\n\
     Content-Type: {content_type}\r\n\
     Content-Length: {}\r\n",
    body.len()
  );

  for (name, value) in extra {
    use std::fmt::Write as _;

    let _ = write!(header, "{name}: {value}\r\n");
  }

  header.push_str("Connection: close\r\n\r\n");
  stream.write_all(header.as_bytes())?;
  stream.write_all(body)?;
  stream.flush()
}

#[cfg(test)]
mod tests {
  use super::*;

  /// The binary body round-trips, and each publication is the next generation.
  #[test]
  fn the_binary_body_round_trips_and_generations_advance() {
    let dev = Dev::default();

    assert!(dev.binary_body().is_none());
    dev.set_binary("/target/debug/app".into(), "/app".into());
    dev.set_binary("/target/debug/app".into(), "/app".into());

    let body = String::from_utf8(dev.binary_body().unwrap()).unwrap();

    assert_eq!(
      Binary::parse(&body),
      Some(Binary {
        generation: 2,
        path: "/target/debug/app".into(),
        dir: "/app".into(),
      })
    );
    assert!(Binary::parse("x").is_none());
  }
}
