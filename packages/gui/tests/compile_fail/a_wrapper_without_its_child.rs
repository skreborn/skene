//! A wrapper is a view only once it carries its child.

use skene_gui::prelude::*;

fn main() {
  let _ = Row::new().set_children((Flexible::new().set_grow_factor(1.),));
}
