//! The built app, served over HTTP from a directory: a path naming a file serves the
//! file, and every other path serves the app's page, which builds the page the path
//! names.

use std::{
  fs,
  io::{BufRead, BufReader, Write},
  net::{TcpListener, TcpStream},
  path::{Component, Path, PathBuf},
  thread,
};

/// The app served on a local port for as long as the harness runs.
pub(crate) struct Site {
  /// The origin it is served at, as `http://127.0.0.1:{port}`.
  origin: String,
}

impl Site {
  /// Serves the directory `root`, which holds the app's `index.html`, on a free local
  /// port.
  pub(crate) fn serve(root: PathBuf) -> Result<Self, String> {
    if !root.join("index.html").is_file() {
      return Err(format!("{} holds no built app", root.display()));
    }

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let origin = format!(
      "http://{}",
      listener.local_addr().map_err(|error| error.to_string())?
    );

    thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        let root = root.clone();

        thread::spawn(move || {
          let _ = respond(&root, stream);
        });
      }
    });

    Ok(Self { origin })
  }

  /// The URL of the page named `page`.
  pub(crate) fn page(&self, page: &str) -> String {
    format!("{}/{page}", self.origin)
  }
}

/// Answers the one request `stream` carries from the files under `root`.
fn respond(root: &Path, mut stream: TcpStream) -> std::io::Result<()> {
  let mut reader = BufReader::new(stream.try_clone()?);
  let mut line = String::new();

  reader.read_line(&mut line)?;

  let path = line
    .split_whitespace()
    .nth(1)
    .unwrap_or("/")
    .split(['?', '#'])
    .next()
    .unwrap_or("/")
    .to_owned();

  loop {
    let mut header = String::new();

    if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
      break;
    }
  }

  let file = file(root, &path).unwrap_or_else(|| root.join("index.html"));
  let body = fs::read(&file)?;
  let head = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: \
     no-store\r\nConnection: close\r\n\r\n",
    content_type(&file),
    body.len()
  );

  stream.write_all(head.as_bytes())?;
  stream.write_all(&body)
}

/// The file under `root` the request path `path` names, where it names one.
fn file(root: &Path, path: &str) -> Option<PathBuf> {
  let relative = Path::new(path.trim_start_matches('/'));

  if relative
    .components()
    .any(|component| !matches!(component, Component::Normal(_)))
  {
    return None;
  }

  Some(root.join(relative)).filter(|file| file.is_file())
}

/// The media type a file of `file`'s extension is served as.
fn content_type(file: &Path) -> &'static str {
  match file.extension().and_then(|extension| extension.to_str()) {
    Some("html") => "text/html; charset=utf-8",
    Some("js") => "text/javascript; charset=utf-8",
    Some("wasm") => "application/wasm",
    Some("css") => "text/css; charset=utf-8",
    Some("woff2") => "font/woff2",
    Some("ttf") => "font/ttf",
    Some("png") => "image/png",
    Some("svg") => "image/svg+xml",
    _ => "application/octet-stream",
  }
}
