//! Embeds a skene app's server build into any Rust program.
//!
//! The app's `wasm32-wasip2` build is a plain command: given a path argument, it writes
//! the rendered page to stdout. [`Runner`] wraps a wasm runtime around that contract so a
//! server renders routes with a function call—no protocol, no bindings, and the same
//! artifact still works standalone (`wasmtime app.wasm /path`).
//!
//! This is a *library* a user's server embeds; skene ships no server. It is host-only:
//! on a wasm target the crate is empty.

#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;

use anyhow::{Context, Result};
use wasmtime::{
  Engine,
  Store,
  component::{Component, Linker},
};
use wasmtime_wasi::p2::{IoView, WasiCtx, WasiCtxBuilder, WasiView, pipe::MemoryOutputPipe};

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

/// The per-instance host state: the WASI context each render runs against.
struct Host {
  ctx: WasiCtx,
  table: wasmtime::component::ResourceTable,
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
    let stdout = MemoryOutputPipe::new(usize::MAX);
    let ctx = WasiCtxBuilder::new()
      .stdout(stdout.clone())
      .inherit_stderr()
      .args(&["app", route_path])
      .build();

    let host = Host {
      ctx,
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
      .map_err(|()| anyhow::anyhow!("app exited with failure"))?;

    drop(store);

    let bytes = stdout.contents();

    String::from_utf8(bytes.to_vec()).context("rendered output is not UTF-8")
  }
}
