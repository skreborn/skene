//! Every field of a snapshot takes part in snapshots.

use skene_gui::prelude::*;

struct Opaque;

#[derive(Snapshot)]
struct Settings {
  opaque: Opaque,
}

fn main() {}
