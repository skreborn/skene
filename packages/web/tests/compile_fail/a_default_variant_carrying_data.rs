//! The variant a snapshot restores to where its recorded one is gone carries nothing.

use skene_web::prelude::*;

#[derive(Snapshot)]
enum Choice {
  #[snapshot(default)]
  Some(u32),
  None,
}

fn main() {}
