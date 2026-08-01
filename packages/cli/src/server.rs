//! The dev server: static files, manifest assets, live-reload over SSE, and the
//! authoritative state snapshot held across reloads.

use std::{
  ffi::OsStr,
  fs,
  io::{BufRead, BufReader, Read, Write},
  net::{TcpListener, TcpStream},
  path::Path,
  sync::{Arc, Mutex},
  thread,
};

use percent_encoding::percent_decode_str;
use url::Url;

use crate::manifest::App;

/// The dev channel: the open live-reload streams, and the authoritative state snapshot
/// held across reloads.
#[derive(Clone, Default)]
pub struct Dev {
  clients: Arc<Mutex<Vec<TcpStream>>>,
  state: Arc<Mutex<Option<Vec<u8>>>>,
}

impl Dev {
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
}

/// Serves the app on `port` until interrupted: `index.html` and `dist/web/` from the
/// app directory, declared assets at their URIs, and—with `dev`—the reload SSE
/// endpoint, the state slot, and the reload script injected into HTML.
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

    ("GET", relative, _) => serve_file(&mut stream, app, relative),

    _ => {
      let _ = write_response(&mut stream, "404 Not Found", "text/plain", b"not found");
    }
  }
}

/// Reads the request body: the remaining headers for `Content-Length`, then exactly
/// that many bytes.
fn read_body(reader: &mut BufReader<TcpStream>) -> Option<Vec<u8>> {
  let mut length: Option<usize> = None;

  loop {
    let mut line = String::new();

    reader.read_line(&mut line).ok()?;

    let line = line.trim_end();

    if line.is_empty() {
      break;
    }

    if let Some(value) = line
      .to_ascii_lowercase()
      .strip_prefix("content-length:")
      .map(str::trim)
    {
      length = value.parse().ok();
    }
  }

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

  let path = if relative.starts_with("dist/web/") {
    Some(app.dir.join(relative)).filter(|path| path.is_file())
  }
  else {
    None
  };
  let path = path.or_else(|| app.resolve_asset(relative));

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
  let header = format!(
    "HTTP/1.1 {status}\r\n\
     Content-Type: {content_type}\r\n\
     Content-Length: {}\r\n\
     Connection: close\r\n\r\n",
    body.len()
  );

  stream.write_all(header.as_bytes())?;
  stream.write_all(body)?;
  stream.flush()
}
