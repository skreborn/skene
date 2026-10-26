//! A grid cell is accepted only by the grid that understands it.

use skene_gui::prelude::*;

fn main() {
  let _ = Row::new().set_children((GridCell::new().set_child(Text::new("a")),));
}
