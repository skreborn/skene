//! A wrapper's properties come before its child.

use skene_gui::prelude::*;

fn main() {
  let _ = Flexible::new().set_child(Text::new("a")).set_grow_factor(1.);
}
