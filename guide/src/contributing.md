# Contributing

This chapter is for working on Skene itself, in its repository. Where an app made by
`skene new` names one platform for plain cargo, the repository names none, since it
holds every platform's crates and a default would build one of them while looking like
the whole.

## Plain cargo at the root

Plain cargo at the repository's root builds its default members: the crates that reach
neither facade and so name no platform, the reactive core, the foundation, the layout
engine, the text engine and the `skene` command among them. `cargo check`, `cargo test`
and `cargo clippy` there work as in any workspace. A crate that reaches a facade
compiles in that facade's platform's build alone, so `cargo check --workspace` with no
platform named fails, with each facade's own message.

## A cargo alias per platform

Each platform's build is one alias away:

```sh
cargo check-web         # the web's browser leaf, on wasm32-unknown-unknown
cargo check-server      # the web's server leaf, natively, in target/server
cargo check-terminal    # the terminal, natively, in target/terminal
```

Each has its `clippy-`, `doc-` and `test-` kin (`cargo clippy-terminal`,
`cargo test-server`), carrying the platform's cfgs and its triple, each building in
the target directory the gate builds that platform in, so the gate and the aliases
reuse each other's work. An alias takes the whole workspace (`--workspace`), so a `-p`
beside it narrows nothing, while a target selection does:

```sh
cargo test-server --test server_snapshot
cargo clippy-terminal --all-targets
```

Run them at the repository's root, since the target directories they name are relative
to the directory cargo runs in. `cargo test-web` runs every test binary through
`wasm-bindgen-test-runner`, and a browser test needs a WebDriver beside it: the gate's
`test:browser` step finds the drivers and runs every browser binary in Firefox and in
Chromium.

## rust-analyzer

The checked-in editor settings (`.zed/settings.json`) point rust-analyzer at the web's
browser leaf, as `cargo check-web` builds it, and run clippy on save. Any other editor
takes the same two settings:

```json
{
  "rust-analyzer.cargo.target": "wasm32-unknown-unknown",
  "rust-analyzer.cargo.extraEnv": {
    "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS": "--cfg skene_platform=\"web\""
  }
}
```

The cfg travels in the target's own rustflags, so it reaches the crates built for the
target and never a build script or a proc macro. To work on the terminal's crates,
point it at the host's triple instead, with that triple's variable naming the
terminal: on an x86-64 Linux host, `x86_64-unknown-linux-gnu` and
`CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS` set to
`--cfg skene_platform="terminal"`, or to `--cfg skene_platform="web" --cfg skene_server`
for the server leaf.

## The gate

A change lands when `cargo xtask gate` reports `gate passed`: every check it passes, in
one command, the one CI runs.

```sh
cargo xtask gate                  # every step
cargo xtask gate test:browser     # one step
cargo xtask gate clippy           # a group: every clippy step
cargo xtask gate --force          # every step, unchanged inputs or not
```

A step logs to `target/gate/<step>.log`, a `:` in its name written `-`, and a failed
step prints why beside the end of its log. A step that builds or runs anything is
skipped while nothing it reads has changed since it last passed. Format with
`cargo +nightly fmt` first. Where a change to a checked output is meant, rewrite it
rather than loosen its check: `cargo xtask api` rewrites the public API snapshots under
`api/`, `TRYBUILD=overwrite` the compile-fail diagnostics, `INSTA_UPDATE=always` the
terminal's frame snapshots and `SKENE_GOLDENS=overwrite` the conformance suite's
goldens. The README's testing section lists what each tier runs and the tools the gate
needs.
