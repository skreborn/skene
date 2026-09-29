//! Real-DOM integration tests for the web realization of the built-in views.
//!
//! Each test installs the web platform rooted at a connected `<div>`, builds a view under
//! it, and asserts against the actual DOM the browser produced. The guard uninstalls
//! the platform and removes the root when it drops, isolating each test.
//!
//! Run in a headless browser:
//! `cargo test -p skene-gui --target wasm32-unknown-unknown --test web_dom`.
#![cfg(skene_leaf = "browser")]

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  time::Duration,
};

use skene_gui::{
  AnyView,
  Color,
  Column,
  Container,
  ContextMenu,
  Deferred,
  Definition,
  Flexible,
  FocusHandle,
  Grid,
  HasChildren,
  Inset,
  Media,
  MenuItem,
  Overlay,
  Pinned,
  RadioMember,
  Range,
  RawCheckbox,
  RawRadio,
  RawRadioGroup,
  RawSlider,
  RawTextField,
  RichText,
  Role,
  Roving,
  Row,
  Span,
  Stack,
  TOOLTIP_GRACE,
  Text,
  TextDecoration,
  TextDecorationStyle,
  TextEntry,
  TextSelection,
  Theme,
  Toggle,
  Tooltip,
  Typography,
  View,
  core::{Dimension, PointerButton},
  derived,
  empty,
  head,
  reactive::{Get, Owner, Set, Signal},
  wasm_bindgen::JsCast,
  web::{self, mount},
};
use skene_simple::gui::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{Document, Element, HtmlElement, HtmlInputElement};

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> Document {
  web_sys::window()
    .expect("a window")
    .document()
    .expect("a document")
}

/// Uninstalls the platform and removes the root when it drops, isolating each test.
struct Installed(Element);

impl Drop for Installed {
  fn drop(&mut self) {
    web::uninstall();
    self.0.remove();
  }
}

/// Installs the platform rooted at a fresh connected `<div>`, returning the guard (drop
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

/// Waits for the browser's next rendering step, after which a box is measured and
/// painted.
async fn next_frame() {
  wait(Duration::from_millis(20)).await;
}

/// Waits `duration` on the browser's clock.
async fn wait(duration: Duration) {
  let millis = i32::try_from(duration.as_millis()).expect("a short wait");
  let promise = js_sys::Promise::new(&mut |resolve, _| {
    let _ = web_sys::window()
      .expect("a window")
      .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis);
  });

  let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
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

/// A color as the platform renders it.
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
    RawTextField::new(
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
      .handle_key_up(move |event| released.set((event.key.clone(), event.code.clone())))
      .set_child(RawTextField::new(TextEntry::new()))
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
      .set_border_width(Inset::all(2.0))
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
  use skene_gui::{
    ActivePlatform,
    Behavior,
    Control,
    ControlHandle,
    core::{ControlKind, EventKind, KeyEvent, Prevent, Role},
  };

  /// Takes `Escape` and nothing else.
  struct Keeper;

  impl Behavior<ActivePlatform> for Keeper {
    fn kind(&self) -> ControlKind {
      ControlKind::Button
    }

    fn role(&self) -> Option<Role> {
      None
    }

    fn listens(&self) -> &'static [EventKind] {
      &[EventKind::KeyDown]
    }

    fn on_key(&self, event: &KeyEvent, _: &ControlHandle) -> Prevent {
      if event.key == "Escape" {
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

/// A wrapper's derived property reaches a run as a slot, following its signal.
#[wasm_bindgen_test]
fn a_derived_wrapper_property_reaches_a_run() {
  let (_guard, root) = fresh_root();
  let grow = Signal::new(2.0f32);

  let _ = mount(move || {
    Row::new()
      .set_child(
        Flexible::new()
          .derive_grow_factor(move || grow.get())
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

/// A press on a slider's thumb asks for the value whose thumb centers under the
/// pointer, measured from the slider's own box rather than the thumb's: 150 pixels into
/// 200, the thumb's 16-pixel center travelling 184, is 77.
#[wasm_bindgen_test]
async fn a_press_on_the_thumb_slides_by_the_slider_box() {
  use skene_gui::core::Dimension;

  let (_guard, root) = fresh_root();
  let value = Signal::new(40.0f32);

  let _ = mount(move || {
    Container::new()
      .set_width(Dimension::Length(200.))
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

  let _ = thumb.dispatch_event(&event);

  assert!((value.get() - 77.).abs() < f32::EPSILON, "{}", value.get());
}

/// A slider's native input holds the slider's range, step and value, and follows the
/// value.
#[wasm_bindgen_test]
fn a_sliders_native_input_holds_its_range_and_value() {
  let (_guard, root) = fresh_root();
  let value = Signal::new(150.0f32);

  let _ = mount(move || {
    RawSlider::new(Range::new().set_max(200.).set_step(5.).bind_value(value)).build()
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

  let input = only_child(&root);

  assert_eq!(input.tag_name(), "INPUT");
  dispatch(&input, "input");

  assert_eq!(&*seen.borrow(), "committed");
}

/// A picker's drawn face keeps its caption before its box, as its scriptless
/// `<select>` does, in a `<label>` `for` nothing, since the face is no form element.
#[wasm_bindgen_test]
fn a_drawn_face_keeps_its_caption_before_its_box() {
  use skene_gui::{ListItem, RawOption, RawPicker};

  let (_guard, root) = fresh_root();
  let value = Signal::new("a".to_owned());

  let _ = mount(move || {
    RawPicker::new()
      .bind_value(value)
      .set_label("Size")
      .set_options(["a", "b"].map(|value| RawOption::new(ListItem::new(value)).set_label(value)))
      .set_face(|shown| Text::derive(move || shown.get()))
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

/// A value written into a focused entry keeps the caret where the browser would move
/// it to the end: an uppercasing handler leaves it after the character typed.
#[wasm_bindgen_test]
fn a_transformed_value_keeps_the_caret() {
  let (_guard, root) = fresh_root();
  let value = Signal::new("ac".to_owned());

  let _ = mount(move || {
    RawTextField::new(
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

  head::set_title("Live title");
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
    head::set_title("Outer");
    head::add_meta("scoped-test", "outer");
  });
  inner.enter(|| {
    head::set_title("Inner");
    head::add_meta("scoped-test", "inner");
  });

  assert_eq!(document().title(), "Inner");
  assert_eq!(scoped(), ["inner"]);

  inner.dispose();

  assert_eq!(document().title(), "Outer");
  assert_eq!(scoped(), ["outer"]);

  outer.dispose();

  assert!(scoped().is_empty());
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
    skene_gui::RichText::new()
      .set_spans([
        skene_gui::Span::new("plain "),
        skene_gui::Span::new("click").handle_click(move || counted.set(counted.get() + 1)),
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
        Definition::from(Color::srgb(1., 0., 0.)).when(Media::MinWidth(1.), green)
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
        .set_letter_spacing(1.),
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
    .set_child_builder(|| RawTextField::new(TextEntry::new().set_value("hi")))
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
  web::end_turn();

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
/// The host tier pins the same thing against the headless platform. This pins it against
/// the browser's: real reconcile, real elements, and a real `keydown`, so the order the
/// platform reports is checked against the order the document actually renders.
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

  web::end_turn();

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
          RawTextField::new(TextEntry::new())
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

/// A lazy column of estimated items measures each one it builds through the browser's
/// size observation: taller measures shrink the window and grow the trailing spacer.
#[wasm_bindgen_test]
async fn a_lazy_column_measures_its_estimated_items() {
  use skene_gui::{ItemExtent, LazyColumn};

  let (_installed, root) = fresh_root();
  let _ = mount(|| {
    Container::new()
      .set_height(50.)
      .set_child(
        LazyColumn::new()
          .set_item_height(ItemExtent::Estimated(10.))
          .set_overscan(0)
          .derive_items(
            || 0..100,
            |index| *index,
            |index| {
              Container::new()
                .set_height(20.)
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
/// the tier runs Firefox with WebGPU on, so the canvas holds a WebGPU context and no 2d
/// one, and the fill shows through it. `web_painted` covers the CPU path.
#[wasm_bindgen_test]
async fn a_painted_box_draws_into_its_canvas() {
  use skene_gui::{Painted, core::Dimension, kurbo::Rect};
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
  let mut pixel = center_pixel(&canvas);

  for _ in 0..100 {
    if pixel[0..4] == [0, 0, 255, 255] {
      break;
    }

    next_frame().await;
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

  assert!(
    matches!(canvas.get_context("2d"), Ok(None)),
    "painted on the GPU, so the canvas holds no 2d context"
  );
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
          .set_focus_order(skene_gui::FocusOrder::Auto)
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
            MenuItem::new("Rename").handle_select(move || chosen.set("rename".to_owned())),
            MenuItem::separator(),
            MenuItem::new("Delete").handle_select(move || chosen.set("delete".to_owned())),
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

  let _ = mount(|| Container::new().set_color(Color::srgb(1., 0., 0.)).build());
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
        .set_edges(Inset::all(Dimension::Length(0.)))
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
        RichText::new().set_spans([Span::group([Span::new("x")]).set_link("/g")])
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
