//! A wrapper is a child of its one container kind, not of every view.

use skene_web::prelude::*;

fn main() {
  let _ = Container::new().set_child(Flexible::new().set_child(Text::new("a")));
}
