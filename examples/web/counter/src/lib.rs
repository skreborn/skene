//! A counter demo: one app crate, depending only on `skene-web`, that runs on any GUI
//! platform the build targets.
//!
//! In the browser `#[main]` launches it; on the server leaf the same entry renders a
//! requested path to stdout; on the host it builds against the headless platform.

use skene_simple_web::prelude::*;
use skene_web::prelude::*;

#[main]
fn app() -> impl View {
  let count = Signal::tagged("count", 0u64);

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    Container::new()
      .set_background(SimplePalette::current().background)
      .set_child(
        Column::new()
          .set_arrangement(Arrangement::Center)
          .set_item_alignment(ItemAlignment::Center)
          .add_child(card(count)),
      )
  })
}

fn card(count: Signal<u64>) -> impl View {
  let palette = SimplePalette::current();

  SimpleCard::new()
    .set_spacing(24)
    .set_item_alignment(ItemAlignment::Center)
    .set_children((
      Text::new("COUNTER")
        .set_font_size(13.)
        .set_font_weight(500.)
        .set_color(palette.muted_foreground),
      Text::derive(move || count.get().to_string())
        .set_font_size(56.)
        .set_font_weight(700.)
        .set_color(palette.foreground),
      Row::new().set_spacing(12).set_children((
        SimpleButton::new("−")
          .set_variant(SimpleVariant::Outline)
          .handle_activate(move || {
            count.update(|value| *value = value.saturating_sub(1));
          }),
        SimpleButton::new("+")
          .set_variant(SimpleVariant::Outline)
          .handle_activate(move || {
            count.update(|value| *value = value.saturating_add(1));
          }),
      )),
    ))
}
