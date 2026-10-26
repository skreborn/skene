//! Links in cells: a link run is a tab stop, `Enter` clicks and opens it, `Space` does
//! not, a click on its cell focuses it, and focus scrolls a link's paragraph into view;
//! a derived link opens where its signal says now; a link group inks its spans in the link ink; a
//! route link navigates the router as `Enter` or a click follows it.

use std::{cell::Cell, rc::Rc};

use skene_terminal::{behavior::Activate, platform::terminal::mount, prelude::*, testing};

use crate::common::{colors_at, locate, red, rgb, white, with_platform};

/// Whether every cell of `needle`'s first appearance in the frame paints in reverse
/// video.
fn reversed(width: u16, height: u16, needle: &str) -> bool {
  let frame = testing::render_to_text(width, height);
  let (x, y) = locate(&frame, needle);
  let reverse = testing::render_to_reverse(width, height);
  let start = usize::from(y) * usize::from(width) + usize::from(x);

  reverse[start..start + needle.chars().count()]
    .iter()
    .all(|cell| *cell)
}

/// `Tab` reaches a text link and a linked span in document order among the controls,
/// each painted in reverse video while its focus shows; `Enter` opens the focused one,
/// `Space` does not, and `Shift+Tab` walks back.
#[test]
fn tab_reaches_links_and_enter_opens_them() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = mount(|| {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new()).set_label("first"),
          Text::new("guide").set_link("https://a.example"),
          RichText::new().add_spans([
            Span::new("see the "),
            Span::new("docs").set_link("https://b.example"),
            Span::new(" here"),
          ]),
          BasicButton::new(Activate::new())
            .set_label("last")
            .handle_activate(move || counted.set(counted.get() + 1)),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 6);

    testing::trigger_key("Tab");

    assert!(!reversed(20, 6, "guide"), "the first button holds focus");

    testing::trigger_key("Tab");

    assert!(reversed(20, 6, "guide"), "the text link holds focus");
    assert!(!reversed(20, 6, "docs"));

    testing::trigger_key(" ");

    assert_eq!(testing::take_opened(), None, "space is not a link's key");

    testing::trigger_key("Enter");

    assert_eq!(testing::take_opened().as_deref(), Some("https://a.example"));

    testing::trigger_key("Tab");

    assert!(reversed(20, 6, "docs"), "the linked span holds focus");
    assert!(!reversed(20, 6, "guide"));
    assert!(!reversed(20, 6, "see the "), "its neighbors do not");

    testing::trigger_key("Enter");

    assert_eq!(testing::take_opened().as_deref(), Some("https://b.example"));

    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    assert_eq!(clicks.get(), 1, "the last button follows the links");
    assert_eq!(testing::take_opened(), None);

    testing::trigger_key("Shift+Tab");

    assert!(reversed(20, 6, "docs"));
  });
}

/// A typography's focused-link colors paint the link whose focus shows, in place of the
/// reverse video its transparent defaults paint, and a text link and a linked span take
/// them alike.
#[test]
fn a_typography_colors_the_focused_link() {
  let highlight = Color::srgb(0.2, 0.35, 0.53);

  with_platform(move || {
    let _ = mount(move || {
      Theme::new(
        Typography::default()
          .set_link_focus_ink(white())
          .set_link_focus_surface(highlight),
      )
      .set_child_builder(|| {
        Column::new().set_children((
          Text::new("guide").set_link("https://a.example"),
          RichText::new().add_spans([
            Span::new("see the "),
            Span::new("docs").set_link("https://b.example"),
          ]),
        ))
      })
      .into_any()
      .build()
    });
    let frame = testing::render_to_text(20, 4);
    let (guide_x, guide_y) = locate(&frame, "guide");
    let (docs_x, docs_y) = locate(&frame, "docs");
    let at = |x: u16, y: u16| usize::from(y) * 20 + usize::from(x);

    testing::trigger_key("Tab");

    let colors = testing::render_to_colors(20, 4);

    assert_eq!(
      colors[at(guide_x, guide_y)],
      (Some(rgb(white())), Some(rgb(highlight))),
      "the focused text link paints in the typography's colors"
    );
    assert!(!reversed(20, 4, "guide"), "and not in reverse video");
    assert_eq!(colors[at(docs_x, docs_y)].1, None);

    testing::trigger_key("Tab");

    let colors = testing::render_to_colors(20, 4);

    assert_eq!(
      colors[at(docs_x, docs_y)],
      (Some(rgb(white())), Some(rgb(highlight))),
      "the focused linked span paints in them too"
    );
    assert_eq!(colors[at(guide_x, guide_y)].1, None);
  });
}

/// A link group that sets no color paints the spans beneath it in the link ink, as a
/// link's own text paints, and the span beside it keeps the ink.
#[test]
fn a_link_groups_spans_take_the_link_ink() {
  with_platform(|| {
    let _ = mount(|| {
      Theme::new(Typography::default().set_ink(white()).set_link_ink(red()))
        .set_child_builder(|| {
          RichText::new().add_spans([
            Span::group([Span::new("x"), Span::new("y")]).set_link("/g"),
            Span::new("z"),
          ])
        })
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(4, 1), "xyz\n");
    assert_eq!(colors_at(4, 1, 0, 0).0, Some(rgb(red())));
    assert_eq!(colors_at(4, 1, 1, 0).0, Some(rgb(red())));
    assert_eq!(colors_at(4, 1, 2, 0).0, Some(rgb(white())));
  });
}

/// A derived link opens at the destination its signal holds now, by a click on its
/// cell and by `Enter` alike.
#[test]
fn a_derived_link_opens_where_its_signal_says() {
  with_platform(|| {
    let destination = Signal::new("https://a.example".to_owned());

    let _ = mount(move || {
      RichText::new()
        .add_spans([
          Span::new("see the "),
          Span::new("docs").derive_link(move || destination.get()),
        ])
        .into_any()
        .build()
    });
    let frame = testing::render_to_text(20, 2);
    let (x, y) = locate(&frame, "docs");

    testing::trigger_mouse_click(x, y);

    assert_eq!(testing::take_opened().as_deref(), Some("https://a.example"));

    destination.set("https://b.example".to_owned());

    let _ = testing::render_to_text(20, 2);

    testing::trigger_mouse_click(x, y);

    assert_eq!(testing::take_opened().as_deref(), Some("https://b.example"));

    testing::trigger_key("Enter");

    assert_eq!(testing::take_opened().as_deref(), Some("https://b.example"));
  });
}

/// A click on a link's cell focuses its run without showing the focus, and opens it;
/// `Enter` shows the focus and is the link's click, reaching the container's handler
/// as a pointer's does.
#[test]
fn click_focuses_a_link_and_enter_is_its_click() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = mount(|| {
      Container::new()
        .handle_click(move || counted.set(counted.get() + 1))
        .set_child(RichText::new().add_spans([
          Span::new("see the "),
          Span::new("docs").set_link("https://b.example"),
        ]))
        .into_any()
        .build()
    });
    let frame = testing::render_to_text(20, 2);
    let (x, y) = locate(&frame, "docs");

    testing::trigger_mouse_click(x, y);

    assert_eq!(testing::take_opened().as_deref(), Some("https://b.example"));
    assert_eq!(clicks.get(), 1);
    assert!(
      !reversed(20, 2, "docs"),
      "the click focused the link without showing it"
    );

    testing::trigger_key("Enter");

    assert_eq!(testing::take_opened().as_deref(), Some("https://b.example"));
    assert_eq!(clicks.get(), 2);
    assert!(reversed(20, 2, "docs"), "the key showed the focused link");

    testing::trigger_mouse_click(0, 0);
    testing::trigger_key("Enter");

    assert!(!reversed(20, 2, "docs"), "a press elsewhere blurred it");
    assert_eq!(testing::take_opened(), None, "so Enter opens nothing");
  });
}

/// A click reaches a linked span where its cells are drawn, a right-to-left span's
/// reordered with the text around it, and a click on a cell of another span opens
/// nothing.
#[test]
fn a_click_finds_a_right_to_left_link_where_it_is_drawn() {
  with_platform(|| {
    let _ = mount(|| {
      RichText::new()
        .add_spans([
          Span::new("x "),
          Span::new("\u{5d0}\u{5d1}").set_link("https://b.example"),
          Span::new(" \u{5d2}"),
        ])
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(10, 1), "x \u{5d2} \u{5d1}\u{5d0}\n");

    testing::trigger_mouse_click(2, 0);

    assert_eq!(testing::take_opened(), None, "\u{5d2} is not linked");

    for column in [4, 5] {
      testing::trigger_mouse_click(column, 0);

      assert_eq!(
        testing::take_opened().as_deref(),
        Some("https://b.example"),
        "the link's cell {column}"
      );
    }
  });
}

/// Focus moving to a linked span below a scroll container's fold scrolls its paragraph
/// into view.
#[test]
fn focus_reveals_a_linked_paragraph() {
  with_platform(|| {
    let _ = mount(|| {
      Container::new()
        .set_width(10)
        .set_height(2)
        .set_overflow(Overflow::Scroll)
        .set_child(Column::new().set_children((
          Text::new("r0"),
          Text::new("r1"),
          Text::new("r2"),
          RichText::new().add_spans([
            Span::new("go "),
            Span::new("docs").set_link("https://b.example"),
          ]),
        )))
        .into_any()
        .build()
    });

    assert!(!testing::render_to_text(10, 2).contains("docs"));

    testing::trigger_key("Tab");

    assert!(testing::render_to_text(10, 2).contains("docs"));
    assert!(reversed(10, 2, "docs"));
  });
}

/// A page's routes, for the route link.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Page {
  Home,
  About,
}

impl skene_terminal::router::Route for Page {
  fn parse(path: &str) -> Option<Self> {
    match path {
      "/" => Some(Self::Home),
      "/about" => Some(Self::About),
      _ => None,
    }
  }

  fn to_path(&self) -> String {
    match self {
      Self::Home => "/".to_owned(),
      Self::About => "/about".to_owned(),
    }
  }
}

/// A route link is a tab stop that `Enter` and a click follow, navigating the router
/// and opening nothing, while `Space` does not.
#[test]
fn a_route_link_follows_on_enter_and_a_click() {
  with_platform(|| {
    let router = Rc::new(Cell::new(None));
    let installed = Rc::clone(&router);

    let _ = mount(move || {
      installed.set(Some(install_router(Page::Home)));

      Column::new()
        .add_child(Link::new(Page::About).set_child(Text::new("about")))
        .build()
    });
    let router = router.get().expect("the mount installed the router");
    let frame = testing::render_to_text(20, 2);

    testing::trigger_key("Tab");
    testing::trigger_key(" ");

    assert_eq!(router.route(), Page::Home, "Space does not follow a link");

    testing::trigger_key("Enter");

    assert_eq!(router.route(), Page::About);
    assert_eq!(
      testing::take_opened(),
      None,
      "the router follows it, not the terminal"
    );

    router.navigate(Page::Home);

    let (x, y) = locate(&frame, "about");

    testing::trigger_mouse_click(x, y);

    assert_eq!(router.route(), Page::About);
  });
}
