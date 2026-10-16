//! A smoke probe: the harness itself.

use probes::{result, run, show};
use skene_gui::{platform::linux::gtk::prelude::*, prelude::*};

fn body() {
  let shown = show(300, -1, || Text::new("hello"));

  result("smoke", shown.window.width() == 300, shown.window.width());
}

fn main() -> std::process::ExitCode {
  run("smoke", body)
}
