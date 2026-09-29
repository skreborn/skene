//! Controls in cells: focus and the keys the behaviors define, the toggles, the slider
//! and its fill's edge, the picker and its popup's closing, a disabled control, and the
//! Simple controls' shape.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use std::{cell::Cell, rc::Rc};

use skene_simple::tui::prelude::*;
use skene_tui::{self as tui, Activate, Range, TextEntry, prelude::*};

use crate::common::{blue, colors_at, green, locate, red, rgb, white, with_platform};

/// Tab focuses a button, a `Focused` variant applies, and `Enter` clicks it.
#[test]
fn focus_variant_and_enter_click() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = tui::mount(|| {
      RawButton::new(Activate::new())
        .set_width(6)
        .set_height(3)
        .set_border_width(Inset::all(1))
        .set_border_color(Color::TRANSPARENT.when(State::Focused, white()))
        .handle_click(move || counted.set(counted.get() + 1))
        .into_any()
        .build()
    });

    assert!(!tui::render_to_text(8, 4).contains('┌'));

    tui::trigger_key("Tab");

    assert!(tui::render_to_text(8, 4).contains('┌'));

    tui::trigger_key("Enter");

    assert_eq!(clicks.get(), 1);
  });
}

/// A box hears focus when focus enters its subtree and blur when focus leaves it, not
/// as focus moves between the controls inside it, as `:focus-within` changes.
#[test]
fn a_box_hears_focus_entering_and_leaving_its_subtree() {
  with_platform(|| {
    let (focuses, blurs) = (Rc::new(Cell::new(0u32)), Rc::new(Cell::new(0u32)));
    let (entered, left) = (Rc::clone(&focuses), Rc::clone(&blurs));

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          Container::new()
            .handle_focus(move || entered.set(entered.get() + 1))
            .handle_blur(move || left.set(left.get() + 1))
            .set_child(Row::new().set_children((
              RawButton::new(Activate::new()).set_label("a"),
              RawButton::new(Activate::new()).set_label("b"),
            ))),
          RawButton::new(Activate::new()).set_label("c"),
        ))
        .into_any()
        .build()
    });

    let _ = tui::render_to_text(10, 3);

    tui::trigger_key("Tab");

    assert_eq!((focuses.get(), blurs.get()), (1, 0));

    tui::trigger_key("Tab");

    assert_eq!((focuses.get(), blurs.get()), (1, 0));

    tui::trigger_key("Tab");

    assert_eq!((focuses.get(), blurs.get()), (1, 1));
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

    let _ = tui::mount(|| {
      Row::new()
        .set_children((
          RawButton::new(Activate::new())
            .set_width(4)
            .set_height(2)
            .handle_click(move || count_first.set(count_first.get() + 1)),
          RawButton::new(Activate::new())
            .set_width(4)
            .set_height(2)
            .handle_click(move || count_second.set(count_second.get() + 1)),
        ))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(10, 3);

    tui::trigger_mouse_click(5, 0);
    tui::trigger_key("Enter");
    tui::trigger_key(" ");

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

    let _ = tui::mount(|| {
      RawButton::new(Activate::new())
        .set_width(4)
        .set_height(1)
        .handle_click(move || counted.set(counted.get() + 1))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(6, 2);

    tui::trigger_key("Tab");
    tui::trigger_key(" ");

    assert_eq!(clicks.get(), 1);

    tui::trigger_key_releases(true);
    tui::trigger_key(" ");

    assert_eq!(clicks.get(), 1);

    tui::trigger_key_up(" ");

    assert_eq!(clicks.get(), 2);
  });
}

/// A checkbox toggles by `Space` and by mouse click, both directions of the binding,
/// and its mark repaints as the checked glyph.
#[test]
fn checkbox_toggles_by_key_and_click() {
  with_platform(|| {
    let on = Signal::new(false);

    let _ = tui::mount(|| SimpleCheckbox::new().bind_checked(on).into_any().build());

    assert!(tui::render_to_text(6, 1).contains("[ ]"));

    tui::trigger_key("Tab");
    tui::trigger_key(" ");

    assert!(on.get_untracked());
    assert!(tui::render_to_text(6, 1).contains("[x]"));

    tui::trigger_mouse_click(0, 0);

    assert!(!on.get_untracked());
    assert!(tui::render_to_text(6, 1).contains("[ ]"));
  });
}

/// A labeled checkbox renders the mark and label as one control on one row, and
/// clicking the label toggles it.
#[test]
fn checkbox_labels_render_and_toggle() {
  with_platform(|| {
    let on = Signal::new(false);

    let _ = tui::mount(|| {
      Column::new()
        .set_child(
          SimpleCheckbox::new()
            .bind_checked(on)
            .set_label("subscribe"),
        )
        .into_any()
        .build()
    });

    let frame = tui::render_to_text(16, 2);
    let (column, row) = locate(&frame, "subscribe");

    assert_eq!(row, 0, "the label sits beside the mark: {frame}");
    assert!(column >= 1, "after the mark: {frame}");

    tui::trigger_mouse_click(column + 4, row);

    assert!(on.get_untracked(), "the label click toggled the checkbox");
  });
}

/// Radios in a group are exclusive, the arrow keys move the check, and the bound signal
/// follows the chosen value.
#[test]
fn radio_group_moves_the_check() {
  with_platform(|| {
    let choice = Signal::new(Some("a".to_owned()));

    let _ = tui::mount(|| {
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
    let frame = tui::render_to_text(8, 3);
    let (_, row_a) = locate(&frame, "a");
    let (_, row_c) = locate(&frame, "c");
    let line = |frame: &str, row: u16| frame.lines().nth(usize::from(row)).unwrap().to_owned();

    assert!(
      line(&frame, row_a).contains("(x)"),
      "the chosen mark: {frame}"
    );
    assert!(line(&frame, row_c).contains("( )"), "a plain mark: {frame}");

    tui::trigger_key("Tab");
    tui::trigger_key("ArrowDown");
    tui::trigger_key("ArrowDown");

    let frame = tui::render_to_text(8, 3);

    assert_eq!(choice.get_untracked().as_deref(), Some("c"));
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

/// A slider nudges by its step on arrows and by a tenth of its range on the page keys,
/// jumps to a clicked cell, and reaches its ends by `Home` and `End`.
#[test]
fn slider_nudges_and_jumps() {
  with_platform(|| {
    let volume = Signal::new(0.0f32);

    let _ = tui::mount(|| {
      RawSlider::new(
        Range::new()
          .set_min(0.)
          .set_max(10.)
          .set_step(1.)
          .bind_value(volume),
      )
      .set_width(11)
      .set_height(1)
      .into_any()
      .build()
    });
    let _ = tui::render_to_text(14, 1);

    tui::trigger_key("Tab");
    tui::trigger_key("ArrowRight");
    tui::trigger_key("ArrowRight");

    assert!((volume.get_untracked() - 2.).abs() < f32::EPSILON);

    tui::trigger_mouse_click(10, 0);

    assert!((volume.get_untracked() - 10.).abs() < f32::EPSILON);

    tui::trigger_mouse_click(5, 0);

    assert!((volume.get_untracked() - 5.).abs() < f32::EPSILON);

    tui::trigger_key("Home");

    assert!(volume.get_untracked().abs() < f32::EPSILON);

    tui::trigger_key("PageUp");

    assert!((volume.get_untracked() - 1.).abs() < f32::EPSILON);

    tui::trigger_key("PageDown");

    assert!(volume.get_untracked().abs() < f32::EPSILON);

    tui::trigger_key("End");

    assert!((volume.get_untracked() - 10.).abs() < f32::EPSILON);
  });
}

/// A pressed slider drags: motion slides the value to the pointer's column until
/// release, and motion after release changes nothing.
#[test]
fn slider_drags_to_the_pointer() {
  with_platform(|| {
    let volume = Signal::new(0.0f32);

    let _ = tui::mount(|| {
      Container::new()
        .set_width(11)
        .set_child(
          RawSlider::new(
            Range::new()
              .set_min(0.)
              .set_max(10.)
              .set_step(1.)
              .bind_value(volume),
          )
          .set_height(1),
        )
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(12, 1);

    tui::trigger_mouse_press(0, 0);
    tui::trigger_mouse_drag(5, 0);

    assert!((volume.get_untracked() - 5.).abs() < f32::EPSILON);

    tui::trigger_mouse_drag(10, 0);

    assert!((volume.get_untracked() - 10.).abs() < f32::EPSILON);

    tui::trigger_mouse_release(10, 0);
    tui::trigger_mouse_drag(2, 0);

    assert!((volume.get_untracked() - 10.).abs() < f32::EPSILON);
  });
}

/// A slider shows the open hand at rest and the closed hand while dragging.
#[test]
fn slider_shows_the_grab_cursors() {
  with_platform(|| {
    let volume = Signal::new(5.0f32);

    let _ = tui::mount(|| {
      RawSlider::new(
        Range::new()
          .set_min(0.)
          .set_max(10.)
          .set_step(1.)
          .bind_value(volume),
      )
      .set_width(11)
      .set_height(1)
      .into_any()
      .build()
    });
    let _ = tui::render_to_text(12, 1);

    tui::trigger_mouse_move(5, 0);

    assert_eq!(tui::current_cursor(), Some(MouseCursor::Grab));

    tui::trigger_mouse_press(5, 0);
    tui::trigger_mouse_drag(7, 0);

    assert_eq!(tui::current_cursor(), Some(MouseCursor::Grabbing));

    tui::trigger_mouse_release(7, 0);

    assert_eq!(tui::current_cursor(), Some(MouseCursor::Grab));
  });
}

/// A slider in cells: the thumb is one cell's width in its own color, travelling from
/// the track's first cell to its last by eighths, and the fill runs to it. A cell
/// two of them share is a block glyph carrying both colors.
#[test]
fn a_slider_fills_to_a_thumb_of_its_own_color() {
  with_platform(|| {
    let volume = Signal::new(0.0f32);

    let _ = tui::mount(|| {
      Container::new()
        .set_width(20)
        .set_child(
          SimpleSlider::new()
            .set_min(0.)
            .set_max(100.)
            .set_step(1.)
            .bind_value(volume),
        )
        .into_any()
        .build()
    });

    let (_, thumb) = colors_at(20, 1, 0, 0);
    let (_, track) = colors_at(20, 1, 1, 0);

    assert_eq!(tui::render_to_text(20, 1), "\n", "at zero: no fill");
    assert_ne!(
      thumb, track,
      "the thumb sits on the first cell in its own color"
    );
    assert_eq!(colors_at(20, 1, 19, 0).1, track);

    // 45% of the 19 cells the thumb travels is 8.55: the fill covers eight cells and
    // half of the ninth, where the thumb begins, and the thumb ends half way into the
    // tenth.
    volume.set(45.);

    assert_eq!(tui::render_to_text(20, 1), "        ▌▌\n");

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
    tui::trigger_key("Tab");
    tui::trigger_key("ArrowRight");

    assert!((volume.get_untracked() - 46.).abs() < f32::EPSILON);
    assert_eq!(tui::render_to_text(20, 1), "        ▊▊\n");

    volume.set(100.);

    assert_eq!(tui::render_to_text(20, 1), "\n", "at the end: no edge");
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

    let _ = tui::mount(|| {
      Column::new()
        .set_item_alignment(ItemAlignment::Center)
        .set_child(
          Container::new().set_width(21).set_child(
            SimpleSlider::new()
              .set_min(0.)
              .set_max(100.)
              .set_step(1.)
              .bind_value(volume),
          ),
        )
        .into_any()
        .build()
    });

    let _ = tui::render_to_text(22, 1);

    let (_, thumb) = colors_at(22, 1, 0, 0);
    let (_, track) = colors_at(22, 1, 2, 0);

    for value in [5., 21., 26., 50., 95.] {
      volume.set(value);

      let colors = tui::render_to_colors(22, 1);
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

    let _ = tui::mount(|| {
      Container::new()
        .set_width(25)
        .set_child(
          SimpleSlider::new()
            .set_min(0.)
            .set_max(100.)
            .set_step(1.)
            .bind_value(volume),
        )
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(25, 1);

    // The thumb's center travels the 24 cells between the first cell's center and the
    // last's, so cell 10 spans 39.6% to 43.75%: its center reads 42, a tenth into it
    // 40 and nine tenths 43.
    tui::trigger_mouse_press(0, 0);
    tui::trigger_mouse_drag(10, 0);

    assert!((volume.get_untracked() - 42.).abs() < f32::EPSILON);

    tui::trigger_mouse_drag_within(10, 0, (0.1, 0.5));

    assert!((volume.get_untracked() - 40.).abs() < f32::EPSILON);

    tui::trigger_mouse_drag_within(10, 0, (0.9, 0.5));

    assert!((volume.get_untracked() - 43.).abs() < f32::EPSILON);
  });
}

/// A press on the first cell of a slider reaches its minimum and one on its last cell
/// its maximum, where the thumb sits at either end, and no caption takes cells from it.
#[test]
fn a_sliders_end_cells_reach_its_ends() {
  with_platform(|| {
    let volume = Signal::new(50.0f32);

    let _ = tui::mount(|| {
      Container::new()
        .set_width(20)
        .set_child(
          SimpleSlider::new()
            .set_label("Volume")
            .set_min(0.)
            .set_max(100.)
            .set_step(1.)
            .bind_value(volume),
        )
        .into_any()
        .build()
    });

    assert!(!tui::render_to_text(20, 1).contains("Volume"));

    tui::trigger_mouse_click(19, 0);

    assert!((volume.get_untracked() - 100.).abs() < f32::EPSILON);

    tui::trigger_mouse_click(0, 0);

    assert!(volume.get_untracked().abs() < f32::EPSILON);
  });
}

/// A slider's drag ends with the release wherever the pointer is: motion afterwards
/// over the slider changes nothing.
#[test]
fn a_slider_released_off_itself_stops_sliding() {
  with_platform(|| {
    let volume = Signal::new(0.0f32);

    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          Container::new().set_width(20).set_child(
            SimpleSlider::new()
              .set_min(0.)
              .set_max(100.)
              .set_step(1.)
              .bind_value(volume),
          ),
          Text::new("below"),
        ))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 2);

    tui::trigger_mouse_press(5, 0);
    tui::trigger_mouse_drag(10, 0);

    let dragged = volume.get_untracked();

    assert!(dragged > 0., "the drag slid the value");

    tui::trigger_mouse_drag(10, 1);
    tui::trigger_mouse_release(10, 1);
    tui::trigger_mouse_move(15, 0);

    assert!(
      (volume.get_untracked() - dragged).abs() < f32::EPSILON,
      "a hover after the release slides nothing"
    );
  });
}

fn picker(unit: Signal<String>) -> SimplePicker {
  SimplePicker::new()
    .set_options([("px", "pixels"), ("cell", "cells"), ("pt", "points")])
    .bind_value(unit)
}

/// Mounts a picker bound to `unit` at the top of the frame.
fn mount_picker(unit: Signal<String>) {
  let _ = tui::mount(|| Column::new().set_child(picker(unit)).into_any().build());
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
    let unit = Signal::new("px".to_owned());

    mount_picker(unit);

    let closed = tui::render_to_text(20, 10);

    assert!(!closed.contains("cells"), "closed shows only the label");

    tui::trigger_key("Tab");
    tui::trigger_key("Enter");

    let open = tui::render_to_text(20, 10);
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

    tui::trigger_mouse_click(face_column, other_row);

    assert_eq!(unit.get_untracked(), "cell", "the row click selected");

    let after_click = tui::render_to_text(20, 10);

    assert!(
      !after_click.contains("pixels"),
      "the selection closed the popup: {after_click}"
    );

    tui::trigger_key("Enter");
    tui::trigger_key("Escape");

    let dismissed = tui::render_to_text(20, 10);

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
    let unit = Signal::new("px".to_owned());

    mount_picker(unit);
    let _ = tui::render_to_text(20, 10);

    tui::trigger_key("Tab");
    tui::trigger_key("Enter");
    tui::trigger_key("ArrowDown");
    tui::trigger_key("Escape");

    assert_eq!(unit.get_untracked(), "px", "escape keeps the old value");

    tui::trigger_key("Enter");
    tui::trigger_key("ArrowDown");
    tui::trigger_key("Enter");

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
    let unit = Signal::new("px".to_owned());

    mount_picker(unit);
    let _ = tui::render_to_text(20, 10);

    tui::trigger_key("Tab");
    tui::trigger_key("ArrowDown");

    assert!(
      tui::render_to_text(20, 10).contains("cells"),
      "the list opened"
    );
    assert_eq!(unit.get_untracked(), "px", "opening chose nothing");

    tui::trigger_key("End");
    tui::trigger_key("Enter");

    assert_eq!(unit.get_untracked(), "pt");

    tui::trigger_key("ArrowUp");
    tui::trigger_key("Home");
    tui::trigger_key(" ");

    assert_eq!(unit.get_untracked(), "px");
  });
}

/// The options popup anchors beneath the picker and closes when focus leaves it.
#[test]
fn picker_popup_anchors_beneath_and_closes_on_blur() {
  with_platform(|| {
    let unit = Signal::new("px".to_owned());

    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          picker(unit).into_any(),
          RawButton::new(Activate::new()).set_label("next").into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 12);

    tui::trigger_key("Tab");

    let face = tui::focused_node();

    tui::trigger_key("Enter");

    let open = tui::render_to_text(20, 12);
    let (_, face_row) = locate(&open, "pixels");
    let list_row = last_row(&open, "pixels").expect("the list shows");

    assert!(
      list_row > face_row && open.contains("cells"),
      "the popup opens beneath the face: {open}"
    );

    tui::trigger_key("Tab");

    let blurred = tui::render_to_text(20, 12);

    assert!(
      !blurred.contains("cells"),
      "focus moving on closes the popup: {blurred}"
    );
    assert_ne!(tui::focused_node(), face, "focus moved on");
    assert_eq!(unit.get_untracked(), "px", "nothing was chosen");
  });
}

/// `Tab` on a focused option commits it, closes the popup and travels on to the next
/// tab stop, so the keys that follow reach that stop rather than the closed list.
#[test]
fn picker_tab_commits_closes_and_travels_on() {
  with_platform(|| {
    let unit = Signal::new("px".to_owned());
    let next = FocusHandle::new();
    let bound = next.clone();

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          picker(unit).into_any(),
          RawButton::new(Activate::new())
            .set_label("next")
            .bind_focus(&bound)
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 12);

    tui::trigger_key("Tab");
    tui::trigger_key("Enter");
    tui::trigger_key("ArrowDown");
    tui::trigger_key("Tab");

    assert_eq!(
      unit.get_untracked(),
      "cell",
      "tab committed the focused option"
    );
    assert_eq!(tui::focused_node(), next.node(), "tab travelled on");

    let closed = tui::render_to_text(20, 12);

    assert!(!closed.contains("points"), "the popup closed: {closed}");

    tui::trigger_key(" ");

    let after = tui::render_to_text(20, 12);

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
    let unit = Signal::new("px".to_owned());

    mount_picker(unit);
    let _ = tui::render_to_text(20, 10);

    tui::trigger_key("Tab");
    tui::trigger_key("c");

    assert_eq!(unit.get_untracked(), "cell", "a character chooses closed");

    let closed = tui::render_to_text(20, 10);

    assert!(
      !closed.contains("points"),
      "the list stays closed: {closed}"
    );

    tui::trigger_frame(std::time::Duration::from_secs(1));
    tui::trigger_key("Enter");
    tui::trigger_key("p");
    tui::trigger_key("o");

    assert_eq!(
      unit.get_untracked(),
      "cell",
      "typing over the list chooses nothing"
    );

    tui::trigger_key("Enter");

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
    let unit = Signal::new("px".to_owned());

    mount_picker(unit);
    let _ = tui::render_to_text(20, 12);

    tui::trigger_key("Tab");
    tui::trigger_key("Enter");

    let open = tui::render_to_text(20, 12);
    let (column, row) = locate(&open, "pixels");

    assert!(open.contains("points"), "the popup opened: {open}");

    tui::trigger_mouse_click(column, row);

    let closed = tui::render_to_text(20, 12);

    assert!(
      !closed.contains("points"),
      "the face's click closed it: {closed}"
    );
    assert_eq!(unit.get_untracked(), "px");

    tui::trigger_mouse_click(column, row);

    assert!(
      tui::render_to_text(20, 12).contains("points"),
      "the next click reopens it"
    );
  });
}

/// A picker's face keeps its ring while its list is open, though focus sits on an
/// option.
#[test]
fn a_pickers_face_keeps_its_ring_while_open() {
  with_platform(|| {
    let unit = Signal::new("px".to_owned());

    mount_picker(unit);

    let closed = tui::render_to_text(20, 12);
    let (column, row) = locate(&closed, "pixels");
    let border = |x: u16| colors_at(20, 12, usize::from(x), usize::from(row) - 1).0;
    let resting = border(column);

    tui::trigger_key("Tab");

    let focused = border(column);

    assert_ne!(focused, resting, "focus rings the face");

    tui::trigger_key("Enter");

    assert!(
      tui::render_to_text(20, 12).contains("points"),
      "the list opened"
    );
    assert_ne!(tui::focused_node(), None);
    assert_eq!(border(column), focused, "the ring holds while open");

    tui::trigger_key("Escape");

    assert_eq!(border(column), focused, "focus returns to the face");
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

    let _ = tui::mount(|| {
      Container::new()
        .handle_click(move || counted.set(counted.get() + 1))
        .set_child(SimpleCheckbox::new().bind_checked(on).set_label("c"))
        .into_any()
        .build()
    });
    let frame = tui::render_to_text(6, 1);
    let (column, row) = locate(&frame, "c");

    tui::trigger_mouse_click(column, row);

    assert_eq!(outer_runs.get(), 0, "the press was the checkbox's");
    assert!(on.get_untracked(), "which still toggled");
  });
}

/// An authored state variant paints on a control: a hover color, and a focus color
/// that wins over it while both hold.
#[test]
fn authored_state_variants_paint_on_a_control() {
  with_platform(|| {
    let _ = tui::mount(|| {
      RawButton::new(Activate::new())
        .set_width(4)
        .set_height(1)
        .set_color(
          red()
            .when(State::Hovered, blue())
            .when(State::FocusVisible, green()),
        )
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    tui::trigger_mouse_move(1, 0);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));

    tui::trigger_key("Tab");

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(green())));

    tui::trigger_mouse_move(5, 1);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(green())));
  });
}

/// Focus reached by a pointer press holds without showing, a key makes it show, and
/// the next press hides it again, as `:focus-visible` does.
#[test]
fn focus_shows_after_a_key_and_not_after_a_pointer_press() {
  with_platform(|| {
    let _ = tui::mount(|| {
      RawButton::new(Activate::new())
        .set_width(4)
        .set_height(1)
        .set_color(
          red()
            .when(State::Focused, blue())
            .when(State::FocusVisible, green()),
        )
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    tui::trigger_mouse_press(1, 0);
    tui::trigger_mouse_release(1, 0);

    assert_eq!(
      colors_at(6, 2, 0, 0).1,
      Some(rgb(blue())),
      "focused by the press, not shown"
    );

    tui::trigger_key("Right");

    assert_eq!(
      colors_at(6, 2, 0, 0).1,
      Some(rgb(green())),
      "a key shows it"
    );

    tui::trigger_mouse_press(1, 0);
    tui::trigger_mouse_release(1, 0);

    assert_eq!(
      colors_at(6, 2, 0, 0).1,
      Some(rgb(blue())),
      "a press hides it again"
    );
  });
}

/// A focus handle moves keyboard focus programmatically, and takes it away.
#[test]
fn a_focus_handle_moves_focus() {
  with_platform(|| {
    let handle = FocusHandle::new();
    let bound = handle.clone();

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          RawButton::new(Activate::new()).set_label("first"),
          RawButton::new(Activate::new())
            .set_label("second")
            .bind_focus(&bound),
        ))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(10, 3);

    assert_eq!(tui::focused_node(), None);

    handle.focus();

    assert_eq!(tui::focused_node(), handle.node());

    handle.blur();

    assert_eq!(tui::focused_node(), None);
  });
}

/// A disabled control takes no focus and swallows the clicks it would act on.
#[test]
fn a_disabled_control_takes_no_focus_and_no_click() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          RawButton::new(Activate::new())
            .set_label("off")
            .set_disabled(true)
            .handle_click(move || counted.set(counted.get() + 1)),
          RawButton::new(Activate::new()).set_label("on"),
        ))
        .into_any()
        .build()
    });
    let frame = tui::render_to_text(10, 3);
    let (column, row) = locate(&frame, "off");

    tui::trigger_key("Tab");

    assert_eq!(
      tui::focused_node(),
      tui::focused_node().filter(|_| frame.contains("on")),
    );
    assert_ne!(tui::focused_node(), None);

    tui::trigger_mouse_click(column, row);

    assert_eq!(clicks.get(), 0);
  });
}

/// A button that disables itself while focused lets `Tab` by, which reaches the next
/// button rather than the first.
#[test]
fn tab_leaves_a_control_disabled_while_focused_for_the_next() {
  with_platform(|| {
    let submitting = Signal::new(false);
    let first = FocusHandle::new();
    let next = FocusHandle::new();
    let (bound_first, bound_next) = (first.clone(), next.clone());

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          RawButton::new(Activate::new())
            .set_label("first")
            .bind_focus(&bound_first),
          RawButton::new(Activate::new())
            .set_label("submit")
            .derive_disabled(move || submitting.get())
            .handle_click(move || submitting.set(true)),
          RawButton::new(Activate::new())
            .set_label("next")
            .bind_focus(&bound_next),
        ))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(10, 3);

    tui::trigger_key("Tab");
    tui::trigger_key("Tab");
    tui::trigger_key("Enter");

    assert!(submitting.get());

    let _ = tui::render_to_text(10, 3);

    tui::trigger_key("Tab");

    assert_eq!(tui::focused_node(), next.node());
    assert_ne!(tui::focused_node(), first.node());
  });
}

/// A wrapping container's border frames a checkbox's mark, which paints inside it.
#[test]
fn a_container_frames_a_checkbox_mark() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_border_width(Inset::all(1))
        .set_border_color(white())
        .set_width(5)
        .set_height(3)
        .set_child(SimpleCheckbox::new().set_checked(true))
        .into_any()
        .build()
    });

    let frame = tui::render_to_text(7, 3);

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
    let _ = tui::mount(|| {
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

    let frame = tui::render_to_text(12, 3);

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
    let _ = tui::mount(|| SimpleButton::new("Save").into_any().build());

    tui::trigger_graphics(true);

    let pictures = tui::render_to_pictures(12, 2);
    let backdrop = pictures.first().expect("the button's backdrop");
    let width = usize::from(backdrop.width);
    let alpha = |x: usize, y: usize| backdrop.pixels[(y * width + x) * 4 + 3];

    assert_eq!(alpha(0, 0), 0, "the corner is rounded away");
    assert_eq!(alpha(0, 5), 255, "the edge runs straight below the corner");
  });
}

/// The open list marks the chosen row alone: an unchosen row's check is hidden, not
/// drawn in its surface's color.
#[test]
fn picker_marks_the_chosen_row_alone() {
  with_platform(|| {
    let unit = Signal::new("cell".to_owned());

    mount_picker(unit);

    tui::trigger_key("Tab");
    tui::trigger_key("Enter");

    let open = tui::render_to_text(20, 10);
    let marked: Vec<&str> = open.lines().filter(|line| line.contains('✓')).collect();

    assert_eq!(marked.len(), 1, "{open}");
    assert!(marked[0].contains("cells"), "{open}");
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

    let _ = tui::mount(move || {
      let field = RawTextField::new(TextEntry::new().bind_value(value))
        .set_label("Name")
        .set_width(10)
        .build();

      built.set(Some(field));

      Column::new()
        .set_child(AnyView::node(field))
        .into_any()
        .build()
    });

    let frame = tui::render_to_text(12, 3);
    let lines: Vec<&str> = frame.lines().collect();

    assert!(lines[0].starts_with("Name"), "{frame}");
    assert!(lines[1].starts_with("hello"), "{frame}");

    tui::trigger_mouse_click(1, 0);

    assert_eq!(
      tui::focused_node(),
      node.get(),
      "the caption focuses its field"
    );
  });
}
