use skene_core::{
  button,
  column,
  label,
  row,
  AlignItems,
  Color,
  IntoView,
  RwSignal,
  SignalGet,
  SignalUpdate,
  Style,
};
use skene_dom::launch;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
  launch(app_view);
}

fn app_view() -> impl IntoView {
  let count = RwSignal::new(0i32);

  let count_label = label(move || format!("Count: {}", count.get())).style(
    Style::new()
      .font_size(32.0)
      .font_weight(700)
      .color(Color::rgb(0x20, 0x20, 0x20)),
  );

  let buttons = row((
    button("−")
      .on_click(move || count.update(|c| *c -= 1))
      .style(button_style()),
    button("+")
      .on_click(move || count.update(|c| *c += 1))
      .style(button_style()),
  ))
  .style(Style::new().gap(12.0));

  column((count_label, buttons)).style(
    Style::new()
      .align_items(AlignItems::Center)
      .gap(20.0)
      .padding(40.0),
  )
}

fn button_style() -> Style {
  Style::new()
    .padding_horiz(20.0)
    .padding_vert(10.0)
    .font_size(20.0)
    .background(Color::rgb(0x37, 0x6e, 0xff))
    .color(Color::WHITE)
    .border_radius(8.0)
    .cursor(skene_core::Cursor::Pointer)
}
