//! A wrapper is a view only once it carries its child.

use skene_web::prelude::*;

fn main() {
  let _ = Row::new().set_children((Flexible::new().set_flexibility(Flexibility::grow(1.)),));
}
