//! The dev server: static files, manifest assets, live-reload over SSE, and—given a
//! renderer—a route rendered per request through the app's server binary against the
//! request's client hints; and the state socket a terminal build loop publishes its
//! binary and its notices on.

use std::{
  borrow::Cow,
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
  manifest::{App, LiveApp},
  render::{Renderer, hint_response_headers},
};

/// A request's headers, as `(name, value)` in arrival order.
type Headers = Vec<(String, String)>;

/// The dev channel: the open live-reload streams, the rebuild-failure notice an
/// attached terminal forwards to the running app, and the terminal binary it runs.
#[derive(Clone, Default)]
pub(crate) struct Dev {
  clients: Arc<Mutex<Vec<TcpStream>>>,
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

/// The terminal binary a `run terminal` loop built: a generation bumped on every swap, the
/// binary's path, and the directory it runs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Binary {
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
  pub(crate) fn parse(body: &str) -> Option<Self> {
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
  pub(crate) fn set_renderer(&self, renderer: Option<Renderer>) {
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
  pub(crate) fn trigger(&self) {
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

  /// Posts a rebuild failure for the running app's dev modal, superseding the last.
  pub(crate) fn set_notice(&self, text: String) {
    let mut notice = self
      .notice
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);

    notice.version += 1;
    notice.text = Some(text);
  }

  /// Clears the failure notice, after a successful rebuild.
  pub(crate) fn clear_notice(&self) {
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
  pub(crate) fn set_binary(&self, path: PathBuf, dir: PathBuf) {
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

/// The directory the state sockets live in, which only this user may enter: the
/// user's runtime directory, or one made for them in the temporary directory.
///
/// # Errors
///
/// Fails when the directory cannot be made, or one there already is not this user's
/// alone.
#[cfg(unix)]
fn socket_dir() -> Result<PathBuf, String> {
  use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};

  if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR").filter(|dir| !dir.is_empty()) {
    let dir = PathBuf::from(runtime).join("skene");

    fs::DirBuilder::new()
      .mode(0o700)
      .recursive(true)
      .create(&dir)
      .map_err(|error| format!("make {}: {error}", dir.display()))?;

    return Ok(dir);
  }

  let user = rustix::process::getuid().as_raw();
  let dir = std::env::temp_dir().join(format!("skene-{user}"));

  match fs::DirBuilder::new().mode(0o700).create(&dir) {
    Ok(()) => {}
    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
    Err(error) => return Err(format!("make {}: {error}", dir.display())),
  }

  let metadata =
    fs::symlink_metadata(&dir).map_err(|error| format!("inspect {}: {error}", dir.display()))?;

  if !metadata.is_dir() || metadata.uid() != user || metadata.mode() & 0o077 != 0 {
    return Err(format!(
      "{} is not a directory this user alone may enter",
      dir.display()
    ));
  }

  Ok(dir)
}

/// The socket the `run terminal` loop with id `id` serves its state on.
///
/// # Errors
///
/// Fails as [`socket_dir`] does.
#[cfg(unix)]
fn state_socket(id: u32) -> Result<PathBuf, String> {
  Ok(socket_dir()?.join(format!("{id}.sock")))
}

/// Fetches `path` from the state server of the `run terminal` loop with id `id`: the body of
/// a `200` reply, `None` for any other or for no server.
#[cfg(unix)]
pub(crate) fn fetch(id: u32, path: &str) -> Option<Vec<u8>> {
  let mut stream = std::os::unix::net::UnixStream::connect(state_socket(id).ok()?).ok()?;

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

/// Serves the binary and the notice on a socket in a directory only this user may
/// enter, for a build loop around a target with no page to serve (the terminal);
/// returns the loop's id an `attach` names.
///
/// # Errors
///
/// Fails when the socket cannot be made.
#[cfg(unix)]
pub(crate) fn spawn_state_server(dev: Dev) -> Result<u32, String> {
  let id = std::process::id();
  let path = state_socket(id)?;
  let _ = fs::remove_file(&path);
  let listener = std::os::unix::net::UnixListener::bind(&path)
    .map_err(|error| format!("bind {}: {error}", path.display()))?;

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

  Ok(id)
}

/// The state server runs on Unix alone.
///
/// # Errors
///
/// Always.
#[cfg(not(unix))]
pub fn spawn_state_server(_dev: Dev) -> Result<u32, String> {
  Err("`run terminal` serves its state on a Unix socket, which this system lacks".to_owned())
}

/// Handles one request against the binary and the notice alone.
#[cfg(unix)]
fn handle_state_request(mut stream: std::os::unix::net::UnixStream, dev: &Dev) {
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

/// The live-reload stream's path, which the dev script listens on.
const RELOAD_PATH: &str = "/skene:reload";

/// Serves the app on `port` until interrupted: `index.html` and the build output from
/// the app's `dist/web`, at its own path and as the site root, declared assets at
/// their URIs, and—with `dev`—the reload SSE endpoint and the reload script injected
/// into HTML.
pub(crate) fn serve(app: &LiveApp, port: u16, dev: Option<&Dev>) -> Result<(), String> {
  let listener = TcpListener::bind(("127.0.0.1", port))
    .map_err(|error| format!("failed to bind 127.0.0.1:{port}: {error}"))?;

  println!(
    "serving {} at http://localhost:{port}",
    app.current().dir.display()
  );

  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        let app = app.current();
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

  let headers = if method == "GET" {
    read_headers(&mut reader).unwrap_or_default()
  }
  else {
    Vec::new()
  };

  if !names_loopback(&headers) {
    let _ = write_response(&mut stream, "403 Forbidden", "text/plain", b"forbidden");

    return;
  }

  if let ("GET", RELOAD_PATH, Some(dev)) = (method, target, dev) {
    drop(reader);
    dev.adopt(stream);

    return;
  }

  let Some(relative) = request_path(target)
  else {
    let _ = write_response(&mut stream, "400 Bad Request", "text/plain", b"bad request");

    return;
  };

  match (method, relative.as_str(), dev) {
    ("GET", relative, Some(dev)) if is_route(relative) => {
      let path = encoded_path(target).unwrap_or_else(|| format!("/{relative}"));

      match dev.render(&path, &headers) {
        Some(Ok(page)) => {
          let _ = write_response_with(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            &hint_response_headers(),
            page.as_bytes(),
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

/// The request target's path as a URL carries it: dot segments resolved, still
/// percent-encoded, so it holds no line break and reads as the browser's location
/// does.
fn encoded_path(target: &str) -> Option<String> {
  let base = Url::parse("http://localhost/").ok()?;

  Some(base.join(target).ok()?.path().to_owned())
}

/// Whether a request names this machine as its host, or names none: a page another
/// site served cannot reach the server by a name it resolved to it.
fn names_loopback(headers: &Headers) -> bool {
  let Some((_, host)) = headers
    .iter()
    .find(|(name, _)| name.eq_ignore_ascii_case("host"))
  else {
    return true;
  };
  let name = if host.starts_with('[') {
    host.split_once(']').map_or(host.as_str(), |(name, _)| name)
  }
  else {
    host.split(':').next().unwrap_or_default()
  };

  matches!(
    name.to_ascii_lowercase().as_str(),
    "localhost" | "127.0.0.1" | "[::1"
  )
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

    if segment == "."
      || segment == ".."
      || segment.contains(['/', '\\', ':'])
      || segment.contains(char::is_control)
    {
      return None;
    }

    segments.push(segment.into_owned());
  }

  Some(segments.join("/"))
}

/// Serves one static file: the generated page and bundle, or a declared asset—never
/// the app's sources or manifests; a route no file answers gets the page, so a
/// client-rendered app's own routes load as it navigates.
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
    .or_else(|| app.resolve_asset(relative))
    .or_else(|| {
      Some(app.dir.join("dist/web/index.html")).filter(|page| is_route(relative) && page.is_file())
    });

  let Some(path) = path
  else {
    let _ = write_response(stream, "404 Not Found", "text/plain", b"not found");

    return;
  };

  match fs::read(&path) {
    Ok(body) => {
      let _ = write_response(stream, "200 OK", &content_type(&path), &body);
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

/// The `Content-Type` a file is served with, by its extension: text in UTF-8, and
/// `application/octet-stream` for an extension no media type is registered for.
fn content_type(path: &Path) -> Cow<'static, str> {
  match path.extension().and_then(OsStr::to_str) {
    Some("html") => "text/html; charset=utf-8".into(),
    Some("js" | "mjs") => "text/javascript; charset=utf-8".into(),
    Some("wasm") => "application/wasm".into(),
    Some("css") => "text/css; charset=utf-8".into(),
    Some("json" | "map") => "application/json".into(),
    _ => match mime_guess::from_path(path).first_raw() {
      Some(text) if text.starts_with("text/") => format!("{text}; charset=utf-8").into(),
      Some(media) => media.into(),
      None => "application/octet-stream".into(),
    },
  }
}

fn write_response(
  stream: &mut impl Write,
  status: &str,
  content_type: &str,
  body: &[u8],
) -> std::io::Result<()> {
  write_response_with(stream, status, content_type, &[], body)
}

/// Writes a response carrying `extra` headers beside the content ones.
fn write_response_with(
  stream: &mut impl Write,
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

  /// A request path is decoded segment by segment, and one that would climb, name a
  /// drive, nest a separator or carry a control character is refused.
  #[test]
  fn a_request_path_stays_inside_the_app() {
    assert_eq!(request_path("/a/b%20c?q=1").as_deref(), Some("a/b c"));
    assert_eq!(request_path("/a/./b/../c").as_deref(), Some("a/c"));
    assert_eq!(request_path("/%2e%2e/x").as_deref(), Some("x"));

    for target in [
      "/C:/Windows/win.ini",
      "/c%3A/x",
      "/a%2Fb",
      "/a%5Cb",
      "/a%0A%0A/b",
      "/a%00",
    ] {
      assert_eq!(request_path(target), None, "{target}");
    }
  }

  /// A file is served as its extension's media type, text in UTF-8, and an extension
  /// with none as bytes.
  #[test]
  fn a_file_is_served_as_its_media_type() {
    for (file, expected) in [
      ("index.html", "text/html; charset=utf-8"),
      ("app.js", "text/javascript; charset=utf-8"),
      ("app_bg.wasm", "application/wasm"),
      ("photo.jpg", "image/jpeg"),
      ("icon.ico", "image/x-icon"),
      ("notes.txt", "text/plain; charset=utf-8"),
      ("font.woff2", "font/woff2"),
      ("data.unknown-extension", "application/octet-stream"),
    ] {
      assert_eq!(content_type(Path::new(file)), expected, "{file}");
    }
  }

  /// A route renders from the path as the URL carries it, a line break still encoded.
  #[test]
  fn a_route_renders_from_its_encoded_path() {
    assert_eq!(
      encoded_path("/user/J%C3%BCrgen/../ada?x=1").as_deref(),
      Some("/user/ada")
    );
    assert_eq!(encoded_path("/a b%0A").as_deref(), Some("/a%20b%0A"));
  }

  /// Only a request naming this machine, or naming no host, is served.
  #[test]
  fn only_a_loopback_host_is_served() {
    let host = |value: &str| vec![("Host".to_owned(), value.to_owned())];

    assert!(names_loopback(&Vec::new()));
    assert!(names_loopback(&host("localhost:8080")));
    assert!(names_loopback(&host("127.0.0.1:8080")));
    assert!(names_loopback(&host("[::1]:8080")));
    assert!(names_loopback(&host("LOCALHOST")));
    assert!(!names_loopback(&host("evil.example:8080")));
    assert!(!names_loopback(&host("localhost.evil.example")));
  }
}
