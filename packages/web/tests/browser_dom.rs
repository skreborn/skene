//! Real-DOM integration tests for the web realization of the built-in views.
//!
//! Each test installs the web platform rooted at a connected `<div>`, builds a view under
//! it, and asserts against the actual DOM the browser produced. The guard uninstalls
//! the platform and removes the root when it drops, isolating each test.
//!
//! Run in a headless browser:
//! `cargo test-web --test browser_dom`.
#![cfg(all(skene_platform = "web", not(skene_server)))]

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  time::Duration,
};

use skene_foundation::{
  Availability,
  Axes,
  Axis,
  BasicPickerBase,
  ClipboardAction,
  ColumnBase,
  ContainerBase,
  ContextMenuBase,
  ControlBase,
  DecorationBase,
  DragAxes,
  DragData,
  EnhanceBase,
  ErrorBoundaryBase,
  FlexBase,
  Flexibility,
  FlexibleBase,
  GridAreas,
  GridBase,
  GridCellBase,
  GridName,
  GridPlacement,
  GridTemplate,
  GridTrack,
  ItemAlignment,
  KeymapBase,
  Layout,
  LazyColumnBase,
  Line,
  LinkBase,
  MotionBase,
  OneOfBase,
  OverlayBase,
  PaintedBase,
  PinnedBase,
  PlainChild,
  PointerButton,
  PointerId,
  PointerType,
  Realization,
  RichTextBase,
  RowBase,
  RunBase,
  RunWithTypeMetrics,
  SpanBase,
  StackBase,
  SuspenseBase,
  TextBase,
  ThemeBase,
  TooltipBase,
  VisibilityBase,
  WheelEvent,
  WheelUnit,
};
use skene_simple_web::prelude::*;
use skene_web::{
  AnyView,
  AsyncView,
  AsyncViewBase,
  BasicButton,
  BasicCheckbox,
  BasicRadio,
  BasicRadioGroup,
  BasicRadioGroupBase,
  BasicSlider,
  BasicTextEditor,
  BasicTextField,
  Color,
  Column,
  Container,
  ContextMenu,
  Deferred,
  DeferredBase,
  Definition,
  Dimension,
  Enhance,
  ErrorBoundary,
  Flexible,
  Grid,
  GridCell,
  HasChild,
  HasChildren,
  Inset,
  Keymap,
  LayoutBox,
  Link,
  Media,
  MenuEntry,
  Modifiers,
  NamedKey,
  NodeHandle,
  OneOf,
  Overlay,
  Pinned,
  RichText,
  Role,
  Row,
  Shortcut,
  Spacing,
  Span,
  Stack,
  Suspense,
  TOOLTIP_GRACE,
  Text,
  TextDecoration,
  TextDecorationStyle,
  TextSelection,
  Theme,
  Tooltip,
  Typography,
  View,
  Visibility,
  When,
  behavior::{Activate, Radio, Slider, SliderRange, TextEntry, Toggle},
  derived,
  empty,
  head,
  platform::web::{self, Web, mount},
  reactive::{Get, Owner, Set, Signal},
  testing::{computed, dispatch, document, frames_until, fresh_root, next_frame, only_child, wait},
  wasm_bindgen::{JsCast, JsValue},
  window,
};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{
  CompositionEvent,
  CompositionEventInit,
  Element,
  HtmlElement,
  HtmlInputElement,
  HtmlSelectElement,
  HtmlTextAreaElement,
  InputEvent,
  InputEventInit,
  MutationObserver,
  MutationObserverInit,
  MutationRecord,
};

wasm_bindgen_test_configure!(run_in_browser);

/// A color as the CSS the platform renders it to.
fn css(color: Color) -> String {
  let [lightness, chroma, hue, alpha] = color.to_oklch();

  format!("oklch({lightness} {chroma} {hue} / {alpha})")
}

/// The computed color a `<div>` under `parent` gets from `color: <value>`, so colors
/// compare as the engine serializes them.
fn probe(parent: &Element, value: &str) -> String {
  let element = document().create_element("div").expect("a probe");

  let _ = element.set_attribute("style", &format!("color:{value}"));
  let _ = parent.append_child(&element);

  let color = computed(&element, "color");

  element.remove();

  color
}

fn html(element: &Element) -> HtmlElement {
  element.clone().dyn_into().expect("an HTML element")
}

/// The text content of each of `parent`'s element children, in order.
fn child_texts(parent: &Element) -> Vec<String> {
  let children = parent.children();

  (0..children.length())
    .map(|index| children.item(index).unwrap().text_content().unwrap())
    .collect()
}

/// A `Deferred` is a streaming boundary only on the server; on the client it builds
/// its content directly, between anchors of its own.
#[wasm_bindgen_test]
fn deferred_builds_its_content_on_the_client() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Deferred::new()
      .set_child_builder(|| Text::new("content"))
      .build()
  });

  let content = only_child(&root);

  assert_eq!(content.tag_name(), "SPAN");
  assert_eq!(content.text_content().as_deref(), Some("content"));
  assert_eq!(
    root.child_nodes().length(),
    3,
    "the content between its two anchors"
  );
}

#[wasm_bindgen_test]
fn container_is_a_div() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| Container::new().build());

  assert_eq!(only_child(&root).tag_name(), "DIV");
}

#[wasm_bindgen_test]
fn role_realizes_its_semantic_element() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| Container::new().set_role(Role::Navigation).build());

  assert_eq!(only_child(&root).tag_name(), "NAV");
}

#[wasm_bindgen_test]
fn text_is_a_span_with_its_content() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| Text::new("hello").build());

  let element = only_child(&root);

  assert_eq!(element.tag_name(), "SPAN");
  assert_eq!(element.text_content().as_deref(), Some("hello"));
}

/// A run's decoration compiles to the `text-decoration` longhands: the line, its style
/// and its color, each its own property.
#[wasm_bindgen_test]
fn a_decorated_text_computes_its_line_style_and_color() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Text::new("marked")
      .set_decoration(TextDecoration::Underline)
      .set_decoration_style(TextDecorationStyle::Wavy)
      .set_decoration_color(Color::srgb(1., 0., 0.))
      .build()
  });

  let element = only_child(&root);

  assert_eq!(computed(&element, "text-decoration-line"), "underline");
  assert_eq!(computed(&element, "text-decoration-style"), "wavy");
  assert_eq!(
    computed(&element, "text-decoration-color"),
    probe(&root, &css(Color::srgb(1., 0., 0.)))
  );
}

/// A text entry's selection is controlled on the web: the bound signal selects inside
/// the input, cluster indices mapped to UTF-16 units and the direction kept, and the
/// browser's selection change writes the signal back in clusters.
#[wasm_bindgen_test]
async fn a_text_entrys_selection_binds_both_ways() {
  let (_guard, root) = fresh_root();
  let selection = Signal::new(TextSelection::new(1, 2));

  let _ = mount(move || {
    BasicTextField::new(
      TextEntry::new()
        .set_value("a\u{1f44d}b")
        .bind_selection(selection),
    )
    .build()
  });

  let input = root
    .query_selector("input")
    .expect("query")
    .expect("the input")
    .dyn_into::<HtmlInputElement>()
    .expect("an input");
  let range = |input: &HtmlInputElement| {
    (
      input.selection_start().expect("a start"),
      input.selection_end().expect("an end"),
      input.selection_direction().expect("a direction"),
    )
  };

  assert_eq!(
    range(&input),
    (Some(1), Some(3), Some("forward".to_owned()))
  );

  selection.set(TextSelection::new(9, 1));
  web::end_turn();

  assert_eq!(
    range(&input),
    (Some(1), Some(4), Some("backward".to_owned())),
    "an index past the value clamps to its end"
  );

  let _ = input.focus();
  let _ = input.set_selection_range(0, 3);

  next_frame().await;

  assert_eq!(selection.get(), TextSelection::new(0, 2));
}

/// A `keyup` reaches the box's key-up handler through dispatch, and a `keydown`
/// carries the browser's repeat flag.
#[wasm_bindgen_test]
fn a_key_release_and_a_repeat_reach_the_box() {
  let (_guard, root) = fresh_root();
  let released = Signal::new((String::new(), String::new()));
  let repeated = Signal::new(false);

  let _ = mount(move || {
    Container::new()
      .handle_key_down(move |event| repeated.set(event.repeat))
      .handle_key_up(move |event| released.set((event.key.to_string(), event.code.to_string())))
      .set_child(BasicTextField::new(TextEntry::new()))
      .build()
  });

  let input = root
    .query_selector("input")
    .expect("query")
    .expect("the input");
  let key = |kind: &str, key: &str, repeat: bool| {
    let init = web_sys::KeyboardEventInit::new();

    init.set_bubbles(true);
    init.set_key(key);
    init.set_code("KeyQ");
    init.set_repeat(repeat);

    let event =
      web_sys::KeyboardEvent::new_with_keyboard_event_init_dict(kind, &init).expect("a key");

    let _ = input.dispatch_event(&event);
  };

  key("keydown", "a", true);
  key("keyup", "a", false);

  assert!(repeated.get(), "the repeat flag came through");
  assert_eq!(
    released.get(),
    ("a".to_owned(), "KeyQ".to_owned()),
    "the key and its physical code came through"
  );
}

#[wasm_bindgen_test]
fn linked_text_is_an_anchor_element() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| Text::new("go").set_link("https://example.com/").build());

  let element = only_child(&root);

  assert_eq!(element.tag_name(), "A");
  assert_eq!(
    element.get_attribute("href").as_deref(),
    Some("https://example.com/"),
  );
}

/// A derived destination's `href` follows its signal, the link opening as its policy has
/// it for each: a new page for an external one, in place for a relative one.
#[wasm_bindgen_test]
fn a_derived_destination_follows_its_signal() {
  let (_guard, root) = fresh_root();
  let destination = Signal::new("/docs".to_owned());

  let _ = mount(move || {
    Text::new("go")
      .derive_link(move || destination.get())
      .build()
  });

  let element = only_child(&root);

  assert_eq!(element.tag_name(), "A");
  assert_eq!(element.get_attribute("href").as_deref(), Some("/docs"));
  assert_eq!(element.get_attribute("target"), None);

  destination.set("https://example.com/".to_owned());
  web::end_turn();

  assert_eq!(
    element.get_attribute("href").as_deref(),
    Some("https://example.com/")
  );
  assert_eq!(element.get_attribute("target").as_deref(), Some("_blank"));
  assert_eq!(
    element.get_attribute("rel").as_deref(),
    Some("noopener noreferrer")
  );

  destination.set("/guide".to_owned());
  web::end_turn();

  assert_eq!(element.get_attribute("href").as_deref(), Some("/guide"));
  assert_eq!(element.get_attribute("target"), None);
  assert_eq!(element.get_attribute("rel"), None);
}

/// A destination that would run script is refused when it is written after the link
/// exists, as when it is created with one: the anchor stays, with no `href`.
#[wasm_bindgen_test]
fn a_script_destination_written_later_is_refused() {
  let (_guard, root) = fresh_root();
  let destination = Signal::new("/safe".to_owned());

  let _ = mount(move || {
    Text::new("go")
      .derive_link(move || destination.get())
      .build()
  });

  let element = only_child(&root);

  assert_eq!(element.get_attribute("href").as_deref(), Some("/safe"));

  destination.set(" JavaScript:alert(1)".to_owned());
  web::end_turn();

  assert_eq!(element.tag_name(), "A");
  assert_eq!(
    element.get_attribute("href"),
    None,
    "{}",
    element.outer_html()
  );

  destination.set("/again".to_owned());
  web::end_turn();

  assert_eq!(element.get_attribute("href").as_deref(), Some("/again"));
}

/// A box's derived destination writes its `href` as its signal changes, and none while
/// the destination would run script, the anchor staying.
#[wasm_bindgen_test]
fn a_box_s_derived_destination_follows_its_signal() {
  let (_guard, root) = fresh_root();
  let destination = Signal::new("/docs".to_owned());

  let _ = mount(move || {
    LayoutBox::<PlainChild>::new(Layout::Document)
      .derive_destination(move || destination.get())
      .add_child(Text::new("go"))
      .build()
  });

  let element = only_child(&root);

  assert_eq!(element.tag_name(), "A");
  assert_eq!(element.get_attribute("href").as_deref(), Some("/docs"));

  destination.set("javascript:alert(1)".to_owned());
  web::end_turn();

  assert_eq!(element.tag_name(), "A");
  assert_eq!(
    element.get_attribute("href"),
    None,
    "{}",
    element.outer_html()
  );

  destination.set("/guide".to_owned());
  web::end_turn();

  assert_eq!(element.get_attribute("href").as_deref(), Some("/guide"));
}

#[wasm_bindgen_test]
fn children_mount_in_order() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Row::new()
      .set_children((Text::new("a"), Text::new("b"), Text::new("c")))
      .build()
  });

  let container = only_child(&root);

  assert_eq!(child_texts(&container), ["a", "b", "c"]);
}

/// A wrapper or a view of one child without its child mounts no element and takes no
/// room, its fallback included, and a theme without its child no scope: the texts
/// around them are the column's only elements, its spacing apart and no more.
#[wasm_bindgen_test]
fn a_wrapper_without_its_child_mounts_nothing() {
  let (_guard, root) = fresh_root();
  let live = Signal::new(true);

  let _ = mount(move || {
    Column::new()
      .set_spacing(Spacing::from(8))
      .set_children((
        Text::new("a"),
        Flexible::new().set_flexibility(Flexibility::grow(1.)),
        Tooltip::new("about"),
        Visibility::new(),
        Overlay::popup().set_open(true),
        Overlay::modal().set_open(true).set_label("Settings"),
      ))
      .add_child(ContextMenu::new().set_items([MenuEntry::new("Inspect")]))
      .add_child(Keymap::new().handle_shortcut(Shortcut::new(Modifiers::CTRL, "s"), || {}))
      .add_child(Link::new(Page::Home))
      .add_child(Enhance::new(live.read_only()).set_fallback_builder(|| Text::new("fallback")))
      .add_child(Suspense::new().set_fallback_builder(|| Text::new("loading")))
      .add_child(ErrorBoundary::new().set_fallback_builder(|_| Text::new("failed")))
      .add_child(Theme::new(Typography::default()))
      .add_child(Deferred::new())
      .add_child(AsyncView::new())
      .add_child(Text::new("b"))
      .build()
  });

  let column = only_child(&root);

  assert_eq!(child_texts(&column), ["a", "b"]);

  let first = column.first_element_child().expect("the first text");
  let last = column.last_element_child().expect("the last text");
  let between = last.get_bounding_client_rect().top() - first.get_bounding_client_rect().bottom();

  assert!(
    (between - 8.).abs() < 0.5,
    "the texts sit the spacing apart, not {between}px"
  );
}

#[wasm_bindgen_test]
fn styling_interns_a_generated_class() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Container::new()
      .set_background(Color::srgb(1.0, 0.0, 0.0))
      .build()
  });

  let class = only_child(&root)
    .get_attribute("class")
    .expect("styled node has a class");

  assert!(
    class
      .split_whitespace()
      .any(|name| name.starts_with("skene-")),
    "expected a generated `skene-` class, got {class:?}",
  );
}

#[wasm_bindgen_test]
fn interned_class_actually_applies_its_style() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Container::new()
      .set_padding(Inset::all(10))
      .set_border_width(Inset::all(2))
      .set_border_color(Color::srgb(0.0, 0.0, 0.0))
      .build()
  });

  let element = only_child(&root);

  assert_eq!(computed(&element, "padding-top"), "10px");
  assert_eq!(computed(&element, "border-top-width"), "2px");
}

/// A stack's children share its one grid cell, so they overlap.
#[wasm_bindgen_test]
fn stack_children_share_one_grid_cell() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Stack::new()
      .set_children((Text::new("under"), Text::new("over")))
      .build()
  });

  let stack = only_child(&root);
  let children = stack.children();

  assert_eq!(children.length(), 2);

  for index in 0..2 {
    let child = children.item(index).expect("a child");

    assert_eq!(computed(&child, "grid-row-start"), "1");
    assert_eq!(computed(&child, "grid-column-start"), "1");
  }
}

/// A stack's cell reaches a child beneath a theme's scope.
#[wasm_bindgen_test]
fn a_themed_stack_child_shares_the_cell() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Stack::new()
      .set_children((
        Text::new("under"),
        Theme::new(Typography::default()).set_child_builder(|| Text::new("over")),
      ))
      .build()
  });

  let stack = only_child(&root);
  let under = stack.first_element_child().expect("the first child");
  let over = only_child(&stack.last_element_child().expect("the theme's scope"));

  assert_eq!(computed(&over, "grid-row-start"), "1");
  assert_eq!(computed(&over, "grid-column-start"), "1");
  assert!(
    (over.get_bounding_client_rect().top() - under.get_bounding_client_rect().top()).abs() < 0.5
  );
}

/// A modal and a popup open at build open once connected, and the modal prevents a
/// cancel though nothing handles its dismissal.
#[wasm_bindgen_test]
fn overlays_open_at_build_open_and_a_modal_prevents_cancel() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Column::new()
      .set_children((
        Overlay::modal()
          .set_open(true)
          .set_child(Text::new("modal")),
        Overlay::popup()
          .set_open(true)
          .set_child(Text::new("popup")),
      ))
      .build()
  });

  let dialog = root
    .query_selector("dialog")
    .expect("a valid selector")
    .expect("the modal");
  let popup = root
    .query_selector("[popover]")
    .expect("a valid selector")
    .expect("the popup");

  assert!(dialog.has_attribute("open"));
  assert!(popup.matches(":popover-open").expect("a valid selector"));

  let init = web_sys::EventInit::new();

  init.set_cancelable(true);

  let cancel = web_sys::Event::new_with_event_init_dict("cancel", &init).expect("an event");
  let _ = dialog.dispatch_event(&cancel);

  assert!(cancel.default_prevented());
  assert!(dialog.has_attribute("open"));
}

/// An `Escape` nothing prevents dismisses the popup opened last and that one alone,
/// and one a control inside a popup takes dismisses nothing.
#[wasm_bindgen_test]
fn escape_dismisses_the_topmost_popup_unless_taken() {
  use skene_foundation::{EventKind, KeyEvent, NativeElement, Prevent, Role};
  use skene_web::{
    Control,
    behavior::{Behavior, ControlHandle},
  };

  /// Takes `Escape` and nothing else.
  struct Keeper;

  impl Behavior for Keeper {
    fn native(&self) -> Option<NativeElement> {
      Some(NativeElement::Button)
    }

    fn role(&self) -> Option<Role> {
      None
    }

    fn listens(&self) -> &'static [EventKind] {
      &[EventKind::KeyDown]
    }

    fn on_key(&self, event: &KeyEvent, _: &ControlHandle) -> Prevent {
      if event.is(NamedKey::Escape) {
        Prevent::All
      }
      else {
        Prevent::None
      }
    }
  }

  let (_guard, root) = fresh_root();
  let outer = Signal::new(false);
  let inner = Signal::new(false);

  let _ = mount(move || {
    Column::new()
      .set_children((
        Overlay::popup()
          .bind_open(outer)
          .set_child(Text::new("outer")),
        Overlay::popup()
          .bind_open(inner)
          .set_child(Control::new(Keeper).set_child(Text::new("keeper"))),
      ))
      .build()
  });

  outer.set(true);
  web::end_turn();
  inner.set(true);
  web::end_turn();

  let escape = |target: &Element| {
    let init = web_sys::KeyboardEventInit::new();

    init.set_bubbles(true);
    init.set_cancelable(true);
    init.set_key("Escape");

    let event =
      web_sys::KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).expect("a key");

    let _ = target.dispatch_event(&event);
  };
  let keeper = root
    .query_selector("[data-skene-control]")
    .expect("a valid selector")
    .expect("the keeper");

  escape(&keeper);

  assert!(outer.get() && inner.get(), "the keeper took it");

  escape(&root);

  assert!(outer.get(), "only the topmost closes");
  assert!(!inner.get());

  escape(&root);

  assert!(!outer.get());
}

/// `Escape` over an open modal is the dialog's own to cancel: the popup beneath the
/// modal stays open.
#[wasm_bindgen_test]
fn escape_over_a_modal_keeps_the_popup_beneath_it() {
  let (_guard, root) = fresh_root();
  let popup = Signal::new(false);
  let modal = Signal::new(false);

  let _ = mount(move || {
    Column::new()
      .set_children((
        Overlay::popup()
          .bind_open(popup)
          .set_child(Text::new("popup")),
        Overlay::modal()
          .bind_open(modal)
          .set_child(Text::new("modal")),
      ))
      .build()
  });

  popup.set(true);
  web::end_turn();
  modal.set(true);
  web::end_turn();

  let init = web_sys::KeyboardEventInit::new();

  init.set_bubbles(true);
  init.set_cancelable(true);
  init.set_key("Escape");

  let event =
    web_sys::KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).expect("a key");
  let _ = root.dispatch_event(&event);

  assert!(popup.get(), "the popup beneath the modal stays open");
}

/// Focus moving to a node outside an open popup and its anchor dismisses the popup, and
/// focus on the anchor keeps it.
#[wasm_bindgen_test]
fn focus_leaving_a_popup_dismisses_it() {
  let (_guard, root) = fresh_root();
  let open = Signal::new(false);
  let anchor = NodeHandle::new();

  let _ = mount(move || {
    Column::new()
      .set_children((
        Container::new()
          .set_focus_order(skene_web::FocusOrder::Auto)
          .bind_handle(anchor)
          .set_child(Text::new("anchor")),
        Container::new()
          .set_focus_order(skene_web::FocusOrder::Auto)
          .set_child(Text::new("elsewhere")),
        Overlay::popup()
          .set_anchor(anchor)
          .bind_open(open)
          .set_child(Text::new("popup")),
      ))
      .build()
  });

  open.set(true);
  web::end_turn();

  let stops = root
    .query_selector_all("[tabindex]")
    .expect("a valid selector");
  let stop = |index: u32| -> HtmlElement {
    stops
      .item(index)
      .expect("a tab stop")
      .dyn_into()
      .expect("an element")
  };

  stop(0).focus().expect("focus the anchor");

  assert!(open.get(), "focus on the anchor keeps the popup");

  stop(1).focus().expect("focus elsewhere");

  assert!(!open.get(), "focus leaving dismisses the popup");
}

/// A wrapper's derived property reaches a run as a slot, following its signal.
#[wasm_bindgen_test]
fn a_derived_wrapper_property_reaches_a_run() {
  let (_guard, root) = fresh_root();
  let grow = Signal::new(2.0f32);

  let _ = mount(move || {
    Row::new()
      .add_child(
        Flexible::new()
          .derive_flexibility(move || Flexibility::grow(grow.get()))
          .set_child(Text::new("grows")),
      )
      .build()
  });

  let text = only_child(&only_child(&root));

  assert_eq!(computed(&text, "flex-grow"), "2");

  grow.set(5.);
  web::end_turn();

  assert_eq!(computed(&text, "flex-grow"), "5");
}

/// Dispatches the key `key` as a `kind` event, `keydown` or `keyup`, at `target`,
/// repeating where `repeat` says, and returns whether the platform's default was
/// prevented.
fn key_at(target: &Element, kind: &str, key: &str, repeat: bool) -> bool {
  let init = web_sys::KeyboardEventInit::new();

  init.set_bubbles(true);
  init.set_cancelable(true);
  init.set_key(key);
  init.set_repeat(repeat);

  let event =
    web_sys::KeyboardEvent::new_with_keyboard_event_init_dict(kind, &init).expect("a key");
  let _ = target.dispatch_event(&event);

  event.default_prevented()
}

/// A counter and the action adding one to it.
fn counter() -> (Rc<Cell<u32>>, impl Fn() + Clone + 'static) {
  let count = Rc::new(Cell::new(0));
  let counted = Rc::clone(&count);

  (count, move || counted.set(counted.get() + 1))
}

/// A clickable container is an activatable row: `Enter` activates it on every press,
/// repeats included, and `Space` once on its release at the row, its press keeping the
/// page from scrolling; a release elsewhere, or after focus left the row and came back,
/// activates nothing.
#[wasm_bindgen_test]
fn an_activatable_row_activates_on_enter_and_space() {
  let (_guard, root) = fresh_root();
  let (clicks, click) = counter();

  let _ = mount(move || {
    Column::new()
      .set_children((
        Container::new()
          .handle_click(click)
          .set_child(Text::new("Row")),
        Container::new()
          .set_focus_order(skene_web::FocusOrder::Auto)
          .set_child(Text::new("Elsewhere")),
      ))
      .build()
  });

  let column = only_child(&root);
  let row = column.first_element_child().expect("the row");
  let elsewhere = row.next_element_sibling().expect("the other box");

  let _ = html(&row).focus();

  assert!(key_at(&row, "keydown", "Enter", false), "Enter acts");
  assert!(key_at(&row, "keydown", "Enter", true));
  assert_eq!(clicks.get(), 2, "every Enter activates, repeats included");

  assert!(key_at(&row, "keydown", " ", false), "Space scrolls nothing");
  assert!(key_at(&row, "keydown", " ", true));
  assert_eq!(clicks.get(), 2, "a held Space waits for its release");
  assert!(key_at(&row, "keyup", " ", false));
  assert!(
    !key_at(&row, "keyup", " ", false),
    "a release activates once"
  );
  assert_eq!(clicks.get(), 3);

  let _ = key_at(&row, "keydown", " ", false);
  let _ = html(&elsewhere).focus();
  let _ = key_at(&elsewhere, "keyup", " ", false);

  assert_eq!(clicks.get(), 3, "focus left the row before the release");

  let _ = html(&row).focus();
  let _ = key_at(&row, "keydown", " ", false);
  let _ = html(&elsewhere).focus();
  let _ = html(&row).focus();

  assert!(
    !key_at(&row, "keyup", " ", false),
    "focus left and came back before the release"
  );
  assert_eq!(clicks.get(), 3);
  assert!(
    !key_at(&elsewhere, "keydown", "Enter", false),
    "a box handling no click is no row"
  );
}

/// A button inside an activatable row owns its press and its keys: none of its click,
/// its `Enter` or its `Space` reaches the row, while a click on the row's own text
/// does.
#[wasm_bindgen_test]
fn a_button_inside_a_row_owns_its_press_and_its_space() {
  let (_guard, root) = fresh_root();
  let (rows, row_click) = counter();
  let (buttons, button_click) = counter();

  let _ = mount(move || {
    Container::new()
      .handle_click(row_click)
      .set_child(
        Row::new().set_children((
          Text::new("Row"),
          BasicButton::new(Activate::new())
            .set_label("Inside")
            .handle_activate(button_click),
        )),
      )
      .build()
  });

  let row = only_child(&root);
  let button = row
    .query_selector("button")
    .expect("query")
    .expect("the button");

  let _ = html(&button).focus();
  html(&button).click();
  let _ = key_at(&button, "keydown", "Enter", false);
  let _ = key_at(&button, "keydown", " ", false);
  let _ = key_at(&button, "keyup", " ", false);

  assert_eq!(buttons.get(), 3, "the button's press and keys are its own");
  assert_eq!(rows.get(), 0, "none reaches the row");

  html(
    &row
      .query_selector("span")
      .expect("query")
      .expect("the text"),
  )
  .click();

  assert_eq!(
    rows.get(),
    1,
    "a click on the row's own text clicks the row"
  );
}

/// An activatable row is a tab stop, `tabindex="0"`, in the scene's child order, the
/// controls it holds after it; it carries no role of its own and takes the one given.
#[wasm_bindgen_test]
fn an_activatable_row_is_a_tab_stop_in_child_order() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Column::new()
      .set_children((
        BasicButton::new(Activate::new()).set_label("Before"),
        Container::new()
          .handle_click(|| {})
          .set_child(BasicButton::new(Activate::new()).set_label("Inside")),
        Container::new()
          .set_role(Role::Group)
          .set_label("Named")
          .handle_click(|| {})
          .set_child(Text::new("Named")),
        Container::new().set_child(Text::new("Plain")),
      ))
      .build()
  });

  let column = only_child(&root);
  let boxes: Vec<Element> = (0..column.child_element_count())
    .filter_map(|index| column.children().item(index))
    .collect();
  let stops = root
    .query_selector_all("button, [tabindex]")
    .expect("query");
  let order: Vec<String> = (0..stops.length())
    .filter_map(|index| stops.item(index))
    .filter_map(|node| node.dyn_into::<Element>().ok())
    .map(|element| {
      element
        .get_attribute("aria-label")
        .unwrap_or_else(|| element.text_content().unwrap_or_default())
    })
    .collect();

  assert_eq!(boxes[1].get_attribute("tabindex").as_deref(), Some("0"));
  assert_eq!(
    boxes[1].get_attribute("role"),
    None,
    "a row has no role of its own"
  );
  assert_eq!(boxes[2].get_attribute("role").as_deref(), Some("group"));
  assert_eq!(boxes[3].get_attribute("tabindex"), None);
  assert_eq!(order, ["Before", "Inside", "Inside", "Named"]);
}

/// A page opens with nothing focused, the body active, where no control asks for focus.
#[wasm_bindgen_test]
fn a_page_opens_with_nothing_focused() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Column::new()
      .set_children((
        BasicButton::new(Activate::new()).set_label("a"),
        BasicTextField::new(TextEntry::new()),
      ))
      .build()
  });

  assert!(root.query_selector("[autofocus]").expect("query").is_none());
  assert_eq!(
    document().active_element(),
    document().body().map(Into::into),
    "the body is active"
  );
}

/// A control set to autofocus takes focus as the mount that opens the page ends, the
/// first in the scene's child order of those that ask and are enabled, and carries
/// `autofocus` as the served page's element does; a mount after the page opened moves
/// no focus.
#[wasm_bindgen_test]
fn an_autofocus_control_is_focused_after_the_mount() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Column::new()
      .set_children((
        BasicButton::new(Activate::new()).set_label("a"),
        BasicButton::new(Activate::new())
          .set_label("off")
          .set_disabled(true)
          .set_autofocus(true),
        BasicButton::new(Activate::new())
          .set_label("b")
          .set_autofocus(true),
        BasicButton::new(Activate::new())
          .set_label("c")
          .set_autofocus(true),
      ))
      .build()
  });

  let buttons = root.query_selector_all("button").expect("query");
  let button = |index| {
    buttons
      .item(index)
      .and_then(|node| node.dyn_into::<Element>().ok())
      .expect("a button")
  };

  assert_eq!(
    document().active_element(),
    Some(button(2)),
    "b takes focus"
  );
  assert!(button(2).has_attribute("autofocus"));
  assert_eq!(
    root
      .query_selector_all("[autofocus]")
      .expect("query")
      .length(),
    1,
    "one element opens focused"
  );

  let _ = mount(|| {
    BasicButton::new(Activate::new())
      .set_label("d")
      .set_autofocus(true)
      .build()
  });

  assert_eq!(
    document().active_element(),
    Some(button(2)),
    "the page has opened"
  );
}

/// Dispatches a press of `key` with `Ctrl` held at `target`, and returns whether the
/// platform's default was prevented.
fn control_key_at(target: &Element, key: &str) -> bool {
  let init = web_sys::KeyboardEventInit::new();

  init.set_bubbles(true);
  init.set_cancelable(true);
  init.set_key(key);
  init.set_ctrl_key(true);

  let event =
    web_sys::KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).expect("a key");
  let _ = target.dispatch_event(&event);

  event.default_prevented()
}

/// A control's shortcut activates it from anywhere in the window, in place of the
/// browser's default: with nothing focused, the key landing on the body, with focus on
/// another control and in a text entry, while a key holding no modifier stays the
/// entry's; a key pressed on the page's own element outside the root is the page's; the
/// control reports it as `aria-keyshortcuts`.
#[wasm_bindgen_test]
fn a_controls_shortcut_activates_it_from_anywhere() {
  let (_guard, root) = fresh_root();
  let (saves, save) = counter();
  let (news, new) = counter();

  let _ = mount(move || {
    Column::new()
      .set_children((
        BasicButton::new(Activate::new())
          .set_label("Save")
          .set_shortcut(Shortcut::new(Modifiers::CTRL, "s"))
          .handle_activate(save),
        BasicButton::new(Activate::new())
          .set_label("New")
          .set_shortcut(Shortcut::new(Modifiers::NONE, "n"))
          .handle_activate(new),
        BasicTextField::new(TextEntry::new()).set_label("Note"),
      ))
      .build()
  });

  let body: Element = document().body().expect("a body").into();
  let buttons = root.query_selector_all("button").expect("query");
  let button = |index| {
    buttons
      .item(index)
      .and_then(|node| node.dyn_into::<Element>().ok())
      .expect("a button")
  };
  let input = root
    .query_selector("input")
    .expect("query")
    .expect("the field");

  assert_eq!(
    button(0).get_attribute("aria-keyshortcuts").as_deref(),
    Some("Control+S")
  );
  assert_eq!(
    button(1).get_attribute("aria-keyshortcuts").as_deref(),
    Some("N")
  );
  assert!(control_key_at(&body, "s"), "the browser's default is kept");
  assert_eq!(saves.get(), 1, "with nothing focused");
  assert!(key_at(&body, "keydown", "n", false));
  assert_eq!(news.get(), 1);

  let _ = html(&button(1)).focus();

  assert!(control_key_at(&button(1), "s"));
  assert_eq!(saves.get(), 2, "with focus on another control");

  let _ = html(&input).focus();

  assert!(control_key_at(&input, "s"));
  assert_eq!(saves.get(), 3, "with focus in a text entry");
  assert!(
    !key_at(&input, "keydown", "n", false),
    "the entry takes its key"
  );
  assert_eq!(news.get(), 1);

  let page = document().create_element("input").expect("an input");
  let _ = body.append_child(&page);
  let _ = html(&page).focus();

  assert!(
    !control_key_at(&page, "s"),
    "the page's element keeps its key"
  );
  assert!(!key_at(&page, "keydown", "n", false));
  assert_eq!((saves.get(), news.get()), (3, 1));

  page.remove();
}

/// A shortcut acts on no disabled control, on none a box's style hides and on none an
/// open modal excludes, the key then left to the browser, and acts again once the
/// control is enabled, shown and the modal closed.
#[wasm_bindgen_test]
fn a_shortcut_passes_over_a_disabled_control_and_one_a_modal_excludes() {
  let (_guard, _root) = fresh_root();
  let (saves, save) = counter();
  let disabled = Signal::new(false);
  let hidden = Signal::new(false);
  let open = Signal::new(false);

  let _ = mount(move || {
    Column::new()
      .set_children((
        Container::new()
          .derive_hidden(move || hidden.get())
          .set_child(
            BasicButton::new(Activate::new())
              .set_label("Save")
              .derive_disabled(move || disabled.get())
              .set_shortcut(Shortcut::new(Modifiers::CTRL, "s"))
              .handle_activate(save),
          )
          .into_any(),
        Overlay::modal()
          .bind_open(open)
          .set_child(BasicButton::new(Activate::new()).set_label("Inside"))
          .into_any(),
      ))
      .build()
  });

  let body: Element = document().body().expect("a body").into();

  disabled.set(true);
  web::end_turn();

  assert!(
    !control_key_at(&body, "s"),
    "disabled, the key is the browser's"
  );
  assert_eq!(saves.get(), 0);

  disabled.set(false);
  hidden.set(true);
  web::end_turn();

  assert!(
    !control_key_at(&body, "s"),
    "hidden, the key is the browser's"
  );
  assert_eq!(saves.get(), 0);

  hidden.set(false);
  open.set(true);
  web::end_turn();

  assert!(!control_key_at(&body, "s"), "the open modal excludes it");
  assert_eq!(saves.get(), 0);

  open.set(false);
  web::end_turn();

  assert!(control_key_at(&body, "s"));
  assert_eq!(saves.get(), 1);
}

#[wasm_bindgen_test]
fn click_handler_runs_on_a_real_click() {
  let (_guard, root) = fresh_root();
  let clicks = Rc::new(Cell::new(0u32));

  let _ = mount({
    let clicks = Rc::clone(&clicks);

    move || {
      Container::new()
        .handle_click(move || clicks.set(clicks.get() + 1))
        .build()
    }
  });

  let button = html(&only_child(&root));

  button.click();
  button.click();

  assert_eq!(clicks.get(), 2);
}

/// A delegated pointer-down handler receives the event with its position and button.
#[wasm_bindgen_test]
fn pointer_down_handler_receives_the_event() {
  let (_guard, root) = fresh_root();
  let seen = Rc::new(Cell::new(None));

  let _ = mount({
    let seen = Rc::clone(&seen);

    move || {
      Container::new()
        .handle_pointer_down(move |event| seen.set(Some((event.x, event.y, event.button))))
        .build()
    }
  });

  let target = only_child(&root);
  let init = web_sys::PointerEventInit::new();

  init.set_bubbles(true);
  init.set_client_x(11);
  init.set_client_y(7);
  init.set_button(0);

  let event =
    web_sys::PointerEvent::new_with_event_init_dict("pointerdown", &init).expect("a pointer event");

  let _ = target.dispatch_event(&event);

  assert_eq!(seen.get(), Some((11., 7., Some(PointerButton::Primary))));
}

/// A pointer event carries its pointer's id and kind, the buttons held and the
/// pressure: a pen that is not the primary pointer is named past the primary one, and
/// the primary pointer is `PointerId::PRIMARY`.
#[wasm_bindgen_test]
fn a_pointer_event_carries_its_pointer_buttons_and_pressure() {
  let (_guard, root) = fresh_root();
  let seen = Rc::new(Cell::new(None));

  let _ = mount({
    let seen = Rc::clone(&seen);

    move || {
      Container::new()
        .handle_pointer_down(move |event| {
          seen.set(Some((event.pointer, event.buttons, event.pressure)));
        })
        .build()
    }
  });

  let target = only_child(&root);
  let press = |primary: bool| {
    let init = web_sys::PointerEventInit::new();

    init.set_bubbles(true);
    init.set_pointer_id(7);
    init.set_pointer_type("pen");
    init.set_is_primary(primary);
    init.set_pressure(0.75);
    init.set_button(0);
    init.set_buttons(1 | 32);

    let event = web_sys::PointerEvent::new_with_event_init_dict("pointerdown", &init)
      .expect("a pointer event");

    let _ = target.dispatch_event(&event);

    seen.take().expect("the press reached the handler")
  };
  let (pointer, buttons, pressure) = press(false);

  assert_eq!(pointer.pointer_id, PointerId::new(9));
  assert_eq!(pointer.pointer_type, PointerType::Pen);
  assert_eq!(
    buttons,
    PointerButton::Primary | PointerButton::PenEraser,
    "the buttons held"
  );
  assert!((pressure - 0.75).abs() < f32::EPSILON, "{pressure}");
  assert_eq!(press(true).0.pointer_id, Some(PointerId::PRIMARY));
}

/// A `wheel` reaches the node under the pointer in the unit the browser counts before
/// the browser scrolls by it: a box's handler leaves the scroll to the browser, and a
/// behavior taking the turn keeps it from happening.
#[wasm_bindgen_test]
fn a_wheel_reaches_the_node_before_it_scrolls() {
  use skene_foundation::{EventKind, NativeElement, Prevent};
  use skene_web::{
    Control,
    behavior::{Behavior, ControlHandle},
  };

  /// Takes every turn for itself.
  struct Dial;

  impl Behavior for Dial {
    fn native(&self) -> Option<NativeElement> {
      Some(NativeElement::Button)
    }

    fn role(&self) -> Option<Role> {
      None
    }

    fn listens(&self) -> &'static [EventKind] {
      &[EventKind::Wheel]
    }

    fn on_wheel(&self, _: &WheelEvent, _: &ControlHandle) -> Prevent {
      Prevent::All
    }
  }

  let (_guard, root) = fresh_root();
  let seen = Rc::new(Cell::new(None));

  let _ = mount({
    let seen = Rc::clone(&seen);

    move || {
      Column::new()
        .set_children((
          Container::new().handle_wheel(move |event| seen.set(Some((event.delta, event.unit)))),
          Control::new(Dial).set_child(Text::new("dial")),
        ))
        .build()
    }
  });

  let turn = |target: &Element| {
    let init = web_sys::WheelEventInit::new();

    init.set_bubbles(true);
    init.set_cancelable(true);
    init.set_delta_y(3.);
    init.set_delta_mode(web_sys::WheelEvent::DOM_DELTA_LINE);

    let event =
      web_sys::WheelEvent::new_with_event_init_dict("wheel", &init).expect("a wheel event");

    let _ = target.dispatch_event(&event);

    event.default_prevented()
  };
  let column = only_child(&root);
  let child = |index| column.children().item(index).expect("a child");

  assert!(!turn(&child(0)), "the browser scrolls");
  assert_eq!(seen.get(), Some((Axes::new(0., 3.), WheelUnit::Lines)));
  assert!(turn(&child(1)), "the dial takes the turn");
}

/// A drag from a box offering data drops on a box taking it: the browser is handed the
/// drag's text, the target lets the drop happen and holds `Targeted` while the drag is
/// over it, and the drop runs its action.
#[wasm_bindgen_test]
fn a_drag_drops_on_a_box_taking_it() {
  let (_guard, root) = fresh_root();
  let dropped = Rc::new(RefCell::new(Vec::new()));

  let _ = mount({
    let dropped = Rc::clone(&dropped);

    move || {
      Column::new()
        .set_children((
          Container::new().set_drag_data(|| DragData::new(String::from("card")).with_text("card")),
          Container::new()
            .handle_drop(move |card: &String| dropped.borrow_mut().push(card.clone())),
        ))
        .build()
    }
  });

  let column = only_child(&root);
  let child = |index| column.children().item(index).expect("a child");
  let transfer = web_sys::DataTransfer::new().expect("a data transfer");
  let drag = |target: &Element, kind: &str| {
    let init = web_sys::DragEventInit::new();

    init.set_bubbles(true);
    init.set_cancelable(true);
    init.set_data_transfer(Some(&transfer));

    let event = web_sys::DragEvent::new_with_event_init_dict(kind, &init).expect("a drag event");

    let _ = target.dispatch_event(&event);

    event.default_prevented()
  };

  assert_eq!(child(0).get_attribute("draggable").as_deref(), Some("true"));

  let _ = drag(&child(0), "dragstart");

  assert_eq!(
    transfer.get_data("text/plain").ok().as_deref(),
    Some("card")
  );
  assert!(child(0).has_attribute("data-skene-dragged"));
  assert!(
    drag(&child(1), "dragover"),
    "the target lets the drop happen"
  );
  assert!(child(1).has_attribute("data-skene-targeted"));

  let _ = drag(&child(1), "drop");
  let _ = drag(&child(0), "dragend");

  assert_eq!(*dropped.borrow(), ["card"]);
  assert!(!child(1).has_attribute("data-skene-targeted"));
  assert!(!child(0).has_attribute("data-skene-dragged"));
}

/// A `dblclick` reaches the box's double-click handler through dispatch.
#[wasm_bindgen_test]
fn a_double_click_reaches_the_box() {
  let (_guard, root) = fresh_root();
  let doubles = Rc::new(Cell::new(0));

  let _ = mount({
    let doubles = Rc::clone(&doubles);

    move || {
      Container::new()
        .handle_double_click(move || doubles.set(doubles.get() + 1))
        .build()
    }
  });

  let init = web_sys::MouseEventInit::new();

  init.set_bubbles(true);
  init.set_detail(2);

  let event =
    web_sys::MouseEvent::new_with_mouse_event_init_dict("dblclick", &init).expect("a mouse event");

  let _ = only_child(&root).dispatch_event(&event);

  assert_eq!(doubles.get(), 1);
}

/// A `pointercancel` reaches the box's cancel handler through dispatch.
#[wasm_bindgen_test]
fn a_pointer_cancel_reaches_the_box() {
  let (_guard, root) = fresh_root();
  let cancels = Rc::new(Cell::new(0));

  let _ = mount({
    let cancels = Rc::clone(&cancels);

    move || {
      Container::new()
        .handle_pointer_cancel(move |_| cancels.set(cancels.get() + 1))
        .build()
    }
  });

  pointer(&only_child(&root), "pointercancel");

  assert_eq!(cancels.get(), 1);
}

/// A box's pointer release, move, entry and exit, focus, blur and scroll handlers each
/// hear their DOM event, through the listener each asks the document for.
#[wasm_bindgen_test]
fn a_boxs_pointer_focus_and_scroll_handlers_hear_their_events() {
  let (_guard, root) = fresh_root();
  let log = Rc::new(RefCell::new(Vec::<String>::new()));
  let note = |name: &'static str| {
    let log = Rc::clone(&log);

    move || log.borrow_mut().push(name.to_owned())
  };
  let (up, moved, entered, left, focused, blurred) = (
    note("up"),
    note("move"),
    note("enter"),
    note("leave"),
    note("focus"),
    note("blur"),
  );
  let scrolled = Rc::clone(&log);

  let _ = mount(move || {
    Container::new()
      .set_height(20)
      .set_overflow(skene_foundation::Overflow::Scroll)
      .set_focus_order(skene_web::FocusOrder::Auto)
      .handle_pointer_up(move |_| up())
      .handle_pointer_move(move |_| moved())
      .handle_pointer_enter(entered)
      .handle_pointer_leave(left)
      .handle_focus(focused)
      .handle_blur(blurred)
      .handle_scroll(move |event| scrolled.borrow_mut().push(format!("scroll {}", event.y)))
      .set_child(Container::new().set_height(200))
      .build()
  });

  let boxed = html(&only_child(&root));

  pointer(&boxed, "pointerup");
  pointer(&boxed, "pointermove");
  pointer(&boxed, "pointerover");
  pointer(&boxed, "pointerout");

  let _ = boxed.focus();
  let _ = boxed.blur();

  boxed.set_scroll_top(10);
  dispatch(&boxed, "scroll");

  assert_eq!(
    log.borrow()[..7],
    ["up", "move", "enter", "leave", "focus", "blur", "scroll 10"]
  );
}

/// A press on a slider's thumb asks for the value whose thumb centers under the
/// pointer, measured from the slider's own box rather than the thumb's: 150 pixels into
/// 200, the thumb's 16-pixel center travelling 184, is 77, once the browser has reported
/// the slider's size.
#[wasm_bindgen_test]
async fn a_press_on_the_thumb_slides_by_the_slider_box() {
  let (_guard, root) = fresh_root();
  let value = Signal::new(40.0f32);

  let _ = mount(move || {
    Container::new()
      .set_width(200)
      .set_child(SimpleSlider::new().bind_value(value))
      .build()
  });

  next_frame().await;

  let slider = only_child(&only_child(&root));
  let stack = only_child(&slider.last_element_child().expect("the drawn body"));
  let thumb = only_child(&stack.last_element_child().expect("the rail"));
  let init = web_sys::PointerEventInit::new();

  init.set_bubbles(true);
  #[expect(
    clippy::cast_possible_truncation,
    reason = "a coordinate inside the test page fits an i32"
  )]
  init.set_client_x((slider.get_bounding_client_rect().left() + 150.).round() as i32);
  init.set_button(0);

  let event =
    web_sys::PointerEvent::new_with_event_init_dict("pointerdown", &init).expect("a pointer event");

  let slid = frames_until(|| {
    let _ = thumb.dispatch_event(&event);

    (value.get() - 40.).abs() > f32::EPSILON
  })
  .await;

  assert!(slid, "the press moved the slider");
  assert!((value.get() - 77.).abs() < f32::EPSILON, "{}", value.get());
}

/// A press on a slider's thumb slides it as the pointer moves, until the browser
/// cancels the press: a move after the cancel slides nothing.
#[wasm_bindgen_test]
async fn a_cancelled_press_stops_sliding() {
  let (_guard, root) = fresh_root();
  let value = Signal::new(40.0f32);

  let _ = mount(move || {
    Container::new()
      .set_width(200)
      .set_child(SimpleSlider::new().bind_value(value))
      .build()
  });

  next_frame().await;

  let slider = only_child(&only_child(&root));
  let left = slider.get_bounding_client_rect().left();
  let stack = only_child(&slider.last_element_child().expect("the drawn body"));
  let thumb = only_child(&stack.last_element_child().expect("the rail"));
  let pointer = |kind: &str, x: f64| {
    let init = web_sys::PointerEventInit::new();

    init.set_bubbles(true);
    #[expect(
      clippy::cast_possible_truncation,
      reason = "a coordinate inside the test page fits an i32"
    )]
    init.set_client_x((left + x).round() as i32);
    init.set_button(0);

    let event =
      web_sys::PointerEvent::new_with_event_init_dict(kind, &init).expect("a pointer event");
    let _ = thumb.dispatch_event(&event);
  };
  let slid = frames_until(|| {
    pointer("pointerdown", 150.);

    (value.get() - 40.).abs() > f32::EPSILON
  })
  .await;

  assert!(slid, "the press moved the slider");

  let pressed = value.get();

  pointer("pointermove", 100.);

  let moved = value.get();

  assert!(moved < pressed, "a move slides it: {pressed} to {moved}");

  pointer("pointercancel", 100.);
  pointer("pointermove", 20.);

  assert!(
    (value.get() - moved).abs() < f32::EPSILON,
    "a move after the cancel slides nothing: {}",
    value.get()
  );
}

/// A slider's native input holds the slider's range, step and value, and follows the
/// value.
#[wasm_bindgen_test]
fn a_sliders_native_input_holds_its_range_and_value() {
  let (_guard, root) = fresh_root();
  let value = Signal::new(150.0f32);

  let _ = mount(move || {
    BasicSlider::new(
      Slider::new()
        .set_range(SliderRange::new(0., 200., 5.))
        .bind_value(value),
    )
    .build()
  });

  let input: HtmlInputElement = root
    .query_selector("input")
    .expect("a valid selector")
    .expect("the slider's input")
    .dyn_into()
    .expect("an input");

  assert_eq!(input.get_attribute("min").as_deref(), Some("0"));
  assert_eq!(input.get_attribute("max").as_deref(), Some("200"));
  assert_eq!(input.get_attribute("step").as_deref(), Some("5"));
  assert_eq!(input.value(), "150");
  assert!(!input.has_attribute("aria-valuenow"));

  value.set(185.);
  web::end_turn();

  assert_eq!(input.value(), "185");
}

/// The attribute names the mutations `observer` holds name, in the order they were made.
fn mutated_attributes(observer: &MutationObserver) -> Vec<String> {
  observer
    .take_records()
    .iter()
    .filter_map(|record| record.dyn_into::<MutationRecord>().ok())
    .filter_map(|record| record.attribute_name())
    .collect()
}

/// An observer of the range attributes `element` is written.
fn observe_attributes(element: &Element) -> MutationObserver {
  let observer =
    MutationObserver::new(&js_sys::Function::new_no_args("")).expect("a mutation observer");
  let options = MutationObserverInit::new();
  let filter: js_sys::Array = ["min", "max", "step", "value"]
    .into_iter()
    .map(JsValue::from_str)
    .collect();

  options.set_attribute_filter(&filter);
  observer
    .observe_with_options(element, &options)
    .expect("observing the element");

  observer
}

/// A slider whose range narrows below its value writes its bounds, its step and its value
/// once each, the bounds first, and its input shows the value clamped into them.
#[wasm_bindgen_test]
fn a_slider_narrowed_below_its_value_shows_it_clamped_in_one_write() {
  let (_guard, root) = fresh_root();
  let value = Signal::new(150.0f32);
  let maximum = Signal::new(200.0f32);

  let _ = mount(move || {
    BasicSlider::new(
      Slider::new()
        .derive_range(move || SliderRange::new(0., maximum.get(), 5.))
        .bind_value(value),
    )
    .build()
  });

  let input: HtmlInputElement = root
    .query_selector("input")
    .expect("a valid selector")
    .expect("the slider's input")
    .dyn_into()
    .expect("an input");
  let observer = observe_attributes(&input);

  maximum.set(100.);
  web::end_turn();

  assert_eq!(
    mutated_attributes(&observer),
    ["min", "max", "step", "value"],
    "one write, the bounds before the value"
  );
  assert_eq!(input.get_attribute("max").as_deref(), Some("100"));
  assert_eq!(input.value(), "100", "the value clamped into the range");
  assert!(
    (value.get() - 150.).abs() < f32::EPSILON,
    "the truth stays the app's"
  );
}

/// A `<select>` whose options and chosen value change together shows the chosen option
/// among the new ones after the one write, rebuilding its options only where they differ,
/// and shows an empty hidden option first and selected where the chosen key names none.
#[wasm_bindgen_test]
fn a_select_shows_its_options_and_choice_as_one() {
  use skene_foundation::{ChoiceOption, Choices, contents};
  use skene_web::{Active, Control, behavior::NativePicker, reactive::Effect};

  let choices = |keys: &[&str], chosen: &str| {
    Choices::new(
      keys
        .iter()
        .map(|key| ChoiceOption::new(*key, key.to_uppercase()))
        .collect(),
      Some(Rc::from(chosen)),
    )
  };
  let (_guard, root) = fresh_root();
  let shown = Signal::new(choices(&["s", "m"], "s"));

  let _ = mount(move || {
    let node = Control::new(NativePicker::new()).build();

    Effect::render(move |_: Option<()>| contents::CHOICE.write::<Active>(node, &shown.get()));

    node
  });

  let select: HtmlSelectElement = root
    .query_selector("select")
    .expect("a valid selector")
    .expect("the picker's select")
    .dyn_into()
    .expect("a select");
  let names = |select: &HtmlSelectElement| {
    let options = select.options();

    (0..options.length())
      .filter_map(|index| options.item(index))
      .map(|option| option.text_content().unwrap_or_default())
      .collect::<Vec<_>>()
  };

  assert_eq!(names(&select), ["S", "M"]);
  assert_eq!(select.value(), "s");

  shown.set(choices(&["l", "xl"], "xl"));
  web::end_turn();

  assert_eq!(names(&select), ["L", "XL"]);
  assert_eq!(
    select.value(),
    "xl",
    "the chosen key among the options it came with"
  );
  assert_eq!(select.selected_index(), 1);

  let first = select.options().item(0).expect("an option");

  shown.set(choices(&["l", "xl"], "l"));
  web::end_turn();

  assert_eq!(select.value(), "l");
  assert!(
    select
      .options()
      .item(0)
      .is_some_and(|option| option == first),
    "options it shows already stay"
  );

  shown.set(choices(&["l", "xl"], "m"));
  web::end_turn();

  assert_eq!(names(&select), ["", "L", "XL"]);
  assert_eq!(
    select.selected_index(),
    0,
    "a key naming no option selects the empty one before them"
  );
  assert!(
    select
      .options()
      .item(0)
      .and_then(|option| option.dyn_into::<web_sys::HtmlElement>().ok())
      .is_some_and(|option| option.hidden()),
    "which is hidden"
  );

  shown.set(choices(&["l", "xl"], "xl"));
  web::end_turn();

  assert_eq!(names(&select), ["L", "XL"], "and goes once one is chosen");
  assert_eq!(select.value(), "xl");
}

/// A change handler receives the value, in real time, when an input event fires on
/// the text field's native input with text the reader typed.
#[wasm_bindgen_test]
fn change_handler_receives_the_committed_value() {
  let (_guard, root) = fresh_root();
  let seen = Rc::new(RefCell::new(String::new()));

  let _ = mount({
    let seen = Rc::clone(&seen);

    move || {
      BasicTextField::new(
        TextEntry::new().handle_change(move |value| value.clone_into(&mut seen.borrow_mut())),
      )
      .build()
    }
  });

  let input = only_child(&root)
    .dyn_into::<HtmlInputElement>()
    .expect("an input");

  input.set_value("committed");
  dispatch(&input, "input");

  assert_eq!(&*seen.borrow(), "committed");
}

/// A picker's drawn face keeps its caption before its box, as its scriptless `<select>`
/// does, in a `<label>` `for` nothing.
#[wasm_bindgen_test]
fn a_drawn_face_keeps_its_caption_before_its_box() {
  use skene_web::{BasicListboxOption, BasicPicker, behavior::ListboxOption};

  let (_guard, root) = fresh_root();
  let value = Signal::new("a");

  let _ = mount(move || {
    BasicPicker::new()
      .bind_chosen(value)
      .set_label("Size")
      .set_options(
        ["a", "b"].map(|value| BasicListboxOption::new(ListboxOption::new(value)).set_label(value)),
      )
      .set_face(|shown| Text::derive(move || shown.get().unwrap_or_default().to_owned()))
      .build()
  });

  let face = root
    .query_selector("[role=combobox]")
    .expect("query")
    .expect("the drawn face");
  let caption = face
    .previous_element_sibling()
    .expect("the caption before the face");

  assert_eq!(caption.tag_name(), "LABEL");
  assert_eq!(caption.text_content().as_deref(), Some("Size"));
  assert!(!caption.has_attribute("for"));
  assert!(
    face.query_selector("label").expect("query").is_none(),
    "nothing captions the face from inside"
  );
}

/// A transformed value repairs a focused entry in place, the caret kept where the reader
/// left it: an uppercasing handler leaves it after the character typed.
#[wasm_bindgen_test]
fn a_transformed_value_keeps_the_caret() {
  let (_guard, root) = fresh_root();
  let value = Signal::new("ac".to_owned());

  let _ = mount(move || {
    BasicTextField::new(
      TextEntry::new()
        .derive_value(move || value.get())
        .handle_change(move |text| value.set(text.to_uppercase())),
    )
    .build()
  });

  let input = only_child(&root)
    .dyn_into::<HtmlInputElement>()
    .expect("an input");

  let _ = input.focus();

  input.set_value("abc");

  let _ = input.set_selection_range(2, 2);

  dispatch(&input, "input");
  web::end_turn();

  assert_eq!(input.value(), "ABC");
  assert_eq!(
    (input.selection_start(), input.selection_end()),
    (Ok(Some(2)), Ok(Some(2)))
  );
}

/// A character the handler refuses is taken back out of the `<input>` by the smallest
/// edit: the element stays, and the caret stays where the reader left it, where
/// rewriting the whole value would leave it after the character that followed.
#[wasm_bindgen_test]
fn a_refused_character_is_repaired_in_place() {
  let (_guard, root) = fresh_root();
  let value = Signal::new("123".to_owned());

  let _ = mount(move || {
    BasicTextField::new(
      TextEntry::new()
        .derive_value(move || value.get())
        .handle_change(move |text| {
          if text.chars().all(|character| character.is_ascii_digit()) {
            value.set(text.to_owned());
          }
        }),
    )
    .build()
  });

  let element = only_child(&root);
  let input = element
    .clone()
    .dyn_into::<HtmlInputElement>()
    .expect("an input");

  let _ = input.focus();

  input.set_value("12x3");

  let _ = input.set_selection_range(3, 3);

  dispatch(&input, "input");
  web::end_turn();

  assert_eq!(input.value(), "123");
  assert_eq!(value.get(), "123", "the refusal wrote nothing");
  assert_eq!(
    (input.selection_start(), input.selection_end()),
    (Ok(Some(2)), Ok(Some(2))),
    "the caret stays where the refused character was typed"
  );
  assert!(only_child(&root) == element, "the element is the same");
}

/// An input method's composition is left alone: an edit reported while it composes is
/// not repaired, nor rewritten by the signal's writes, and the composition's end
/// repairs it.
#[wasm_bindgen_test]
fn a_repair_waits_out_composition() {
  let (_guard, root) = fresh_root();
  let value = Signal::new("ab".to_owned());
  let selection = Signal::new(TextSelection::new(2, 2));

  let _ = mount(move || {
    BasicTextField::new(
      TextEntry::new()
        .derive_value(move || value.get())
        .handle_change(move |text| value.set(text.to_uppercase()))
        .bind_selection(selection),
    )
    .build()
  });

  let input = only_child(&root)
    .dyn_into::<HtmlInputElement>()
    .expect("an input");

  let _ = input.focus();

  input.set_value("abx");

  let _ = input.set_selection_range(3, 3);

  let init = InputEventInit::new();

  init.set_bubbles(true);
  init.set_is_composing(true);

  let composing = InputEvent::new_with_event_init_dict("input", &init).expect("an input event");
  let _ = input.dispatch_event(&composing);

  web::end_turn();

  assert_eq!(value.get(), "ABX", "the composing edit reached the handler");
  assert_eq!(
    input.value(),
    "abx",
    "nothing is repaired while the input method composes"
  );

  selection.set(TextSelection::new(0, 0));
  web::end_turn();

  assert_eq!(
    (input.selection_start(), input.selection_end()),
    (Ok(Some(3)), Ok(Some(3))),
    "a selection written while the input method composes waits too"
  );

  let init = CompositionEventInit::new();

  init.set_bubbles(true);
  init.set_data("x");

  let end = CompositionEvent::new_with_event_init_dict("compositionend", &init)
    .expect("a composition event");
  let _ = input.dispatch_event(&end);

  web::end_turn();

  assert_eq!(input.value(), "ABX", "the composition's end repairs it");
  assert_eq!(
    (input.selection_start(), input.selection_end()),
    (Ok(Some(3)), Ok(Some(3))),
    "the reader's caret stands"
  );
  assert_eq!(value.get(), "ABX");
  assert_eq!(selection.get(), TextSelection::new(3, 3));
}

/// A character the handler refuses is taken back out of a `<textarea>` by the smallest
/// edit across its lines: the element stays, and the caret stays where the reader left
/// it, counted past a character two UTF-16 units wide and a line break.
#[wasm_bindgen_test]
fn a_refused_character_is_repaired_in_place_in_an_editor() {
  let (_guard, root) = fresh_root();
  let value = Signal::new("\u{1F600}\nab".to_owned());

  let _ = mount(move || {
    BasicTextEditor::new(
      TextEntry::new()
        .set_lines(3)
        .derive_value(move || value.get())
        .handle_change(move |text| {
          if !text.contains('x') {
            value.set(text.to_owned());
          }
        }),
    )
    .build()
  });

  let element = only_child(&root);
  let area = element
    .clone()
    .dyn_into::<HtmlTextAreaElement>()
    .expect("a textarea");

  let _ = area.focus();

  area.set_value("\u{1F600}\nxab");

  let _ = area.set_selection_range(4, 4);

  dispatch(&area, "input");
  web::end_turn();

  assert_eq!(area.value(), "\u{1F600}\nab");
  assert_eq!(value.get(), "\u{1F600}\nab", "the refusal wrote nothing");
  assert_eq!(
    (area.selection_start(), area.selection_end()),
    (Ok(Some(3)), Ok(Some(3))),
    "the caret stays where the refused character was typed"
  );
  assert!(only_child(&root) == element, "the element is the same");
}

/// An input method's composition in a `<textarea>` is left alone: an edit reported while
/// it composes is not repaired, and the composition's end repairs it, the caret kept.
#[wasm_bindgen_test]
fn a_repair_in_an_editor_waits_out_composition() {
  let (_guard, root) = fresh_root();
  let value = Signal::new("a\nb".to_owned());
  let selection = Signal::new(TextSelection::new(3, 3));

  let _ = mount(move || {
    BasicTextEditor::new(
      TextEntry::new()
        .set_lines(3)
        .derive_value(move || value.get())
        .handle_change(move |text| value.set(text.to_uppercase()))
        .bind_selection(selection),
    )
    .build()
  });

  let area = only_child(&root)
    .dyn_into::<HtmlTextAreaElement>()
    .expect("a textarea");

  let _ = area.focus();

  area.set_value("a\nbx");

  let _ = area.set_selection_range(4, 4);

  let init = InputEventInit::new();

  init.set_bubbles(true);
  init.set_is_composing(true);

  let composing = InputEvent::new_with_event_init_dict("input", &init).expect("an input event");
  let _ = area.dispatch_event(&composing);

  web::end_turn();

  assert_eq!(
    value.get(),
    "A\nBX",
    "the composing edit reached the handler"
  );
  assert_eq!(
    area.value(),
    "a\nbx",
    "nothing is repaired while the input method composes"
  );

  let init = CompositionEventInit::new();

  init.set_bubbles(true);
  init.set_data("x");

  let end = CompositionEvent::new_with_event_init_dict("compositionend", &init)
    .expect("a composition event");
  let _ = area.dispatch_event(&end);

  web::end_turn();

  assert_eq!(area.value(), "A\nBX", "the composition's end repairs it");
  assert_eq!(
    (area.selection_start(), area.selection_end()),
    (Ok(Some(4)), Ok(Some(4))),
    "the reader's caret stands"
  );
  assert_eq!(selection.get(), TextSelection::new(4, 4));
}

/// Stands in for the system clipboard: `readText` answers `pasted`, and `writeText`
/// keeps what it is given for [`written`]; [`restore_clipboard`] takes the stand-in
/// away.
fn stub_clipboard(pasted: &str) {
  let clipboard = web_sys::window().expect("a window").navigator().clipboard();
  let read = js_sys::Function::new_with_args("", "return Promise.resolve(this.pasted);");
  let write =
    js_sys::Function::new_with_args("text", "this.written = text; return Promise.resolve();");

  for (name, value) in [
    ("pasted", JsValue::from_str(pasted)),
    ("readText", read.into()),
    ("writeText", write.into()),
  ] {
    js_sys::Reflect::set(&clipboard, &JsValue::from_str(name), &value).expect("stub");
  }
}

/// What the clipboard's stand-in was last given to write.
fn written() -> Option<String> {
  let clipboard = web_sys::window().expect("a window").navigator().clipboard();

  js_sys::Reflect::get(&clipboard, &JsValue::from_str("written"))
    .ok()
    .and_then(|text| text.as_string())
}

/// Takes the clipboard's stand-in away.
fn restore_clipboard() {
  let clipboard = web_sys::window().expect("a window").navigator().clipboard();

  for name in ["pasted", "readText", "writeText", "written"] {
    let _ = js_sys::Reflect::delete_property(&clipboard, &JsValue::from_str(name));
  }
}

/// A cut from a bound field writes the selection to the clipboard and reports the edit
/// it makes: the signal holds what is left and the caret sits where the selection was.
#[wasm_bindgen_test]
fn a_cut_reports_its_edit() {
  let (_guard, root) = fresh_root();
  let value = Signal::new("abcd".to_owned());
  let selection = Signal::new(TextSelection::new(0, 0));

  let _ = mount(move || {
    BasicTextField::new(TextEntry::new().bind_value(value).bind_selection(selection)).build()
  });

  let input = only_child(&root)
    .dyn_into::<HtmlInputElement>()
    .expect("an input");

  let _ = input.focus();
  let _ = input.set_selection_range(1, 3);

  stub_clipboard("");
  Web::clipboard(ClipboardAction::Cut);
  web::end_turn();

  let cut = written();

  restore_clipboard();

  assert_eq!(
    cut.as_deref(),
    Some("bc"),
    "the selection reached the clipboard"
  );
  assert_eq!(value.get(), "ad", "the edit reached the signal");
  assert_eq!(input.value(), "ad");
  assert_eq!(
    (input.selection_start(), input.selection_end()),
    (Ok(Some(1)), Ok(Some(1)))
  );
  assert_eq!(selection.get(), TextSelection::new(1, 1));
}

/// A secure field keeps its text from the clipboard as Chromium's password field does:
/// with its value selected, copying and cutting are offered disabled and pasting enabled,
/// and a cut writes nothing to the clipboard and leaves the value whole.
#[wasm_bindgen_test]
fn a_secure_field_neither_copies_nor_cuts() {
  let (_guard, root) = fresh_root();
  let value = Signal::new("hunter2".to_owned());

  let _ =
    mount(move || BasicTextField::new(TextEntry::new().set_secure(true).bind_value(value)).build());

  let input = only_child(&root)
    .dyn_into::<HtmlInputElement>()
    .expect("an input");

  assert_eq!(input.type_(), "password");

  let _ = input.focus();
  let _ = input.set_selection_range(0, 7);

  assert_eq!(
    [
      ClipboardAction::Copy,
      ClipboardAction::Cut,
      ClipboardAction::Paste
    ]
    .map(Web::clipboard_availability),
    [
      Availability::Disabled,
      Availability::Disabled,
      Availability::Enabled
    ]
  );

  stub_clipboard("");
  Web::clipboard(ClipboardAction::Copy);
  Web::clipboard(ClipboardAction::Cut);
  web::end_turn();

  let written = written();

  restore_clipboard();

  assert_eq!(written, None, "nothing reached the clipboard");
  assert_eq!(value.get(), "hunter2", "the cut removed nothing");
  assert_eq!(input.value(), "hunter2");
}

/// A paste into a bound field replaces its selection and reports the edit it makes: the
/// signal holds the result, and the caret sits after the pasted text.
#[wasm_bindgen_test]
async fn a_paste_reports_its_edit_with_the_caret_after_it() {
  let (_guard, root) = fresh_root();
  let value = Signal::new("abc".to_owned());
  let selection = Signal::new(TextSelection::new(0, 0));

  let _ = mount(move || {
    BasicTextField::new(TextEntry::new().bind_value(value).bind_selection(selection)).build()
  });

  let input = only_child(&root)
    .dyn_into::<HtmlInputElement>()
    .expect("an input");

  let _ = input.focus();
  let _ = input.set_selection_range(1, 2);

  stub_clipboard("XY");
  Web::clipboard(ClipboardAction::Paste);

  let pasted = frames_until(|| value.get() == "aXYc").await;

  restore_clipboard();

  assert!(pasted, "the edit reached the signal: {}", value.get());
  assert_eq!(input.value(), "aXYc");
  assert_eq!(
    (input.selection_start(), input.selection_end()),
    (Ok(Some(3)), Ok(Some(3))),
    "the caret sits after the pasted text"
  );
  assert_eq!(selection.get(), TextSelection::new(3, 3));
}

/// A checkbox bound to a signal reflects it on its native input and writes toggles
/// back through a real change event.
#[wasm_bindgen_test]
fn checkbox_binding_round_trips() {
  let (_guard, root) = fresh_root();
  let checked = Signal::new(false);

  let _ = mount(move || BasicCheckbox::new(Toggle::new().bind_checked(checked)).build());

  let target: HtmlInputElement = only_child(&only_child(&root))
    .dyn_into()
    .expect("an input element");

  assert_eq!(target.get_attribute("type").as_deref(), Some("checkbox"));
  assert!(!target.checked());

  checked.set(true);
  web::end_turn();

  assert!(target.checked());

  target.set_checked(false);
  dispatch(&target, "change");

  assert!(!checked.get());
}

/// Head registrations set the live document title and language and add each element
/// once.
#[wasm_bindgen_test]
fn head_registrations_apply_once() {
  let (_guard, _root) = fresh_root();

  window::set_title("Live title");
  head::set_language("fr");
  head::add_meta("live-test", "value");
  head::add_meta("live-test", "value");

  assert_eq!(document().title(), "Live title");
  assert_eq!(
    document()
      .document_element()
      .and_then(|root| root.get_attribute("lang"))
      .as_deref(),
    Some("fr")
  );

  let head = document().head().expect("a head");
  let mut matches = 0;

  for index in 0..head.children().length() {
    let element = head.children().item(index).expect("an element");

    if element.get_attribute("name").as_deref() == Some("live-test") {
      matches += 1;

      element.remove();
    }
  }

  assert_eq!(matches, 1);
}

/// A registration leaves the live head with its owner, and the title and description it
/// replaced return.
#[wasm_bindgen_test]
fn head_registrations_leave_with_their_owner() {
  let (_guard, _root) = fresh_root();
  let outer = Owner::new();
  let inner = Owner::new();
  let scoped = || {
    let found = document()
      .head()
      .expect("a head")
      .query_selector_all("meta[name=scoped-test]")
      .expect("a valid selector");

    (0..found.length())
      .filter_map(|index| found.item(index))
      .filter_map(|node| node.dyn_into::<Element>().ok())
      .filter_map(|meta| meta.get_attribute("content"))
      .collect::<Vec<_>>()
  };

  outer.enter(|| {
    window::set_title("Outer");
    head::add_meta("scoped-test", "outer");
  });
  inner.enter(|| {
    window::set_title("Inner");
    head::add_meta("scoped-test", "inner");
  });

  assert_eq!(document().title(), "Inner");
  assert_eq!(scoped(), ["inner"]);

  inner.dispose();

  assert_eq!(document().title(), "Outer");
  assert_eq!(scoped(), ["outer"]);

  outer.dispose();

  assert_eq!(scoped(), Vec::<String>::new());
}

/// A registration adopts the identical element a server render left in the head, and
/// takes it along when its owner goes.
#[wasm_bindgen_test]
fn a_head_registration_adopts_the_served_element() {
  let (_guard, _root) = fresh_root();
  let parent = document().head().expect("a head");
  let served = document().create_element("meta").expect("a meta");

  let _ = served.set_attribute("name", "adopt-test");
  let _ = served.set_attribute("content", "x");
  let _ = parent.append_child(&served);

  let owner = Owner::new();

  owner.enter(|| head::add_meta("adopt-test", "x"));

  assert_eq!(
    parent
      .query_selector_all("meta[name=adopt-test]")
      .expect("a valid selector")
      .length(),
    1
  );
  assert!(served.is_connected());

  owner.dispose();

  assert!(!served.is_connected());
}

/// A focus handle moves real keyboard focus to the native input of its bound control.
#[wasm_bindgen_test]
fn focus_handle_focuses_the_bound_view() {
  let (_guard, root) = fresh_root();
  let handle = NodeHandle::new();

  let _ = mount(move || {
    BasicTextField::new(TextEntry::new())
      .bind_handle(handle)
      .build()
  });

  handle.focus();

  let input = only_child(&root);

  assert_eq!(document().active_element(), Some(input.clone()));

  handle.blur();

  assert_ne!(document().active_element(), Some(input));
}

/// A handler may re-enter the platform—building nodes from inside one does not deadlock
/// the scene or the DOM state.
#[wasm_bindgen_test]
fn a_click_handler_may_build_nodes() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Container::new()
      .handle_click(|| {
        let _ = Container::new().build();
      })
      .build()
  });

  html(&only_child(&root)).click();
}

/// A span's click handler runs for a click on the span, and not for one on the
/// paragraph beside it.
#[wasm_bindgen_test]
fn a_span_runs_its_click_handler() {
  let (_guard, root) = fresh_root();
  let clicks = Rc::new(Cell::new(0u32));
  let counted = Rc::clone(&clicks);

  let _ = mount(move || {
    skene_web::RichText::new()
      .add_spans([
        skene_web::Span::new("plain "),
        skene_web::Span::new("click").handle_click(move || counted.set(counted.get() + 1)),
      ])
      .build()
  });

  let paragraph = only_child(&root);
  let span = paragraph.last_element_child().expect("the clickable span");

  html(&span).click();
  html(&paragraph).click();

  assert_eq!(clicks.get(), 1);
}

#[wasm_bindgen_test]
fn derived_text_updates_the_dom_on_change() {
  let (_guard, root) = fresh_root();
  let count = Signal::new(0);

  let _ = mount(move || Text::derive(move || count.get().to_string()).build());

  let element = only_child(&root);

  assert_eq!(element.text_content().as_deref(), Some("0"));

  count.set(5);
  web::end_turn();

  assert_eq!(element.text_content().as_deref(), Some("5"));
}

/// Text keeps the guaranteed core: its blanks and newlines are kept, the blanks starting
/// or ending it taking their room, those ending a line where it wraps hanging past it,
/// a word too wide for its line breaks where the line fills, and an empty text, fixed or
/// derived, lays out the one line a letter does. The word's line is a fixed grid track.
#[wasm_bindgen_test]
fn text_keeps_the_guaranteed_core() {
  let (_guard, root) = fresh_root();
  let shown = Signal::new("a".to_owned());

  let _ = mount(move || {
    Column::new()
      .set_item_alignment(ItemAlignment::Start)
      .set_children((
        Text::new("a b"),
        Text::new("a  b"),
        Text::new("a"),
        Text::new("a\nb"),
        Grid::new()
          .set_columns(vec![GridTrack::length(40)])
          .set_children((Text::new("unbreakable".repeat(4)),)),
        Text::new(""),
        Text::derive(move || shown.get()),
        Text::new("   a b"),
        Text::new("a b   "),
        Grid::new()
          .set_columns(vec![GridTrack::length(40)])
          .set_children((Text::new("aaa     bbb"),)),
      ))
      .build()
  });

  let column = only_child(&root);
  let child = |index| column.children().item(index).expect("a child");
  let rect = |index| child(index).get_bounding_client_rect();
  let letter = rect(2).height();

  assert!(letter > 0., "a letter lays out a line");
  assert!(
    rect(1).width() > rect(0).width() + 1.,
    "two blanks are wider than one: {} against {}",
    rect(1).width(),
    rect(0).width(),
  );
  assert!(
    rect(3).height() > letter * 1.5,
    "a newline opens a second line: {} against {letter}",
    rect(3).height(),
  );

  let track = child(4);
  let word = only_child(&track);
  let bounds = word.get_bounding_client_rect();

  assert!(
    (bounds.width() - 40.).abs() < 0.5,
    "the word's line is its track's width: {}",
    bounds.width(),
  );
  assert!(
    bounds.height() > letter * 2.5,
    "the word breaks onto several lines: {} against {letter}",
    bounds.height(),
  );
  assert!(
    word.scroll_width() <= 40,
    "no line is wider than the track: {}",
    word.scroll_width(),
  );
  assert!(
    (rect(5).height() - letter).abs() < 0.5,
    "an empty text is as tall as a letter's line: {} against {letter}",
    rect(5).height(),
  );
  assert!(
    (rect(6).height() - letter).abs() < 0.5,
    "a derived text is as tall as a letter's line: {} against {letter}",
    rect(6).height(),
  );

  for (index, blanks) in [(7, "leading"), (8, "trailing")] {
    assert!(
      rect(index).width() > rect(0).width() + 1.,
      "the {blanks} blanks take their room: {} against {}",
      rect(index).width(),
      rect(0).width(),
    );
  }

  let hung = only_child(&child(9)).get_bounding_client_rect();

  assert!(
    (hung.height() - rect(3).height()).abs() < 0.5,
    "the blanks ending a wrapped line hang past it, the text two lines: {} against {}",
    hung.height(),
    rect(3).height(),
  );

  shown.set(String::new());
  web::end_turn();

  assert_eq!(child(6).text_content().as_deref(), Some(""));
  assert!(
    (rect(6).height() - letter).abs() < 0.5,
    "a derived text emptied keeps its line: {} against {letter}",
    rect(6).height(),
  );
}

/// A derived property writes its slot on change, and the class reading the slot
/// applies it.
#[wasm_bindgen_test]
fn derived_style_updates_its_slot_on_change() {
  let (_guard, root) = fresh_root();
  let size = Signal::new(12.0f32);

  let _ = mount(move || Text::new("t").derive_font_size(move || size.get()).build());

  let element = only_child(&root);

  assert_eq!(computed(&element, "font-size"), "12px");

  size.set(20.0);
  web::end_turn();

  assert_eq!(computed(&element, "font-size"), "20px");
}

/// The typography's link ink reaches a link and follows the palette as it changes.
#[wasm_bindgen_test]
fn typography_link_ink_reaches_a_link_and_updates() {
  let (_guard, root) = fresh_root();
  let link_ink = Signal::new(Color::srgb(1., 0., 0.));

  let _ = mount(move || {
    Theme::derive(move || Typography::default().set_link_ink(link_ink.get()))
      .set_child_builder(|| Text::new("go").set_link("https://example.com/"))
      .build()
  });

  let scope = only_child(&root);
  let link = only_child(&scope);

  assert_eq!(link.tag_name(), "A");
  assert_eq!(
    computed(&link, "color"),
    probe(&scope, &css(Color::srgb(1., 0., 0.)))
  );

  link_ink.set(Color::srgb(0., 0., 1.));
  web::end_turn();

  assert_eq!(
    computed(&link, "color"),
    probe(&scope, &css(Color::srgb(0., 0., 1.)))
  );
}

/// A palette that switches from inline definitions back to conditional ones takes effect.
#[wasm_bindgen_test]
fn a_conditional_palette_after_an_unconditional_one_takes_effect() {
  let (_guard, root) = fresh_root();
  let conditional = Signal::new(true);
  let green = Color::srgb(0., 1., 0.);
  let blue = Color::srgb(0., 0., 1.);

  let _ = mount(move || {
    Theme::derive(move || {
      let ink = if conditional.get() {
        Definition::from(Color::srgb(1., 0., 0.)).when(Media::min_width(1), green)
      }
      else {
        Definition::from(blue)
      };

      Typography::default()
        .set_ink(ink)
        .set_link_ink(blue)
        .set_selection_ink(blue)
        .set_selection_surface(blue)
    })
    .set_child_builder(|| Text::new("ink"))
    .build()
  });

  let scope = only_child(&root);
  let text = only_child(&scope);

  assert_eq!(computed(&text, "color"), probe(&scope, &css(green)));

  conditional.set(false);
  web::end_turn();

  assert_eq!(computed(&text, "color"), probe(&scope, &css(blue)));

  conditional.set(true);
  web::end_turn();

  assert_eq!(computed(&text, "color"), probe(&scope, &css(green)));
}

/// A typography theme sizes, weights, spaces and leads the text beneath it, and a text
/// setting its own size keeps it.
#[wasm_bindgen_test]
fn a_typography_theme_reaches_the_text_beneath_it() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Theme::new(
      Typography::default()
        .set_size(20.)
        .set_weight(700.)
        .set_line_height(2.)
        .set_letter_spacing(1),
    )
    .set_child_builder(|| {
      Column::new().set_children((Text::new("themed"), Text::new("own").set_font_size(12.)))
    })
    .build()
  });

  let scope = only_child(&root);
  let column = only_child(&scope);
  let themed = column.first_element_child().expect("the themed text");
  let own = column.last_element_child().expect("the sized text");

  assert_eq!(computed(&themed, "font-size"), "20px");
  assert_eq!(computed(&themed, "font-weight"), "700");
  assert_eq!(computed(&themed, "line-height"), "40px");
  assert_eq!(computed(&themed, "letter-spacing"), "1px");
  assert_eq!(computed(&own, "font-size"), "12px");
}

/// A text entry's value is inked and weighted by the typography in force, as a run is.
#[wasm_bindgen_test]
fn a_text_entry_takes_the_typography_ink_and_weight() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Theme::new(
      Typography::default()
        .set_ink(Color::srgb(1., 0., 0.))
        .set_weight(700.),
    )
    .set_child_builder(|| BasicTextField::new(TextEntry::new().set_value("hi")))
    .build()
  });

  let input = root
    .query_selector("input")
    .expect("a valid selector")
    .expect("the entry's input");

  assert_eq!(
    computed(&input, "color"),
    probe(&root, &css(Color::srgb(1., 0., 0.)))
  );
  assert_eq!(computed(&input, "font-weight"), "700");
}

/// The link ink never reaches a run that is not a link.
#[wasm_bindgen_test]
fn typography_link_ink_skips_non_link_text() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Theme::new(Typography::default().set_link_ink(Color::srgb(1., 0., 0.)))
      .set_child_builder(|| Text::new("plain"))
      .build()
  });

  let scope = only_child(&root);
  let text = only_child(&scope);

  assert_eq!(text.tag_name(), "SPAN");
  assert_eq!(
    computed(&text, "color"),
    probe(&scope, "var(--typography-ink)")
  );
  assert_ne!(
    computed(&text, "color"),
    probe(&scope, &css(Color::srgb(1., 0., 0.)))
  );
}

#[wasm_bindgen_test]
fn container_is_a_single_cell_grid() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| Container::new().build());

  let element = only_child(&root);

  assert_eq!(computed(&element, "display"), "grid");

  // One track each way. The resolved size follows the viewport; the count does not.
  let tracks = |axis| computed(&element, axis).split_whitespace().count();

  assert_eq!(tracks("grid-template-columns"), 1);
  assert_eq!(tracks("grid-template-rows"), 1);
}

#[wasm_bindgen_test]
fn column_container_is_a_flex_column() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| Column::new().build());

  let element = only_child(&root);

  assert_eq!(computed(&element, "display"), "flex");
  assert_eq!(computed(&element, "flex-direction"), "column");
}

#[wasm_bindgen_test]
fn grid_container_is_a_grid() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| Grid::uniform(2).build());

  assert_eq!(computed(&only_child(&root), "display"), "grid");
}

/// A child placed by an area's name lands on the area's tracks, and an adaptive grid fits
/// as many columns as its minimum allows.
#[wasm_bindgen_test]
fn named_areas_and_adaptive_tracks_lay_out() {
  const HEAD: GridName = GridName::new("head");
  const MAIN: GridName = GridName::new("main");

  let (_guard, root) = fresh_root();
  let cell = || Container::new().set_height(10);

  let _ = mount(move || {
    Column::new()
      .set_children((
        Container::new().set_width(300).set_child(
          Grid::new()
            .set_columns(vec![GridTrack::length(100), GridTrack::Flex(1.)])
            .set_areas(GridAreas::new(["head head", "nav main"]))
            .set_children((
              GridCell::new()
                .set_row(MAIN)
                .set_column(MAIN)
                .set_child(cell()),
              GridCell::new()
                .set_row(HEAD)
                .set_column(HEAD)
                .set_child(cell()),
            )),
        ),
        Container::new()
          .set_width(250)
          .set_child(Grid::adaptive(80).set_children((cell(), cell(), cell(), cell()))),
      ))
      .build()
  });

  let column = only_child(&root);
  let areas = column
    .first_element_child()
    .and_then(|container| container.first_element_child())
    .expect("the areas' grid");
  let adaptive = column
    .last_element_child()
    .and_then(|container| container.first_element_child())
    .expect("the adaptive grid");
  let rect = |element: &Element| element.get_bounding_client_rect();
  let origin = rect(&areas);
  let (main, head) = (
    rect(&areas.children().item(0).expect("main")),
    rect(&areas.children().item(1).expect("head")),
  );

  assert!((main.left() - origin.left() - 100.).abs() < 0.5);
  assert!((head.width() - 300.).abs() < 0.5);
  assert!(main.top() > head.top());

  let (first, fourth) = (
    rect(&adaptive.children().item(0).expect("first")),
    rect(&adaptive.children().item(3).expect("fourth")),
  );

  assert!((first.width() - 250. / 3.).abs() < 0.5);
  assert!(fourth.top() > first.top());
}

/// A size or a padding below zero under a condition that holds lays out as one below
/// zero in the base does, as the terminal lays it out, never as a variant CSS drops
/// for the base's: the box sizes to its content, and the padding's negative edge
/// takes no room.
#[wasm_bindgen_test]
fn a_negative_variant_lays_out_as_a_negative_base() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Row::new()
      .set_item_alignment(ItemAlignment::Start)
      .set_children((
        Container::new()
          .set_width(Dimension::from(100).when(Media::min_width(1), -4))
          .set_child(Text::new("ab")),
        Container::new().set_child(Text::new("ab")),
        Container::new()
          .set_padding(Inset::<f32>::all(20).when(Media::min_width(1), Inset::new(-1, 0, 4, 0)))
          .set_child(Text::new("p")),
        Container::new().set_child(Text::new("p")),
      ))
      .build()
  });

  let row = only_child(&root);
  let rect = |index| {
    row
      .children()
      .item(index)
      .expect("a child")
      .get_bounding_client_rect()
  };

  assert!(
    (rect(0).width() - rect(1).width()).abs() < 0.5,
    "the negative width sizes the box to its content: {} against {}",
    rect(0).width(),
    rect(1).width(),
  );
  assert!(
    (rect(2).width() - rect(3).width() - 4.).abs() < 0.5,
    "the padding's start takes no room and its end four: {} against {}",
    rect(2).width(),
    rect(3).width(),
  );
  assert!(
    (rect(2).height() - rect(3).height()).abs() < 0.5,
    "the padding's top and bottom are the variant's: {} against {}",
    rect(2).height(),
    rect(3).height(),
  );
}

/// A child placed between named lines lands on the tracks between them.
#[wasm_bindgen_test]
fn named_lines_place_a_child() {
  const CONTENT: GridName = GridName::new("content");
  const END: GridName = GridName::new("end");

  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Grid::new()
      .set_columns(
        GridTemplate::new()
          .add_track(100)
          .add_line(CONTENT)
          .add_track(100)
          .add_line(END)
          .add_track(100),
      )
      .set_children((GridCell::new()
        .set_column(Line {
          start: GridPlacement::Named(CONTENT, 1),
          end: GridPlacement::Named(END, 1),
        })
        .set_child(Container::new().set_height(10)),))
      .build()
  });

  let grid = only_child(&root);
  let child = only_child(&grid);
  let (origin, placed) = (
    grid.get_bounding_client_rect(),
    child.get_bounding_client_rect(),
  );

  assert!((placed.left() - origin.left() - 100.).abs() < 0.5);
  assert!((placed.width() - 100.).abs() < 0.5);
}

/// A node a press's pointer drags takes a touch along the axes it drags and leaves the
/// others to the page's pan: a slider along its axis, a box along those it declares; a
/// checkbox, which drags nothing, is left to the page.
#[wasm_bindgen_test]
fn a_dragging_node_takes_a_touch_along_its_axes() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Column::new()
      .set_children((
        BasicSlider::new(Slider::new()),
        BasicSlider::new(Slider::new().set_axis(Axis::Vertical)),
        BasicCheckbox::new(Toggle::new()),
        Container::new().set_drag_axes(DragAxes::Both),
      ))
      .build()
  });

  let column = only_child(&root);
  let child = |index| -> Element { column.children().item(index).expect("a child") };
  let drags = |index| child(index).get_attribute("data-skene-drags");

  assert_eq!(drags(0).as_deref(), Some("horizontal"));
  assert_eq!(computed(&child(0), "touch-action"), "pan-y pinch-zoom");
  assert_eq!(drags(1).as_deref(), Some("vertical"));
  assert_eq!(computed(&child(1), "touch-action"), "pan-x pinch-zoom");
  assert_eq!(drags(2), None);
  assert_eq!(computed(&child(2), "touch-action"), "auto");
  assert_eq!(drags(3).as_deref(), Some("both"));
  assert_eq!(computed(&child(3), "touch-action"), "pinch-zoom");
}

/// `empty` renders no element at all: an empty segment, its two anchors alone.
#[wasm_bindgen_test]
fn empty_renders_no_element() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| empty().build());

  assert_eq!(root.children().length(), 0);
  assert_eq!(root.child_nodes().length(), 2);
}

/// The escape hatch: a view that builds a node and then reaches its real element
/// through the platform, no trait involved.
#[wasm_bindgen_test]
fn a_low_level_view_reaches_the_platform_directly() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    AnyView::new(|| {
      let node = Container::new().set_role(Role::Region).build();
      let element = web::element(node).expect("the realized element");

      element.set_text_content(Some("raw"));

      node
    })
    .build()
  });

  let element = only_child(&root);

  assert_eq!(element.tag_name(), "SECTION");
  assert_eq!(element.text_content().as_deref(), Some("raw"));
}

#[wasm_bindgen_test]
fn reconcile_reorders_the_real_dom_preserving_nodes() {
  let (_guard, root) = fresh_root();
  let items = Signal::new(vec![1, 2, 3]);

  let _ = mount(move || {
    Column::new()
      .derive_children(
        move || items.get(),
        |item: &i32| *item,
        |item| Text::new(item.to_string()),
      )
      .build()
  });

  let parent = only_child(&root);

  assert_eq!(child_texts(&parent), ["1", "2", "3"]);

  // Mark the "2" node so a rebuild (rather than a move) would drop the marker.
  parent
    .children()
    .item(1)
    .unwrap()
    .set_attribute("data-keep", "yes")
    .unwrap();

  items.set(vec![3, 1, 2]);
  web::end_turn();

  assert_eq!(child_texts(&parent), ["3", "1", "2"]);

  let moved = parent.children().item(2).unwrap();

  assert_eq!(moved.text_content().as_deref(), Some("2"));
  assert_eq!(moved.get_attribute("data-keep").as_deref(), Some("yes"));
}

/// The primitive a library user reaches for: swap between two whole trees on a state
/// change, in place between the subtree's anchors.
#[wasm_bindgen_test]
fn derived_view_swaps_between_trees_on_state_change() {
  let (_guard, root) = fresh_root();
  let logged_in = Signal::new(false);

  let _ = mount(move || {
    derived(move || {
      if logged_in.get() {
        Text::new("welcome").build()
      }
      else {
        Container::new().set_role(Role::Navigation).build()
      }
    })
  });

  assert_eq!(only_child(&root).tag_name(), "NAV");

  logged_in.set(true);
  web::end_turn();

  let swapped = only_child(&root);

  assert_eq!(root.children().length(), 1, "the old tree is torn down");
  assert_eq!(swapped.tag_name(), "SPAN");
  assert_eq!(swapped.text_content().as_deref(), Some("welcome"));
}

/// A scrollable box holding a focused box, with the scroll offset set after the focus,
/// which would otherwise scroll it.
fn scroller() -> Container {
  Container::new()
    .set_height(40)
    .set_overflow(skene_foundation::Overflow::ScrollIfNeeded)
    .set_child(
      Column::new().set_children((
        Container::new()
          .set_min_height(20)
          .set_focus_order(skene_web::FocusOrder::Auto),
        Container::new().set_min_height(200),
      )),
    )
}

/// Focuses the focusable box inside `scroller` and scrolls it by 30 pixels.
fn focus_and_scroll(scroller: &Element) -> Element {
  let focusable = scroller
    .query_selector("[tabindex]")
    .expect("query")
    .expect("the focusable box");

  html(&focusable).focus().expect("focus the box");
  scroller.set_scroll_top(30);

  assert_eq!(scroller.scroll_top(), 30);
  assert_eq!(document().active_element(), Some(focusable.clone()));

  focusable
}

/// A hidden subtree keeps its scroll offsets and the focus inside it, and gets both back
/// when it shows again.
#[wasm_bindgen_test]
fn a_hidden_subtree_keeps_its_scroll_and_focus() {
  let (_guard, root) = fresh_root();
  let shown = Signal::new(true);

  let _ = mount(move || {
    Visibility::new()
      .derive_shown(move || shown.get())
      .set_child(scroller())
      .build()
  });

  let scroller = only_child(&root);
  let focusable = focus_and_scroll(&scroller);

  shown.set(false);
  web::end_turn();

  assert!(!scroller.is_connected(), "the subtree left the document");

  shown.set(true);
  web::end_turn();

  assert_eq!(only_child(&root), scroller, "the same subtree returned");
  assert_eq!(scroller.scroll_top(), 30, "the offset returned with it");
  assert_eq!(
    document().active_element(),
    Some(focusable),
    "the focus returned with it"
  );
}

/// A switch shows the case its key names, keeps a case it hides and shows that same
/// case again, built once, and shows nothing while its key names no case.
#[wasm_bindgen_test]
fn a_switch_shows_the_case_its_key_names() {
  let (_guard, root) = fresh_root();
  let key = Signal::new("first");
  let builds = Rc::new(Cell::new(0));

  let _ = mount({
    let builds = Rc::clone(&builds);

    move || {
      Container::new()
        .set_child(
          OneOf::new()
            .derive_key(move || key.get())
            .add_case("first", move || {
              builds.set(builds.get() + 1);

              Text::new("first")
            })
            .add_case("second", || Text::new("second")),
        )
        .build()
    }
  });

  let holder = only_child(&root);
  let first = holder.first_element_child().expect("the first case");

  assert_eq!(holder.text_content().as_deref(), Some("first"));

  key.set("second");
  web::end_turn();

  assert_eq!(holder.text_content().as_deref(), Some("second"));
  assert!(!first.is_connected(), "the hidden case left the document");

  key.set("none");
  web::end_turn();

  assert_eq!(holder.text_content().as_deref(), Some(""));

  key.set("first");
  web::end_turn();

  assert_eq!(
    holder.first_element_child(),
    Some(first),
    "the same case returned"
  );
  assert_eq!(builds.get(), 1, "a case builds once");
}

/// A rebuilt subtree of the same shape starts afresh: what the content that left held
/// leaves with it.
#[wasm_bindgen_test]
fn a_derived_rebuild_keeps_nothing() {
  let (_guard, root) = fresh_root();
  let generation = Signal::new(0);

  let _ = mount(move || {
    derived(move || {
      let _ = generation.get();

      scroller().build()
    })
  });

  let scroller = only_child(&root);
  let focusable = focus_and_scroll(&scroller);

  generation.set(1);
  web::end_turn();

  let rebuilt = only_child(&root);

  assert!(rebuilt != scroller, "the subtree was rebuilt");
  assert_eq!(rebuilt.scroll_top(), 0, "no offset carried over");
  assert!(
    document().active_element() != Some(focusable),
    "no focus carried over"
  );
}

/// A reordered list moves its rows in place: a focused row keeps its focus.
#[wasm_bindgen_test]
fn a_moved_row_keeps_its_focus() {
  let (_guard, root) = fresh_root();
  let items = Signal::new(vec![1, 2, 3]);

  let _ = mount(move || {
    Column::new()
      .derive_children(
        move || items.get(),
        |item: &i32| *item,
        |item| {
          Container::new()
            .set_focus_order(skene_web::FocusOrder::Auto)
            .set_child(Text::new(item.to_string()))
        },
      )
      .build()
  });

  let parent = only_child(&root);
  let first = parent.children().item(0).expect("the first row");

  html(&first).focus().expect("focus the first row");
  items.set(vec![3, 2, 1]);
  web::end_turn();

  assert_eq!(child_texts(&parent), ["3", "2", "1"]);
  assert_eq!(parent.children().item(2), Some(first.clone()));
  assert_eq!(
    document().active_element(),
    Some(first),
    "the moved row kept its focus"
  );
}

/// Only `items` decides when the list is diffed: a signal an item happens to read while
/// building must not subscribe the reconcile effect and re-run the whole diff.
#[wasm_bindgen_test]
fn an_items_own_signal_does_not_re_run_the_list_diff() {
  let (_guard, root) = fresh_root();
  let items = Signal::new(vec![1, 2]);
  let extra = Signal::new(0u32);
  let runs = Rc::new(Cell::new(0u32));

  let _ = mount({
    let runs = Rc::clone(&runs);

    move || {
      Column::new()
        .derive_children(
          {
            let runs = Rc::clone(&runs);

            move || {
              runs.set(runs.get() + 1);

              items.get()
            }
          },
          |item: &i32| *item,
          // Read a signal directly while building this item.
          move |item| Text::new(format!("{item}-{}", extra.get())),
        )
        .build()
    }
  });

  assert_eq!(runs.get(), 1, "one diff on mount");

  extra.set(9);
  web::end_turn();

  assert_eq!(
    runs.get(),
    1,
    "an item's own signal must not re-run the list diff"
  );

  // The list itself still drives the diff.
  items.set(vec![1, 2, 3]);
  web::end_turn();

  assert_eq!(runs.get(), 2, "changing the list re-runs the diff");
  assert_eq!(child_texts(&only_child(&root)).len(), 3);
}

/// The flip side: untracking an item's build must not muzzle the item's *own* effects. A
/// `derive_` property inside a reconciled item still tracks and updates.
#[wasm_bindgen_test]
fn a_derived_property_inside_an_item_still_updates() {
  let (_guard, root) = fresh_root();
  let items = Signal::new(vec![1]);
  let size = Signal::new(12.0f32);

  let _ = mount(move || {
    Column::new()
      .derive_children(
        move || items.get(),
        |item: &i32| *item,
        move |item| Text::new(item.to_string()).derive_font_size(move || size.get()),
      )
      .build()
  });

  let item = only_child(&only_child(&root));

  assert_eq!(computed(&item, "font-size"), "12px");

  size.set(24.0);
  web::end_turn();

  assert_eq!(computed(&item, "font-size"), "24px");
}

/// Selection is opt-in: a plain run computes `user-select: none` through the reset,
/// and `set_selectable(true)` restores it.
#[wasm_bindgen_test]
fn unselectable_text_computes_user_select_none() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Column::new()
      .set_children((
        Text::new("content").set_selectable(true),
        Text::new("chrome"),
      ))
      .build()
  });

  let children = only_child(&root).children();
  let run = |index: u32| computed(&children.item(index).expect("a run"), "user-select");

  // The reset turns selection off: a plain run computes `auto`, which resolves to the
  // body's `none` at use time, or the body's `none` itself where the engine inherits
  // it, as Chromium does; the opt-in overrides it in place.
  assert_eq!(
    computed(&document().body().expect("a body").into(), "user-select"),
    "none"
  );
  assert_eq!(run(0), "text");
  assert!(matches!(run(1).as_str(), "auto" | "none"), "{}", run(1));
}

/// A link is inked in the link ink and underlined, while the base ink never colors it.
#[wasm_bindgen_test]
fn links_are_inked_in_the_link_ink_and_underlined() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Theme::new(Typography::default().set_ink(Color::srgb(0.9, 0.9, 0.9)))
      .set_child_builder(|| Text::new("docs").set_link("https://example.com"))
      .build()
  });

  let scope = only_child(&root);
  let link = only_child(&scope);

  assert_eq!(
    computed(&link, "color"),
    probe(&scope, "var(--typography-link-ink)")
  );
  assert_ne!(
    computed(&link, "color"),
    probe(&scope, &css(Color::srgb(0.9, 0.9, 0.9)))
  );
  assert_eq!(computed(&link, "text-decoration-line"), "underline");
}

#[wasm_bindgen_test]
fn a_widget_role_and_state_reach_the_real_dom() {
  use skene_foundation::AccessProp;

  let (_guard, root) = fresh_root();
  let checked = Signal::new(false);

  let _ = mount(move || {
    Container::new()
      .set_role(Role::CheckBox)
      .derive_accessibility(move || vec![AccessProp::Checked(checked.get().into())])
      .build()
  });

  let element = only_child(&root);

  assert_eq!(element.tag_name(), "DIV");
  assert_eq!(element.get_attribute("role").as_deref(), Some("checkbox"));
  assert_eq!(
    element.get_attribute("aria-checked").as_deref(),
    Some("false"),
  );

  checked.set(true);
  web::end_turn();

  assert_eq!(
    element.get_attribute("aria-checked").as_deref(),
    Some("true"),
    "the attribute follows the signal in the live DOM",
  );
}

/// A checkbox is a label over the browser's own input, which carries its focus, its
/// accessible name and its state; a real click on it flips the bound signal.
#[wasm_bindgen_test]
fn a_checkbox_is_a_label_over_a_native_input() {
  let (_guard, root) = fresh_root();
  let checked = Signal::new(false);

  let _ = mount(move || {
    BasicCheckbox::new(Toggle::new().bind_checked(checked))
      .set_label("Subscribed")
      .build()
  });

  let label = only_child(&root);
  let input = only_child(&label);

  assert_eq!(label.tag_name(), "LABEL");
  assert_eq!(input.tag_name(), "INPUT");
  assert_eq!(input.get_attribute("type").as_deref(), Some("checkbox"));
  assert_eq!(input.get_attribute("tabindex").as_deref(), Some("0"));
  assert_eq!(
    input.get_attribute("aria-label").as_deref(),
    Some("Subscribed"),
  );

  html(&input).click();

  assert!(checked.get(), "a real click toggles it");
  assert!(
    input
      .dyn_ref::<HtmlInputElement>()
      .expect("an input")
      .checked()
  );
}

/// A radio group's arrow traversal follows document order, including a member
/// reconciled into the middle of the set after the group was built: real reconcile,
/// real elements and a real `keydown`, so the order the platform reports is checked
/// against the order the document renders.
#[wasm_bindgen_test]
fn a_radio_group_traverses_in_document_order() {
  let (_guard, root) = fresh_root();
  let flavors = Signal::new(vec![String::from("mint"), String::from("fig")]);
  let chosen = Signal::new(String::from("mint"));

  let _ = mount(move || {
    BasicRadioGroup::new()
      .bind_chosen(chosen)
      .set_child(Column::new().derive_children(
        move || flavors.get(),
        |flavor: &String| flavor.clone(),
        |flavor| BasicRadio::new(Radio::new(flavor.clone())).set_label(flavor),
      ))
      .build()
  });

  let column = only_child(&only_child(&root));
  let rows = || {
    let children = column.children();

    (0..children.length())
      .filter_map(|index| children.item(index))
      .collect::<Vec<_>>()
  };
  let press = |target: &Element, key: &str| {
    let init = web_sys::KeyboardEventInit::new();

    init.set_bubbles(true);
    init.set_key(key);

    let event =
      web_sys::KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).expect("a key");

    let _ = target.dispatch_event(&event);
  };

  assert_eq!(rows().len(), 2);

  press(&rows()[0], "ArrowDown");

  assert_eq!(chosen.get(), "fig");

  chosen.set(String::from("mint"));
  flavors.set(vec![
    String::from("mint"),
    String::from("plum"),
    String::from("fig"),
  ]);

  web::end_turn();

  assert_eq!(rows().len(), 3, "the newcomer is in the document");
  assert_eq!(
    only_child(&rows()[1]).get_attribute("type").as_deref(),
    Some("radio"),
    "and it is the middle one",
  );

  press(&rows()[0], "ArrowDown");

  assert_eq!(
    chosen.get(),
    "plum",
    "traversal follows the document, not the order members joined",
  );
}

/// A radio group's radios share one `name`, so a click on one unchecks the chosen one
/// natively; a choice its handler refuses checks the chosen one again.
#[wasm_bindgen_test]
fn a_refused_radio_choice_keeps_the_chosen_one_checked() {
  let (_guard, root) = fresh_root();
  let asked = Rc::new(RefCell::new(Vec::new()));

  let _ = mount({
    let asked = Rc::clone(&asked);

    move || {
      BasicRadioGroup::new()
        .derive_chosen(|| "mint")
        .handle_choose(move |value| asked.borrow_mut().push(value))
        .set_child(
          Column::new()
            .set_children(["mint", "fig"].map(|flavor| BasicRadio::new(Radio::new(flavor)))),
        )
        .build()
    }
  });

  let radios = root
    .query_selector_all("input[type=radio]")
    .expect("query the radios");
  let radio = |index: u32| -> HtmlInputElement {
    radios
      .item(index)
      .and_then(|node| node.dyn_into().ok())
      .expect("a radio")
  };
  let name = radio(0).name();

  assert!(!name.is_empty(), "the radios are named");
  assert_eq!(radio(1).name(), name, "after their group");
  assert!(radio(0).checked());

  radio(1).click();
  web::end_turn();

  assert_eq!(*asked.borrow(), ["fig"], "the choice was asked for");
  assert!(radio(0).checked(), "the chosen radio is checked again");
  assert!(!radio(1).checked(), "the refused one is not");
}

/// Every page of the routed test app.
#[derive(Clone, PartialEq)]
enum Page {
  Home,
  About,
}

impl skene_web::router::Route for Page {
  fn parse(path: &str) -> Option<Self> {
    match path {
      "/about" => Some(Self::About),
      _ => Some(Self::Home),
    }
  }

  fn to_path(&self) -> String {
    match self {
      Self::Home => "/".to_owned(),
      Self::About => "/about".to_owned(),
    }
  }
}

/// With the opt-in, a route change commits inside `document.startViewTransition`, and
/// the new route is in the DOM once the transition's callback ran.
#[wasm_bindgen_test]
fn a_route_change_commits_under_a_view_transition() {
  use skene_web::{install_router, router::Router, view_transitions};
  use wasm_bindgen::{JsValue, closure::Closure};

  let (_guard, root) = fresh_root();
  let calls = Rc::new(Cell::new(0u32));
  let counted = Rc::clone(&calls);
  let stub = Closure::wrap(Box::new(move |callback: js_sys::Function| {
    counted.set(counted.get() + 1);

    let _ = callback.call0(&JsValue::NULL);
  }) as Box<dyn FnMut(js_sys::Function)>);
  let document: JsValue = document().into();
  let key = JsValue::from_str("startViewTransition");
  let had_native = js_sys::Reflect::has(&document, &key).unwrap_or(false);

  js_sys::Reflect::set(&document, &key, stub.as_ref()).expect("stub the transition");

  let router: Rc<RefCell<Option<Router<Page>>>> = Rc::new(RefCell::new(None));
  let held = Rc::clone(&router);
  let _ = mount(move || {
    view_transitions::enable();

    let installed = install_router(Page::Home);

    *held.borrow_mut() = Some(installed);

    derived(move || match installed.route() {
      Page::Home => Text::new("home page").build(),
      Page::About => Text::new("about page").build(),
    })
  });

  assert!(
    root
      .text_content()
      .unwrap_or_default()
      .contains("home page")
  );

  router
    .borrow()
    .as_ref()
    .expect("a router")
    .navigate(Page::About);

  assert_eq!(calls.get(), 1, "the navigation ran under the transition");
  assert!(
    root
      .text_content()
      .unwrap_or_default()
      .contains("about page")
  );

  let _ = router.borrow().as_ref().expect("a router").apply_path("/");

  assert_eq!(calls.get(), 2, "an applied path commits the same way");
  web::push_path("/");

  if !had_native {
    let _ = js_sys::Reflect::delete_property(&document.unchecked_into::<js_sys::Object>(), &key);
  }

  drop(stub);
}

/// A keyframe animation whose frames hold a token applies in the real engine, its
/// keyframes referencing the token's variable.
#[wasm_bindgen_test]
fn a_token_keyframe_animation_applies() {
  use skene_foundation::{Animation, BoxStyle, Keyframe};
  use skene_web::WebStyle;

  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Theme::new(SimplePalette::default())
      .set_child_builder(|| {
        let mut start: BoxStyle<f32, Keyframe, WebStyle> = BoxStyle::default();
        let mut end: BoxStyle<f32, Keyframe, WebStyle> = BoxStyle::default();

        start.background = Some(SimplePalette::CARD.into());
        end.background = Some(Color::srgb(1., 0., 0.).into());

        Container::new().add_animation(
          Animation::new(2., [(0., start), (100., end)]).set_respects_reduced_motion(false),
        )
      })
      .build()
  });

  let scope = only_child(&root);
  let element = only_child(&scope);

  assert!(
    computed(&element, "animation-name").starts_with("skene-k-"),
    "{}",
    computed(&element, "animation-name")
  );

  let sheets = document()
    .query_selector_all("style[data-skene-style]")
    .expect("the style blocks");
  let mut text = String::new();

  for index in 0..sheets.length() {
    text.push_str(
      &sheets
        .item(index)
        .and_then(|node| node.text_content())
        .unwrap_or_default(),
    );
  }

  assert!(
    text.contains("0%{background-color:var(--simple-card);}"),
    "{text}"
  );
}

/// Focus and scroll offsets ride the state snapshot: restored into a fresh install of
/// the same tree, the same field holds focus and the same box holds its offset.
#[wasm_bindgen_test]
fn focus_and_scroll_survive_a_snapshot_roundtrip() {
  use skene_foundation::Overflow;
  use skene_web::{BasicTextField, NodeId, behavior::TextEntry, reactive};

  fn build() -> NodeId {
    Column::new()
      .set_children((
        Container::new()
          .set_height(40)
          .set_overflow(Overflow::Scroll)
          .set_child(Container::new().set_height(400)),
        BasicTextField::new(TextEntry::new()),
      ))
      .build()
  }

  let bytes = {
    let (_installed, root) = fresh_root();

    reactive::record_snapshots();

    let _ = mount(build);
    let column = only_child(&root);
    let scroller = column.first_element_child().expect("the scroller");
    let field = column
      .query_selector("input")
      .expect("query")
      .expect("the field");

    scroller.set_scroll_top(120);
    html(&field).focus().expect("focus the field");

    assert_eq!(document().active_element().as_ref(), Some(&field));

    reactive::snapshot()
  };
  let (_installed, root) = fresh_root();

  reactive::restore(&bytes);

  let _ = mount(build);

  web::adopt_restoration();

  let column = only_child(&root);
  let scroller = column.first_element_child().expect("the scroller");
  let field = column
    .query_selector("input")
    .expect("query")
    .expect("the field");

  assert_eq!(scroller.scroll_top(), 120, "the offset resumed");
  assert_eq!(
    document().active_element().as_ref(),
    Some(&field),
    "focus resumed on the field"
  );
}

/// Focus and the selection follow a keyed item's key: the list reordered on the next
/// run, the same item's field resumes focused with its selection.
#[wasm_bindgen_test]
fn focus_follows_a_keyed_item_across_a_reorder() {
  use skene_web::{BasicTextField, NodeId, behavior::TextEntry, reactive};

  fn build(items: Vec<u8>) -> NodeId {
    Column::new()
      .derive_children(
        move || items.clone(),
        |item| *item,
        |_| BasicTextField::new(TextEntry::new().bind_value(Signal::new(String::from("hello")))),
      )
      .build()
  }

  /// The `index`th field under `root`.
  fn field_at(root: &Element, index: u32) -> HtmlInputElement {
    only_child(root)
      .query_selector_all("input")
      .expect("query")
      .get(index)
      .and_then(|node| node.dyn_into::<HtmlInputElement>().ok())
      .expect("the field")
  }

  let bytes = {
    let (_installed, root) = fresh_root();

    reactive::record_snapshots();

    let _ = mount(|| build(vec![1, 2, 3]));
    let field = field_at(&root, 1);

    field.focus().expect("focus the field");
    field.set_selection_range(1, 3).expect("select");

    assert_eq!(document().active_element().as_deref(), Some(field.as_ref()));

    reactive::snapshot()
  };
  let (_installed, root) = fresh_root();

  reactive::restore(&bytes);

  let _ = mount(|| build(vec![3, 1, 2]));

  web::adopt_restoration();

  let field = field_at(&root, 2);

  assert_eq!(
    document().active_element().as_deref(),
    Some(field.as_ref()),
    "focus resumed on the same item's field at its new place"
  );
  assert_eq!(
    (
      field.selection_start().ok().flatten(),
      field.selection_end().ok().flatten()
    ),
    (Some(1), Some(3)),
    "the selection resumed"
  );
}

/// A text field's caption is a `<label for>` placed before the input among its
/// siblings, naming the input's id, and it leaves with the field.
#[wasm_bindgen_test]
fn a_text_fields_caption_is_a_label_before_the_input() {
  let (_installed, root) = fresh_root();
  let shown = Signal::new(true);
  let _ = mount(move || {
    Container::new()
      .set_child(derived(move || {
        if shown.get() {
          BasicTextField::new(TextEntry::new())
            .set_label("Name")
            .build()
        }
        else {
          empty().build()
        }
      }))
      .build()
  });

  let container = only_child(&root);
  let label = container
    .query_selector("label")
    .expect("query")
    .expect("the caption");
  let input = container
    .query_selector("input")
    .expect("query")
    .expect("the input");

  assert_eq!(
    label.next_element_sibling().as_ref(),
    Some(&input),
    "the label sits right before the input"
  );
  assert_eq!(label.get_attribute("for"), Some(input.id()));
  assert!(!input.id().is_empty(), "the input carries an id");
  assert_eq!(label.text_content().as_deref(), Some("Name"));
  assert_eq!(
    input.child_element_count(),
    0,
    "nothing lands inside the input"
  );

  shown.set(false);
  web::end_turn();

  assert!(
    container.query_selector("label").expect("query").is_none(),
    "the caption left with the field"
  );
  assert!(container.query_selector("input").expect("query").is_none());
}

/// A node placed before a text field whose caption stands beside it lands before the
/// caption's `<label>`, never between the label and its input.
#[wasm_bindgen_test]
fn a_node_placed_before_a_captioned_field_lands_before_its_label() {
  let (_installed, root) = fresh_root();
  let items = Signal::new(vec![0]);
  let _ = mount(move || {
    Column::new()
      .derive_children(
        move || items.get(),
        |item: &i32| *item,
        |item| {
          if item == 0 {
            BasicTextField::new(TextEntry::new())
              .set_label("Name")
              .build()
          }
          else {
            Text::new(item.to_string()).build()
          }
        },
      )
      .build()
  });

  let column = only_child(&root);
  let tags = |parent: &Element| {
    let children = parent.children();

    (0..children.length())
      .map(|index| children.item(index).unwrap().tag_name())
      .collect::<Vec<_>>()
  };

  assert_eq!(tags(&column), ["LABEL", "INPUT"]);

  items.set(vec![1, 0]);
  web::end_turn();

  assert_eq!(
    tags(&column),
    ["SPAN", "LABEL", "INPUT"],
    "the new node stands before the caption"
  );
}

/// A lazy column of estimated items measures each one it builds through the browser's
/// size observation: taller measures shrink the window and grow the trailing spacer.
#[wasm_bindgen_test]
async fn a_lazy_column_measures_its_estimated_items() {
  use skene_web::{ItemExtent, LazyColumn};

  let (_installed, root) = fresh_root();
  let _ = mount(|| {
    Container::new()
      .set_height(50)
      .set_child(
        LazyColumn::new()
          .set_item_height(ItemExtent::Estimated(10.))
          .set_overscan(0)
          .derive_items(
            || 0..100,
            |index| *index,
            |index| {
              Container::new()
                .set_height(20)
                .set_child(Text::new(index.to_string()))
            },
          ),
      )
      .build()
  });

  let list = only_child(&root).first_element_child().expect("the list");
  let items = |list: &Element| list.child_element_count() - 2;

  // The viewport measures first, five items of the estimate build, and their measures
  // arrive next, each through the browser's observer.
  for _ in 0..20 {
    if items(&list) == 3 {
      break;
    }

    next_frame().await;
  }

  assert_eq!(items(&list), 3, "three measured items fill the 50 pixels");

  let trailing = list.last_element_child().expect("the trailing spacer");

  assert_eq!(
    computed(&trailing, "height"),
    "990px",
    "the observer reported all five before the window shrank, so five measures and \
     ninety-five estimates make the spacer"
  );
}

/// A painted box carries a canvas the size of the box, drawn through vello on the GPU:
/// the tier runs each engine with WebGPU on, so the canvas holds a WebGPU context and no
/// 2d one where the engine keeps its device, and the fill shows through it either way.
/// `browser_painted` covers the CPU path.
#[wasm_bindgen_test]
async fn a_painted_box_draws_into_its_canvas() {
  use skene_web::{Painted, kurbo::Rect};
  use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

  /// The canvas's center pixel, read through a scratch 2d canvas so a WebGPU canvas
  /// reads the same as a blitted one.
  fn center_pixel(canvas: &HtmlCanvasElement) -> Vec<u8> {
    let scratch: HtmlCanvasElement = web_sys::window()
      .expect("a window")
      .document()
      .expect("a document")
      .create_element("canvas")
      .expect("a canvas")
      .dyn_into()
      .expect("a canvas element");

    scratch.set_width(canvas.width());
    scratch.set_height(canvas.height());

    let context: CanvasRenderingContext2d = scratch
      .get_context("2d")
      .expect("a context")
      .expect("the 2d context")
      .dyn_into()
      .expect("a 2d context");

    context
      .draw_image_with_html_canvas_element(canvas, 0., 0.)
      .expect("the canvas drawn");
    context
      .get_image_data(
        f64::from(canvas.width() / 2),
        f64::from(canvas.height() / 2),
        1.,
        1.,
      )
      .expect("image data")
      .data()
      .to_vec()
  }

  let (_installed, root) = fresh_root();
  let _ = mount(|| {
    Painted::new(|canvas, size| {
      canvas.fill(
        &Rect::new(0., 0., f64::from(size.width), f64::from(size.height)),
        Color::srgb(0., 0., 1.),
      );
    })
    .set_width(40)
    .set_height(20)
    .build()
  });
  next_frame().await;
  next_frame().await;

  let painted = only_child(&root);
  // The canvas in the box now: a GPU canvas that fails is replaced by a fresh one.
  let current = || -> HtmlCanvasElement {
    painted
      .first_element_child()
      .expect("the canvas")
      .dyn_into()
      .expect("a canvas element")
  };
  let mut canvas = current();
  let mut pixel = center_pixel(&canvas);
  let deadline = js_sys::Date::now() + 60_000.;

  while pixel[0..4] != [0, 0, 255, 255] && js_sys::Date::now() < deadline {
    next_frame().await;
    canvas = current();
    pixel = center_pixel(&canvas);
  }

  assert!(
    canvas.width() >= 40 && canvas.height() >= 20,
    "sized to the box"
  );
  assert_eq!(
    &pixel[0..4],
    &[0, 0, 255, 255],
    "the center pixel is the fill"
  );

  // Headless Chromium's software adapter loses its device after the first frame, and a
  // lost device with no successor paints on the CPU.
  let software = web_sys::window()
    .and_then(|window| window.navigator().user_agent().ok())
    .is_some_and(|agent| agent.contains("HeadlessChrome"));

  if !software {
    assert!(
      matches!(canvas.get_context("2d"), Ok(None)),
      "painted on the GPU, so the canvas holds no 2d context"
    );
  }
}

/// Dispatches a bubbling pointer event of `kind` at `element`.
fn pointer(element: &Element, kind: &str) {
  let init = web_sys::PointerEventInit::new();

  init.set_bubbles(true);

  let event =
    web_sys::PointerEvent::new_with_event_init_dict(kind, &init).expect("a pointer event");

  let _ = element.dispatch_event(&event);
}

/// A hydrated tooltip carries no `title`: the bubble is a hint popover the described
/// box names, opening once the pointer has rested for the delay, staying open while the
/// pointer rests on it, closing the grace period after the pointer leaves both, and
/// opening at once on focus.
#[wasm_bindgen_test]
async fn a_tooltip_opens_on_hover_after_its_delay_and_on_focus_at_once() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Tooltip::new("Saves your work")
      .set_delay(Duration::from_millis(10))
      .set_child(
        Container::new()
          .set_focus_order(skene_web::FocusOrder::Auto)
          .set_child(Text::new("Save")),
      )
      .build()
  });

  let described = root
    .query_selector("[aria-describedby]")
    .expect("query")
    .expect("the described box");
  let bubble = root
    .query_selector("[popover]")
    .expect("query")
    .expect("the bubble");
  let open = || bubble.matches(":popover-open").unwrap_or(false);

  assert_eq!(described.get_attribute("title"), None, "no browser tooltip");
  assert_eq!(
    described.get_attribute("aria-describedby"),
    bubble.get_attribute("id")
  );
  assert_eq!(bubble.get_attribute("popover").as_deref(), Some("hint"));
  assert!(!open());

  pointer(&described, "pointerover");

  assert!(!open(), "the pointer has not rested yet");

  next_frame().await;

  assert!(open(), "the delay passed");

  pointer(&described, "pointerout");

  assert!(open(), "open through the grace period");

  pointer(&bubble, "pointerover");
  wait(TOOLTIP_GRACE * 2).await;

  assert!(open(), "open while the pointer rests on the bubble");

  pointer(&bubble, "pointerout");
  wait(TOOLTIP_GRACE * 2).await;

  assert!(!open(), "the pointer left both");

  html(&described).focus().expect("focus the box");

  assert!(open(), "focus opens at once");

  html(&described).blur().expect("blur the box");

  assert!(!open());
}

/// Dispatches a bubbling, cancelable `contextmenu` from a secondary button at
/// `element`, returning whether the browser's own menu was prevented.
fn context_menu(element: &Element) -> bool {
  let init = web_sys::MouseEventInit::new();

  init.set_bubbles(true);
  init.set_cancelable(true);
  init.set_button(2);
  init.set_client_x(30);
  init.set_client_y(20);

  let event = web_sys::MouseEvent::new_with_mouse_event_init_dict("contextmenu", &init)
    .expect("a mouse event");
  let _ = element.dispatch_event(&event);

  event.default_prevented()
}

/// A context menu is opted into per view: a `contextmenu` on the wrapped view opens
/// the menu's list as a popover pinned at the pointer with the browser's own menu
/// prevented, an item runs and closes it, and a view outside the wrapper keeps the
/// browser's menu.
#[wasm_bindgen_test]
fn a_context_menu_takes_the_browsers_only_where_a_view_opts_in() {
  let (_guard, root) = fresh_root();
  let chosen = Signal::new(String::new());
  let _ = mount(|| {
    Column::new()
      .set_children((
        ContextMenu::new()
          .set_items([
            MenuEntry::new("Rename").handle_activate(move || chosen.set("rename".to_owned())),
            MenuEntry::separator(),
            MenuEntry::new("Delete").handle_activate(move || chosen.set("delete".to_owned())),
          ])
          .set_child(Container::new().set_child(Text::new("note"))),
        Container::new().set_child(Text::new("plain")),
      ))
      .build()
  });
  let column = only_child(&root);
  let served = column.first_element_child().expect("the served box");
  let plain = column.last_element_child().expect("the plain box");
  let list = root
    .query_selector("[popover]")
    .expect("query")
    .expect("the list");
  let open = || list.matches(":popover-open").unwrap_or(false);

  assert!(
    list.query_selector("[role=menu]").expect("query").is_some(),
    "the list holds a menu"
  );
  assert!(!open());
  assert!(
    !context_menu(&plain),
    "a plain box keeps the browser's menu"
  );
  assert!(!open());
  assert!(context_menu(&served), "the wrapped box prevents it");
  assert!(open(), "and opens the list");

  let style = html(&list).style();

  assert_eq!(style.get_property_value("position").as_deref(), Ok("fixed"));
  assert_eq!(style.get_property_value("left").as_deref(), Ok("30px"));
  assert_eq!(style.get_property_value("top").as_deref(), Ok("20px"));

  let items = list.query_selector_all("[role=menuitem]").expect("query");

  assert_eq!(items.length(), 2, "a separator is no item");

  let first: Element = items.item(0).expect("the first item").unchecked_into();
  let second: Element = items.item(1).expect("the second item").unchecked_into();

  assert!(
    document()
      .active_element()
      .is_some_and(|active| active == first),
    "the first item takes focus"
  );

  html(&second).click();

  assert_eq!(chosen.get(), "delete");
  assert!(!open(), "running an item closes the list");
}

/// An app's own rule outranks a compiled one more specific than it, every compiled
/// rule sitting in the `skene.style` layer.
#[wasm_bindgen_test]
fn an_app_rule_outranks_a_compiled_one() {
  let (_installed, root) = fresh_root();
  let sheet = document().create_element("style").expect("a style element");

  sheet.set_text_content(Some("div { background-color: rgb(0, 0, 255); }"));
  document()
    .head()
    .expect("a head")
    .append_child(&sheet)
    .expect("add the app's sheet");

  let _ = mount(|| {
    Container::new()
      .set_background(Color::srgb(1., 0., 0.))
      .build()
  });
  let color = computed(&only_child(&root), "background-color");

  sheet.remove();

  assert_eq!(color, "rgb(0, 0, 255)");
}

/// A pinned checkbox leaves the flow, the reset positioning its label sitting in a layer
/// beneath the compiled rules.
#[wasm_bindgen_test]
fn a_pinned_checkbox_leaves_the_flow() {
  let (_installed, root) = fresh_root();
  let _ = mount(|| {
    Stack::new()
      .set_children((Pinned::new()
        .set_edges(Inset::all(0))
        .set_child(SimpleCheckbox::new().set_label("pinned")),))
      .build()
  });
  let label = root
    .query_selector("label[data-skene-control]")
    .ok()
    .flatten()
    .expect("the checkbox's label");

  assert_eq!(computed(&label, "position"), "absolute");
}

/// The span inside a link group takes the link's ink, as the link's own text does.
#[wasm_bindgen_test]
fn a_link_groups_span_takes_the_link_ink() {
  let (_installed, root) = fresh_root();
  let _ = mount(|| {
    Theme::new(Typography::default().set_link_ink(Color::srgb(1., 0., 0.)))
      .set_child_builder(|| {
        RichText::new().add_spans([Span::group([Span::new("x")]).set_link("/g")])
      })
      .build()
  });
  let link = root.query_selector("a").ok().flatten().expect("the link");
  let span = link
    .first_element_child()
    .expect("the span inside the link");

  let paragraph = link.parent_element().expect("the paragraph");

  assert_ne!(computed(&link, "color"), computed(&paragraph, "color"));
  assert_eq!(computed(&span, "color"), computed(&link, "color"));
}
