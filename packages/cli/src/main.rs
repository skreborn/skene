//! The `skene` command: build, statically generate, and develop skene applications.
//!
//! Reads `skene.toml`—the framework manifest—beside the app's `Cargo.toml`; every
//! setting is optional. `skene dev` is the dev loop: build, serve, watch, rebuild, and
//! reload the page over SSE.

#[cfg(not(target_arch = "wasm32"))]
mod attach;
#[cfg(not(target_arch = "wasm32"))]
mod command;
#[cfg(not(target_arch = "wasm32"))]
mod dev;
#[cfg(not(target_arch = "wasm32"))]
mod manifest;
#[cfg(not(target_arch = "wasm32"))]
mod pipeline;
#[cfg(not(target_arch = "wasm32"))]
mod render;
#[cfg(not(target_arch = "wasm32"))]
mod server;
#[cfg(not(target_arch = "wasm32"))]
mod split;
#[cfg(not(target_arch = "wasm32"))]
mod tty;

#[cfg(target_arch = "wasm32")]
const fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
  use clap::Parser;

  match command::Cli::parse().run() {
    Ok(()) => std::process::ExitCode::SUCCESS,
    Err(message) => {
      eprintln!("error: {message}");

      std::process::ExitCode::FAILURE
    }
  }
}
