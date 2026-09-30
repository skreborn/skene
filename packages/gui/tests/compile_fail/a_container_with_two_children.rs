//! A container decorates exactly one child.

use skene_gui::prelude::*;

fn main() {
  let _ = Container::new().set_children((Text::new("a"), Text::new("b")));
}
