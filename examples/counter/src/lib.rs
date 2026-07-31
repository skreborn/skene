//! A counter demo: one app crate, depending only on `skene`, that runs on any GUI
//! platform the build targets.
//!
//! Gated to the browser platform (the only one with an interactive entry point): building
//! a view for a target with no platform is a compile error, so an app is built for the
//! platform(s) it targets.

#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use skene::prelude::*;

#[main]
fn app() -> impl IntoView {
  let count = RwSignal::new(0u64);

  Container::column()
    .set_main_axis_alignment(MainAxisAlignment::Center)
    .set_cross_axis_alignment(CrossAxisAlignment::Center)
    .set_color(Color::srgb(0.1, 0.1, 0.1))
    .set_child(card(count))
}

fn card(count: RwSignal<u64>) -> impl IntoView {
  Container::column()
    .set_cross_axis_alignment(CrossAxisAlignment::Center)
    .set_spacing(24.)
    .set_min_width(240.)
    .set_padding(Inset::symmetric(40., 48.))
    .set_color(Color::srgb(0.15, 0.15, 0.15))
    .set_border(Border::new(1., Color::srgb(0.2, 0.2, 0.2)))
    .set_border_radius(8.)
    .set_children((
      Text::new("COUNTER")
        .set_font_size(13.)
        .set_font_weight(500.)
        .set_color(Color::srgb(0.55, 0.57, 0.63)),
      Text::derive(move || count.get().to_string())
        .set_font_size(56.)
        .set_font_weight(700.)
        .set_color(Color::srgb(0.78, 0.87, 0.8)),
      Container::row().set_spacing(12.).set_children((
        button("−", move || {
          count.update(|value| *value = value.saturating_sub(1));
        }),
        button("+", move || {
          count.update(|value| *value = value.saturating_add(1));
        }),
      )),
    ))
}

fn button(symbol: &'static str, action: impl Fn() + 'static) -> impl IntoView {
  Container::row()
    .set_main_axis_alignment(MainAxisAlignment::Center)
    .set_cross_axis_alignment(CrossAxisAlignment::Center)
    .set_padding(Inset::symmetric(16., 12.))
    .set_color(Color::srgb(0.195, 0.215, 0.195))
    .set_border(Border::new(1., Color::srgb(0.2, 0.2, 0.2)))
    .set_border_radius(8.)
    .set_cursor(MouseCursor::Click)
    .set_role(Role::Button)
    .set_child(
      Text::new(symbol)
        .set_font_size(22.)
        .set_font_weight(500.)
        .set_color(Color::srgb(0.835, 0.9, 0.84)),
    )
    .handle_click(action)
}
