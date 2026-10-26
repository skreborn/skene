//! The picker in cells: the face opens the list and focus lands on the chosen option,
//! the keys walk and choose, the value signal is the truth, `Escape`, a dismiss and the
//! face's own click close it with the value untouched, typing picks by name on the
//! closed face and in the open list, and the list opens where the picker places it.

use std::{cell::RefCell, rc::Rc};

use skene_foundation::TYPE_AHEAD_RESET;
use skene_simple_terminal::prelude::*;
use skene_terminal::{
  Label,
  NodeId,
  Part,
  Scene,
  behavior::{Activate, ListboxOption},
  platform::terminal::mount,
  prelude::*,
  testing,
};

use crate::common::{colors_at, locate, read, red, rgb, rows, swatches, with_platform};

fn picker(unit: Signal<&'static str>) -> Picker<&'static str> {
  SimplePicker::new()
    .set_options((
      SimpleListboxOption::new("px", "pixels"),
      SimpleListboxOption::new("cell", "cells"),
      SimpleListboxOption::new("pt", "points"),
    ))
    .bind_chosen(unit)
}

/// Mounts a picker bound to `unit` at the top of the frame.
fn mount_picker(unit: Signal<&'static str>) {
  let _ = mount(|| Column::new().add_child(picker(unit)).build());
}

/// The row of the last line containing `needle`: the popup's row, beneath the face
/// that shows the same label.
fn last_row(frame: &str, needle: &str) -> Option<u16> {
  frame
    .lines()
    .collect::<Vec<_>>()
    .iter()
    .rposition(|line| line.contains(needle))
    .and_then(|row| u16::try_from(row).ok())
}

/// `Enter` opens the picker's options popup beneath the face with the chosen row
/// highlighted, a row click selects and closes, and `Escape` dismisses without
/// selecting.
#[test]
fn picker_opens_a_popup_and_selects_by_click() {
  with_platform(|| {
    let unit = Signal::new("px");

    mount_picker(unit);

    let closed = testing::render_to_text(20, 10);

    assert!(!closed.contains("cells"), "closed shows only the label");

    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    let open = testing::render_to_text(20, 10);
    let (face_column, face_row) = locate(&open, "pixels");
    let chosen_row = last_row(&open, "pixels").expect("the popup lists the chosen option");
    let other_row = last_row(&open, "cells").expect("the popup lists the other option");

    assert!(
      chosen_row > face_row,
      "the popup opens beneath the face: {open}"
    );
    assert_ne!(
      colors_at(20, 10, usize::from(face_column), usize::from(chosen_row)).1,
      colors_at(20, 10, usize::from(face_column), usize::from(other_row)).1,
      "the chosen row is highlighted: {open}",
    );

    testing::trigger_mouse_click(face_column, other_row);

    assert_eq!(unit.get_untracked(), "cell", "the row click selected");

    let after_click = testing::render_to_text(20, 10);

    assert!(
      !after_click.contains("pixels"),
      "the selection closed the popup: {after_click}"
    );

    testing::trigger_key("Enter");
    testing::trigger_key("Escape");

    let dismissed = testing::render_to_text(20, 10);

    assert!(
      !dismissed.contains("pixels"),
      "escape dismissed the reopened popup: {dismissed}"
    );
    assert_eq!(unit.get_untracked(), "cell");
  });
}

/// Popup keyboard navigation moves focus without committing: `Escape` keeps the old
/// value, `Enter` chooses the focused option.
#[test]
fn picker_popup_navigates_without_committing() {
  with_platform(|| {
    let unit = Signal::new("px");

    mount_picker(unit);
    let _ = testing::render_to_text(20, 10);

    testing::trigger_key("Tab");
    testing::trigger_key("Enter");
    testing::trigger_key("ArrowDown");
    testing::trigger_key("Escape");

    assert_eq!(unit.get_untracked(), "px", "escape keeps the old value");

    testing::trigger_key("Enter");
    testing::trigger_key("ArrowDown");
    testing::trigger_key("Enter");

    assert_eq!(
      unit.get_untracked(),
      "cell",
      "enter chooses the focused option"
    );
  });
}

/// `ArrowDown` on a closed picker opens the list rather than cycling the value, and
/// the list's `End` and `Home` reach its ends.
#[test]
fn picker_arrows_open_the_list_and_home_and_end_reach_its_ends() {
  with_platform(|| {
    let unit = Signal::new("px");

    mount_picker(unit);
    let _ = testing::render_to_text(20, 10);

    testing::trigger_key("Tab");
    testing::trigger_key("ArrowDown");

    assert!(
      testing::render_to_text(20, 10).contains("cells"),
      "the list opened"
    );
    assert_eq!(unit.get_untracked(), "px", "opening chose nothing");

    testing::trigger_key("End");
    testing::trigger_key("Enter");

    assert_eq!(unit.get_untracked(), "pt");

    testing::trigger_key("ArrowUp");
    testing::trigger_key("Home");
    testing::trigger_key(" ");

    assert_eq!(unit.get_untracked(), "px");
  });
}

/// The options popup anchors beneath the picker and closes when focus leaves it.
#[test]
fn picker_popup_anchors_beneath_and_closes_on_blur() {
  with_platform(|| {
    let unit = Signal::new("px");

    let _ = mount(|| {
      Column::new()
        .set_children((
          picker(unit).into_any(),
          BasicButton::new(Activate::new())
            .set_label("next")
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 12);

    testing::trigger_key("Tab");

    let face = testing::focused_node();

    testing::trigger_key("Enter");

    let open = testing::render_to_text(20, 12);
    let (_, face_row) = locate(&open, "pixels");
    let list_row = last_row(&open, "pixels").expect("the list shows");

    assert!(
      list_row > face_row && open.contains("cells"),
      "the popup opens beneath the face: {open}"
    );

    testing::trigger_key("Tab");

    let blurred = testing::render_to_text(20, 12);

    assert!(
      !blurred.contains("cells"),
      "focus moving on closes the popup: {blurred}"
    );
    assert_ne!(testing::focused_node(), face, "focus moved on");
    assert_eq!(unit.get_untracked(), "px", "nothing was chosen");
  });
}

/// `Tab` on a focused option commits it, closes the popup and travels on to the next
/// tab stop, so the keys that follow reach that stop rather than the closed list.
#[test]
fn picker_tab_commits_closes_and_travels_on() {
  with_platform(|| {
    let unit = Signal::new("px");
    let next = NodeHandle::new();

    let _ = mount(move || {
      Column::new()
        .set_children((
          picker(unit).into_any(),
          BasicButton::new(Activate::new())
            .set_label("next")
            .bind_handle(next)
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 12);

    testing::trigger_key("Tab");
    testing::trigger_key("Enter");
    testing::trigger_key("ArrowDown");
    testing::trigger_key("Tab");

    assert_eq!(
      unit.get_untracked(),
      "cell",
      "tab committed the focused option"
    );
    assert_eq!(testing::focused_node(), next.node(), "tab travelled on");

    let closed = testing::render_to_text(20, 12);

    assert!(!closed.contains("points"), "the popup closed: {closed}");

    testing::trigger_key(" ");

    let after = testing::render_to_text(20, 12);

    assert!(
      !after.contains("points"),
      "space reached the next stop: {after}"
    );
    assert_eq!(unit.get_untracked(), "cell");
  });
}

/// Typing over the closed picker chooses by the option's caption without opening the
/// list, and typing over the open list moves focus to the matching row for `Enter` to
/// choose.
#[test]
fn picker_type_ahead_chooses_closed_and_focuses_open() {
  with_platform(|| {
    let unit = Signal::new("px");

    mount_picker(unit);
    let _ = testing::render_to_text(20, 10);

    testing::trigger_key("Tab");
    testing::trigger_key("c");

    assert_eq!(unit.get_untracked(), "cell", "a character chooses closed");

    let closed = testing::render_to_text(20, 10);

    assert!(
      !closed.contains("points"),
      "the list stays closed: {closed}"
    );

    testing::trigger_frame(std::time::Duration::from_secs(1));
    testing::trigger_key("Enter");
    testing::trigger_key("p");
    testing::trigger_key("o");

    assert_eq!(
      unit.get_untracked(),
      "cell",
      "typing over the list chooses nothing"
    );

    testing::trigger_key("Enter");

    assert_eq!(
      unit.get_untracked(),
      "pt",
      "enter chooses the focused match"
    );
  });
}

/// A press on the face of an open picker is the face's click, which closes the popup;
/// it is never a dismissal the click would reopen.
#[test]
fn a_press_on_an_open_pickers_face_closes_it() {
  with_platform(|| {
    let unit = Signal::new("px");

    mount_picker(unit);
    let _ = testing::render_to_text(20, 12);

    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    let open = testing::render_to_text(20, 12);
    let (column, row) = locate(&open, "pixels");

    assert!(open.contains("points"), "the popup opened: {open}");

    testing::trigger_mouse_click(column, row);

    let closed = testing::render_to_text(20, 12);

    assert!(
      !closed.contains("points"),
      "the face's click closed it: {closed}"
    );
    assert_eq!(unit.get_untracked(), "px");

    testing::trigger_mouse_click(column, row);

    assert!(
      testing::render_to_text(20, 12).contains("points"),
      "the next click reopens it"
    );
  });
}

/// A picker's face keeps its ring while its list is open, though focus sits on an
/// option.
#[test]
fn a_pickers_face_keeps_its_ring_while_open() {
  with_platform(|| {
    let unit = Signal::new("px");

    mount_picker(unit);

    let closed = testing::render_to_text(20, 12);
    let (column, row) = locate(&closed, "pixels");
    let border = |x: u16| colors_at(20, 12, usize::from(x), usize::from(row) - 1).0;
    let resting = border(column);

    testing::trigger_key("Tab");

    let focused = border(column);

    assert_ne!(focused, resting, "focus rings the face");

    testing::trigger_key("Enter");

    assert!(
      testing::render_to_text(20, 12).contains("points"),
      "the list opened"
    );
    assert_ne!(testing::focused_node(), None);
    assert_eq!(border(column), focused, "the ring holds while open");

    testing::trigger_key("Escape");

    assert_eq!(border(column), focused, "focus returns to the face");
  });
}

/// The open list marks the chosen row alone: an unchosen row's check is hidden, not
/// drawn in its surface's color.
#[test]
fn picker_marks_the_chosen_row_alone() {
  with_platform(|| {
    let unit = Signal::new("cell");

    mount_picker(unit);

    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    let open = testing::render_to_text(20, 10);
    let marked: Vec<&str> = open.lines().filter(|line| line.contains('✓')).collect();

    assert_eq!(marked.len(), 1, "{open}");
    assert!(marked[0].contains("cells"), "{open}");
  });
}

/// A picker named "Size" bound to `value`, over the options `a`, `b` and `c`, its face
/// showing the chosen value.
fn size_picker(value: Signal<&'static str>) -> BasicPicker<&'static str> {
  BasicPicker::new()
    .bind_chosen(value)
    .set_label("Size")
    .set_options(
      ["a", "b", "c"]
        .map(|value| BasicListboxOption::new(ListboxOption::new(value)).set_label(value)),
    )
    .set_face(|shown| Text::derive(move || shown.get().unwrap_or_default().to_owned()))
}

/// A picker named "Fruit" bound to `value`, its options named apart from their values
/// for type-ahead: `Label::set_name` supplies the name the characters match.
fn fruit_picker(value: Signal<&'static str>) -> BasicPicker<&'static str> {
  BasicPicker::new()
    .bind_chosen(value)
    .set_label("Fruit")
    .set_options(
      [("ap", "Apple"), ("av", "Avocado"), ("ba", "Banana")].map(|(value, name)| {
        BasicListboxOption::new(ListboxOption::new(value))
          .set_label(Label::new(Text::new(name)).set_name(name))
      }),
    )
    .set_face(|shown| Text::derive(move || shown.get().unwrap_or_default().to_owned()))
}

/// Mounts `picker` above a button, inside a box whose key handler logs the keys that
/// climb to it, and returns the log.
fn mount_logged(picker: BasicPicker<&'static str>) -> Rc<RefCell<Vec<String>>> {
  let heard = Rc::new(RefCell::new(Vec::new()));
  let log = Rc::clone(&heard);

  let _ = mount(move || {
    Container::new()
      .handle_key_down(move |event| log.borrow_mut().push(event.key.to_string()))
      .set_child(
        Column::new().set_children((
          picker.into_any(),
          BasicButton::new(Activate::new())
            .set_label("next")
            .into_any(),
        )),
      )
      .into_any()
      .build()
  });
  let _ = testing::render_to_text(20, 10);

  heard
}

/// The face a reader finds by its role and `name`, after a frame.
fn face(name: &str) -> NodeId {
  let _ = testing::render_to_text(20, 10);

  testing::find_by_role(Role::ComboBox, name).expect("the face")
}

/// The option a reader finds by its role and `name` in the open list, after a frame.
fn option(name: &str) -> NodeId {
  let _ = testing::render_to_text(20, 10);

  testing::find_by_role(Role::ListBoxOption, name).expect("the option")
}

/// Whether the list is open, as the face reports it.
fn is_expanded(name: &str) -> bool {
  read(face(name)).is_expanded() == Some(true)
}

/// What the face shows on its row, beneath its caption.
fn shown() -> String {
  testing::render_to_text(20, 10)
    .lines()
    .nth(1)
    .unwrap_or_default()
    .trim()
    .to_owned()
}

/// The face is a combobox reporting whether it is expanded; `ArrowDown` opens the
/// anchored list and focus lands on the chosen option, which reports itself selected,
/// and the selection follows the signal.
#[test]
fn arrow_down_opens_the_list_with_focus_on_the_chosen_option() {
  with_platform(|| {
    let value = Signal::new("b");
    let _ = mount_logged(size_picker(value));

    assert_eq!(read(face("Size")).role(), Role::ComboBox);
    assert_eq!(shown(), "b");
    assert!(!is_expanded("Size"));
    assert!(
      testing::find_all_by_role(Role::ListBox).is_empty(),
      "the closed list is out of the tree"
    );

    testing::trigger_key("Tab");
    testing::trigger_key("ArrowDown");

    assert!(is_expanded("Size"));
    assert_eq!(testing::find_all_by_role(Role::ListBox).len(), 1);
    assert_eq!(testing::find_all_by_role(Role::ListBoxOption).len(), 3);
    assert_eq!(testing::focused_node(), Some(option("b")));
    assert_eq!(read(option("a")).is_selected(), Some(false));
    assert_eq!(read(option("b")).is_selected(), Some(true));

    value.set("c");

    assert_eq!(read(option("b")).is_selected(), Some(false));
    assert_eq!(read(option("c")).is_selected(), Some(true));

    testing::trigger_key("Escape");

    assert_eq!(shown(), "c");
  });
}

/// A picker without its face renders nothing: no face, no list, no option and no
/// caption.
#[test]
fn a_picker_without_its_face_renders_nothing() {
  with_platform(|| {
    let _ = mount(|| {
      Column::new()
        .add_child(
          BasicPicker::<&str>::new()
            .set_label("Size")
            .set_options((BasicListboxOption::new(ListboxOption::new("a")).set_label("a"),)),
        )
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(20, 4).trim(), "");
    assert_eq!(
      testing::find_all_by_role(Role::ComboBox),
      Vec::<NodeId>::new()
    );
    assert_eq!(
      testing::find_all_by_role(Role::ListBoxOption),
      Vec::<NodeId>::new()
    );
    assert_eq!(testing::find_by_text("Size"), None);
  });
}

/// A picker given its face takes its setters still: the label, the value and the
/// options set after the face reach the face and the list.
#[test]
fn a_picker_given_its_face_takes_its_setters() {
  with_platform(|| {
    let value = Signal::new("b");
    let picker = BasicPicker::<&str>::new()
      .set_face(|shown| Text::derive(move || shown.get().unwrap_or_default().to_owned()));
    let _ = mount_logged(picker.bind_chosen(value).set_label("Size").set_options(
      ["a", "b"].map(|value| BasicListboxOption::new(ListboxOption::new(value)).set_label(value)),
    ));

    assert_eq!(shown(), "b");

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), Some(face("Size")));

    testing::trigger_key("ArrowDown");

    assert_eq!(testing::find_all_by_role(Role::ListBoxOption).len(), 2);
    assert_eq!(read(option("b")).is_selected(), Some(true));
  });
}

/// The arrows move focus over the options and clamp at both ends, `Home` and `End`
/// reach them, nothing is chosen on the way and a key the list means nothing by climbs,
/// and `Enter` chooses the focused option, closes the list and returns focus to the
/// face; a click on the face opens the list and one on an option chooses it.
#[test]
fn arrows_clamp_over_the_options_and_enter_chooses() {
  with_platform(|| {
    let value = Signal::new("b");
    let heard = mount_logged(size_picker(value));

    testing::trigger_key("Tab");
    testing::trigger_key("ArrowDown");

    for (key, focused) in [
      ("ArrowDown", "c"),
      ("ArrowDown", "c"),
      ("Home", "a"),
      ("ArrowUp", "a"),
      ("End", "c"),
    ] {
      testing::trigger_key(key);

      assert_eq!(testing::focused_node(), Some(option(focused)), "{key}");
    }

    assert_eq!(value.get_untracked(), "b");
    assert!(is_expanded("Size"));

    heard.borrow_mut().clear();
    testing::trigger_key("F5");

    assert_eq!(*heard.borrow(), ["F5"]);

    testing::trigger_key("Enter");

    assert_eq!(value.get_untracked(), "c");
    assert!(!is_expanded("Size"));
    assert_eq!(testing::focused_node(), Some(face("Size")));

    testing::user::click(face("Size"));

    assert!(is_expanded("Size"));
    assert_eq!(testing::focused_node(), Some(option("c")));
    assert_eq!(read(option("c")).is_selected(), Some(true));

    testing::user::click(option("a"));

    assert_eq!(value.get_untracked(), "a");
    assert!(!is_expanded("Size"));
  });
}

/// The value signal is the truth: a choice the app does not write back leaves the
/// selection, the face and the option focus lands on where they were.
#[test]
fn the_value_signal_is_the_truth() {
  with_platform(|| {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&asked);
    let _ = mount_logged(
      BasicPicker::new()
        .set_chosen("a")
        .set_label("Size")
        .handle_choose(move |value| log.borrow_mut().push(value))
        .set_options(
          ["a", "b"]
            .map(|value| BasicListboxOption::new(ListboxOption::new(value)).set_label(value)),
        )
        .set_face(|shown| Text::derive(move || shown.get().unwrap_or_default().to_owned())),
    );

    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    assert_eq!(testing::focused_node(), Some(option("a")));

    testing::trigger_key("ArrowDown");
    testing::trigger_key(" ");

    assert_eq!(*asked.borrow(), ["b"]);
    assert_eq!(shown(), "a");
    assert!(!is_expanded("Size"));

    testing::trigger_key("Enter");

    assert_eq!(testing::focused_node(), Some(option("a")));
    assert_eq!(read(option("a")).is_selected(), Some(true));
    assert_eq!(read(option("b")).is_selected(), Some(false));
  });
}

/// `Escape` on the closed face climbs; `Escape` from an option and a dismiss of the
/// popup close the list with the value untouched, `Escape` returning focus to the face,
/// and a click on the face toggles the list.
#[test]
fn escape_and_a_dismiss_close_without_changing_the_value() {
  with_platform(|| {
    let value = Signal::new("b");
    let heard = mount_logged(size_picker(value));

    testing::trigger_key("Tab");
    testing::trigger_key("Escape");

    assert_eq!(*heard.borrow(), ["Escape"], "the closed face lets it climb");

    testing::trigger_key(" ");

    assert!(is_expanded("Size"));

    testing::trigger_key("ArrowDown");
    testing::trigger_key("Escape");

    assert!(!is_expanded("Size"));
    assert_eq!(value.get_untracked(), "b");
    assert_eq!(testing::focused_node(), Some(face("Size")));
    assert_eq!(*heard.borrow(), ["Escape"], "the list took its Escape");

    testing::user::click(face("Size"));

    assert!(is_expanded("Size"));

    testing::trigger_mouse_click(19, 9);

    assert!(!is_expanded("Size"), "a press outside dismisses the list");
    assert_eq!(value.get_untracked(), "b");

    testing::user::click(face("Size"));

    assert!(is_expanded("Size"));

    testing::user::click(face("Size"));

    assert!(!is_expanded("Size"));
  });
}

/// A derived option name keeps type-ahead current: the characters match the name the
/// option goes by now.
#[test]
fn type_ahead_matches_a_derived_name_as_it_reads_now() {
  with_platform(|| {
    let value = Signal::new("ba");
    let renamed = Signal::new("Apple".to_owned());
    let _ = mount_logged(
      BasicPicker::new()
        .bind_chosen(value)
        .set_label("Fruit")
        .set_options([
          BasicListboxOption::new(ListboxOption::new("ap")).set_label(
            Label::new(Text::derive(move || renamed.get())).derive_name(move || renamed.get()),
          ),
          BasicListboxOption::new(ListboxOption::new("ba"))
            .set_label(Label::new(Text::new("Banana")).set_name("Banana")),
        ])
        .set_face(|shown| Text::derive(move || shown.get().unwrap_or_default().to_owned())),
    );

    renamed.set("Cherry".to_owned());
    testing::trigger_key("Tab");
    testing::trigger_key("c");

    assert_eq!(value.get_untracked(), "ap", "the name as it reads now");
  });
}

/// Typing over the closed face chooses by name without opening: two characters in
/// quick succession match a prefix, a character repeated cycles through the options
/// starting with it, and once the buffer's reset has elapsed the next character starts
/// afresh. `Space` extends a buffer already begun and toggles the list otherwise.
#[test]
fn typing_over_the_closed_face_chooses_by_name() {
  with_platform(|| {
    let value = Signal::new("ba");
    let _ = mount_logged(fruit_picker(value));

    testing::trigger_key("Tab");
    testing::trigger_key("a");

    assert_eq!(value.get_untracked(), "ap");
    assert!(!is_expanded("Fruit"), "type-ahead keeps the list closed");

    testing::trigger_key("v");

    assert_eq!(value.get_untracked(), "av", "the buffer matches a prefix");

    testing::trigger_frame(TYPE_AHEAD_RESET);
    testing::trigger_key("a");

    assert_eq!(
      value.get_untracked(),
      "ap",
      "a fresh character cycles past the chosen"
    );

    testing::trigger_key("a");

    assert_eq!(
      value.get_untracked(),
      "av",
      "a repeated character cycles on"
    );

    testing::trigger_key("z");

    assert_eq!(
      value.get_untracked(),
      "av",
      "no match leaves the value alone"
    );

    testing::trigger_key(" ");

    assert!(!is_expanded("Fruit"), "space extends the buffer begun");

    testing::trigger_frame(TYPE_AHEAD_RESET);
    testing::trigger_key(" ");

    assert!(is_expanded("Fruit"), "space opens the list without one");
  });
}

/// Typing over the open list moves focus to the option whose name the characters
/// begin, without choosing; `Space` extends a buffer already begun.
#[test]
fn typing_over_the_open_list_moves_focus_by_name() {
  with_platform(|| {
    let value = Signal::new("ap");
    let _ = mount_logged(fruit_picker(value));

    testing::trigger_key("Tab");
    testing::trigger_key("ArrowDown");

    assert_eq!(testing::focused_node(), Some(option("Apple")));

    testing::trigger_key("b");

    assert_eq!(
      testing::focused_node(),
      Some(option("Banana")),
      "focus reaches the match"
    );
    assert_eq!(value.get_untracked(), "ap", "typing chooses nothing");
    assert!(is_expanded("Fruit"));

    testing::trigger_frame(TYPE_AHEAD_RESET);
    testing::trigger_key("a");

    assert_eq!(
      testing::focused_node(),
      Some(option("Apple")),
      "a character wraps around"
    );

    testing::trigger_key("v");

    assert_eq!(
      testing::focused_node(),
      Some(option("Avocado")),
      "the buffer matches a prefix"
    );

    testing::trigger_key(" ");

    assert!(
      is_expanded("Fruit"),
      "space extends the buffer rather than choosing"
    );
    assert_eq!(value.get_untracked(), "ap");

    testing::trigger_frame(TYPE_AHEAD_RESET);
    testing::trigger_key(" ");

    assert_eq!(value.get_untracked(), "av", "a fresh space chooses");
    assert!(!is_expanded("Fruit"));
  });
}

/// The list opens below the face unless the picker places it elsewhere: placed on top,
/// its rows draw above the face.
#[test]
fn a_picker_places_its_list() {
  with_platform(|| {
    let value = Signal::new("a");

    let _ = mount(move || {
      Column::new()
        .set_children((
          Container::new().set_height(6).into_any(),
          size_picker(value).set_placement(Placement::Top).into_any(),
        ))
        .into_any()
        .build()
    });
    let closed = testing::render_to_text(20, 10);
    let (_, face_row) = locate(&closed, "a");

    testing::trigger_key("Tab");
    testing::trigger_key("ArrowDown");

    let open = testing::render_to_text(20, 10);
    let (_, list_row) = locate(&open, "c");

    assert!(list_row < face_row, "the list opens above: {open}");
  });
}

/// The built-in picker of sizes bound to `size`, its options built-in too.
fn styled_picker(size: Signal<u8>) -> Picker<u8> {
  Picker::new()
    .set_label("Size")
    .bind_chosen(size)
    .set_options((
      skene_terminal::ListboxOption::new(1_u8).set_label("Small"),
      skene_terminal::ListboxOption::new(2_u8).set_label("Large"),
    ))
}

/// Whether `node` sits inside a node recorded as `part`.
fn inside_part(node: NodeId, part: Part) -> bool {
  Scene::with(|scene| {
    std::iter::successors(Some(node), |node| scene.parent(*node))
      .any(|node| scene.part(node) == Some(part))
  })
}

/// The built-in picker draws its defaults in cells: a face showing the chosen option's
/// name and the indicator, its caption on the row above, the accent while focus shows;
/// open, its list framed in the border color, each option's check showing on the chosen
/// one alone, the list recorded as the picker's [`Part::List`].
#[test]
fn the_built_in_picker_draws_its_face_and_list_in_cells() {
  with_platform(|| {
    let size = Signal::new(2);
    let _ = mount(move || {
      Column::new()
        .set_children((
          styled_picker(size),
          swatches(&[
            SystemColors::CONTROL,
            SystemColors::ACCENT,
            SystemColors::BORDER,
            SystemColors::FIELD,
          ]),
        ))
        .build()
    });

    assert_eq!(
      rows(&testing::render_to_text(12, 8)),
      "Size\n Large ▾\n\n\n\n\n\n\n"
    );

    let swatch = |column: usize| colors_at(12, 8, column, 2).1;
    let (control, accent, border, field) = (swatch(0), swatch(1), swatch(2), swatch(3));

    assert!(
      control.is_some() && accent != control && border != field,
      "the swatches tell the colors apart"
    );
    assert_eq!(
      colors_at(12, 8, 1, 1).1,
      control,
      "the face in the control color"
    );

    testing::trigger_key("Tab");

    assert_eq!(
      colors_at(12, 8, 1, 1).1,
      accent,
      "the face takes the accent while focus shows"
    );

    testing::trigger_key("Enter");

    assert_eq!(
      rows(&testing::render_to_text(12, 8)),
      "Size\n Large ▾\n┌──────────┐\n│    Small │\n│ ✓  Large │\n└──────────┘\n\n\n",
      "the list opens beneath the face, the chosen option checked"
    );
    assert_eq!(
      colors_at(12, 8, 0, 2).0,
      border,
      "the list framed in the border color"
    );
    assert_eq!(
      colors_at(12, 8, 10, 3).1,
      field,
      "the list filled with the field color"
    );
    assert!(
      inside_part(option("Small"), Part::List),
      "the list is the picker's list"
    );

    testing::trigger_key("ArrowUp");
    testing::trigger_key("Enter");

    assert_eq!(size.get_untracked(), 1, "a choice reaches the signal");
    assert_eq!(
      rows(&testing::render_to_text(12, 8)),
      "Size\n Small ▾\n\n\n\n\n\n\n"
    );
  });
}

/// The built-in picker's face and list are values its setters replace: a face of the
/// app's own and a list box of its own.
#[test]
fn the_built_in_pickers_face_and_list_are_replaced_by_its_setters() {
  with_platform(|| {
    let size = Signal::new(2);
    let _ = mount(move || {
      Column::new()
        .add_child(
          styled_picker(size)
            .set_list(Container::new().set_background(red()))
            .set_face(|chosen| Text::derive(move || format!("<{}>", chosen.get().unwrap_or(0)))),
        )
        .build()
    });

    assert_eq!(
      rows(&testing::render_to_text(12, 8)).lines().nth(1),
      Some(" <2>")
    );

    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    let frame = rows(&testing::render_to_text(12, 8));

    assert!(
      !frame.contains('┌'),
      "the list's own box has no border: {frame}"
    );
    assert_eq!(
      colors_at(12, 8, 6, 2).1,
      Some(rgb(red())),
      "the list's own fill: {frame}"
    );
  });
}

/// The built-in listbox draws its options in a framed box beneath its caption, a check
/// before the chosen one; the arrows move focus and `Space` chooses, and a mark of the
/// app's own replaces the check, in a cell of its own width.
#[test]
fn the_built_in_listbox_draws_its_options_in_cells() {
  with_platform(|| {
    let fruit = Signal::new(Some(2));
    let _ = mount(move || {
      Column::new()
        .add_child(
          skene_terminal::Listbox::new()
            .set_label("Fruit")
            .bind_chosen(fruit)
            .set_children((
              skene_terminal::ListboxOption::new(1).set_label("Apple"),
              skene_terminal::ListboxOption::new(2).set_label("Banana"),
              skene_terminal::ListboxOption::new(3)
                .set_label("Cherry")
                .set_part(Part::Mark, Text::new("*")),
            )),
        )
        .build()
    });

    assert_eq!(
      rows(&testing::render_to_text(14, 8)),
      "Fruit\n\n┌───────────┐\n│    Apple  │\n│ ✓  Banana │\n│ * Cherry  │\n└───────────┘\n\n"
    );

    let resting = colors_at(14, 8, 4, 4).1;

    testing::trigger_key("Tab");

    assert_ne!(
      colors_at(14, 8, 4, 4).1,
      resting,
      "focus lands on the chosen option, filled while focused"
    );

    testing::trigger_key("ArrowUp");
    testing::trigger_key(" ");

    assert_eq!(
      fruit.get_untracked(),
      Some(1),
      "Space chooses the focused option"
    );
    assert!(
      rows(&testing::render_to_text(14, 8)).contains("│ ✓  Apple"),
      "the check follows the choice"
    );
  });
}
