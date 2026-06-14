use skene_core::{
  Button,
  Color,
  CrossAxisAlignment,
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
        .child(Button::new("−").action(move || count.update(|c| *c -= 1)))
        .child(Button::new("+").action(move || count.update(|c| *c += 1))),
    )
}
