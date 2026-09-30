//! The build script of every crate compiling per skene leaf: it names the leaf a build
//! compiles as `cfg(skene_leaf)` and declares the cfgs a skene build reads, so neither is
//! reported as unexpected.
//!
//! A crate's `build.rs` calls [`configure`] and depends on this crate under
//! `[build-dependencies]`.

use std::env;

/// The web leaf in the browser.
const BROWSER: &str = "browser";
/// The web leaf rendering on a server.
const SERVER: &str = "server";
/// Every other host: the headless platform.
const HOST: &str = "host";

/// Names the leaf this build compiles and declares `skene_leaf` and `skene_platform`.
///
/// The leaf is `server` where the build sets `--cfg skene_platform="server"`, `browser`
/// on `wasm32-unknown-unknown` otherwise, and `host` on every other target.
///
/// # Panics
///
/// Panics, failing the build, for a `skene_platform` naming no platform that builds, or
/// for a wasm target a skene build has no leaf for (wasip1, wasip2, emscripten,
/// wasm32v1-none).
pub fn configure() {
  println!(
    "cargo::rustc-check-cfg=cfg(skene_leaf, values(\"{BROWSER}\", \"{SERVER}\", \"{HOST}\"))"
  );
  println!("cargo::rustc-check-cfg=cfg(skene_platform, values(\"{SERVER}\", \"native\"))");
  println!("cargo::rerun-if-env-changed=CARGO_CFG_SKENE_PLATFORM");
  println!("cargo::rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");
  println!("cargo::rerun-if-env-changed=CARGO_CFG_TARGET_OS");

  let variable = |name: &str| env::var(name).unwrap_or_default();
  let leaf = leaf(
    &variable("CARGO_CFG_TARGET_ARCH"),
    &variable("CARGO_CFG_TARGET_OS"),
    &variable("CARGO_CFG_SKENE_PLATFORM"),
  )
  .unwrap_or_else(|reason| panic!("{reason}"));

  println!("cargo::rustc-cfg=skene_leaf=\"{leaf}\"");
}

/// The leaf a build for `arch` and `os` compiles under the platform the build names.
fn leaf(arch: &str, os: &str, platform: &str) -> Result<&'static str, String> {
  match platform {
    SERVER => return Ok(SERVER),
    "" => {}
    "native" => {
      return Err(
        "skene_platform = \"native\" names the native platform, which does not build yet"
          .to_owned(),
      );
    }
    other => {
      return Err(format!(
        "skene_platform = \"{other}\" names no platform; the one a build may name is \"{SERVER}\""
      ));
    }
  }

  match (arch, os) {
    ("wasm32", "unknown") => Ok(BROWSER),
    ("wasm32" | "wasm64", _) => Err(format!(
      "a skene build has no leaf for {arch}-{os}: the browser's target is wasm32-unknown-unknown"
    )),
    _ => Ok(HOST),
  }
}

#[cfg(test)]
mod tests {
  use super::leaf;

  /// The server cfg wins on every target; otherwise the browser's triple is the browser
  /// leaf and every other host the headless one.
  #[test]
  fn a_build_names_its_leaf() {
    assert_eq!(leaf("x86_64", "linux", ""), Ok("host"));
    assert_eq!(leaf("aarch64", "macos", ""), Ok("host"));
    assert_eq!(leaf("wasm32", "unknown", ""), Ok("browser"));
    assert_eq!(leaf("x86_64", "linux", "server"), Ok("server"));
    assert_eq!(leaf("wasm32", "unknown", "server"), Ok("server"));
  }

  /// An unknown platform, the reserved native one and a wasm target without a leaf fail
  /// the build rather than landing on the host leaf.
  #[test]
  fn a_build_without_a_leaf_is_rejected() {
    assert!(leaf("x86_64", "linux", "servr").is_err());
    assert!(leaf("x86_64", "linux", "native").is_err());
    assert!(leaf("wasm32", "wasi", "").is_err());
    assert!(leaf("wasm32", "emscripten", "").is_err());
    assert!(leaf("wasm32", "none", "").is_err());
  }
}
