//! Two fields of a snapshot record under distinct keys; a pair whose names hash alike
//! is tagged apart.

use skene_web::prelude::*;

#[derive(Snapshot)]
struct Colliding {
  field_10779: u32,
  field_106513: u32,
}

fn main() {}
