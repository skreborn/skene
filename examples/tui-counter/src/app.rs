//! The counter's views, in cells.

use skene_tui::prelude::*;

pub(crate) fn app() -> impl IntoView {
  let count = RwSignal::new(0i64);

  Container::column()
    .set_main_axis_alignment(MainAxisAlignment::Center)
    .set_cross_axis_alignment(CrossAxisAlignment::Center)
    .set_color(Color::srgb(0.08, 0.08, 0.1))
    .set_child(card(count))
}

fn card(count: RwSignal<i64>) -> impl IntoView {
  Container::column()
    .set_cross_axis_alignment(CrossAxisAlignment::Center)
    .set_spacing(1)
    .set_min_width(24)
    .set_padding(Inset::symmetric(4, 1))
    .set_color(Color::srgb(0.13, 0.13, 0.16))
    .set_border(Color::srgb(0.35, 0.35, 0.45))
    .set_children((
      Text::new("COUNTER").set_bold(true),
      Text::derive(move || count.get().to_string()).set_color(Color::srgb(0.8, 0.9, 0.8)),
      Container::row().set_spacing(2).set_children((
        button("[-]", move || count.update(|value| *value -= 1)),
        button("[+]", move || count.update(|value| *value += 1)),
      )),
    ))
}

fn button(label: &'static str, action: impl Fn() + 'static) -> impl IntoView {
  Container::row()
    .set_padding(Inset::horizontal(1))
    .set_color(Color::srgb(0.2, 0.22, 0.2))
    .set_cursor(MouseCursor::Click)
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
