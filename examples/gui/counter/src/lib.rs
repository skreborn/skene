//! A counter demo: one app crate, depending only on `skene`, that runs on any GUI
//! platform the build targets.
//!
//! Gated to `wasm32` (both web leaves): in the browser `#[main]` launches it; on the
//! server leaf the same entry renders a requested path to stdout. Building a view for a
//! target with no driver is a compile error, so an app is built for the platform(s) it
//! targets.

#![cfg(target_arch = "wasm32")]

use skene_gui::prelude::*;

#[main]
fn app() -> impl IntoView {
  let count = Signal::tagged("count", 0u64);

  Column::new()
    .set_arrangement(Arrangement::Center)
    .set_item_alignment(ItemAlignment::Center)
    .set_color(SimplePalette::background())
    .set_child(card(count))
}

fn card(count: Signal<u64>) -> impl IntoView {
  SimpleCard::new()
    .set_spacing(24.)
    .set_item_alignment(ItemAlignment::Center)
    .set_children((
      Text::new("COUNTER")
        .set_font_size(13.)
        .set_font_weight(500.)
        .set_color(SimplePalette::muted_foreground()),
      Text::derive(move || count.get().to_string())
        .set_font_size(56.)
        .set_font_weight(700.)
        .set_color(SimplePalette::foreground()),
      Row::new().set_spacing(12.).set_children((
        SimpleButton::new("−")
          .set_variant(SimpleVariant::Outline)
          .handle_click(move || {
            count.update(|value| *value = value.saturating_sub(1));
          }),
        SimpleButton::new("+")
          .set_variant(SimpleVariant::Outline)
          .handle_click(move || {
            count.update(|value| *value = value.saturating_add(1));
          }),
      )),
    ))
}
