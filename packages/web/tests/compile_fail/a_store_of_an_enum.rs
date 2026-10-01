//! A store is derived for a struct alone.

use skene_web::prelude::*;

#[derive(Store)]
enum Mode {
  Light,
  Dark,
}

fn main() {}
