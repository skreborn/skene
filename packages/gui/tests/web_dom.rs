//! Real-DOM integration tests for the web realization of the built-in views.
//!
//! Each test installs the web driver rooted at a connected `<div>`, builds a view under
//! it, and asserts against the actual DOM the browser produced. The guard uninstalls
//! the driver and removes the root when it drops, isolating each test.
//!
//! Run in a headless browser:
//! `cargo test -p skene-gui --target wasm32-unknown-unknown --test web_dom`.
#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_gui::{
  AnyView,
  Border,
  Color,
  Column,
  Container,
  Deferred,
  FocusHandle,
  Grid,
  HasChildren,
  Inset,
  RadioMember,
  RawCheckbox,
  RawRadio,
  RawRadioGroup,
  RawTextField,
  Role,
  Roving,
  Row,
  Stack,
  Text,
  TextEntry,
  Theme,
  Toggle,
  Typography,
  View,
  core::PointerButton,
  derived,
  empty,
  head,
  reactive::{Get, Set, Signal},
  wasm_bindgen::JsCast,
  web::{self, mount},
};
use skene_simple_gui::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{Document, Element, HtmlElement, HtmlInputElement};

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> Document {
  web_sys::window()
    .expect("a window")
    .document()
    .expect("a document")
}

/// Uninstalls the driver and removes the root when it drops, isolating each test.
struct Installed(Element);

impl Drop for Installed {
  fn drop(&mut self) {
    web::uninstall();
    self.0.remove();
  }
}

/// Installs the driver rooted at a fresh connected `<div>`, returning the guard (drop
/// it to reset) and that root. Build a view under it with [`mount`], then read the
/// root's subtree; computed styles resolve because the root is connected.
fn fresh_root() -> (Installed, Element) {
  let root = document().create_element("div").expect("create root div");

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect root");
  web::install(root.clone());

  (Installed(root.clone()), root)
}

/// The single view element built directly under the root.
fn only_child(root: &Element) -> Element {
  root.first_element_child().expect("root has a built child")
}

/// A computed style `property` of `element`.
fn computed(element: &Element, property: &str) -> String {
  web_sys::window()
    .expect("a window")
    .get_computed_style(element)
    .expect("computed style")
    .expect("a computed style")
    .get_property_value(property)
    .expect("a property value")
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

/// A color as the driver renders it.
fn css(color: Color) -> String {
  let [lightness, chroma, hue, alpha] = color.to_oklch();

  format!("oklch({lightness} {chroma} {hue} / {alpha})")
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

/// Dispatches a bubbling `name` event on `target`.
fn dispatch(target: &Element, name: &str) {
  let init = web_sys::EventInit::new();

  init.set_bubbles(true);

  let event = web_sys::Event::new_with_event_init_dict(name, &init).expect("an event");
  let _ = target.dispatch_event(&event);
}

/// A `Deferred` is a streaming boundary only on the server; on the client it builds
/// its content directly, between anchors of its own.
#[wasm_bindgen_test]
fn deferred_builds_its_content_on_the_client() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| Deferred::new(|| Text::new("content")).build());

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

#[wasm_bindgen_test]
fn styling_interns_a_generated_class() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Container::new()
      .set_color(Color::srgb(1.0, 0.0, 0.0))
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
      .set_padding(Inset::all(10.0))
      .set_border(Border::new(2.0, Color::srgb(0.0, 0.0, 0.0)))
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

/// A change handler receives the value, in real time, when an input event fires on
/// the text field's native input.
#[wasm_bindgen_test]
fn change_handler_receives_the_committed_value() {
  let (_guard, root) = fresh_root();
  let seen = Rc::new(RefCell::new(String::new()));

  let _ = mount({
    let seen = Rc::clone(&seen);

    move || {
      RawTextField::new(
        TextEntry::new()
          .set_value("committed")
          .handle_change(move |value| value.clone_into(&mut seen.borrow_mut())),
      )
      .build()
    }
  });

  let input = only_child(&only_child(&root));

  assert_eq!(input.tag_name(), "INPUT");
  dispatch(&input, "input");

  assert_eq!(&*seen.borrow(), "committed");
}

/// A checkbox bound to a signal reflects it on its native input and writes toggles
/// back through a real change event.
#[wasm_bindgen_test]
fn checkbox_binding_round_trips() {
  let (_guard, root) = fresh_root();
  let checked = Signal::new(false);

  let _ = mount(move || RawCheckbox::new(Toggle::new().bind_checked(checked)).build());

  let target: HtmlInputElement = only_child(&only_child(&root))
    .dyn_into()
    .expect("an input element");

  assert_eq!(target.get_attribute("type").as_deref(), Some("checkbox"));
  assert!(!target.checked());

  checked.set(true);

  assert!(target.checked());

  target.set_checked(false);
  dispatch(&target, "change");

  assert!(!checked.get());
}

/// Head registrations set the live document title and add each element once.
#[wasm_bindgen_test]
fn head_registrations_apply_once() {
  let (_guard, _root) = fresh_root();

  head::set_title("Live title");
  head::add_meta("live-test", "value");
  head::add_meta("live-test", "value");

  assert_eq!(document().title(), "Live title");

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

/// A focus handle moves real keyboard focus to the native input of its bound control.
#[wasm_bindgen_test]
fn focus_handle_focuses_the_bound_view() {
  let (_guard, root) = fresh_root();
  let handle = FocusHandle::new();

  let _ = mount({
    let handle = handle.clone();

    move || {
      RawTextField::new(TextEntry::new())
        .bind_focus(&handle)
        .build()
    }
  });

  handle.focus();

  let input = only_child(&only_child(&root));

  assert_eq!(document().active_element(), Some(input.clone()));

  handle.blur();

  assert_ne!(document().active_element(), Some(input));
}

/// A handler may re-enter the driver—building nodes from inside one does not deadlock
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

#[wasm_bindgen_test]
fn derived_text_updates_the_dom_on_change() {
  let (_guard, root) = fresh_root();
  let count = Signal::new(0);

  let _ = mount(move || Text::derive(move || count.get().to_string()).build());

  let element = only_child(&root);

  assert_eq!(element.text_content().as_deref(), Some("0"));

  count.set(5);

  assert_eq!(element.text_content().as_deref(), Some("5"));
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

  assert_eq!(
    computed(&link, "color"),
    probe(&scope, &css(Color::srgb(0., 0., 1.)))
  );
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

/// `empty` renders no element at all: an empty segment, its two anchors alone.
#[wasm_bindgen_test]
fn empty_renders_no_element() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| empty().build());

  assert_eq!(root.children().length(), 0);
  assert_eq!(root.child_nodes().length(), 2);
}

/// The escape hatch: a view that builds a node and then reaches its real element
/// through the driver, no trait involved.
#[wasm_bindgen_test]
fn a_low_level_view_reaches_the_platform_directly() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    AnyView::new(|| {
      let node = Container::new().set_role(Role::Section).build();
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

  let swapped = only_child(&root);

  assert_eq!(root.children().length(), 1, "the old tree is torn down");
  assert_eq!(swapped.tag_name(), "SPAN");
  assert_eq!(swapped.text_content().as_deref(), Some("welcome"));
}

/// A scrollable box inside a rebuilt subtree of the same shape: the content that
/// enters the segment takes the scroll offset and the focus the content that left it
/// held.
#[wasm_bindgen_test]
fn a_derived_rebuild_restores_scroll_and_focus() {
  let (_guard, root) = fresh_root();
  let generation = Signal::new(0);

  let _ = mount(move || {
    derived(move || {
      let _ = generation.get();

      Container::new()
        .set_height(40.)
        .set_overflow(skene_gui::core::Overflow::Auto)
        .set_child(
          Column::new().set_children((
            Container::new()
              .set_min_height(20.)
              .set_focus_order(skene_gui::FocusOrder::Auto),
            Container::new().set_min_height(200.),
          )),
        )
        .build()
    })
  });

  let scroller = only_child(&root);
  let focusable = scroller
    .query_selector("[tabindex]")
    .expect("query")
    .expect("the focusable box");

  // Focus first: focusing scrolls the box into view, which would undo the offset.
  html(&focusable).focus().expect("focus the box");
  scroller.set_scroll_top(30);

  assert_eq!(scroller.scroll_top(), 30);
  assert_eq!(document().active_element(), Some(focusable));

  generation.set(1);

  let rebuilt = only_child(&root);
  let refocused = rebuilt
    .query_selector("[tabindex]")
    .expect("query")
    .expect("the rebuilt focusable box");

  assert!(rebuilt != scroller, "the subtree was rebuilt");
  assert_eq!(rebuilt.scroll_top(), 30, "the offset carried over");
  assert_eq!(
    document().active_element(),
    Some(refocused),
    "the focus carried over",
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

  assert_eq!(
    runs.get(),
    1,
    "an item's own signal must not re-run the list diff"
  );

  // The list itself still drives the diff.
  items.set(vec![1, 2, 3]);

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

  // A plain run computes `auto`, which resolves to the body's `none` at use time—the
  // reset is what turns selection off; the opt-in overrides it in place.
  assert_eq!(
    computed(&document().body().expect("a body").into(), "user-select"),
    "none"
  );
  assert_eq!(run(0), "text");
  assert_eq!(run(1), "auto");
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
  use skene_gui::core::AccessProp;

  let (_guard, root) = fresh_root();
  let checked = Signal::new(false);

  let _ = mount(move || {
    Container::new()
      .set_role(Role::Checkbox)
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
    RawCheckbox::new(Toggle::new().bind_checked(checked))
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
/// reconciled into the middle of the set after the group was built.
///
/// The host tier pins the same thing against the headless driver. This pins it against
/// the browser's: real reconcile, real elements, and a real `keydown`, so the order the
/// driver reports is checked against the order the document actually renders.
#[wasm_bindgen_test]
fn a_radio_group_traverses_in_document_order() {
  let (_guard, root) = fresh_root();
  let flavors = Signal::new(vec![String::from("mint"), String::from("fig")]);
  let chosen: Signal<Option<String>> = Signal::new(Some(String::from("mint")));

  let _ = mount(move || {
    RawRadioGroup::new(Roving::new().bind_chosen(chosen))
      .set_child(Column::new().derive_children(
        move || flavors.get(),
        |flavor: &String| flavor.clone(),
        |flavor| RawRadio::new(RadioMember::new(flavor.clone())).set_label(flavor),
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

  assert_eq!(chosen.get().as_deref(), Some("fig"));

  chosen.set(Some(String::from("mint")));
  flavors.set(vec![
    String::from("mint"),
    String::from("plum"),
    String::from("fig"),
  ]);

  assert_eq!(rows().len(), 3, "the newcomer is in the document");
  assert_eq!(
    only_child(&rows()[1]).get_attribute("type").as_deref(),
    Some("radio"),
    "and it is the middle one",
  );

  press(&rows()[0], "ArrowDown");

  assert_eq!(
    chosen.get().as_deref(),
    Some("plum"),
    "traversal follows the document, not the order members joined",
  );
}

/// Every page of the routed test app.
#[derive(Clone, PartialEq)]
enum Page {
  Home,
  About,
}

impl skene_gui::router::Route for Page {
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
  use skene_gui::{install_router, router::Router, view_transitions};
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

    *held.borrow_mut() = Some(installed.clone());

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
  use skene_gui::{
    GuiExt,
    core::{Animation, BoxStyle, Keyframe},
  };

  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Theme::new(SimplePalette::default())
      .set_child_builder(|| {
        let mut start: BoxStyle<f32, Keyframe, GuiExt> = BoxStyle::default();
        let mut end: BoxStyle<f32, Keyframe, GuiExt> = BoxStyle::default();

        start.color = Some(SimplePalette::CARD.into());
        end.color = Some(Color::srgb(1., 0., 0.).into());

        Container::new().set_animation(
          Animation::new(2., [(0., start), (100., end)]).set_respect_reduced_motion(false),
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
  use skene_gui::{
    NodeId,
    RawTextField,
    TextEntry,
    core::{Dimension, Overflow},
    reactive,
  };

  fn build() -> NodeId {
    Column::new()
      .set_children((
        Container::new()
          .set_height(Dimension::Length(40.))
          .set_overflow(Overflow::Scroll)
          .set_child(Container::new().set_height(Dimension::Length(400.))),
        RawTextField::new(TextEntry::new()),
      ))
      .build()
  }

  let bytes = {
    let (_installed, root) = fresh_root();
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
  use skene_gui::{NodeId, RawTextField, TextEntry, reactive};

  fn build(items: Vec<u8>) -> NodeId {
    Column::new()
      .derive_children(
        move || items.clone(),
        |item| *item,
        |_| RawTextField::new(TextEntry::new().bind_value(Signal::new(String::from("hello")))),
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

/// A painted box carries a canvas the size of the box, drawn through vello on the CPU
/// and blitted, so the browser shows the fill without WebGPU.
#[wasm_bindgen_test]
async fn a_painted_box_draws_into_its_canvas() {
  use skene_gui::{Painted, core::Dimension, kurbo::Rect};
  use wasm_bindgen_futures::JsFuture;
  use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

  /// Waits for the browser's next rendering step, after which the box is measured and
  /// painted.
  async fn next_frame() {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
      let _ = web_sys::window()
        .expect("a window")
        .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 20);
    });

    let _ = JsFuture::from(promise).await;
  }

  let (_installed, root) = fresh_root();
  let _ = mount(|| {
    Painted::new(|canvas, size| {
      canvas.fill(
        &Rect::new(0., 0., f64::from(size.width), f64::from(size.height)),
        Color::srgb(0., 0., 1.),
      );
    })
    .set_width(Dimension::Length(40.))
    .set_height(Dimension::Length(20.))
    .build()
  });
  next_frame().await;
  next_frame().await;

  let painted = only_child(&root);
  let canvas: HtmlCanvasElement = painted
    .first_element_child()
    .expect("the canvas")
    .dyn_into()
    .expect("a canvas element");

  assert!(
    canvas.width() >= 40 && canvas.height() >= 20,
    "sized to the box"
  );

  let context: CanvasRenderingContext2d = canvas
    .get_context("2d")
    .expect("a context")
    .expect("the 2d context")
    .dyn_into()
    .expect("a 2d context");
  let pixel = context
    .get_image_data(
      f64::from(canvas.width() / 2),
      f64::from(canvas.height() / 2),
      1.,
      1.,
    )
    .expect("image data")
    .data();

  assert_eq!(
    &pixel[0..4],
    &[0, 0, 255, 255],
    "the center pixel is the fill"
  );
}
