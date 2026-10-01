//! The incremental frame against a frame drawn from scratch: whatever a user and an app
//! do, the frame that restyles, measures and repaints only what changed, its tokens
//! kept where they are defined, shows what a frame resolving, laying out and painting
//! everything shows.

use std::time::Duration;

use proptest::prelude::*;
use skene_simple_terminal::prelude::*;
use skene_terminal::{self as terminal, prelude::*, testing};

use crate::common::{blue, green, red, white, with_platform};

/// The frame's width.
const WIDTH: u16 = 32;

/// The frame's height.
const HEIGHT: u16 = 22;

/// One thing a user or the app does between two frames.
#[derive(Debug, Clone)]
enum Action {
  Move(u16, u16),
  Click(u16, u16),
  Key(&'static str),
  Toggle,
  Rename(u8),
  Push,
  Pop,
  Paint,
  Wheel(u16, u16, i64),
  Advance(u64),
  Palette,
  Scheme(bool),
}

/// A definition taking `dark`'s value under the dark scheme and `light`'s otherwise.
fn pair(light: Color, dark: Color) -> Definition<Color> {
  Definition::from(light).when(Media::Dark, dark)
}

/// The built-in palette warmed over: its cards, borders and ring redefined, each
/// answering the scheme.
fn warm() -> SimplePalette {
  SimplePalette::default()
    .set_card(pair(
      Color::srgb(0.99, 0.96, 0.91),
      Color::srgb(0.16, 0.13, 0.09),
    ))
    .set_border(pair(red(), blue()))
    .set_ring(pair(green(), white()))
}

/// Actions mostly of the kinds that change what a frame shows.
fn arbitrary_action() -> impl Strategy<Value = Action> {
  prop_oneof![
    3 => (0..WIDTH, 0..HEIGHT).prop_map(|(x, y)| Action::Move(x, y)),
    2 => (0..WIDTH, 0..HEIGHT).prop_map(|(x, y)| Action::Click(x, y)),
    2 => proptest::sample::select(&["Tab", " ", "a", "Backspace", "Escape", "ArrowDown"][..])
      .prop_map(Action::Key),
    1 => Just(Action::Toggle),
    1 => (0..4_u8).prop_map(Action::Rename),
    1 => Just(Action::Push),
    1 => Just(Action::Pop),
    1 => Just(Action::Paint),
    2 => (0..WIDTH, 0..HEIGHT, -3..4_i64).prop_map(|(x, y, rows)| Action::Wheel(x, y, rows)),
    2 => (1..400_u64).prop_map(Action::Advance),
    1 => Just(Action::Palette),
    1 => any::<bool>().prop_map(Action::Scheme),
  ]
}

proptest! {
  #![proptest_config(ProptestConfig::with_cases(64))]

  /// After any run of pointer moves, clicks, keys, wheel turns, app writes, palette and
  /// scheme changes and time passing, the incremental frame is the frame drawn from
  /// scratch once a resize has invalidated everything.
  #[test]
  fn the_incremental_frame_is_the_whole_one(
    actions in proptest::collection::vec(arbitrary_action(), 1..14),
  ) {
    with_platform(|| {
      let label = Signal::new("label 0".to_owned());
      let checked = Signal::new(false);
      let value = Signal::new(String::new());
      let shown = Signal::new(true);
      let rows = Signal::new(vec![0_u32, 1, 2]);
      let painted = Signal::new(false);
      let season = Signal::new("spring".to_owned());
      let warmed = Signal::new(false);

      let _ = terminal::mount(move || {
        Theme::derive(move || if warmed.get() { warm() } else { SimplePalette::default() })
          .set_child_builder(move || {
        Column::new()
          .set_children((
            Container::new()
              .set_height(1)
              .set_color(red().when(State::Hovered, blue()))
              .set_child(Text::derive(move || label.get())),
            SimpleButton::new("press"),
            SimpleCheckbox::new().bind_checked(checked).set_label("check"),
            SimpleTextField::new().bind_value(value),
            Show::new()
              .derive_shown(move || shown.get())
              .set_child(Text::new("shown")),
            Container::new()
              .derive_color(move || if painted.get() { green() } else { white() })
              .set_child(Text::new("paint")),
            Column::new().derive_children(
              move || rows.get(),
              |row| *row,
              |row| Text::new(format!("row {row}")),
            ),
            Container::new()
              .set_height(3)
              .set_overflow(Overflow::Auto)
              .set_child(
                Column::new().set_children(
                  (0..12)
                    .map(|index| Text::new(format!("item {index}")).into_any())
                    .collect::<Vec<_>>(),
                ),
              ),
            SimplePicker::new()
              .set_label("season")
              .set_options([("spring", "Spring"), ("summer", "Summer"), ("autumn", "Autumn")])
              .bind_value(season),
            Container::new()
              .set_color(SimplePalette::CARD)
              .set_border_width(Inset::all(1))
              .set_border_color(SimplePalette::BORDER)
              .set_child(Theme::new(warm()).set_child_builder(|| {
                Container::new()
                  .set_color(SimplePalette::CARD)
                  .set_child(Text::new("nested").set_color(SimplePalette::FOREGROUND))
              })),
          ))
          })
          .into_any()
          .build()
      });

      let _ = testing::render_to_styled(WIDTH, HEIGHT);

      for action in &actions {
        match action {
          Action::Move(x, y) => testing::trigger_mouse_move(*x, *y),
          Action::Click(x, y) => testing::trigger_mouse_click(*x, *y),
          Action::Key(key) => testing::trigger_key(key),
          Action::Toggle => shown.update(|shown| *shown = !*shown),
          Action::Rename(number) => label.set(format!("label {number}")),
          Action::Push => rows.update(|rows| {
            let next = rows.iter().max().map_or(0, |last| last + 1);

            rows.insert(0, next);
          }),
          Action::Pop => rows.update(|rows| {
            rows.pop();
          }),
          Action::Paint => painted.update(|painted| *painted = !*painted),
          Action::Wheel(x, y, rows) => testing::trigger_mouse_scroll(*x, *y, 0, *rows),
          Action::Advance(milliseconds) => {
            testing::trigger_frame(Duration::from_millis(*milliseconds));
          }
          Action::Palette => warmed.update(|warmed| *warmed = !*warmed),
          Action::Scheme(dark) => testing::trigger_color_scheme(if *dark {
            ColorScheme::Dark
          }
          else {
            ColorScheme::Light
          }),
        }

        let incremental = testing::render_to_styled(WIDTH, HEIGHT);

        testing::trigger_resize(WIDTH + 1, HEIGHT);

        let _ = testing::render_to_styled(WIDTH + 1, HEIGHT);

        testing::trigger_resize(WIDTH, HEIGHT);

        let whole = testing::render_to_styled(WIDTH, HEIGHT);

        prop_assert_eq!(incremental, whole, "after {:?}", action);
      }

      Ok(())
    })?;
  }
}
