//! A counter for the terminal: `skene-tui` is its single dependency, and its vocabulary
//! is the terminal's own—cells, colors, bold. `Tab` moves focus, `Enter` clicks,
//! `Ctrl-Q` quits.

#![cfg(skene_platform = "terminal")]

use skene_simple_tui::prelude::*;
use skene_tui::prelude::*;

#[main]
fn app() -> impl View {
  let count = Signal::tagged("count", 0u64);

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    Container::new()
      .set_background(SimplePalette::BACKGROUND)
      .set_child(
        Column::new()
          .set_arrangement(Arrangement::Center)
          .set_item_alignment(ItemAlignment::Center)
          .add_child(card(count)),
      )
  })
}

fn card(count: Signal<u64>) -> impl View {
  SimpleCard::new().set_child(
    Column::new()
      .set_spacing(SimpleCard::spacing())
      .set_item_alignment(ItemAlignment::Center)
      .set_children((
        Text::new("COUNTER")
          .set_bold(true)
          .set_selectable(true)
          .set_color(SimplePalette::MUTED_FOREGROUND),
        Text::derive(move || count.get().to_string()).set_color(SimplePalette::FOREGROUND),
        Row::new().set_spacing(2).set_children((
          SimpleButton::outline("-").handle_activate(move || {
            count.update(|value| *value = value.saturating_sub(1));
          }),
          SimpleButton::outline("+").handle_activate(move || {
            count.update(|value| *value = value.saturating_add(1));
          }),
        )),
      )),
  )
}
