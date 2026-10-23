//! A saved or tagged signal's or store's value takes part in snapshots, and a plain
//! one's need not.

use skene_web::prelude::*;

struct Opaque;

fn main() {
  let _plain = Signal::new(Opaque);
  let _saved = Signal::saved(Opaque);
  let _tagged = Signal::tagged("opaque", Opaque);
  let _plain_store = Store::new(Opaque);
  let _saved_store = Store::saved(Opaque);
  let _tagged_store = Store::tagged("opaque store", Opaque);
}
