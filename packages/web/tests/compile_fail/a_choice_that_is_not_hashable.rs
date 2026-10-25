//! A choice's value is compared and hashed, so a picker over a type that is not `Hash`
//! is no picker.

use skene_web::prelude::*;

#[derive(Clone, PartialEq, Eq)]
enum Size {
  Small,
}

fn main() {
  let _picker = BasicPicker::new().set_chosen(Size::Small);
}
