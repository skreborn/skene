//! A snapshot is derived for a struct or an enum.

use skene_web::prelude::*;

#[derive(Snapshot)]
union Bits {
  whole: u32,
  halves: [u16; 2],
}

fn main() {}
