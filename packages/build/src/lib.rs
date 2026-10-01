//! The build script of every crate compiling per web leaf: it names the leaf a build
//! compiles for as `cfg(skene_web_platform)` and declares the cfgs a skene build reads,
//! so neither is reported as unexpected.
//!
//! A crate's `build.rs` calls [`configure`] and depends on this crate under
//! `[build-dependencies]`.

use std::env;

/// The web's leaf in the browser.
const BROWSER: &str = "browser";
/// The web's leaf rendering on a server.
const SERVER: &str = "server";
/// The headless platform, on every other host.
const HEADLESS: &str = "headless";

/// Names the web leaf this build compiles for and declares `skene_web_platform` and
/// `skene_platform`.
///
/// The leaf is `server` where the build sets `--cfg skene_platform="server"`, `browser`
/// on `wasm32-unknown-unknown` otherwise, and `headless` on every other target.
///
/// # Panics
///
/// Panics, failing the build, for a `skene_platform` naming no leaf, or for a wasm
/// target the web has no leaf for (wasip1, wasip2, emscripten, wasm32v1-none).
pub fn configure() {
  println!(
    "cargo::rustc-check-cfg=cfg(skene_web_platform, values(\"{BROWSER}\", \"{SERVER}\", \"{HEADLESS}\"))"
  );
  println!("cargo::rustc-check-cfg=cfg(skene_platform, values(\"{SERVER}\"))");
  println!("cargo::rerun-if-env-changed=CARGO_CFG_SKENE_PLATFORM");
  println!("cargo::rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");
  println!("cargo::rerun-if-env-changed=CARGO_CFG_TARGET_OS");

  let variable = |name: &str| env::var(name).unwrap_or_default();
  let web_platform = web_platform(
    &variable("CARGO_CFG_TARGET_ARCH"),
    &variable("CARGO_CFG_TARGET_OS"),
    &variable("CARGO_CFG_SKENE_PLATFORM"),
  )
  .unwrap_or_else(|reason| panic!("{reason}"));

  println!("cargo::rustc-cfg=skene_web_platform=\"{web_platform}\"");
}

/// The web leaf a build for `arch` and `os` compiles for under the platform the build
/// names.
fn web_platform(arch: &str, os: &str, platform: &str) -> Result<&'static str, String> {
  match platform {
    SERVER => return Ok(SERVER),
    "" => {}
    other => {
      return Err(format!(
        "skene_platform = \"{other}\" names no platform; the one a build may name is \"{SERVER}\""
      ));
    }
  }

  match (arch, os) {
    ("wasm32", "unknown") => Ok(BROWSER),
    ("wasm32" | "wasm64", _) => Err(format!(
      "the web has no leaf for {arch}-{os}: the browser's target is wasm32-unknown-unknown"
    )),
    _ => Ok(HEADLESS),
  }
}

#[cfg(test)]
mod tests {
  use super::web_platform;

  /// The server cfg wins on every target; otherwise the browser's triple is the browser
  /// leaf and every other host the headless platform.
  #[test]
  fn a_build_names_its_web_platform() {
    assert_eq!(web_platform("x86_64", "linux", ""), Ok("headless"));
    assert_eq!(web_platform("aarch64", "macos", ""), Ok("headless"));
    assert_eq!(web_platform("wasm32", "unknown", ""), Ok("browser"));
    assert_eq!(web_platform("x86_64", "linux", "server"), Ok("server"));
    assert_eq!(web_platform("wasm32", "unknown", "server"), Ok("server"));
  }

  /// An unknown platform and a wasm target without a web leaf fail the build rather
  /// than landing on the headless platform.
  #[test]
  fn a_build_without_a_web_platform_is_rejected() {
    assert!(web_platform("x86_64", "linux", "servr").is_err());
    assert!(web_platform("x86_64", "linux", "native").is_err());
    assert!(web_platform("wasm32", "wasi", "").is_err());
    assert!(web_platform("wasm32", "emscripten", "").is_err());
    assert!(web_platform("wasm32", "none", "").is_err());
  }
}
