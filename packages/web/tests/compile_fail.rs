//! The rules the types enforce, each a program that must not compile, with the
//! diagnostic it raises beside it: layout's typed children, lengths as plain numbers,
//! the expression kinds, a choice's value, the derives and the entry.
//!
//! `TRYBUILD=overwrite` rewrites the diagnostics once a change to one is meant.

#![cfg(all(skene_platform = "web", skene_server))]

#[test]
fn what_the_rules_forbid_does_not_compile() {
  name_the_server_leaf();
  trybuild::TestCases::new().compile_fail("tests/compile_fail/*.rs");
}

/// Names the server leaf for the programs' builds, on the host's target, as the build of
/// this test named it: trybuild's cargo reads a target's flags from the environment
/// alone, never from the command that built the test.
fn name_the_server_leaf() {
  const LEAF: &str = r#"--cfg skene_platform="web" --cfg skene_server"#;

  let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
  let version = std::process::Command::new(cargo)
    .arg("-vV")
    .output()
    .expect("cargo reports its host");
  let host = String::from_utf8_lossy(&version.stdout)
    .lines()
    .find_map(|line| line.strip_prefix("host: "))
    .expect("cargo reports its host")
    .to_uppercase()
    .replace(['-', '.'], "_");
  let variable = format!("CARGO_TARGET_{host}_RUSTFLAGS");
  let flags = std::env::var(&variable).unwrap_or_default();

  if !flags.contains("skene_server") {
    // SAFETY: the test is its binary's only one, so no other thread reads the
    // environment while it is written.
    unsafe { std::env::set_var(&variable, format!("{flags} {LEAF}").trim_start()) };
  }
}
