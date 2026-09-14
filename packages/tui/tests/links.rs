//! Links in cells: a link run is a tab stop, `Enter` clicks and opens it, `Space` does
//! not, a click on its cell focuses it, and focus scrolls a link's paragraph into view.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use std::{cell::Cell, rc::Rc};

use skene_tui::{self as tui, Activate, prelude::*};

use crate::common::{locate, with_driver};

/// Whether every cell of `needle`'s first appearance in the frame paints in reverse
/// video.
fn reversed(width: u16, height: u16, needle: &str) -> bool {
  let frame = tui::render_to_text(width, height);
  let (x, y) = locate(&frame, needle);
  let reverse = tui::render_to_reverse(width, height);
  let start = usize::from(y) * usize::from(width) + usize::from(x);

  reverse[start..start + needle.chars().count()]
    .iter()
    .all(|cell| *cell)
}

/// `Tab` reaches a text link and a linked span in document order among the controls,
/// each painted in reverse video while focused; `Enter` opens the focused one, `Space`
/// does not, and `Shift+Tab` walks back.
#[test]
fn tab_reaches_links_and_enter_opens_them() {
  with_driver(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          RawButton::new(Activate::new()).set_label("first"),
          Text::new("guide").set_link("https://a.example"),
          RichText::new().set_spans([
            Span::new("see the "),
            Span::new("docs").set_link("https://b.example"),
            Span::new(" here"),
          ]),
          RawButton::new(Activate::new())
            .set_label("last")
            .handle_click(move || counted.set(counted.get() + 1)),
        ))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 6);

    tui::trigger_key("Tab");

    assert!(!reversed(20, 6, "guide"), "the first button holds focus");

    tui::trigger_key("Tab");

    assert!(reversed(20, 6, "guide"), "the text link holds focus");
    assert!(!reversed(20, 6, "docs"));

    tui::trigger_key(" ");

    assert_eq!(tui::take_opened(), None, "space is not a link's key");

    tui::trigger_key("Enter");

    assert_eq!(tui::take_opened().as_deref(), Some("https://a.example"));

    tui::trigger_key("Tab");

    assert!(reversed(20, 6, "docs"), "the linked span holds focus");
    assert!(!reversed(20, 6, "guide"));
    assert!(!reversed(20, 6, "see the "), "its neighbors do not");

    tui::trigger_key("Enter");

    assert_eq!(tui::take_opened().as_deref(), Some("https://b.example"));

    tui::trigger_key("Tab");
    tui::trigger_key("Enter");

    assert_eq!(clicks.get(), 1, "the last button follows the links");
    assert_eq!(tui::take_opened(), None);

    tui::trigger_key("Shift+Tab");

    assert!(reversed(20, 6, "docs"));
  });
}

/// A click on a link's cell focuses its run and opens it, so `Enter` opens it again;
/// `Enter` is the link's click, reaching the container's handler as a pointer's does.
#[test]
fn click_focuses_a_link_and_enter_is_its_click() {
  with_driver(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = tui::mount(|| {
      Container::new()
        .handle_click(move || counted.set(counted.get() + 1))
        .set_child(RichText::new().set_spans([
          Span::new("see the "),
          Span::new("docs").set_link("https://b.example"),
        ]))
        .into_any()
        .build()
    });
    let frame = tui::render_to_text(20, 2);
    let (x, y) = locate(&frame, "docs");

    tui::trigger_mouse_click(x, y);

    assert_eq!(tui::take_opened().as_deref(), Some("https://b.example"));
    assert_eq!(clicks.get(), 1);
    assert!(reversed(20, 2, "docs"), "the click focused the link");

    tui::trigger_key("Enter");

    assert_eq!(tui::take_opened().as_deref(), Some("https://b.example"));
    assert_eq!(clicks.get(), 2);

    tui::trigger_mouse_click(0, 0);

    assert!(!reversed(20, 2, "docs"), "a press elsewhere blurs it");
    assert_eq!(tui::take_opened(), None);
  });
}

/// Focus moving to a linked span below a scroll container's fold scrolls its paragraph
/// into view.
#[test]
fn focus_reveals_a_linked_paragraph() {
  with_driver(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(10)
        .set_height(2)
        .set_overflow(Overflow::Scroll)
        .set_child(Column::new().set_children((
          Text::new("r0"),
          Text::new("r1"),
          Text::new("r2"),
          RichText::new().set_spans([
            Span::new("go "),
            Span::new("docs").set_link("https://b.example"),
          ]),
        )))
        .into_any()
        .build()
    });

    assert!(!tui::render_to_text(10, 2).contains("docs"));

    tui::trigger_key("Tab");

    assert!(tui::render_to_text(10, 2).contains("docs"));
    assert!(reversed(10, 2, "docs"));
  });
}
