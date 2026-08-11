//! The counter's views, in cells.

use skene_tui::prelude::*;

pub(crate) fn app() -> impl IntoView {
  let count = Signal::tagged("count", 0u64);

  Column::new()
    .set_arrangement(Arrangement::Center)
    .set_item_alignment(ItemAlignment::Center)
    .set_color(Color::srgb(0.08, 0.08, 0.1))
    .set_child(card(count))
}

fn card(count: Signal<u64>) -> impl IntoView {
  Column::new()
    .set_item_alignment(ItemAlignment::Center)
    .set_spacing(1)
    .set_min_width(24)
    .set_padding(Inset::symmetric(4, 1))
    .set_color(Color::srgb(0.13, 0.13, 0.16))
    .set_border(Color::srgb(0.35, 0.35, 0.45))
    .set_children((
      Text::new("COUNTER").set_bold(true).set_selectable(true),
      Text::derive(move || count.get().to_string()).set_color(Color::srgb(0.8, 0.9, 0.8)),
      Row::new().set_spacing(2).set_children((
        button("-", move || {
          count.update(|value| *value = value.saturating_sub(1));
        }),
        button("+", move || {
          count.update(|value| *value = value.saturating_add(1));
        }),
      )),
    ))
}

fn button(label: &'static str, action: impl Fn() + 'static) -> impl IntoView {
  Button::new()
    .set_style(
      BoxStyle::new()
        .set_padding(Inset::horizontal(1))
        .set_color(Color::srgb(0.2, 0.22, 0.2))
        .set_cursor(MouseCursor::Click),
    )
    .set_when(
      Condition::Focused,
      BoxStyle::new().set_color(Color::srgb(0.3, 0.4, 0.3)),
    )
    .set_when(
      Condition::Hovered,
      BoxStyle::new().set_color(Color::srgb(0.28, 0.32, 0.28)),
    )
    .set_child(Text::new(label))
    .handle_click(action)
}
