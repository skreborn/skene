//! Embeds a skene app's server build into any Rust program.
//!
//! The app's `wasm32-wasip2` build is a plain command: given a path argument, it writes
//! the rendered page to stdout, answering the client hints the `SKENE_HINTS` variable
//! carries. [`Runner`] wraps a wasm runtime around that contract so a server renders
//! routes with a function call—no protocol, no bindings, and the same artifact still
//! works standalone (`wasmtime app.wasm /path`). A server hands each request's headers
//! to [`Runner::render_with_headers`] and adds [`hint_response_headers`] to its
//! response, so a client that can send hints does.
//!
//! This is a *library* a user's server embeds; skene ships no server. It is host-only:
//! on a wasm target the crate is empty.

#![cfg(not(target_arch = "wasm32"))]

use std::{path::Path, sync::mpsc};

use anyhow::{Context, Result};
use bytes::Bytes;
use wasmtime::{
  Engine,
  Store,
  component::{Component, Linker},
};
use wasmtime_wasi::p2::{
  IoView,
  OutputStream,
  Pollable,
  StdoutStream,
  StreamResult,
  WasiCtx,
  WasiCtxBuilder,
  WasiView,
  pipe::MemoryOutputPipe,
};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

/// The environment variable the app reads its client hints from: the request's
/// `Sec-CH-*` headers, one `Name: value` per line.
const HINTS_VARIABLE: &str = "SKENE_HINTS";

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

/// The `Sec-CH-*` headers among `headers`, in the form the app reads.
fn hint_lines<'a>(headers: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
  use std::fmt::Write as _;

  headers
    .into_iter()
    .filter(|(name, _)| name.len() >= 7 && name[..7].eq_ignore_ascii_case("sec-ch-"))
    .fold(String::new(), |mut lines, (name, value)| {
      let _ = writeln!(lines, "{name}: {value}");

      lines
    })
}

/// A loaded skene app, ready to render routes.
///
/// Loading compiles the component once; each [`render`](Self::render) call instantiates
/// a fresh, isolated instance of it.
#[must_use]
pub struct Runner {
  engine: Engine,
  component: Component,
  linker: Linker<Host>,
}

/// The per-instance host state: the WASI context each render runs against, and the
/// outgoing HTTP the render may make through `wasi:http`.
struct Host {
  ctx: WasiCtx,
  http: WasiHttpCtx,
  table: wasmtime::component::ResourceTable,
}

impl WasiHttpView for Host {
  fn ctx(&mut self) -> &mut WasiHttpCtx {
    &mut self.http
  }
}

impl IoView for Host {
  fn table(&mut self) -> &mut wasmtime::component::ResourceTable {
    &mut self.table
  }
}

impl WasiView for Host {
  fn ctx(&mut self) -> &mut WasiCtx {
    &mut self.ctx
  }
}

impl Runner {
  /// Loads and compiles the app component at `path` (the `wasm32-wasip2` build).
  ///
  /// # Errors
  ///
  /// Fails when the file cannot be read or is not a valid component.
  pub fn load(path: impl AsRef<Path>) -> Result<Self> {
    let engine = Engine::default();
    let component = Component::from_file(&engine, path.as_ref())
      .with_context(|| format!("load component {}", path.as_ref().display()))?;

    let mut linker = Linker::new(&engine);

    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).context("link WASI")?;
    wasmtime_wasi_http::add_only_http_to_linker_sync(&mut linker).context("link wasi:http")?;

    Ok(Self {
      engine,
      component,
      linker,
    })
  }

  /// Renders `route_path`, returning the complete HTML document the app produced.
  ///
  /// # Errors
  ///
  /// Fails when the app traps, exits with failure, or writes non-UTF-8 output.
  pub fn render(&self, route_path: &str) -> Result<String> {
    self.invoke(route_path, "")
  }

  /// Renders `route_path` against the client hints among the request `headers`
  /// (`Sec-CH-Prefers-Color-Scheme`, `Sec-CH-Viewport-Width` and their kin); every
  /// other header is dropped before the app sees it.
  ///
  /// # Errors
  ///
  /// Fails when the app traps, exits with failure, or writes non-UTF-8 output.
  pub fn render_with_headers<'a>(
    &self,
    route_path: &str,
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
  ) -> Result<String> {
    self.invoke(route_path, &hint_lines(headers))
  }

  /// The routes the app declares statically renderable, in declaration order; `/`
  /// alone when the app declares none.
  ///
  /// # Errors
  ///
  /// Fails when the app traps, exits with failure, or writes non-UTF-8 output.
  pub fn routes(&self) -> Result<Vec<String>> {
    let output = self.invoke("--routes", "")?;

    Ok(
      output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect(),
    )
  }

  /// Renders `route_path` as a progressive stream: `sink` receives each piece of
  /// output as the app flushes it, while the render is still running—the shell first,
  /// a chunk per deferred boundary as it settles, the closing tail last. The pieces
  /// concatenate to the full document.
  ///
  /// # Errors
  ///
  /// Fails when the app traps or exits with failure.
  pub fn render_stream(&self, route_path: &str, sink: impl FnMut(&[u8])) -> Result<()> {
    self.render_stream_with_headers(route_path, [], sink)
  }

  /// [`render_stream`](Self::render_stream) against the client hints among the request
  /// `headers`, as [`render_with_headers`](Self::render_with_headers) reads them.
  ///
  /// # Errors
  ///
  /// Fails when the app traps or exits with failure.
  pub fn render_stream_with_headers<'a>(
    &self,
    route_path: &str,
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
    mut sink: impl FnMut(&[u8]),
  ) -> Result<()> {
    let (sender, receiver) = mpsc::channel();
    let pipe = ChannelPipe { sender };
    let hints = hint_lines(headers);

    std::thread::scope(|scope| {
      let guest = scope.spawn(move || self.run(&["app", "--stream", route_path], &hints, pipe));

      // Every sender lives in the guest's store; the loop ends when the run finishes
      // and drops it.
      for bytes in receiver {
        sink(&bytes);
      }

      guest
        .join()
        .map_err(|_| anyhow::anyhow!("the render thread panicked"))?
    })
  }

  /// Runs one fresh instance of the app with `argument` and the hint lines `hints`,
  /// returning its stdout.
  fn invoke(&self, argument: &str, hints: &str) -> Result<String> {
    let stdout = MemoryOutputPipe::new(usize::MAX);

    self.run(&["app", argument], hints, stdout.clone())?;

    let bytes = stdout.contents();

    String::from_utf8(bytes.to_vec()).context("rendered output is not UTF-8")
  }

  /// Runs one fresh instance of the app with `arguments` and the hint lines `hints`,
  /// its stdout wired to `stdout`.
  fn run(
    &self,
    arguments: &[&str],
    hints: &str,
    stdout: impl StdoutStream + 'static,
  ) -> Result<()> {
    let mut builder = WasiCtxBuilder::new();

    builder.stdout(stdout).inherit_stderr().args(arguments);

    if !hints.is_empty() {
      builder.env(HINTS_VARIABLE, hints);
    }

    let ctx = builder.build();

    let host = Host {
      ctx,
      http: WasiHttpCtx::new(),
      table: wasmtime::component::ResourceTable::new(),
    };

    let mut store = Store::new(&self.engine, host);
    let command = wasmtime_wasi::p2::bindings::sync::Command::instantiate(
      &mut store,
      &self.component,
      &self.linker,
    )
    .context("instantiate app")?;

    command
      .wasi_cli_run()
      .call_run(&mut store)
      .context("run app")?
      .map_err(|()| anyhow::anyhow!("app exited with failure"))
  }
}

/// Guest stdout forwarded through a channel write by write, so a caller streams the
/// output while the render still runs.
#[derive(Clone)]
struct ChannelPipe {
  sender: mpsc::Sender<Vec<u8>>,
}

#[async_trait::async_trait]
impl Pollable for ChannelPipe {
  async fn ready(&mut self) {}
}

impl OutputStream for ChannelPipe {
  fn write(&mut self, bytes: Bytes) -> StreamResult<()> {
    // A closed receiver means the caller stopped listening; the render finishes
    // regardless, as it would printing to a closed pipe.
    let _ = self.sender.send(bytes.to_vec());

    Ok(())
  }

  fn flush(&mut self) -> StreamResult<()> {
    Ok(())
  }

  fn check_write(&mut self) -> StreamResult<usize> {
    Ok(usize::MAX)
  }
}

impl StdoutStream for ChannelPipe {
  fn stream(&self) -> Box<dyn OutputStream> {
    Box::new(self.clone())
  }

  fn isatty(&self) -> bool {
    false
  }
}
