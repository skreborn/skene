//! The dev server: static files, manifest assets, and live-reload over SSE.

use std::{
  ffi::OsStr,
  fs,
  io::{BufRead, BufReader, Write},
  net::{TcpListener, TcpStream},
  path::Path,
  sync::{Arc, Mutex},
  thread,
};

use crate::manifest::App;

/// The `<script>` a served HTML page gains in dev, reloading it when the server says so.
const RELOAD_SCRIPT: &str =
  "<script>new EventSource(\"/__skene/reload\").onmessage = () => location.reload();</script>";

/// The open live-reload streams, told to reload after each successful rebuild.
#[derive(Clone, Default)]
pub struct Reload(Arc<Mutex<Vec<TcpStream>>>);

impl Reload {
  /// Tells every connected page to reload, dropping streams that went away.
  pub fn trigger(&self) {
    let mut clients = self
      .0
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
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(stream);
    }
  }
}

/// Serves the app on `port` until interrupted: `index.html` and `dist/` from the app
/// directory, declared assets at their URIs, and—with `reload`—the SSE endpoint plus
/// the reload script injected into HTML.
pub fn serve(app: &Arc<App>, port: u16, reload: Option<&Reload>) -> Result<(), String> {
  let listener = TcpListener::bind(("127.0.0.1", port))
    .map_err(|error| format!("failed to bind 127.0.0.1:{port}: {error}"))?;

  println!("serving {} at http://localhost:{port}", app.dir.display());

  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        let app = Arc::clone(app);
        let reload = reload.cloned();

        thread::spawn(move || handle_request(stream, &app, reload.as_ref()));
      }
      Err(error) => eprintln!("connection failed: {error}"),
    }
  }

  Ok(())
}

fn handle_request(mut stream: TcpStream, app: &App, reload: Option<&Reload>) {
  let Some(target) = read_target(&stream)
  else {
    let _ = write_response(&mut stream, "400 Bad Request", "text/plain", b"bad request");

    return;
  };

  if target == "/__skene/reload" {
    if let Some(reload) = reload {
      reload.adopt(stream);
    }

    return;
  }

  let relative = target.trim_start_matches('/');
  let relative = if relative.is_empty() {
    "index.html"
  }
  else {
    relative
  };

  // Only the page, the generated bundle, and declared assets are reachable—never the
  // app's sources or manifests.
  let path = if relative == "index.html" || relative.starts_with("dist/") {
    Some(app.dir.join(relative)).filter(|path| path.is_file())
  }
  else {
    None
  };
  let path = path.or_else(|| app.resolve_asset(relative));

  let Some(path) = path
  else {
    let _ = write_response(&mut stream, "404 Not Found", "text/plain", b"not found");

    return;
  };

  match fs::read(&path) {
    Ok(mut body) => {
      let kind = content_type(&path);

      if reload.is_some() && kind.starts_with("text/html") {
        body.extend_from_slice(RELOAD_SCRIPT.as_bytes());
      }

      let _ = write_response(&mut stream, "200 OK", kind, &body);
    }
    Err(_) => {
      let _ = write_response(
        &mut stream,
        "500 Internal Server Error",
        "text/plain",
        b"read error",
      );
    }
  }
}

/// Parses the request path from the first line, rejecting directory traversal.
fn read_target(stream: &TcpStream) -> Option<String> {
  let mut line = String::new();

  BufReader::new(stream).read_line(&mut line).ok()?;

  let target = line.split_whitespace().nth(1)?;
  let target = target.split(['?', '#']).next().unwrap_or(target);

  if target.contains("..") {
    return None;
  }

  Some(target.to_string())
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
