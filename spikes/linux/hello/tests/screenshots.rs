//! Hello drawn for its author: light and dark, narrow and wide, into
//! `$SKENE_SCREENSHOTS/hello-<scheme>-<width>.png` where that variable names a directory.

use skene_gui::testing::{self, Role};

#[skene_gui::test]
fn screenshots() {
  let Some(directory) = std::env::var_os("SKENE_SCREENSHOTS") else {
    return;
  };
  let screen = testing::render(hello::app);

  screen
    .find_by_role(Role::TextField, "Name")
    .type_text("Ada");
  screen.find_by_role(Role::Button, "Greet").click();

  for scheme in ["light", "dark"] {
    if scheme == "dark" {
      // The app's own setting, as its user flips it.
      screen.find_by_role(Role::Switch, "Dark").click();
    }

    for (name, width, height) in [("narrow", 360, 720), ("wide", 1000, 720)] {
      screen.set_size(width, height);
      screen
        .render_to_png(std::path::Path::new(&directory).join(format!("hello-{scheme}-{name}.png")));
    }
  }
}
