//! An entry returns the app's view and takes nothing: the build names the platform.

use skene_web::prelude::*;

#[skene_web::main]
fn app(greeting: &'static str) -> impl View {
  Text::new(greeting)
}

fn main() {}
