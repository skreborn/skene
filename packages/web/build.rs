//! Names the leaf this build compiles: the browser on `wasm32-unknown-unknown`, the
//! server where the build sets `--cfg skene_platform="server"`, the host elsewhere.

fn main() {
  println!("cargo::rustc-check-cfg=cfg(skene_leaf, values(\"browser\", \"server\", \"host\"))");
  println!("cargo::rerun-if-env-changed=CARGO_CFG_SKENE_PLATFORM");
  println!("cargo::rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");
  println!("cargo::rerun-if-env-changed=CARGO_CFG_TARGET_OS");

  let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
  let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
  let platform = std::env::var("CARGO_CFG_SKENE_PLATFORM").unwrap_or_default();
  let leaf = if arch == "wasm32" && os == "unknown" {
    "browser"
  }
  else if platform == "server" {
    "server"
  }
  else {
    "host"
  };

  println!("cargo::rustc-cfg=skene_leaf=\"{leaf}\"");
}
