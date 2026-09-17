//! The app's server binary as a renderer: a worker process rendering one request
//! after another over its stdin and stdout, and the routes it enumerates.

use std::{
  io::{BufRead, BufReader, Read, Write},
  path::{Path, PathBuf},
  process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

/// The client hints a render reads, as the response asks for them.
const HINT_NAMES: &str = "Sec-CH-Prefers-Color-Scheme, Sec-CH-Prefers-Reduced-Motion, \
                          Sec-CH-Viewport-Width, Sec-CH-Viewport-Height, Sec-CH-DPR";

/// The response headers a server adds beside a rendered page, so a client sends the
/// hints the next render answers with: `Accept-CH`, `Critical-CH` and `Vary`.
#[must_use]
pub const fn hint_response_headers() -> [(&'static str, &'static str); 3] {
  [
    ("Accept-CH", HINT_NAMES),
    ("Critical-CH", HINT_NAMES),
    ("Vary", HINT_NAMES),
  ]
}

/// The app's server binary, kept running as a render worker.
///
/// Loading spawns the binary with `--serve`; each [`render`](Self::render) writes one
/// request to its stdin and reads the page back, and a worker that died is spawned
/// afresh for the next request.
pub struct Renderer {
  binary: PathBuf,
  dir: PathBuf,
  worker: Option<Worker>,
}

/// One spawned worker: the child and its pipes.
struct Worker {
  child: Child,
  stdin: ChildStdin,
  stdout: BufReader<ChildStdout>,
}

impl Worker {
  fn spawn(binary: &Path, dir: &Path) -> Result<Self, String> {
    let mut child = Command::new(binary)
      .arg("--serve")
      .current_dir(dir)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .spawn()
      .map_err(|error| format!("{}: {error}", binary.display()))?;
    let stdin = child.stdin.take().ok_or("the worker has no stdin")?;
    let stdout = child.stdout.take().ok_or("the worker has no stdout")?;

    Ok(Self {
      child,
      stdin,
      stdout: BufReader::new(stdout),
    })
  }

  /// Writes one request and reads its page.
  fn render(&mut self, route: &str, hints: &str) -> Result<String, String> {
    write!(self.stdin, "{route}\n{hints}\n")
      .and_then(|()| self.stdin.flush())
      .map_err(|error| format!("write to the render worker: {error}"))?;

    let mut header = String::new();

    self
      .stdout
      .read_line(&mut header)
      .map_err(|error| format!("read from the render worker: {error}"))?;

    let length: usize = header
      .trim()
      .strip_prefix("Content-Length:")
      .and_then(|length| length.trim().parse().ok())
      .ok_or_else(|| format!("the render worker answered {header:?}"))?;
    let mut blank = String::new();

    self
      .stdout
      .read_line(&mut blank)
      .map_err(|error| format!("read from the render worker: {error}"))?;

    let mut page = vec![0; length];

    self
      .stdout
      .read_exact(&mut page)
      .map_err(|error| format!("read the page from the render worker: {error}"))?;

    String::from_utf8(page).map_err(|_| "the rendered page is not UTF-8".to_owned())
  }
}

impl Drop for Worker {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

impl Renderer {
  /// Spawns the server binary at `binary` in `dir` as a render worker.
  ///
  /// # Errors
  ///
  /// Fails when the binary cannot be spawned.
  pub fn load(binary: PathBuf, dir: PathBuf) -> Result<Self, String> {
    let worker = Worker::spawn(&binary, &dir)?;

    Ok(Self {
      binary,
      dir,
      worker: Some(worker),
    })
  }

  /// Renders `route` against the client hints among the request `headers`
  /// (`Sec-CH-Prefers-Color-Scheme`, `Sec-CH-Viewport-Width` and their kin); every
  /// other header is dropped before the app sees it.
  ///
  /// # Errors
  ///
  /// Fails when the worker cannot be spawned or answers malformed output.
  pub fn render<'a>(
    &mut self,
    route: &str,
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
  ) -> Result<String, String> {
    let hints = hint_lines(headers);

    if self.worker.is_none() {
      self.worker = Some(Worker::spawn(&self.binary, &self.dir)?);
    }

    let outcome = self
      .worker
      .as_mut()
      .expect("spawned above")
      .render(route, &hints);

    // A worker that failed is dropped, so the next request starts a fresh one.
    if outcome.is_err() {
      self.worker = None;
    }

    outcome
  }

  /// The routes the app declares statically renderable, in declaration order; `/`
  /// alone when the app declares none.
  ///
  /// # Errors
  ///
  /// Fails when the binary cannot be run or writes non-UTF-8 output.
  pub fn routes(&self) -> Result<Vec<String>, String> {
    let output = Command::new(&self.binary)
      .arg("--routes")
      .current_dir(&self.dir)
      .output()
      .map_err(|error| format!("{}: {error}", self.binary.display()))?;

    if !output.status.success() {
      return Err(format!(
        "{} --routes exited with {}",
        self.binary.display(),
        output.status
      ));
    }

    let text = String::from_utf8(output.stdout).map_err(|_| "the routes are not UTF-8")?;

    Ok(
      text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect(),
    )
  }
}

/// The `Sec-CH-*` headers among `headers`, in the form the worker reads.
fn hint_lines<'a>(headers: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
  headers
    .into_iter()
    .filter(|(name, _)| name.len() >= 7 && name[..7].eq_ignore_ascii_case("sec-ch-"))
    .fold(String::new(), |mut lines, (name, value)| {
      use std::fmt::Write as _;

      let _ = writeln!(lines, "{name}: {value}");

      lines
    })
}
