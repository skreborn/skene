use skene_core::{
  Button,
  Color,
  Container,
  CrossAxisAlignment,
  Cursor,
  EdgeInsets,
  Flex,
  IntoView,
  Label,
  RwSignal,
  SignalGet,
  SignalUpdate,
};
use skene_dom::launch;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
  launch(app_view);
}

fn app_view() -> impl IntoView {
  let count = RwSignal::new(0i32);

  Flex::vertical()
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap(20.0)
    .padding(40.0)
    .child(
      Label::derived(move || format!("Count: {}", count.get()))
        .font_size(32.0)
        .font_weight(700)
        .color(Color::rgb(0x20, 0x20, 0x20)),
    )
    .child(
      Flex::horizontal()
        .gap(12.0)
        .child(counter_button("−", move || count.update(|c| *c -= 1)))
        .child(counter_button("+", move || count.update(|c| *c += 1))),
    )
}

/// A blue pill button — styles a [`Button`] by overriding its container builder.
fn counter_button(symbol: &'static str, action: impl Fn() + 'static) -> impl IntoView {
  Button::new(Label::new(symbol).color(Color::WHITE).font_size(20.0))
    .action(action)
    .container(|label| {
      Container::new(label)
        .padding(EdgeInsets::symmetric(20.0, 10.0))
        .background(Color::rgb(0x37, 0x6e, 0xff))
        .border_radius(8.0)
        .cursor(Cursor::Pointer)
    })
}
