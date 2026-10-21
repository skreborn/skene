# cargo-public-api 0.52.0: listings change with the toolchain's `Self` in derived impls

Repository: cargo-public-api/cargo-public-api. Not filed.

## What happens

The same source listed by cargo-public-api 0.52.0 (the latest release) gives different
lines under two nightlies, with no change to the crate's surface. Under
nightly-2026-08-13 a derived impl's methods name the type; under nightly-2026-10-21
(rustc 1.101.0-nightly 19833af72) they name `Self`:

```
- pub fn skene_conformance::Reading::clone(&self) -> skene_conformance::Reading
+ pub fn skene_conformance::Reading::clone(&self) -> Self
- pub fn skene_conformance::Reading::eq(&self, &skene_conformance::Reading) -> bool
+ pub fn skene_conformance::Reading::eq(&self, &Self) -> bool
- pub fn skene_conformance::Probes::default() -> skene_conformance::Probes
+ pub fn skene_conformance::Probes::default() -> Self
```

Across Skene's 18 snapshots this is 792 lines added and 792 removed (556 in
skene-foundation alone), every one of them this rewrite. A project that pins its
snapshots to one nightly cannot move the nightly without rewriting every snapshot, though
nothing public changed. Whether the change comes from rustc's derive expansion or from
rustdoc's JSON was not determined.

## Reproduction

```sh
cargo +nightly-2026-08-13 public-api --package <crate with #[derive(Clone)] pub struct> --omit blanket-impls,auto-trait-impls
cargo +nightly-2026-10-21 public-api --package <same crate> --omit blanket-impls,auto-trait-impls
```

Run on Skene's workspace (`cargo xtask gate api` with its `TOOLCHAIN` set to each
nightly): the first matches the committed snapshots, the second differs as above.

## Ask

Render `Self` in an impl item's signature as the impl's self type (or the reverse,
always), so a listing does not depend on how the toolchain spells it.
