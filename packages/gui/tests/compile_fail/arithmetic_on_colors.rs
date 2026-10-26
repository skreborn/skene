//! Arithmetic is defined on lengths and numbers, never on colors.

use skene_gui::prelude::*;

fn main() {
  let _ = Expr::from(Color::srgb(1., 1., 1.)) + Expr::from(Color::TRANSPARENT);
}
