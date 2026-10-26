//! The incremental frame: whatever a user and an app do, the frame that restyles,
//! measures and repaints only what changed, its tokens kept where they are defined,
//! shows what a frame resolving, laying out and painting everything shows; the layout
//! follows the scene across frames, and a change resolves only what it touched.

use std::{cell::RefCell, rc::Rc, time::Duration};

use proptest::prelude::*;
use skene_simple_tui::prelude::*;
use skene_tui::{platform::terminal::mount, prelude::*, testing};

use crate::common::{blue, colors_at, green, red, rgb, white, with_platform};

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
      let season = Signal::new("spring");
      let warmed = Signal::new(false);

      let _ = mount(move || {
        Theme::derive(move || if warmed.get() { warm() } else { SimplePalette::default() })
          .set_child_builder(move || {
        Column::new()
          .set_children((
            Container::new()
              .set_height(1)
              .set_background(red().when(State::Hovered, blue()))
              .set_child(Text::derive(move || label.get())),
            SimpleButton::new("press"),
            SimpleCheckbox::new().bind_checked(checked).set_label("check"),
            SimpleTextField::new().bind_value(value),
            Visibility::new()
              .derive_shown(move || shown.get())
              .set_child(Text::new("shown")),
            Container::new()
              .derive_background(move || if painted.get() { green() } else { white() })
              .set_child(Text::new("paint")),
            Column::new().derive_children(
              move || rows.get(),
              |row| *row,
              |row| Text::new(format!("row {row}")),
            ),
            Container::new()
              .set_height(3)
              .set_overflow(Overflow::ScrollIfNeeded)
              .set_child(
                Column::new().set_children(
                  (0..12)
                    .map(|index| Text::new(format!("item {index}")).into_any())
                    .collect::<Vec<_>>(),
                ),
              ),
            SimplePicker::new()
              .set_label("season")
              .set_options((SimpleListboxOption::new("spring", "Spring"), SimpleListboxOption::new("summer", "Summer"), SimpleListboxOption::new("autumn", "Autumn")))
              .bind_chosen(season),
            Container::new()
              .set_background(SimplePalette::CARD)
              .set_border_width(Inset::all(1))
              .set_border_color(SimplePalette::BORDER)
              .set_child(Theme::new(warm()).set_child_builder(|| {
                Container::new()
                  .set_background(SimplePalette::CARD)
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

/// The layout follows the scene from frame to frame: rows reordered, removed and added,
/// only the added one built, and a row's width moved by a signal.
#[test]
fn the_layout_follows_the_scene_across_frames() {
  with_platform(|| {
    let rows = Signal::new(vec![1_u8, 2, 3]);
    let width = Signal::new(4_i32);
    let built = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&built);

    let _ = mount(move || {
      Column::new()
        .derive_children(
          move || rows.get(),
          |row| *row,
          move |row| {
            log.borrow_mut().push(row);

            Container::new()
              .derive_width(move || if row == 2 { width.get() } else { 3 })
              .set_height(i32::from(row))
              .set_background(red())
              .set_child(Text::new(row.to_string()))
          },
        )
        .into_any()
        .build()
    });
    let painted = |y: usize| {
      (0..12)
        .filter(|x| colors_at(12, 10, *x, y).1 == Some(rgb(red())))
        .count()
    };

    assert_eq!(testing::render_to_text(12, 10), "1\n2\n\n3\n\n\n\n\n\n\n");
    assert_eq!(painted(1), 4);

    rows.set(vec![3, 2, 4]);
    width.set(6);

    assert_eq!(testing::render_to_text(12, 10), "3\n\n\n2\n\n4\n\n\n\n\n");
    assert_eq!(painted(3), 6);
    assert_eq!(painted(0), 3);
    assert_eq!(
      *built.borrow(),
      [1, 2, 3, 4],
      "the kept rows are not built again"
    );
  });
}

/// A change to one box's style resolves that box alone at the next frame, which lays
/// it out afresh beside the boxes it kept; a change to a span's text resolves no style,
/// the paragraph it flows in laid out afresh.
#[test]
fn a_change_resolves_only_what_it_touched() {
  with_platform(|| {
    let width = Signal::new(2_i32);
    let words = Signal::new("one".to_owned());

    let _ = mount(move || {
      Column::new()
        .set_children((
          Row::new().set_children((
            Container::new()
              .derive_width(move || width.get())
              .set_height(1),
            Text::new("|"),
          )),
          Text::new("kept"),
          RichText::new().add_spans([Span::derive(move || words.get())]),
          Text::new("after"),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 4), "  |\nkept\none\nafter\n");

    let resolved = testing::style_resolutions();

    width.set(5);

    assert_eq!(testing::render_to_text(12, 4), "     |\nkept\none\nafter\n");
    assert_eq!(testing::style_resolutions() - resolved, 1);

    let resolved = testing::style_resolutions();

    words.set("one two".to_owned());

    assert_eq!(
      testing::render_to_text(12, 4),
      "     |\nkept\none two\nafter\n"
    );
    assert_eq!(testing::style_resolutions(), resolved);
  });
}
