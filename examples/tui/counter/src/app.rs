//! The counter's views, in cells.

use skene_tui::prelude::*;

pub(crate) fn app() -> impl IntoView {
  let count = Signal::tagged("count", 0u64);

  Container::new()
    .set_color(SimplePalette::background())
    .set_child(
      Column::new()
        .set_arrangement(Arrangement::Center)
        .set_item_alignment(ItemAlignment::Center)
        .set_child(card(count)),
    )
}

fn card(count: Signal<u64>) -> impl IntoView {
  SimpleCard::new()
    .set_item_alignment(ItemAlignment::Center)
    .set_children((
      Text::new("COUNTER")
        .set_bold(true)
        .set_selectable(true)
        .set_color(SimplePalette::muted_foreground()),
      Text::derive(move || count.get().to_string()).set_color(SimplePalette::foreground()),
      Row::new().set_spacing(2).set_children((
        SimpleButton::new("-")
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
