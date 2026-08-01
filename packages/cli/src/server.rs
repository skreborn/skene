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

use crate::manifest::App;

/// The `<script>` a served HTML page gains in dev: it beacons the app's state snapshot
/// to the CLI before every reload (the app exposes `skene_snapshot` when built with the
/// `snapshot` feature), and reloads when the server says so.
const RELOAD_SCRIPT: &str = "<meta name=\"skene-dev\"><script>(() => {\n\
  const save = () => {\n\
    if (window.skene_snapshot) navigator.sendBeacon(\"/skene:state\", window.skene_snapshot());\n\
  };\n\
  addEventListener(\"pagehide\", save);\n\
  new EventSource(\"/skene:reload\").onmessage = () => {\n\
    save();\n\
    location.reload();\n\
  };\n\
})();</script>";

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

  let target = target.split(['?', '#']).next().unwrap_or(target);

  if target.contains("..") {
    let _ = write_response(&mut stream, "400 Bad Request", "text/plain", b"bad request");

    return;
  }

  match (method, target, dev) {
    ("GET", "/skene:reload", Some(dev)) => {
      drop(reader);
      dev.adopt(stream);
    }

    ("POST", "/skene:state", Some(dev)) => match read_body(&mut reader) {
      Some(body) => {
        dev.store(body);

        let _ = write_response(&mut stream, "204 No Content", "text/plain", b"");
      }
      None => {
        let _ = write_response(&mut stream, "400 Bad Request", "text/plain", b"bad request");
      }
    },

    ("GET", "/skene:state", Some(dev)) => match dev.stored() {
      Some(bytes) => {
        let _ = write_response(&mut stream, "200 OK", "application/octet-stream", &bytes);
      }
      None => {
        let _ = write_response(&mut stream, "404 Not Found", "text/plain", b"no state");
      }
    },

    ("GET", target, _) => serve_file(&mut stream, app, target, dev.is_some()),

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

/// Serves one static file: the page, the generated bundle, or a declared asset—never
/// the app's sources or manifests.
fn serve_file(stream: &mut TcpStream, app: &App, target: &str, dev: bool) {
  let relative = target.trim_start_matches('/');
  let relative = if relative.is_empty() {
    "index.html"
  }
  else {
    relative
  };

  let path = if relative == "index.html" || relative.starts_with("dist/web/") {
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
    Ok(mut body) => {
      let kind = content_type(&path);

      if dev && kind.starts_with("text/html") {
        body.extend_from_slice(RELOAD_SCRIPT.as_bytes());
      }

      let _ = write_response(stream, "200 OK", kind, &body);
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
