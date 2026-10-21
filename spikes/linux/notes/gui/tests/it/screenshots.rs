//! The notes app drawn for its author: light and dark, narrow and wide, into
//! `$SKENE_SCREENSHOTS/notes-<scheme>-<width>.png` where that variable names a directory.

use skene_gui::testing::{self, Role};

#[skene_gui::test]
fn screenshots() {
  let Some(directory) = std::env::var_os("SKENE_SCREENSHOTS") else {
    return;
  };
  let screen = testing::render(notes_gui::app);

  screen
    .find_by_role(Role::TextField, "New note")
    .type_text("Milk\n");
  screen.find_by_role(Role::Row, "Call Ada").click();

  for scheme in ["light", "dark"] {
    if scheme == "dark" {
      // The app's own setting, as its user flips it.
      screen.find_by_role(Role::Switch, "Dark mode").click();
    }

    for (name, width, height) in [("narrow", 360, 640), ("wide", 1000, 640)] {
      screen.set_size(width, height);
      screen
        .render_to_png(std::path::Path::new(&directory).join(format!("notes-{scheme}-{name}.png")));
    }
  }
}
