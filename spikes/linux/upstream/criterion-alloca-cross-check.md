# criterion 0.8: the unconditional `alloca` dependency breaks cross-target checks

Repository: bheisler/criterion.rs. Not filed.

## What happens

criterion 0.8.0 to 0.8.2 depend on `alloca = "0.4"` (in 0.8.2 under
`cfg(any(windows, unix))`), with no feature to turn it off. alloca's build script
compiles `alloca.c` with cc-rs for the *target*. A crate that keeps criterion as a
dev-dependency can then no longer be checked for another target than the host's
without that target's C toolchain: `cargo clippy --all-targets --target
aarch64-apple-darwin` on x86-64 Linux fails before any Rust is checked.

```
warning: alloca@0.4.0: cc: error: unrecognized command-line option '-arch'
warning: alloca@0.4.0: cc: error: unrecognized command-line option '-mmacosx-version-min=11.0'
error: failed to run custom build command for `alloca v0.4.0`
  error occurred in cc-rs: command did not execute successfully (status code exit status: 1):
  "cc" "-O2" "-ffunction-sections" "-fdata-sections" "-fPIC" "-arch" "arm64"
  "-mmacosx-version-min=11.0" "-Wall" "-Wextra" "-o" ".../alloca.o" "-c" "alloca.c"
```

criterion 0.7.0 and earlier have no C build script, and the same check passes.

## Reproduction

On x86-64 Linux with the `aarch64-apple-darwin` standard library installed
(`rustup target add aarch64-apple-darwin`) and no Apple toolchain:

```sh
cargo new --lib probe && cd probe
cargo add --dev criterion@0.8.2 --no-default-features --features cargo_bench_support
cargo clippy --all-targets --target aarch64-apple-darwin   # fails in alloca's build script
cargo add --dev criterion@0.7.0 --no-default-features --features cargo_bench_support
cargo clippy --all-targets --target aarch64-apple-darwin   # passes
```

Seen in Skene's gate (`cargo clippy --all-targets --target aarch64-apple-darwin -p
skene-terminal`, whose frame bench uses criterion): it failed with criterion 0.8.2 and
passed with 0.7.0. The standalone probe above was not run.

## Ask

Make `alloca` optional behind a feature (default on is fine), or fall back to a pure-Rust
path where the target's C compiler is absent, so `cargo check`/`clippy` for a foreign
target works without a C cross-toolchain, as it did through 0.7.
