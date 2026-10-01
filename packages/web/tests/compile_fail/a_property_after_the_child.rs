//! A wrapper's properties come before its child.

use skene_web::prelude::*;

fn main() {
  let _ = Flexible::new().set_child(Text::new("a")).set_flexibility(Flexibility::grow(1.));
}
