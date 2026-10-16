//! Hello's entry: the binary a Linux build runs.

/// The app's root.
#[skene_gui::main]
fn app() -> impl skene_gui::View {
  hello::app()
}
