//! An entry returns the app's view and takes nothing: the build names the platform.

use skene_gui::prelude::*;

#[skene_gui::main]
fn app(greeting: &'static str) -> impl View {
  Text::new(greeting)
}

fn main() {}
