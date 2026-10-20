//! Lazy lists on the real platform, through the driver: a list of ten thousand items realizes
//! about its viewport and tells a reader the whole count and each realized row's place; the
//! wheel, the keys and a reader's own requests move the window; rows measuring taller than
//! estimated above the viewport move nothing in it; `Tab` reaches a stop beyond the realized
//! rows and leaves a list whose next stop is beyond a page; and a list box moves, types ahead
//! and chooses across options it has not built.

use std::collections::HashSet;

use skene_gui::{
  AccessRequest, Rect,
  prelude::*,
  testing::{self, Role, Screen},
};

/// A list 300 pixels tall named "Items" of `count` items, "Item 0" onwards, each a text in a
/// box 30 pixels tall, estimated at 30.
fn texts(count: usize) -> impl View {
  Column::new().add_child(
    LazyColumn::new()
      .set_height(300)
      .set_label("Items")
      .set_item_extent(ItemExtent::Estimated(30.))
      .derive_items(
        move || (0..count).collect::<Vec<_>>(),
        |index| *index,
        |index: usize| {
          Container::new()
            .set_height(30)
            .set_child(Text::new(format!("Item {index}")))
        },
      ),
  )
}

/// The realized items a reader finds, in its order: each one's place in its set (from 1) and
/// the set's size.
fn realized(screen: &Screen) -> Vec<(usize, usize)> {
  screen
    .find_all_by_role(Role::ListItem)
    .iter()
    .map(|item| {
      let read = item.read();

      (
        read.position_in_set.expect("an item tells its place"),
        read.set_size.expect("an item tells its set's size"),
      )
    })
    .collect()
}

/// Whether `inner` lies within `outer`, give or take the pixel a reader's extents round.
fn within(inner: Rect, outer: Rect) -> bool {
  inner.y >= outer.y - 1. && inner.y + inner.height <= outer.y + outer.height + 1.
}

#[skene_gui::test]
fn a_reader_finds_the_whole_count_and_the_window_of_rows() {
  let screen = testing::render(|| texts(10_000));
  let shown = realized(&screen);

  assert!(
    (11..=16).contains(&shown.len()),
    "about the viewport's ten rows and a few beyond are realized: {shown:?}"
  );
  assert!(
    shown
      .iter()
      .enumerate()
      .all(|(at, (place, size))| *place == at + 1 && *size == 10_000),
    "each realized row tells its place in the whole: {shown:?}"
  );
  screen.find_by_text("Item 0");
  assert!(
    screen.query_by_text("Item 40").is_none(),
    "an item far below the viewport is not realized"
  );
}

#[skene_gui::test]
fn the_wheel_and_the_keys_move_the_window_to_the_end() {
  let screen = testing::render(|| texts(10_000));
  let list = screen.find_by_role(Role::List, "Items");
  let bounds = list.read().bounds;

  // Twenty detents: 300 pixels where a detent is libinput's 15 (KWin's fake wheel), 960 where
  // it is a line.
  list.scroll(0, 20);

  let moved = realized(&screen);

  assert!(
    moved.first().is_some_and(|(place, _)| *place > 5),
    "the wheel moved the window down: {moved:?}"
  );

  // Nothing in the list takes focus: the list does, and its keys scroll it.
  screen.press("Tab");
  assert_eq!(screen.focused().name(), "Items");
  screen.press("End");

  let last = screen.find_by_text("Item 9999").read().bounds;

  assert!(
    (last.y + last.height - (bounds.y + bounds.height)).abs() <= 1.,
    "the last item ends where the list does: {last:?} in {bounds:?}"
  );
  assert_eq!(
    realized(&screen).last().map(|(place, _)| *place),
    Some(10_000),
    "the last realized item is the ten thousandth"
  );

  screen.press("Home");
  assert!(within(screen.find_by_text("Item 0").read().bounds, bounds));
  assert_eq!(realized(&screen).first(), Some(&(1, 10_000)));
}

#[skene_gui::test]
fn a_readers_scroll_to_the_last_realized_row_realizes_beyond_it() {
  let screen = testing::render(|| texts(10_000));
  let bounds = screen.find_by_role(Role::List, "Items").read().bounds;
  let items = screen.find_all_by_role(Role::ListItem);
  let last = items.last().expect("rows are realized");
  let place = last.read().position_in_set.expect("a place");

  last.trigger_access(AccessRequest::ScrollIntoView);

  let row = screen.find_by_text(&format!("Item {}", place - 1)).read();

  assert!(
    within(row.bounds, bounds),
    "the row scrolled into view: {:?} in {bounds:?}",
    row.bounds
  );
  screen.find_by_text(&format!("Item {place}"));
}

/// A list 300 pixels tall named "Rows" of a thousand rows estimated at 20 pixels and each 40
/// tall, the first visible row's place written into `top` as the list reports its position.
fn taller(top: Signal<usize>) -> impl View {
  Column::new().add_child(
    LazyColumn::new()
      .set_height(300)
      .set_label("Rows")
      .set_item_extent(ItemExtent::Estimated(20.))
      .handle_scroll(move |position| top.set(position.index))
      .derive_items(
        || (0..1000).collect::<Vec<_>>(),
        |index| *index,
        |index: usize| {
          Container::new()
            .set_height(40)
            .set_child(Text::new(format!("Row {index}")))
        },
      ),
  )
}

#[skene_gui::test]
fn rows_measuring_taller_than_estimated_move_nothing_in_view() {
  let top = Signal::new(0);
  let screen = testing::render(move || taller(top));
  let list = screen.find_by_role(Role::List, "Rows");

  list.scroll(0, 20);

  let first = realized(&screen)[0].0;
  let shown: Vec<(String, f32)> = (first..first + 12)
    .filter_map(|place| {
      let name = format!("Row {}", place - 1);
      let found = screen.query_by_text(&name)?;

      Some((name, found.read().bounds.y))
    })
    .collect();

  assert!(
    shown.len() >= 8,
    "rows are realized after the wheel: {shown:?}"
  );

  // The rows below measure as they are realized, the estimate is re-based at rest, and
  // nothing in view moves.
  list.scroll(0, 1);
  list.scroll(0, -1);

  for (name, y) in &shown {
    let now = screen.find_by_text(name).read().bounds.y;

    assert!(
      (now - y).abs() <= 1.,
      "{name} stays where it was: {y} then {now}"
    );
  }

  assert!(
    top.get_untracked() + 1 >= first,
    "the list reports its position as it scrolls: {} for the first at {first}",
    top.get_untracked()
  );
}

/// A list 300 pixels tall named "Actions" of a thousand rows 40 pixels tall, each a button
/// "Open {n}" where `every` divides its index and a text otherwise, between the buttons
/// "Before" and "After".
fn buttons(every: usize) -> impl View {
  Column::new()
    .add_child(Button::new("Before"))
    .add_child(
      LazyColumn::new()
        .set_height(300)
        .set_label("Actions")
        .set_item_extent(ItemExtent::Estimated(40.))
        .derive_items(
          || (0..1000).collect::<Vec<_>>(),
          |index| *index,
          move |index: usize| {
            let row = Container::new().set_height(40);

            if index.is_multiple_of(every) {
              row.set_child(Button::new(format!("Open {index}")))
            } else {
              row.set_child(Text::new(format!("Note {index}")))
            }
          },
        ),
    )
    .add_child(Button::new("After"))
}

#[skene_gui::test]
fn tab_reaches_a_stop_beyond_the_realized_rows_and_comes_back() {
  let screen = testing::render(|| buttons(1));

  screen.press("Tab");
  assert_eq!(screen.focused().name(), "Before");

  // Twenty stops on: past the rows in view and the few beyond.
  for _ in 0..20 {
    screen.press("Tab");
  }

  let focused = screen.focused();

  assert_eq!(focused.name(), "Open 19");

  let list = screen.find_by_role(Role::List, "Actions");
  let bounds = list.read().bounds;

  assert!(
    within(focused.read().bounds, bounds),
    "the focused row shows: {:?} in {bounds:?}",
    focused.read().bounds
  );

  // The wheel away and back keeps focus on it.
  list.scroll(0, 30);
  assert_eq!(screen.focused().name(), "Open 19");
  list.scroll(0, -30);
  assert_eq!(screen.focused().name(), "Open 19");

  screen.press("shift+Tab");
  assert_eq!(screen.focused().name(), "Open 18");
}

#[skene_gui::test]
fn tab_leaves_a_list_whose_next_stop_is_beyond_a_page() {
  let screen = testing::render(|| buttons(300));

  screen.press("Tab");
  screen.press("Tab");
  assert_eq!(screen.focused().name(), "Open 0");

  // The next button is 300 rows on, past a page of look-ahead: Tab leaves the list.
  screen.press("Tab");
  assert_eq!(screen.focused().name(), "After");

  // Scrolled there first, it is reached.
  let list = screen.find_by_role(Role::List, "Actions");

  list.scroll(0, 100);

  // However far a detent goes (15 pixels where the wheel is KWin's fake one, a line and a
  // half where it reaches KWin's Xwayland), learnt from those hundred: as many as bring the
  // button's row to the middle of the view, then one at a time until it shows whole.
  let bounds = list.read().bounds;
  let item = screen.find_all_by_role(Role::ListItem)[0].read();
  let place = item.position_in_set.expect("an item tells its place") - 1;
  let offset = place as f32 * 40. - (item.bounds.y - bounds.y);
  let detent = offset / 100.;
  let middle = 300. * 40. - (bounds.height - 40.) / 2.;

  list.scroll(0, ((middle - offset) / detent).round() as i32);

  for _ in 0..20 {
    let shown = screen
      .query_by_role(Role::Button, "Open 300")
      .is_some_and(|button| within(button.read().bounds, bounds));

    if shown {
      break;
    }

    list.scroll(0, 1);
  }

  screen.find_by_role(Role::Button, "Open 300").click();
  assert_eq!(screen.focused().name(), "Open 300");
}

/// A list box "Fruit" choosing several of five thousand options, "Fruit 0" onwards, bound to
/// `chosen`.
fn fruit(chosen: Signal<HashSet<usize>>) -> impl View {
  Column::new()
    .set_padding(8)
    .add_child(Listbox::new("Fruit").bind_chosen(chosen).derive_options(
      || (0..5000).collect::<Vec<_>>(),
      |value: &usize| format!("Fruit {value}"),
    ))
}

#[skene_gui::test]
fn a_list_box_moves_and_chooses_through_options_it_has_not_built() {
  let chosen = Signal::new(HashSet::new());
  let screen = testing::render(move || fruit(chosen));
  let options = screen.find_all_by_role(Role::Option);

  assert!(
    (8..=20).contains(&options.len()),
    "about the list's view of options is built: {}",
    options.len()
  );
  assert_eq!(options[0].read().set_size, Some(5000));
  assert!(
    screen.query_by_role(Role::Option, "Fruit 4999").is_none(),
    "the last option is not built"
  );

  screen.find_by_role(Role::Option, "Fruit 0").click();
  assert_eq!(chosen.get_untracked(), HashSet::from([0]));

  // End reaches the last option, built as focus moves there.
  screen.press("End");

  let last = screen.focused();

  assert_eq!(last.name(), "Fruit 4999");
  assert_eq!(last.read().position_in_set, Some(5000));

  // Shift and a click choose every option from the one chosen alone.
  let point = screen.driver().point_in(last.element(), 0.5, 0.5);

  screen.driver().shift_click_point(point);
  screen.settle();
  assert_eq!(
    chosen.get_untracked().len(),
    5000,
    "Shift and a click chose the whole range, built or not"
  );

  // A click takes one away; Ctrl and A choose every option.
  screen.find_by_role(Role::Option, "Fruit 4999").click();
  assert_eq!(chosen.get_untracked().len(), 4999);
  screen.press("ctrl+a");
  assert_eq!(
    chosen.get_untracked().len(),
    5000,
    "Ctrl and A chose every option"
  );

  // Typing a name's start finds an option the list has not built.
  screen.press("Home");
  assert_eq!(screen.focused().name(), "Fruit 0");
  screen.type_text("Fruit 2500");
  assert_eq!(screen.focused().name(), "Fruit 2500");
}

/// A list 300 pixels tall named "Items" of the items `items` holds, each "Item {n}" in a box
/// 30 pixels tall.
fn held(items: Signal<Vec<usize>>) -> impl View {
  Column::new().add_child(
    LazyColumn::new()
      .set_height(300)
      .set_label("Items")
      .set_item_extent(ItemExtent::Estimated(30.))
      .derive_items(
        move || items.get(),
        |index| *index,
        |index: usize| {
          Container::new()
            .set_height(30)
            .set_child(Text::new(format!("Item {index}")))
        },
      ),
  )
}

#[skene_gui::test]
fn insertions_above_the_viewport_move_nothing_and_at_the_top_they_show() {
  let items = Signal::new((0..1000).collect::<Vec<_>>());
  let screen = testing::render(move || held(items));
  let list = screen.find_by_role(Role::List, "Items");

  list.scroll(0, 5);

  let first = realized(&screen)[0].0;
  let name = format!("Item {}", first + 4);
  let before = screen.find_by_text(&name).read().bounds.y;

  // Five items inserted at the start, above the viewport.
  items.update(|items| items.splice(0..0, 10_000..10_005).for_each(drop));
  screen.settle();

  let after = screen.find_by_text(&name).read().bounds.y;

  assert!(
    (after - before).abs() <= 1.,
    "{name} stays where it was: {before} then {after}"
  );

  // Scrolled to the start, an insertion there shows, as Chromium anchors nothing at zero.
  screen.press("Tab");
  screen.press("Home");
  items.update(|items| items.insert(0, 20_000));
  screen.settle();

  let top = screen.find_by_text("Item 20000").read().bounds;

  assert!(
    within(top, list.read().bounds) && (top.y - list.read().bounds.y).abs() <= 1.,
    "the item inserted at the top shows there: {top:?}"
  );
}

#[skene_gui::test]
fn a_bound_position_scrolls_there_and_follows_the_scroll() {
  let position = Signal::new(ScrollPosition::at(5000));
  let screen = testing::render(move || {
    Column::new().add_child(
      LazyColumn::new()
        .set_height(300)
        .set_label("Items")
        .set_item_extent(ItemExtent::Estimated(30.))
        .bind_scroll_position(position)
        .derive_items(
          || (0..10_000).collect::<Vec<_>>(),
          |index| *index,
          |index: usize| {
            Container::new()
              .set_height(30)
              .set_child(Text::new(format!("Item {index}")))
          },
        ),
    )
  });
  let list = screen.find_by_role(Role::List, "Items");
  let top = screen.find_by_text("Item 5000").read().bounds;

  assert!(
    (top.y - list.read().bounds.y).abs() <= 1.,
    "the item the position names is at the top: {top:?} in {:?}",
    list.read().bounds
  );

  list.scroll(0, 3);

  let now = position.get_untracked();

  assert!(
    now.index > 5000,
    "the bound position follows the wheel: {now:?}"
  );

  // It names the item at the top as the list shows it after the wheel, not before.
  let shown = screen
    .find_by_text(&format!("Item {}", now.index))
    .read()
    .bounds;

  assert!(
    (list.read().bounds.y + now.offset - shown.y).abs() <= 1.,
    "the bound position is the one shown: {now:?}, the item at {shown:?} in {:?}",
    list.read().bounds
  );

  // Written, the position scrolls the list there.
  position.set(ScrollPosition::at(42));
  screen.settle();

  let top = screen.find_by_text("Item 42").read().bounds;

  assert!(
    (top.y - list.read().bounds.y).abs() <= 1.,
    "the written position scrolled the list: {top:?}"
  );
}

#[skene_gui::test]
fn a_reordering_keeps_focus_on_the_moved_row() {
  let items = Signal::new((0..1000).collect::<Vec<_>>());
  let screen = testing::render(move || {
    Column::new().add_child(
      LazyColumn::new()
        .set_height(300)
        .set_label("Actions")
        .set_item_extent(ItemExtent::Estimated(40.))
        .derive_items(
          move || items.get(),
          |index| *index,
          |index: usize| {
            Container::new()
              .set_height(40)
              .set_child(Button::new(format!("Open {index}")))
          },
        ),
    )
  });

  for _ in 0..4 {
    screen.press("Tab");
  }

  assert_eq!(screen.focused().name(), "Open 3");

  let before = screen.focused().read().bounds.y;

  // The first six rows reversed: row 3 moves up a row and keeps focus.
  items.update(|items| items[..6].reverse());
  screen.settle();

  let focused = screen.focused();

  assert_eq!(focused.name(), "Open 3", "focus stays on the moved row");
  assert!(
    (before - focused.read().bounds.y - 40.).abs() <= 1.,
    "the row moved up a row: from {before} to {}",
    focused.read().bounds.y
  );
}

#[skene_gui::test]
fn a_grid_lays_its_cells_in_lines_and_keeps_the_first_in_view_through_a_resize() {
  let screen = testing::render(|| {
    Column::new().add_child(
      LazyVerticalGrid::new()
        .set_columns(GridCells::Adaptive(100.))
        .set_height(300)
        .set_label("Photos")
        .set_item_extent(ItemExtent::Estimated(40.))
        .derive_items(
          || (0..3000).collect::<Vec<_>>(),
          |index| *index,
          |index: usize| {
            Container::new()
              .set_height(40)
              .set_child(Text::new(format!("Photo {index}")))
          },
        ),
    )
  });
  let first = screen.find_by_text("Photo 0").read().bounds;
  let second = screen.find_by_text("Photo 1").read().bounds;

  assert!(
    (first.y - second.y).abs() <= 1. && second.x > first.x,
    "two cells share a line: {first:?} {second:?}"
  );

  let per_line = (0..10)
    .take_while(|index| {
      screen
        .query_by_text(&format!("Photo {index}"))
        .is_some_and(|found| (found.read().bounds.y - first.y).abs() <= 1.)
    })
    .count();

  assert_eq!(per_line, 4, "four cells of 100 or more fit in 480");
  assert!(
    realized(&screen).len() < 60,
    "about the viewport's lines are realized"
  );

  let list = screen.find_by_role(Role::List, "Photos");

  list.scroll(0, 10);

  // The first photo wholly in view, however far the wheel went.
  let bounds = list.read().bounds;
  let anchor = realized(&screen)
    .into_iter()
    .map(|(place, _)| format!("Photo {}", place - 1))
    .find(|name| {
      screen
        .query_by_text(name)
        .is_some_and(|found| within(found.read().bounds, bounds))
    })
    .expect("a photo is in view");
  let at = screen.find_by_text(&anchor).read().bounds.y;

  // Narrower, two a line: the first line in view still shows.
  screen.set_width(240);

  let now = screen.find_by_text(&anchor).read().bounds;
  let bounds = list.read().bounds;

  assert!(
    within(now, bounds),
    "{anchor} stays in view through the resize: {at} then {now:?} in {bounds:?}"
  );
}

#[skene_gui::test]
fn a_lazy_row_realizes_what_its_width_shows() {
  let screen = testing::render(|| {
    Column::new().add_child(
      LazyRow::new()
        .set_width(400)
        .set_height(40)
        .set_label("Chips")
        .set_item_extent(80)
        .derive_items(
          || (0..1000).collect::<Vec<_>>(),
          |index| *index,
          |index: usize| Container::new().set_child(Text::new(format!("Chip {index}"))),
        ),
    )
  });
  let shown = realized(&screen);

  assert!(
    (5..=10).contains(&shown.len()),
    "about the five chips in view and a few beyond: {shown:?}"
  );

  let first = screen.find_by_text("Chip 0").read().bounds;
  let second = screen.find_by_text("Chip 1").read().bounds;

  assert!(
    (second.x - first.x - 80.).abs() <= 1.,
    "each chip is 80 wide: {first:?} {second:?}"
  );

  screen.find_by_role(Role::List, "Chips").scroll(20, 0);
  assert!(
    realized(&screen)[0].0 > shown[0].0,
    "the horizontal wheel moved the window: {:?}",
    realized(&screen)
  );
}
