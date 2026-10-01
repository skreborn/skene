//! Every field of a snapshot takes part in snapshots.

use skene_web::prelude::*;

struct Opaque;

#[derive(Snapshot)]
struct Settings {
  opaque: Opaque,
}

fn main() {}
