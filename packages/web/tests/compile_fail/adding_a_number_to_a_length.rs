//! A length and a number do not add: only a number scales a length.

use skene_web::{Length, Number, prelude::*};

fn main() {
  let _ = Expr::<Length>::from(8.) + Expr::<Number>::from(2.);
}
