//! Controls in cells: focus and the keys the behaviors define, the toggles, the slider
//! and its fill's edge, the picker and its popup's closing, a disabled control, and the
//! Simple controls' shape.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use accesskit::{HasPopup, Invalid, Orientation, Toggled};
use skene_simple_terminal::prelude::*;
use skene_terminal::{
  self as terminal,
  BasicListbox,
  NativeElement,
  NodeId,
  Part,
  Prevent,
  Scene,
  WheelEvent,
  behavior::{
    Activate,
    Behavior,
    ControlHandle,
    EventKind,
    ListboxOption,
    Popup,
    PopupKind,
    Radio,
    Slider,
    SliderRange,
    TextEntry,
    Toggle,
  },
  platform::terminal::Terminal,
  prelude::*,
  testing,
  testing::user,
};

use crate::common::{
  blue,
  colors_at,
  green,
  is_live,
  locate,
  mount_nodes,
  read,
  red,
  rgb,
  rows,
  white,
  with_platform,
};

/// Tab focuses a button, a `Focused` variant applies, and `Enter` clicks it.
#[test]
fn focus_variant_and_enter_click() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = terminal::mount(|| {
      BasicButton::new(Activate::new())
        .set_width(6)
        .set_height(3)
        .set_border_width(Inset::all(1))
        .set_border_color(Color::TRANSPARENT.when(State::Focused, white()))
        .handle_activate(move || counted.set(counted.get() + 1))
        .into_any()
        .build()
    });

    assert!(!testing::render_to_text(8, 4).contains('┌'));

    testing::trigger_key("Tab");

    assert!(testing::render_to_text(8, 4).contains('┌'));

    testing::trigger_key("Enter");

    assert_eq!(clicks.get(), 1);
  });
}

/// A window opens with nothing focused where no control asks for focus.
#[test]
fn a_window_opens_with_nothing_focused() {
  with_platform(|| {
    let _ = mount_nodes(|| {
      [
        BasicButton::new(Activate::new()).set_label("a").build(),
        BasicTextField::new(TextEntry::new()).set_width(6).build(),
      ]
    });
    let _ = testing::render_to_text(10, 3);

    assert_eq!(testing::focused_node(), None);
  });
}

/// A control set to autofocus takes focus as its window opens, the first in the
/// scene's child order of those that ask and are enabled, without showing it, as
/// Chromium's autofocus does; `Tab` goes on from it, and a control built after the
/// window opened takes nothing.
#[test]
fn an_autofocus_control_takes_focus_as_its_window_opens() {
  with_platform(|| {
    let [_, _, b, _] = mount_nodes(|| {
      [
        BasicButton::new(Activate::new()).set_label("a").build(),
        BasicButton::new(Activate::new())
          .set_label("off")
          .set_disabled(true)
          .set_autofocus(true)
          .build(),
        BasicButton::new(Activate::new())
          .set_label("b")
          .set_width(3)
          .set_background(red().when(State::FocusVisible, blue()))
          .set_autofocus(true)
          .build(),
        BasicButton::new(Activate::new())
          .set_label("c")
          .set_autofocus(true)
          .build(),
      ]
    });
    let frame = testing::render_to_text(10, 6);
    let (column, row) = locate(&frame, "b");

    assert_eq!(testing::focused_node(), Some(b));
    assert_eq!(
      colors_at(10, 6, usize::from(column), usize::from(row)).1,
      Some(rgb(red())),
      "focus does not show: {frame}"
    );

    let later = mount_nodes(|| {
      [BasicButton::new(Activate::new())
        .set_label("d")
        .set_autofocus(true)
        .build()]
    });

    assert_eq!(testing::focused_node(), Some(b), "the window has opened");
    assert_ne!(Some(later[0]), testing::focused_node());

    testing::trigger_key("Tab");

    assert_ne!(testing::focused_node(), Some(b), "Tab goes on from it");

    testing::trigger_key("Shift+Tab");

    assert_eq!(testing::focused_node(), Some(b));
    assert_eq!(
      colors_at(10, 6, usize::from(column), usize::from(row)).1,
      Some(rgb(blue())),
      "focus a key moves shows"
    );
  });
}

/// A radio group set to autofocus opens its window focused on the radio holding its
/// stop, the chosen one.
#[test]
fn a_radio_group_set_to_autofocus_focuses_its_chosen_radio() {
  with_platform(|| {
    let chosen = Signal::new(Some("b"));

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new()).set_label("before"),
          BasicRadioGroup::new()
            .set_label("Size")
            .bind_chosen(chosen)
            .set_autofocus(true)
            .set_child(Column::new().set_children((
              BasicRadio::new(Radio::new(Some("a"))).set_label("a"),
              BasicRadio::new(Radio::new(Some("b"))).set_label("b"),
            ))),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 6);

    assert_eq!(
      testing::focused_node(),
      testing::find_by_role(Role::RadioButton, "b"),
      "the chosen radio takes focus"
    );
  });
}

/// A listbox set to autofocus opens its window focused on the option holding its stop,
/// the first where nothing is chosen.
#[test]
fn a_listbox_set_to_autofocus_focuses_its_stop() {
  with_platform(|| {
    let chosen: Signal<Option<&str>> = Signal::new(None);

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new()).set_label("before"),
          BasicListbox::new()
            .set_accessible_name("Fruit")
            .bind_chosen(chosen)
            .set_autofocus(true)
            .set_child(Column::new().set_children((
              BasicListboxOption::new(ListboxOption::new("a")).set_label("a"),
              BasicListboxOption::new(ListboxOption::new("c")).set_label("c"),
            ))),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 6);

    assert_eq!(
      testing::focused_node(),
      testing::find_by_role(Role::ListBoxOption, "a"),
      "the first option takes focus"
    );
  });
}

/// A mouse click hits the topmost control at the cell, runs its click handlers, and
/// focuses it, so `Enter` and `Space` click it again.
#[test]
fn mouse_click_hits_and_focuses() {
  with_platform(|| {
    let first = Rc::new(Cell::new(0u32));
    let second = Rc::new(Cell::new(0u32));
    let (count_first, count_second) = (Rc::clone(&first), Rc::clone(&second));

    let _ = terminal::mount(|| {
      Row::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_width(4)
            .set_height(2)
            .handle_activate(move || count_first.set(count_first.get() + 1)),
          BasicButton::new(Activate::new())
            .set_width(4)
            .set_height(2)
            .handle_activate(move || count_second.set(count_second.get() + 1)),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 3);

    testing::trigger_mouse_click(5, 0);
    testing::trigger_key("Enter");
    testing::trigger_key(" ");

    assert_eq!(first.get(), 0);
    assert_eq!(second.get(), 3);
  });
}

/// Where the terminal reports no key releases, each `Space` press clicks a button, its
/// release following at once; where it reports them, the release clicks it.
#[test]
fn space_clicks_on_the_release_the_terminal_reports_or_at_once() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = terminal::mount(|| {
      BasicButton::new(Activate::new())
        .set_width(4)
        .set_height(1)
        .handle_activate(move || counted.set(counted.get() + 1))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(6, 2);

    testing::trigger_key("Tab");
    testing::trigger_key(" ");

    assert_eq!(clicks.get(), 1);

    testing::trigger_key_releases(true);
    testing::trigger_key(" ");

    assert_eq!(clicks.get(), 1);

    testing::trigger_key_up(" ");

    assert_eq!(clicks.get(), 2);
  });
}

/// A checkbox toggles by `Space` and by mouse click, both directions of the binding,
/// and its mark repaints as the checked glyph.
#[test]
fn checkbox_toggles_by_key_and_click() {
  with_platform(|| {
    let on = Signal::new(false);

    let _ = terminal::mount(|| SimpleCheckbox::new().bind_checked(on).build());

    assert!(testing::render_to_text(6, 1).contains("[ ]"));

    testing::trigger_key("Tab");
    testing::trigger_key(" ");

    assert!(on.get_untracked());
    assert!(testing::render_to_text(6, 1).contains("[x]"));

    testing::trigger_mouse_click(0, 0);

    assert!(!on.get_untracked());
    assert!(testing::render_to_text(6, 1).contains("[ ]"));
  });
}

/// A labeled checkbox renders the mark and label as one control on one row, and
/// clicking the label toggles it.
#[test]
fn checkbox_labels_render_and_toggle() {
  with_platform(|| {
    let on = Signal::new(false);

    let _ = terminal::mount(|| {
      Column::new()
        .add_child(
          SimpleCheckbox::new()
            .bind_checked(on)
            .set_label("subscribe"),
        )
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(16, 2);
    let (column, row) = locate(&frame, "subscribe");

    assert_eq!(row, 0, "the label sits beside the mark: {frame}");
    assert!(column >= 1, "after the mark: {frame}");

    testing::trigger_mouse_click(column + 4, row);

    assert!(on.get_untracked(), "the label click toggled the checkbox");
  });
}

/// Radios in a group are exclusive, the arrow keys move the check, and the bound signal
/// follows the chosen value.
#[test]
fn radio_group_moves_the_check() {
  with_platform(|| {
    let choice = Signal::new("a");

    let _ = terminal::mount(|| {
      SimpleRadioGroup::new()
        .bind_chosen(choice)
        .set_child(Column::new().set_children((
          SimpleRadio::new("a").set_label("a"),
          SimpleRadio::new("b").set_label("b"),
          SimpleRadio::new("c").set_label("c"),
        )))
        .into_any()
        .build()
    });
    let frame = testing::render_to_text(8, 3);
    let (_, row_a) = locate(&frame, "a");
    let (_, row_c) = locate(&frame, "c");
    let line = |frame: &str, row: u16| frame.lines().nth(usize::from(row)).unwrap().to_owned();

    assert!(
      line(&frame, row_a).contains("(x)"),
      "the chosen mark: {frame}"
    );
    assert!(line(&frame, row_c).contains("( )"), "a plain mark: {frame}");

    testing::trigger_key("Tab");
    testing::trigger_key("ArrowDown");
    testing::trigger_key("ArrowDown");

    let frame = testing::render_to_text(8, 3);

    assert_eq!(choice.get_untracked(), "c");
    assert!(
      line(&frame, row_c).contains("(x)"),
      "the check moved: {frame}"
    );
    assert!(
      line(&frame, row_a).contains("( )"),
      "the check left: {frame}"
    );
  });
}

/// A press maps through the slider's track, not its whole box: a caption and the
/// spacing before the track inside the box leave the value the press's place on the
/// track asks for.
#[test]
fn a_slider_maps_the_pointer_through_its_track() {
  with_platform(|| {
    let volume = Signal::new(0.0f32);

    let _ = terminal::mount(|| {
      BasicSlider::new(
        Slider::new()
          .set_range(SliderRange::new(0., 10., 1.))
          .bind_value(volume),
      )
      .set_width(12)
      .set_part(terminal::Part::Label, Text::new("vol"))
      .set_part(
        terminal::Part::Track,
        Container::new().set_width(8).set_height(1),
      )
      .into_any()
      .build()
    });
    let _ = testing::render_to_text(16, 1);

    testing::trigger_mouse_click(8, 0);

    assert!(
      (volume.get_untracked() - 6.).abs() < f32::EPSILON,
      "{}",
      volume.get_untracked()
    );
  });
}

/// A slider nudges by its step on arrows and by a tenth of its range on the page keys,
/// jumps to a clicked cell, and reaches its ends by `Home` and `End`.
#[test]
fn slider_nudges_and_jumps() {
  with_platform(|| {
    let volume = Signal::new(0.0f32);

    let _ = terminal::mount(|| {
      BasicSlider::new(
        Slider::new()
          .set_range(SliderRange::new(0., 10., 1.))
          .bind_value(volume),
      )
      .set_width(11)
      .set_height(1)
      .into_any()
      .build()
    });
    let _ = testing::render_to_text(14, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("ArrowRight");
    testing::trigger_key("ArrowRight");

    assert!((volume.get_untracked() - 2.).abs() < f32::EPSILON);

    testing::trigger_mouse_click(10, 0);

    assert!((volume.get_untracked() - 10.).abs() < f32::EPSILON);

    testing::trigger_mouse_click(5, 0);

    assert!((volume.get_untracked() - 5.).abs() < f32::EPSILON);

    testing::trigger_key("Home");

    assert!(volume.get_untracked().abs() < f32::EPSILON);

    testing::trigger_key("PageUp");

    assert!((volume.get_untracked() - 1.).abs() < f32::EPSILON);

    testing::trigger_key("PageDown");

    assert!(volume.get_untracked().abs() < f32::EPSILON);

    testing::trigger_key("End");

    assert!((volume.get_untracked() - 10.).abs() < f32::EPSILON);
  });
}

/// A pressed slider drags: motion slides the value to the pointer's column until
/// release, and motion after release changes nothing.
#[test]
fn slider_drags_to_the_pointer() {
  with_platform(|| {
    let volume = Signal::new(0.0f32);

    let _ = terminal::mount(|| {
      Container::new()
        .set_width(11)
        .set_child(
          BasicSlider::new(
            Slider::new()
              .set_range(SliderRange::new(0., 10., 1.))
              .bind_value(volume),
          )
          .set_height(1),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 1);

    testing::trigger_mouse_press(0, 0);
    testing::trigger_mouse_drag(5, 0);

    assert!((volume.get_untracked() - 5.).abs() < f32::EPSILON);

    testing::trigger_mouse_drag(10, 0);

    assert!((volume.get_untracked() - 10.).abs() < f32::EPSILON);

    testing::trigger_mouse_release(10, 0);
    testing::trigger_mouse_drag(2, 0);

    assert!((volume.get_untracked() - 10.).abs() < f32::EPSILON);
  });
}

/// A slider shows the open hand at rest and the closed hand while dragging.
#[test]
fn slider_shows_the_grab_cursors() {
  with_platform(|| {
    let volume = Signal::new(5.0f32);

    let _ = terminal::mount(|| {
      BasicSlider::new(
        Slider::new()
          .set_range(SliderRange::new(0., 10., 1.))
          .bind_value(volume),
      )
      .set_width(11)
      .set_height(1)
      .into_any()
      .build()
    });
    let _ = testing::render_to_text(12, 1);

    testing::trigger_mouse_move(5, 0);

    assert_eq!(testing::mouse_cursor(), Some(MouseCursor::Grab));

    testing::trigger_mouse_press(5, 0);
    testing::trigger_mouse_drag(7, 0);

    assert_eq!(testing::mouse_cursor(), Some(MouseCursor::Grabbing));

    testing::trigger_mouse_release(7, 0);

    assert_eq!(testing::mouse_cursor(), Some(MouseCursor::Grab));
  });
}

/// A slider in cells: the thumb is one cell's width in its own color, travelling from
/// the track's first cell to its last by eighths, and the fill runs to it. A cell
/// two of them share is a block glyph carrying both colors.
#[test]
fn a_slider_fills_to_a_thumb_of_its_own_color() {
  with_platform(|| {
    let volume = Signal::new(0.0f32);

    let _ = terminal::mount(|| {
      Container::new()
        .set_width(20)
        .set_child(
          SimpleSlider::new()
            .set_range(SliderRange::new(0., 100., 1.))
            .bind_value(volume),
        )
        .into_any()
        .build()
    });

    let (_, thumb) = colors_at(20, 1, 0, 0);
    let (_, track) = colors_at(20, 1, 1, 0);

    assert_eq!(testing::render_to_text(20, 1), "\n", "at zero: no fill");
    assert_ne!(
      thumb, track,
      "the thumb sits on the first cell in its own color"
    );
    assert_eq!(colors_at(20, 1, 19, 0).1, track);

    // 45% of the 19 cells the thumb travels is 8.55: the fill covers eight cells and
    // half of the ninth, where the thumb begins, and the thumb ends half way into the
    // tenth.
    volume.set(45.);

    assert_eq!(testing::render_to_text(20, 1), "        ▌▌\n");

    let (_, filled) = colors_at(20, 1, 7, 0);

    assert_ne!(filled, track);
    assert_ne!(filled, thumb);
    assert_eq!(
      colors_at(20, 1, 8, 0),
      (filled, thumb),
      "the fill's end meets the thumb's start in one cell"
    );
    assert_eq!(
      colors_at(20, 1, 9, 0),
      (thumb, track),
      "the thumb's end meets the track in the next"
    );
    assert_eq!(colors_at(20, 1, 10, 0).1, track);

    // An arrow moves the thumb by a step's share of a cell, not by a cell.
    testing::trigger_key("Tab");
    testing::trigger_key("ArrowRight");

    assert!((volume.get_untracked() - 46.).abs() < f32::EPSILON);
    assert_eq!(testing::render_to_text(20, 1), "        ▊▊\n");

    volume.set(100.);

    assert_eq!(testing::render_to_text(20, 1), "\n", "at the end: no edge");
    assert_eq!(colors_at(20, 1, 18, 0).1, filled);
    assert_eq!(
      colors_at(20, 1, 19, 0).1,
      thumb,
      "the thumb ends on the last cell"
    );
  });
}

/// A slider centered on a half cell paints its fill and thumb from the same numbers:
/// the fill's last cell and the thumb's first are neighbours at every value.
#[test]
fn a_slider_on_a_half_cell_leaves_no_gap_before_its_thumb() {
  with_platform(|| {
    let volume = Signal::new(0.0f32);

    let _ = terminal::mount(|| {
      Column::new()
        .set_item_alignment(ItemAlignment::Center)
        .add_child(
          Container::new().set_width(21).set_child(
            SimpleSlider::new()
              .set_range(SliderRange::new(0., 100., 1.))
              .bind_value(volume),
          ),
        )
        .into_any()
        .build()
    });

    let _ = testing::render_to_text(22, 1);

    let (_, thumb) = colors_at(22, 1, 0, 0);
    let (_, track) = colors_at(22, 1, 2, 0);

    for value in [5., 21., 26., 50., 95.] {
      volume.set(value);

      let colors = testing::render_to_colors(22, 1);
      let thumb_at = colors
        .iter()
        .position(|(_, surface)| *surface == thumb)
        .expect("the thumb paints");
      let fill_end = colors[..thumb_at]
        .iter()
        .rposition(|(_, surface)| *surface != track && *surface != thumb);

      assert_eq!(
        fill_end.map_or(0, |end| end + 1),
        thumb_at,
        "at {value}%: the fill reaches the thumb: {colors:?}"
      );
    }
  });
}

/// A pointer placed within its cell, as a terminal reporting pixels places it, slides
/// a slider to that position rather than to the cell's center.
#[test]
fn a_slider_drag_reads_the_pointer_within_its_cell() {
  with_platform(|| {
    let volume = Signal::new(0.0f32);

    let _ = terminal::mount(|| {
      Container::new()
        .set_width(25)
        .set_child(
          SimpleSlider::new()
            .set_range(SliderRange::new(0., 100., 1.))
            .bind_value(volume),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(25, 1);

    // The thumb's center travels the 24 cells between the first cell's center and the
    // last's, so cell 10 spans 39.6% to 43.75%: its center reads 42, a tenth into it
    // 40 and nine tenths 43.
    testing::trigger_mouse_press(0, 0);
    testing::trigger_mouse_drag(10, 0);

    assert!((volume.get_untracked() - 42.).abs() < f32::EPSILON);

    testing::trigger_mouse_drag_within(10, 0, (0.1, 0.5));

    assert!((volume.get_untracked() - 40.).abs() < f32::EPSILON);

    testing::trigger_mouse_drag_within(10, 0, (0.9, 0.5));

    assert!((volume.get_untracked() - 43.).abs() < f32::EPSILON);
  });
}

/// A press on the first cell of a slider reaches its minimum and one on its last cell
/// its maximum, where the thumb sits at either end, and no caption takes cells from it.
#[test]
fn a_sliders_end_cells_reach_its_ends() {
  with_platform(|| {
    let volume = Signal::new(50.0f32);

    let _ = terminal::mount(|| {
      Container::new()
        .set_width(20)
        .set_child(
          SimpleSlider::new()
            .set_label("Volume")
            .set_range(SliderRange::new(0., 100., 1.))
            .bind_value(volume),
        )
        .into_any()
        .build()
    });

    assert!(!testing::render_to_text(20, 1).contains("Volume"));

    testing::trigger_mouse_click(19, 0);

    assert!((volume.get_untracked() - 100.).abs() < f32::EPSILON);

    testing::trigger_mouse_click(0, 0);

    assert!(volume.get_untracked().abs() < f32::EPSILON);
  });
}

/// A slider's drag ends with the release wherever the pointer is: motion afterwards
/// over the slider changes nothing.
#[test]
fn a_slider_released_off_itself_stops_sliding() {
  with_platform(|| {
    let volume = Signal::new(0.0f32);

    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Container::new().set_width(20).set_child(
            SimpleSlider::new()
              .set_range(SliderRange::new(0., 100., 1.))
              .bind_value(volume),
          ),
          Text::new("below"),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 2);

    testing::trigger_mouse_press(5, 0);
    testing::trigger_mouse_drag(10, 0);

    let dragged = volume.get_untracked();

    assert!(dragged > 0., "the drag slid the value");

    testing::trigger_mouse_drag(10, 1);
    testing::trigger_mouse_release(10, 1);
    testing::trigger_mouse_move(15, 0);

    assert!(
      (volume.get_untracked() - dragged).abs() < f32::EPSILON,
      "a hover after the release slides nothing"
    );
  });
}

/// A control consumes its own press, so a clickable ancestor never fires under it—and
/// the control still does what it does.
#[test]
fn a_control_keeps_its_press_from_an_ancestor() {
  with_platform(|| {
    let outer_runs = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&outer_runs);
    let on = Signal::new(false);

    let _ = terminal::mount(|| {
      Container::new()
        .handle_click(move || counted.set(counted.get() + 1))
        .set_child(SimpleCheckbox::new().bind_checked(on).set_label("c"))
        .into_any()
        .build()
    });
    let frame = testing::render_to_text(6, 1);
    let (column, row) = locate(&frame, "c");

    testing::trigger_mouse_click(column, row);

    assert_eq!(outer_runs.get(), 0, "the press was the checkbox's");
    assert!(on.get_untracked(), "which still toggled");
  });
}

/// An authored state variant paints on a control: a hover color, and a focus color
/// that wins over it while both hold.
#[test]
fn authored_state_variants_paint_on_a_control() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      BasicButton::new(Activate::new())
        .set_width(4)
        .set_height(1)
        .set_background(
          red()
            .when(State::Hovered, blue())
            .when(State::FocusVisible, green()),
        )
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    testing::trigger_mouse_move(1, 0);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));

    testing::trigger_key("Tab");

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(green())));

    testing::trigger_mouse_move(5, 1);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(green())));
  });
}

/// A focus handle moves keyboard focus programmatically, and takes it away.
#[test]
fn a_focus_handle_moves_focus() {
  with_platform(|| {
    let handle = NodeHandle::new();

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new()).set_label("first"),
          BasicButton::new(Activate::new())
            .set_label("second")
            .bind_handle(handle),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 3);

    assert_eq!(testing::focused_node(), None);

    handle.focus();

    assert_eq!(testing::focused_node(), handle.node());

    handle.blur();

    assert_eq!(testing::focused_node(), None);
  });
}

/// A disabled control takes no focus and swallows the clicks it would act on.
#[test]
fn a_disabled_control_takes_no_focus_and_no_click() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("off")
            .set_disabled(true)
            .handle_activate(move || counted.set(counted.get() + 1)),
          BasicButton::new(Activate::new()).set_label("on"),
        ))
        .into_any()
        .build()
    });
    let frame = testing::render_to_text(10, 3);
    let (column, row) = locate(&frame, "off");

    testing::trigger_key("Tab");

    assert_eq!(
      testing::focused_node(),
      testing::focused_node().filter(|_| frame.contains("on")),
    );
    assert_ne!(testing::focused_node(), None);

    testing::trigger_mouse_click(column, row);

    assert_eq!(clicks.get(), 0);
  });
}

/// A button that disables itself while focused lets `Tab` by, which reaches the next
/// button rather than the first.
#[test]
fn tab_leaves_a_control_disabled_while_focused_for_the_next() {
  with_platform(|| {
    let submitting = Signal::new(false);
    let first = NodeHandle::new();
    let next = NodeHandle::new();

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("first")
            .bind_handle(first),
          BasicButton::new(Activate::new())
            .set_label("submit")
            .derive_disabled(move || submitting.get())
            .handle_activate(move || submitting.set(true)),
          BasicButton::new(Activate::new())
            .set_label("next")
            .bind_handle(next),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 3);

    testing::trigger_key("Tab");
    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    assert!(submitting.get());

    let _ = testing::render_to_text(10, 3);

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), next.node());
    assert_ne!(testing::focused_node(), first.node());
  });
}

/// A wrapping container's border frames a checkbox's mark, which paints inside it.
#[test]
fn a_container_frames_a_checkbox_mark() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_border_width(Inset::all(1))
        .set_border_color(white())
        .set_width(5)
        .set_height(3)
        .set_child(SimpleCheckbox::new().set_checked(true))
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(7, 3);

    assert!(frame.contains('┌'), "{frame}");
    assert!(
      frame.contains("│[x]│"),
      "the mark paints inside the frame: {frame}"
    );
  });
}

/// A theme defining the glyph set re-inks the marks beneath it: the checkbox and radio
/// marks and the picker's indicator are its glyphs.
#[test]
fn a_themed_glyph_set_draws_the_marks() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Theme::new(
        SimpleGlyphs::default()
          .set_checkbox("\u{2611}", "\u{2610}")
          .set_radio("\u{25c9}", "\u{25cb}"),
      )
      .set_child_builder(|| {
        Column::new().set_children((
          SimpleCheckbox::new().set_checked(true).set_label("agree"),
          SimpleRadioGroup::new()
            .set_chosen("a")
            .set_child(Column::new().set_children((
              SimpleRadio::new("a").set_label("a"),
              SimpleRadio::new("b").set_label("b"),
            ))),
        ))
      })
      .into_any()
      .build()
    });

    let frame = testing::render_to_text(12, 3);

    assert!(frame.contains("\u{2611} agree"), "{frame}");
    assert!(frame.contains("\u{25c9} a"), "{frame}");
    assert!(frame.contains("\u{25cb} b"), "{frame}");
  });
}

/// Where graphics show, a Simple button's corners round by a fraction of a row rather
/// than by the pixel radius read as rows: its edge runs straight below the corner.
#[test]
fn a_simple_buttons_corners_round_by_a_fraction_of_a_row() {
  with_platform(|| {
    let _ = terminal::mount(|| SimpleButton::new("Save").build());

    testing::trigger_graphics(true);

    let pictures = testing::render_to_pictures(12, 2);
    let backdrop = pictures.first().expect("the button's backdrop");
    let width = usize::from(backdrop.width);
    let alpha = |x: usize, y: usize| backdrop.pixels[(y * width + x) * 4 + 3];

    assert_eq!(alpha(0, 0), 0, "the corner is rounded away");
    assert_eq!(alpha(0, 5), 255, "the edge runs straight below the corner");
  });
}

/// A text field keeps its caption before its box among its siblings, as the web's
/// `<label>` sits before its `<input>`, so the caption paints clear of the value; a
/// press on the caption focuses the field.
#[test]
fn a_text_fields_caption_sits_before_its_box() {
  with_platform(|| {
    let value = Signal::new("hello".to_owned());
    let node = Rc::new(Cell::new(None));
    let built = Rc::clone(&node);

    let _ = terminal::mount(move || {
      let field = BasicTextField::new(TextEntry::new().bind_value(value))
        .set_label("Name")
        .set_width(10)
        .build();

      built.set(Some(field));

      Column::new()
        .add_child(AnyView::node(field))
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(12, 3);
    let lines: Vec<&str> = frame.lines().collect();

    assert!(lines[0].starts_with("Name"), "{frame}");
    assert!(lines[1].starts_with("hello"), "{frame}");

    testing::trigger_mouse_click(1, 0);

    assert_eq!(
      testing::focused_node(),
      node.get(),
      "the caption focuses its field"
    );
  });
}

/// The Simple controls take the cell shape: a button is one row with one-cell insets,
/// a field is framed, and a checkbox's mark is one glyph-sized cell beside its label.
#[test]
fn simple_controls_take_the_cell_shape() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Theme::new(SimplePalette::default())
        .set_child_builder(|| {
          Column::new().set_children((
            SimpleButton::new("Go").into_any(),
            SimpleTextField::new().set_placeholder("name").into_any(),
            SimpleCheckbox::new().set_label("agree").into_any(),
          ))
        })
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(20, 8);
    let (go_column, go_row) = locate(&frame, "Go");
    let (_, name_row) = locate(&frame, "name");
    let (agree_column, agree_row) = locate(&frame, "agree");

    assert_eq!((go_column, go_row), (1, 0), "a one-row button: {frame}");
    assert!(
      colors_at(20, 8, 0, 0).1.is_some(),
      "the inset cell carries the surface"
    );
    assert_eq!(name_row, 2, "a framed field beneath: {frame}");
    assert!(
      frame
        .lines()
        .nth(1)
        .is_some_and(|line| line.starts_with('╭'))
    );
    assert_eq!(
      (agree_column, agree_row),
      (4, 4),
      "a glyph mark, a gap, the label: {frame}"
    );
    assert!(
      frame
        .lines()
        .nth(4)
        .is_some_and(|line| line.starts_with("[ ] agree")),
      "the mark is the unchecked glyph: {frame}"
    );
  });
}

/// A `note` of `name` into `log`, as an action.
fn note(log: &Rc<RefCell<Vec<&'static str>>>, name: &'static str) -> impl Fn() + 'static {
  let log = Rc::clone(log);

  move || log.borrow_mut().push(name)
}

/// Whether a reader finds `node` checked.
fn is_checked(node: NodeId) -> bool {
  read(node).toggled() == Some(Toggled::True)
}

/// The node a `Tab` from `before` focuses: the next tab stop.
fn stop_after(before: NodeHandle) -> Option<NodeId> {
  before.focus();
  testing::trigger_key("Tab");

  testing::focused_node()
}

/// Whether `signal` holds `expected`, to a slider's precision.
fn holds(signal: Signal<f32>, expected: f32) -> bool {
  (signal.get_untracked() - expected).abs() < 1e-4
}

/// A button's click is the button's alone and never reaches the box it sits in, while
/// a plain box's click on the same path still climbs; `Enter` on the button is a click
/// the ancestors never hear, and a key it means nothing to climbs.
#[test]
fn a_button_click_stays_on_the_button_while_a_box_click_climbs() {
  with_platform(|| {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (outer, outer_key, button, plain) = (
      note(&log, "outer"),
      note(&log, "outer key"),
      note(&log, "button"),
      note(&log, "box"),
    );

    let _ = terminal::mount(move || {
      Container::new()
        .handle_click(outer)
        .handle_key_down(move |_| outer_key())
        .set_child(
          Column::new().set_children((
            BasicButton::new(Activate::new())
              .handle_activate(button)
              .set_label("Save"),
            Container::new()
              .handle_click(plain)
              .set_child(Text::new("box")),
          )),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 2);
    let button = testing::find_by_role(Role::Button, "Save").expect("the button");

    assert_eq!(read(button).role(), Role::Button);

    user::click(button);

    assert_eq!(*log.borrow(), ["button"]);

    testing::trigger_key("Enter");

    assert_eq!(*log.borrow(), ["button", "button"]);

    testing::trigger_key("a");

    assert_eq!(*log.borrow(), ["button", "button", "outer key"]);

    user::click(testing::find_by_text("box").expect("the box's text"));

    assert_eq!(
      *log.borrow(),
      ["button", "button", "outer key", "box", "outer"]
    );
  });
}

/// Where the terminal reports releases, `Space` activates a button once, as it is
/// released after a press the button saw: the press and its repeats do nothing, a
/// second release nothing, and a press the button lost focus after never activates it;
/// `Enter` activates on every press, repeats included.
#[test]
fn space_activates_a_button_once_on_its_release() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);
    let go = NodeHandle::new();

    let _ = terminal::mount(move || {
      BasicButton::new(Activate::new())
        .set_label("go")
        .bind_handle(go)
        .handle_activate(move || counted.set(counted.get() + 1))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(6, 1);
    let repeat = |name| KeyEvent {
      repeat: true,
      ..KeyEvent::new(name)
    };

    testing::trigger_key_releases(true);
    go.focus();
    testing::trigger_key(" ");
    testing::trigger_key_event(&repeat(" "));

    assert_eq!(clicks.get(), 0);

    testing::trigger_key_up(" ");

    assert_eq!(clicks.get(), 1);

    testing::trigger_key_up(" ");

    assert_eq!(clicks.get(), 1);

    testing::trigger_key(" ");
    go.blur();
    go.focus();
    testing::trigger_key_up(" ");

    assert_eq!(clicks.get(), 1, "the press was lost with focus");

    testing::trigger_key("Enter");
    testing::trigger_key_event(&repeat("Enter"));

    assert_eq!(clicks.get(), 3);
  });
}

/// `Space`, on its release, and a click each flip the signal a checkbox is bound to;
/// what the checkbox reports follows the signal, and `Enter` means nothing to it.
#[test]
fn space_and_a_click_flip_a_bound_checkbox() {
  with_platform(|| {
    let bound = Signal::new(false);
    let [node] = mount_nodes(|| {
      [BasicCheckbox::new(Toggle::new().bind_checked(bound))
        .set_label("agree")
        .build()]
    });

    assert!(!is_checked(node));

    user::click(node);

    assert!(bound.get_untracked());
    assert!(is_checked(node));

    testing::trigger_key_releases(true);
    testing::trigger_key(" ");

    assert!(bound.get_untracked(), "the press flips nothing");

    testing::trigger_key_up(" ");

    assert!(!bound.get_untracked());

    testing::trigger_key("Enter");

    assert!(!bound.get_untracked());

    bound.set(true);

    assert!(is_checked(node));
  });
}

/// A checkbox is controlled: with a derived state and no handler, a click and `Space`
/// change nothing.
#[test]
fn a_controlled_checkbox_keeps_its_state_without_a_handler() {
  with_platform(|| {
    let bound = Signal::new(false);
    let [node] = mount_nodes(|| {
      [
        BasicCheckbox::new(Toggle::new().derive_checked(move || bound.get()))
          .set_label("agree")
          .build(),
      ]
    });

    user::click(node);
    testing::trigger_key(" ");

    assert!(!bound.get_untracked());
    assert!(!is_checked(node));
  });
}

/// A press maps to the value whose thumb centers under it: the thumb's center travels
/// the extent less the thumb, so a press on the thumb at either end holds that end.
#[test]
fn a_press_centers_the_thumb_under_the_pointer() {
  with_platform(|| {
    let bound = Signal::new(50.0f32);

    let _ = terminal::mount(|| {
      BasicSlider::new(Slider::new().set_thumb_extent(4).bind_value(bound))
        .set_width(20)
        .set_height(1)
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 1);

    // The thumb's center travels the 16 cells from 2 to 18: the center of cell 18
    // passes the end, that of cell 4 is 2.5 cells along, and that of cell 1 falls
    // before the start.
    for (cell, value) in [(18, 100.), (4, 16.), (1, 0.)] {
      testing::trigger_mouse_click(cell, 0);

      assert!(holds(bound, value), "{cell}: {}", bound.get_untracked());
    }
  });
}

/// The arrows, page keys, `Home` and `End` move a slider's bound value by the range's
/// rules, what it reports follows, `Tab` leaves it, a press a quarter of the way along
/// its width sets a quarter of the range, and a press of another button than the
/// primary slides nothing.
#[test]
fn slider_keys_and_a_press_move_the_bound_value_by_the_ranges_rules() {
  with_platform(|| {
    let bound = Signal::new(50.0f32);
    let next = NodeHandle::new();
    let [node, _] = mount_nodes(|| {
      [
        BasicSlider::new(
          Slider::new()
            .set_range(SliderRange::new(0., 100., 5.))
            .bind_value(bound),
        )
        .set_width(21)
        .set_height(1)
        .build(),
        BasicButton::new(Activate::new())
          .set_label("next")
          .bind_handle(next)
          .build(),
      ]
    });
    let reported = || {
      let access = read(node);

      (
        access.numeric_value(),
        access.min_numeric_value(),
        access.max_numeric_value(),
        access.numeric_value_step(),
        access.orientation(),
      )
    };

    assert_eq!(
      reported(),
      (
        Some(50.),
        Some(0.),
        Some(100.),
        Some(5.),
        Some(Orientation::Horizontal)
      )
    );

    testing::trigger_key("Tab");

    for (key, value) in [
      ("ArrowRight", 55.),
      ("ArrowLeft", 50.),
      ("PageUp", 60.),
      ("ArrowUp", 65.),
      ("PageDown", 55.),
      ("Home", 0.),
      ("ArrowDown", 0.),
      ("End", 100.),
    ] {
      testing::trigger_key(key);

      assert!(holds(bound, value), "{key}: {}", bound.get_untracked());
    }

    assert_eq!(reported().0, Some(100.));

    testing::trigger_key("Tab");

    assert_eq!(
      testing::focused_node(),
      next.node(),
      "Tab leaves the slider"
    );

    testing::trigger_mouse_click(5, 0);

    assert!(holds(bound, 25.), "{}", bound.get_untracked());

    testing::trigger_mouse_right_click(15, 0);

    assert!(holds(bound, 25.), "{}", bound.get_untracked());
  });
}

/// A radio group with nothing chosen has its first member as the one tab stop,
/// choosing moves the stop and the checked state, the arrows step with wrap and focus,
/// and a member joining ahead of the others re-evaluates the stop.
#[test]
fn a_radio_groups_tab_stop_follows_the_choice_and_the_membership() {
  with_platform(|| {
    let chosen: Signal<Option<&str>> = Signal::new(None);
    let items = Signal::new(vec!["a", "b"]);
    let (before, after) = (NodeHandle::new(), NodeHandle::new());

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("before")
            .bind_handle(before),
          BasicRadioGroup::new()
            .bind_chosen(chosen)
            .set_child(Column::new().derive_children(
              move || items.get(),
              |item| *item,
              |item| BasicRadio::new(Radio::new(Some(item))).set_label(item),
            )),
          BasicButton::new(Activate::new())
            .set_label("after")
            .bind_handle(after),
        ))
        .into_any()
        .build()
    });
    let radio = |name: &str| {
      let _ = testing::render_to_text(10, 6);

      testing::find_by_role(Role::RadioButton, name).expect("the radio")
    };

    assert_eq!(stop_after(before), Some(radio("a")));

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), after.node(), "b is no stop");

    user::click(radio("b"));

    assert_eq!(chosen.get_untracked(), Some("b"));
    assert!(is_checked(radio("b")));
    assert!(!is_checked(radio("a")));
    assert_eq!(stop_after(before), Some(radio("b")));

    testing::trigger_key("ArrowRight");

    assert_eq!(chosen.get_untracked(), Some("a"));
    assert_eq!(testing::focused_node(), Some(radio("a")));

    testing::trigger_key("ArrowUp");

    assert_eq!(chosen.get_untracked(), Some("b"));
    assert_eq!(testing::focused_node(), Some(radio("b")));

    chosen.set(None);

    assert_eq!(stop_after(before), Some(radio("a")));

    items.set(vec!["z", "a", "b"]);

    assert_eq!(testing::accessible_name(radio("z")).as_deref(), Some("z"));
    assert_eq!(stop_after(before), Some(radio("z")));

    testing::trigger_key("ArrowLeft");

    assert_eq!(chosen.get_untracked(), Some("b"));
    assert_eq!(testing::focused_node(), Some(radio("b")));

    before.focus();
    testing::trigger_key("Tab");
    testing::trigger_key("Tab");

    assert_eq!(
      testing::focused_node(),
      after.node(),
      "one stop for the group"
    );
  });
}

/// A radio group's disabled members are passed over: the tab stop is the first enabled
/// member while nothing is chosen, the arrows skip the disabled ones, and a member
/// disabled later gives up the stop.
#[test]
fn a_radio_group_passes_over_its_disabled_members() {
  with_platform(|| {
    let chosen: Signal<Option<&str>> = Signal::new(None);
    let second_disabled = Signal::new(false);
    let before = NodeHandle::new();

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label("before")
            .bind_handle(before),
          BasicRadioGroup::new().bind_chosen(chosen).set_child(
            Column::new().set_children((
              BasicRadio::new(Radio::new(Some("a")))
                .set_label("a")
                .set_disabled(true),
              BasicRadio::new(Radio::new(Some("b")))
                .set_label("b")
                .derive_disabled(move || second_disabled.get()),
              BasicRadio::new(Radio::new(Some("c"))).set_label("c"),
            )),
          ),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 5);
    let [_, b, c] = ["a", "b", "c"]
      .map(|name| testing::find_by_role(Role::RadioButton, name).expect("the radio"));

    assert_eq!(stop_after(before), Some(b));

    testing::trigger_key("ArrowDown");

    assert_eq!(chosen.get_untracked(), Some("c"));

    testing::trigger_key("ArrowDown");

    assert_eq!(chosen.get_untracked(), Some("b"));

    second_disabled.set(true);

    assert_eq!(stop_after(before), Some(c));
  });
}

/// A listbox's arrows skip its disabled options, and `Tab` from an option leaves the
/// list without choosing it.
#[test]
fn a_listbox_skips_disabled_options_and_tab_chooses_nothing() {
  with_platform(|| {
    let chosen: Signal<Option<&str>> = Signal::new(None);
    let after = NodeHandle::new();

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          BasicListbox::new().bind_chosen(chosen).set_child(
            Column::new().set_children((
              BasicListboxOption::new(ListboxOption::new("a")).set_label("a"),
              BasicListboxOption::new(ListboxOption::new("b"))
                .set_label("b")
                .set_disabled(true),
              BasicListboxOption::new(ListboxOption::new("c")).set_label("c"),
            )),
          ),
          BasicButton::new(Activate::new())
            .set_label("after")
            .bind_handle(after),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 5);
    let [a, c] =
      ["a", "c"].map(|name| testing::find_by_role(Role::ListBoxOption, name).expect("an option"));

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), Some(a));

    testing::trigger_key("ArrowDown");

    assert_eq!(
      testing::focused_node(),
      Some(c),
      "the disabled option is skipped"
    );

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), after.node());
    assert_eq!(chosen.get_untracked(), None);
  });
}

/// A part's focus variant resolves when the control is focused, though focus is the
/// control's and not the part's, and its checked variant follows the control's state.
#[test]
fn a_parts_focus_and_checked_variants_follow_the_control() {
  with_platform(|| {
    let bound = Signal::new(false);

    let _ = terminal::mount(|| {
      BasicCheckbox::new(Toggle::new().bind_checked(bound))
        .set_part(
          Part::Face,
          Container::new()
            .set_width(1)
            .set_height(1)
            .set_background(red().when(State::FocusVisible, blue())),
        )
        .set_part(
          Part::Mark,
          Container::new()
            .set_width(1)
            .set_height(1)
            .set_background(red().when(State::Checked, green())),
        )
        .into_any()
        .build()
    });
    let surfaces = || {
      let colors = testing::render_to_colors(4, 1);

      (colors[0].1, colors[2].1)
    };

    assert_eq!(surfaces(), (Some(rgb(red())), Some(rgb(red()))));

    testing::trigger_key("Tab");

    assert_ne!(testing::focused_node(), None);
    assert_eq!(surfaces(), (Some(rgb(blue())), Some(rgb(red()))));

    testing::trigger_key(" ");

    assert!(bound.get_untracked());
    assert_eq!(surfaces(), (Some(rgb(blue())), Some(rgb(green()))));
  });
}

/// A disabled control reports it, takes no focus and ignores activation, a click on it
/// climbing no further and a key letting by where it holds focus, the activatable row
/// around it never activated by either; enabling it again restores all three.
#[test]
fn a_disabled_control_reports_it_and_ignores_activation() {
  with_platform(|| {
    let disabled = Signal::new(true);
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);
    let heard = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&heard);
    let box_log = Rc::clone(&heard);
    let other = NodeHandle::new();
    let row = NodeHandle::new();
    let mut nodes = None;

    let _ = terminal::mount(|| {
      let node = BasicButton::new(Activate::new())
        .set_label("go")
        .derive_disabled(move || disabled.get())
        .handle_activate(move || counted.set(counted.get() + 1))
        .build();
      let fixed = BasicButton::new(Activate::new())
        .set_label("fixed")
        .set_disabled(true)
        .build();

      nodes = Some((node, fixed));

      Container::new()
        .bind_handle(row)
        .handle_click(move || box_log.borrow_mut().push("click".to_owned()))
        .handle_key_down(move |event| log.borrow_mut().push(event.key.to_string()))
        .set_child(
          Column::new().set_children((
            AnyView::node(node),
            AnyView::node(fixed),
            BasicButton::new(Activate::new())
              .set_label("other")
              .bind_handle(other)
              .into_any(),
          )),
        )
        .build()
    });
    let (node, fixed) = nodes.expect("the buttons built");
    let _ = testing::render_to_text(10, 3);

    assert!(read(node).is_disabled());
    assert!(read(fixed).is_disabled());

    testing::trigger_key("Tab");

    assert_eq!(
      testing::focused_node(),
      row.node(),
      "the row is the first stop"
    );

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), other.node(), "no disabled stop");

    heard.borrow_mut().clear();
    user::click(node);

    assert_eq!(clicks.get(), 0);
    assert!(heard.borrow().is_empty(), "the click climbs no further");

    disabled.set(false);

    let _ = testing::render_to_text(10, 3);

    assert!(!read(node).is_disabled());

    user::click(node);

    assert_eq!(clicks.get(), 1);
    assert_eq!(testing::focused_node(), Some(node));

    disabled.set(true);
    heard.borrow_mut().clear();
    testing::trigger_key("Enter");

    assert_eq!(clicks.get(), 1);
    assert_eq!(*heard.borrow(), ["Enter"], "the key lets by");
  });
}

/// A control given its body takes its setters still: a caption and a handler set after
/// the body name the control and hear its activation, the body laid out before the
/// trailing caption.
#[test]
fn a_control_given_its_body_takes_its_setters() {
  with_platform(|| {
    let log = Rc::new(RefCell::new(Vec::new()));
    let save = note(&log, "save");
    let [button] = mount_nodes(move || {
      let button = BasicButton::new(Activate::new()).set_child(
        Container::new()
          .set_width(4)
          .set_height(1)
          .set_background(red()),
      );

      [button.set_label("Save").handle_activate(save).build()]
    });
    let frame = testing::render_to_text(40, 12);
    let (column, row) = locate(&frame, "Save");

    assert_eq!(row, 0);
    assert!(column >= 4, "the body comes first: {frame}");
    assert_eq!(colors_at(40, 12, 0, 0).1, Some(rgb(red())));
    assert_eq!(testing::accessible_name(button).as_deref(), Some("Save"));

    user::click(button);

    assert_eq!(*log.borrow(), ["save"]);
  });
}

/// A popup face opening a list is a combobox, and one opening a menu or a dialog a
/// button, each reporting the popup it opens.
#[test]
fn a_popup_face_is_what_its_popup_makes_it() {
  with_platform(|| {
    let kinds = [PopupKind::Listbox, PopupKind::Menu, PopupKind::Dialog];
    let faces = mount_nodes(|| {
      kinds.map(|kind| {
        Control::new(Popup::new().set_kind(kind))
          .set_width(4)
          .set_height(1)
          .build()
      })
    });

    for (kind, face) in kinds.into_iter().zip(faces) {
      let (role, popup) = match kind {
        PopupKind::Listbox => (Role::ComboBox, HasPopup::Listbox),
        PopupKind::Menu => (Role::Button, HasPopup::Menu),
        PopupKind::Dialog => (Role::Button, HasPopup::Dialog),
      };

      assert_eq!(read(face).role(), role, "{kind:?}");
      assert_eq!(read(face).has_popup(), Some(popup), "{kind:?}");
    }
  });
}

/// A control's own observers hear the events they observe beside the ones its behavior
/// acts on: a button heard focusing and the pointer moving over it.
#[test]
fn a_controls_observers_hear_the_events_they_observe() {
  with_platform(|| {
    let log = Rc::new(RefCell::new(Vec::new()));
    let (focused, moved) = (note(&log, "focus"), note(&log, "move"));
    let [button] = mount_nodes(move || {
      [BasicButton::new(Activate::new())
        .set_label("go")
        .handle_focus(focused)
        .handle_pointer_move(move |_| moved())
        .build()]
    });

    testing::trigger_key("Tab");

    assert_eq!(*log.borrow(), ["focus"]);

    user::hover(button);

    assert_eq!(*log.borrow(), ["focus", "move"]);
  });
}

/// A stepper: a third party's behavior, built through the public trait alone and the
/// facade's `behavior` module.
struct Stepper {
  count: Signal<i32>,
  order: FocusOrder,
}

impl Behavior for Stepper {
  fn native(&self) -> Option<NativeElement> {
    Some(NativeElement::Button)
  }

  fn role(&self) -> Option<Role> {
    Some(Role::Button)
  }

  fn listens(&self) -> &'static [EventKind] {
    &[EventKind::KeyDown, EventKind::Wheel]
  }

  fn on_key(&self, event: &KeyEvent, _: &ControlHandle) -> Prevent {
    match event.key {
      Key::Named(NamedKey::ArrowUp) => self.count.set(self.count.get() + 1),
      Key::Named(NamedKey::ArrowDown) => self.count.set(self.count.get() - 1),
      _ => return Prevent::None,
    }

    Prevent::All
  }

  fn on_wheel(&self, event: &WheelEvent, _: &ControlHandle) -> Prevent {
    let step = if event.delta.vertical < 0. { 1 } else { -1 };

    self.count.set(self.count.get() + step);

    Prevent::All
  }

  #[expect(
    clippy::cast_precision_loss,
    reason = "a test's count stays far below the integers f32 holds exactly"
  )]
  fn access(&self) -> Vec<AccessProp> {
    vec![AccessProp::Value(self.count.get() as f32)]
  }

  fn focus_order(&self) -> Option<FocusOrder> {
    Some(self.order)
  }
}

/// A third-party behavior gets focus, decoration, parts, observers and reactive reports
/// from the same builder as the shipped ones: its own focus order is honored, one that
/// skips passed over and one reached in the scene's child order, its keys and the wheel
/// step it, and its focus variant paints.
#[test]
fn a_third_party_behavior_builds_through_public_types_alone() {
  with_platform(|| {
    let count = Signal::new(0);
    let seen = Rc::new(Cell::new(0u32));
    let observed = Rc::clone(&seen);
    let [first, _, node] = mount_nodes(move || {
      [
        BasicButton::new(Activate::new()).set_label("first").build(),
        Control::new(Stepper {
          count: Signal::new(0),
          order: FocusOrder::Skip,
        })
        .set_width(4)
        .set_height(1)
        .set_accessible_name("Skipped")
        .build(),
        Control::new(Stepper {
          count,
          order: FocusOrder::Auto,
        })
        .set_width(4)
        .set_height(1)
        .set_accessible_name("Count")
        .set_background(red().when(State::FocusVisible, blue()))
        .set_part(Part::Face, Text::new("0"))
        .handle_key_down(move |_| observed.set(observed.get() + 1))
        .build(),
      ]
    });
    let frame = testing::render_to_text(40, 12);
    let (column, row) = locate(&frame, "0");

    assert_eq!(row, 2, "the face part draws in the control: {frame}");

    testing::trigger_key("Tab");

    assert_eq!(
      testing::focused_node(),
      Some(first),
      "the button before it comes first"
    );

    testing::trigger_key("Tab");

    assert_eq!(
      testing::focused_node(),
      Some(node),
      "the stepper that skips is passed over, and the child order goes on"
    );
    assert_eq!(
      colors_at(40, 12, usize::from(column), usize::from(row)).1,
      Some(rgb(blue()))
    );

    testing::trigger_key("ArrowUp");
    testing::trigger_key("ArrowUp");
    testing::trigger_key("x");

    assert_eq!(count.get_untracked(), 2);
    assert_eq!(seen.get(), 3);
    assert_eq!(read(node).numeric_value(), Some(2.));

    testing::trigger_mouse_scroll(column, row, 0, -3);

    assert_eq!(count.get_untracked(), 3, "the stepper takes the turn");
  });
}

/// A control's required, read-only and invalid flags report as accessibility, the
/// invalid flag reaches the control-scoped state, and a described-by names its node.
#[test]
fn a_controls_flags_report_as_accessibility_and_state() {
  with_platform(|| {
    let invalid = Signal::new(false);
    let [hint, field] = mount_nodes(|| {
      let hint = Text::new("Required").build();
      let field = BasicTextField::new(TextEntry::new())
        .set_width(6)
        .set_required(true)
        .set_read_only(true)
        .derive_invalid(move || invalid.get())
        .set_described_by(hint)
        .set_background(red().when(State::Invalid, blue()))
        .build();

      [hint, field]
    });
    let surface = || colors_at(40, 12, 0, 1).1;

    assert!(read(field).is_required());
    assert!(read(field).is_read_only());
    assert_eq!(read(field).described_by(), [hint.access_id()]);
    assert_eq!(read(field).invalid(), None);
    assert_eq!(surface(), Some(rgb(red())));

    invalid.set(true);

    assert_eq!(read(field).invalid(), Some(Invalid::True));
    assert_eq!(surface(), Some(rgb(blue())));
  });
}

/// A vertical range reports its orientation and reads the pointer along its height,
/// bottom to top.
#[test]
fn a_vertical_range_reads_the_pointer_bottom_to_top() {
  with_platform(|| {
    let bound = Signal::new(50.0f32);
    let [node] = mount_nodes(|| {
      [
        BasicSlider::new(Slider::new().set_axis(Axis::Vertical).bind_value(bound))
          .set_width(1)
          .set_height(10)
          .build(),
      ]
    });

    assert_eq!(read(node).orientation(), Some(Orientation::Vertical));

    testing::trigger_mouse_click(0, 1);

    assert!(
      holds(bound, 85.),
      "near the top reads high: {}",
      bound.get_untracked()
    );
  });
}

/// The built-in button draws its label centered on the control's fill, takes the
/// accent where prominent and re-inks in place as a prominence it derives changes, and
/// a setter its user calls replaces the default it overrides.
#[test]
fn a_button_draws_its_defaults_and_its_prominence_re_inks_it_in_place() {
  with_platform(|| {
    let prominent = Signal::new(false);
    let [plain, primary, overridden] = mount_nodes(|| {
      [
        terminal::Button::new("Save").build(),
        terminal::Button::new("Send")
          .derive_prominent(move || prominent.get())
          .build(),
        terminal::Button::new("Stop")
          .set_background(red())
          .set_text_color(green())
          .build(),
      ]
    });
    let frame = testing::render_to_text(10, 3);
    let column = usize::from(locate(&frame, "Save").0);

    assert_eq!(
      rows(&frame),
      "   Save\n   Send\n   Stop\n",
      "centered: {frame}"
    );
    assert_eq!(
      testing::find_by_role(Role::Button, "Save"),
      Some(plain),
      "the label names the button"
    );

    let fill = |row: usize| colors_at(10, 3, 0, row).1;
    let ink = |row: usize| colors_at(10, 3, column, row).0;
    let control = fill(0);

    assert!(
      control.is_some(),
      "the control's fill paints the whole button"
    );
    assert_eq!(
      colors_at(10, 3, column, 0).1,
      control,
      "beneath its label too"
    );
    assert_eq!(fill(1), control, "not prominent yet");

    prominent.set(true);

    assert_ne!(fill(1), control, "prominent: the accent");
    assert!(is_live(primary), "re-inked in place");

    prominent.set(false);

    assert_eq!(fill(1), control);
    assert_eq!(fill(2), Some(rgb(red())), "a fill its user sets wins");
    assert_eq!(ink(2), Some(rgb(green())), "and an ink");
    assert!(is_live(overridden));
  });
}

/// The built-in button's focus shows in the accent, and a disabled one keeps the
/// control's fill under the muted ink and takes no focus.
#[test]
fn a_buttons_focus_shows_in_the_accent_and_a_disabled_one_is_muted() {
  with_platform(|| {
    let [_, _] = mount_nodes(|| {
      [
        terminal::Button::new("Save").build(),
        terminal::Button::new("Save").set_disabled(true).build(),
      ]
    });
    let column = usize::from(locate(&testing::render_to_text(10, 2), "Save").0);
    let fill = |row: usize| colors_at(10, 2, 0, row).1;
    let ink = |row: usize| colors_at(10, 2, column, row).0;
    let (rest, rest_ink) = (fill(0), ink(0));

    assert_eq!(fill(1), rest, "disabled: the control's fill");
    assert_ne!(ink(1), rest_ink, "under the muted ink");

    testing::trigger_key("Tab");

    assert_ne!(fill(0), rest, "focus shows in the accent");

    testing::trigger_key("Tab");

    assert_ne!(fill(0), rest, "the disabled button took no focus");
    assert_eq!(fill(1), rest);
  });
}

/// The built-in checkbox draws its mark as glyphs before its caption, flips by `Space`
/// and by a click on its caption, and a mark its user sets replaces the default.
#[test]
fn a_checkbox_draws_its_mark_and_a_mark_of_its_users_replaces_it() {
  with_platform(|| {
    let on = Signal::new(false);
    let other = Signal::new(true);
    let [checkbox, _] = mount_nodes(|| {
      [
        terminal::Checkbox::new()
          .set_label("Agree")
          .bind_checked(on)
          .build(),
        terminal::Checkbox::new()
          .set_label("Mine")
          .set_part(Part::Mark, Text::new("<>"))
          .bind_checked(other)
          .build(),
      ]
    });
    let frame = testing::render_to_text(12, 2);

    assert_eq!(rows(&frame), "[ ] Agree\n<> Mine\n", "{frame}");
    assert_eq!(read(checkbox).role(), Role::CheckBox);
    assert!(
      Scene::<Terminal>::with(|scene| scene.find_part(checkbox, Part::Mark)).is_some(),
      "the default mark is the checkbox's mark"
    );

    testing::trigger_key("Tab");
    testing::trigger_key(" ");

    assert!(on.get_untracked());
    assert!(testing::render_to_text(12, 2).starts_with("[x] Agree"));
    assert!(is_checked(checkbox));

    testing::trigger_mouse_click(5, 0);

    assert!(!on.get_untracked(), "a click on the caption flips it");
    assert!(testing::render_to_text(12, 2).starts_with("[ ] Agree"));
  });
}

/// The built-in switch reports itself a switch, draws a track of three cells with its
/// thumb at the start, which moves to the end and fills the track with the accent as it
/// turns on.
#[test]
fn a_switch_is_reported_as_one_and_its_thumb_moves_as_it_turns_on() {
  with_platform(|| {
    let on = Signal::new(false);
    let [switch] = mount_nodes(|| {
      [terminal::Toggle::new()
        .set_label("Wi-Fi")
        .bind_checked(on)
        .build()]
    });
    let frame = testing::render_to_text(12, 1);

    assert_eq!(rows(&frame), "\u{25cf}   Wi-Fi\n", "{frame}");
    assert_eq!(read(switch).role(), Role::Switch);
    assert!(!is_checked(switch));

    let (track, thumb) = Scene::<Terminal>::with(|scene| {
      (
        scene.find_part(switch, Part::Track),
        scene.find_part(switch, Part::Thumb),
      )
    });
    let track = track.expect("the default track is the switch's track");
    let thumb = thumb.expect("its thumb is the switch's thumb");

    assert!(
      Scene::<Terminal>::with(|scene| {
        std::iter::successors(Some(thumb), |node| scene.parent(*node)).any(|node| node == track)
      }),
      "the track holds the thumb"
    );

    let track = colors_at(12, 1, 1, 0).1;

    user::click(switch);

    assert!(on.get_untracked());
    assert!(is_checked(switch));
    assert_eq!(rows(&testing::render_to_text(12, 1)), "  \u{25cf} Wi-Fi\n");
    assert_ne!(
      colors_at(12, 1, 1, 0).1,
      track,
      "the track takes the accent"
    );
  });
}

/// The built-in radio group lays its radios out in a column beneath its caption, each
/// radio's mark a glyph before its caption, the chosen one filled; the arrows choose,
/// and a mark its user sets replaces the default.
#[test]
fn a_radio_group_lays_its_radios_in_a_column_and_marks_the_chosen_one() {
  with_platform(|| {
    let size = Signal::new(1);
    let [group] = mount_nodes(|| {
      [terminal::RadioGroup::new()
        .set_label("Size")
        .bind_chosen(size)
        .set_children((
          terminal::Radio::new(1).set_label("Small"),
          terminal::Radio::new(2).set_label("Medium"),
          terminal::Radio::new(3)
            .set_label("Mine")
            .set_part(Part::Mark, Text::new("<>")),
        ))
        .build()]
    });
    let frame = testing::render_to_text(12, 5);

    assert_eq!(
      rows(&frame),
      "Size\n\n\u{25c9} Small\n\u{25cb} Medium\n<> Mine\n",
      "{frame}"
    );
    assert_eq!(read(group).role(), Role::RadioGroup);

    let small = testing::find_by_role(Role::RadioButton, "Small").expect("a radio");

    assert!(
      Scene::<Terminal>::with(|scene| scene.find_part(small, Part::Mark)).is_some(),
      "the default mark is the radio's mark"
    );
    assert!(is_checked(small));

    testing::trigger_key("Tab");
    testing::trigger_key("ArrowDown");

    assert_eq!(size.get_untracked(), 2, "the arrow chooses the next");
    assert_eq!(
      rows(&testing::render_to_text(12, 5)),
      "Size\n\n\u{25cb} Small\n\u{25c9} Medium\n<> Mine\n"
    );

    let medium = testing::find_by_role(Role::RadioButton, "Medium").expect("a radio");

    assert!(is_checked(medium));
    assert_ne!(
      colors_at(12, 5, 0, 3).0,
      colors_at(12, 5, 0, 2).0,
      "the focused radio's mark takes the accent"
    );
  });
}

/// A body its user sets takes the place of the radio group's default column.
#[test]
fn a_radio_groups_body_of_its_users_replaces_its_column() {
  with_platform(|| {
    let size = Signal::new(1);
    let _ = mount_nodes(|| {
      [terminal::RadioGroup::new()
        .bind_chosen(size)
        .set_child(Row::new().set_spacing(2).set_children((
          terminal::Radio::new(1).set_label("S"),
          terminal::Radio::new(2).set_label("M"),
        )))
        .build()]
    });

    assert_eq!(
      rows(&testing::render_to_text(12, 1)),
      "\u{25c9} S  \u{25cb} M\n"
    );
  });
}

/// The built-in slider draws its track in cells, filled in the accent to its thumb, a
/// cell of its own color travelling the track as the value moves, by a fraction of a
/// cell; the keys move it, and its track holds its fill and its thumb as its parts.
#[test]
fn a_slider_draws_its_track_fill_and_thumb_in_cells() {
  with_platform(|| {
    let volume = Signal::new(0.0f32);
    let [slider] = mount_nodes(|| {
      [terminal::Slider::new()
        .set_label("Vol")
        .set_range(SliderRange::new(0., 100., 1.))
        .bind_value(volume)
        .build()]
    });

    assert_eq!(read(slider).role(), Role::Slider);

    let (track, fill, thumb) = Scene::<Terminal>::with(|scene| {
      (
        scene.find_part(slider, Part::Track),
        scene.find_part(slider, Part::Fill),
        scene.find_part(slider, Part::Thumb),
      )
    });
    let track = track.expect("the default track is the slider's track");

    for (part, name) in [(fill, "fill"), (thumb, "thumb")] {
      let part = part.unwrap_or_else(|| panic!("its {name} is the slider's {name}"));

      assert!(
        Scene::<Terminal>::with(|scene| {
          std::iter::successors(Some(part), |node| scene.parent(*node)).any(|node| node == track)
        }),
        "the track holds the {name}"
      );
    }

    // Twenty cells, the caption's three and a cell between them leaving sixteen to the
    // track.
    assert_eq!(
      rows(&testing::render_to_text(24, 1)),
      "                 Vol\n"
    );

    let surface = |column: usize| colors_at(24, 1, column, 0).1;
    let (knob, rail) = (surface(0), surface(1));

    assert_ne!(
      knob, rail,
      "the thumb sits on the first cell in its own color"
    );
    assert_eq!(surface(15), rail);

    // Half of the fifteen cells the thumb travels is seven and a half: the fill covers
    // seven cells and half of the eighth, where the thumb begins.
    volume.set(50.);

    assert_eq!(
      rows(&testing::render_to_text(24, 1)),
      "       \u{258c}\u{258c}        Vol\n"
    );

    let accent = surface(0);

    assert_ne!(accent, rail);
    assert_ne!(accent, knob);
    assert_eq!(surface(6), accent, "the fill runs to the thumb");
    assert_eq!(surface(10), rail);

    testing::trigger_key("Tab");
    testing::trigger_key("End");

    assert!(holds(volume, 100.));
    assert_eq!(surface(14), accent);
    assert_ne!(
      surface(15),
      accent,
      "the thumb ends on the track's last cell"
    );
    assert_ne!(surface(15), rail, "the thumb, not the bare rail");
    assert_ne!(surface(15), knob, "and shows focus");
    assert!(is_live(slider));
  });
}

/// A width its user sets on the slider gives its track the rest, and a track its user
/// sets replaces the default with its fill and its thumb.
#[test]
fn a_sliders_width_and_track_of_its_users_replace_the_defaults() {
  with_platform(|| {
    let volume = Signal::new(100.0f32);
    let [_, replaced] = mount_nodes(|| {
      [
        terminal::Slider::new()
          .set_width(10)
          .bind_value(volume)
          .build(),
        terminal::Slider::new()
          .set_part(Part::Track, Text::new("----"))
          .bind_value(volume)
          .build(),
      ]
    });
    let surface = |column: usize, row: usize| colors_at(24, 2, column, row).1;
    let accent = surface(0, 0);

    assert_eq!(surface(8, 0), accent, "the fill spans the narrower track");
    assert_ne!(surface(9, 0), accent, "to the thumb on its last cell");
    assert_eq!(surface(10, 0), surface(23, 0), "and nothing past it");
    assert_eq!(rows(&testing::render_to_text(24, 2)), "\n----\n");
    assert!(
      Scene::<Terminal>::with(|scene| scene.find_part(replaced, Part::Thumb)).is_none(),
      "a track its user sets brings no default thumb"
    );
  });
}
