//! The web facade's views realized in a real browser: the elements they build, the
//! styles the engine computes for them, the events that reach them, and the signals
//! their bindings write.
//!
//! Run in a headless browser:
//! `cargo test-web --test browser_views`.
#![cfg(all(skene_platform = "web", not(skene_server)))]

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_foundation::{
  Alignment,
  Arrangement,
  Axis,
  BasicListboxBase,
  BasicPickerBase,
  BoxFacts,
  ColorScheme,
  ColumnBase,
  Condition,
  ContainerBase,
  Content,
  ContentKey,
  ContentType,
  ContentValue,
  ControlBase,
  ControlSink,
  Controllable,
  Controller,
  DecorationBase,
  DecorationWithCornerRadius,
  Edge,
  Edit,
  EventKind,
  FlexBase,
  FlexChild,
  GridBase,
  Inset,
  ItemAlignment,
  LabelPlacement,
  Layout,
  LowLevel,
  Media,
  MotionBase,
  NativeElement,
  NodeFacts,
  NodeSpec,
  OverlayBase,
  Placement,
  PointerPrecision,
  Property,
  Realization,
  Report,
  RichTextBase,
  RowBase,
  RunBase,
  RunWithTypeMetrics,
  Size,
  Spacing,
  SpanBase,
  StackBase,
  StaticLeaf,
  TextBase,
  TextEntryBase,
  ThemeBase,
  TooltipBase,
  Transition,
  Value,
  Verdict,
  When,
  report,
};
use skene_simple_web::prelude::*;
use skene_web::{
  AnyView,
  AsyncView,
  AsyncViewBase,
  Autofill,
  BasicButton,
  BasicCheckbox,
  BasicListboxOption,
  BasicPicker,
  BasicRadio,
  BasicRadioGroup,
  BasicRadioGroupBase,
  BasicSlider,
  BasicTextEditor,
  BasicTextField,
  Bubbling,
  Color,
  Column,
  Container,
  Control,
  Expr,
  FocusOrder,
  Grid,
  HasChild,
  HasChildren,
  Keyboard,
  LayoutBox,
  Length,
  Link,
  LinkBase,
  NodeHandle,
  NodeId,
  Overlay,
  RichText,
  Role,
  Row,
  Span,
  Stack,
  SystemColors,
  Text,
  TextSelection,
  Theme,
  ThemedIn,
  TokenName,
  TokenRef,
  TokenSet,
  Tooltip,
  Typography,
  View,
  behavior::{Activate, Behavior, ListboxOption, Radio, Slider, TextEntry, Toggle},
  head,
  install_router,
  media,
  platform::web::{self, Web, mount},
  reactive::{Effect, Get, Owner, Set, Signal},
  testing::{computed, dispatch, document, frames_until, fresh_root, only_child, tick},
  wasm_bindgen::{JsCast, closure::Closure},
  window,
};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{
  Element,
  HtmlElement,
  HtmlInputElement,
  KeyboardEvent,
  KeyboardEventInit,
  MouseEvent,
  MouseEventInit,
};

wasm_bindgen_test_configure!(run_in_browser);

/// The element children of `parent`, in order.
fn elements(parent: &Element) -> Vec<Element> {
  let children = parent.children();

  (0..children.length())
    .filter_map(|index| children.item(index))
    .collect()
}

/// A color as the CSS the platform renders it to.
fn css(color: Color) -> String {
  let [lightness, chroma, hue, alpha] = color.to_oklch();

  format!("oklch({lightness} {chroma} {hue} / {alpha})")
}

/// The computed `property` a `<div>` under `parent` gets from `<property>: <color>`, so
/// colors compare as the engine serializes them.
fn probe(parent: &Element, property: &str, color: Color) -> String {
  let element = document().create_element("div").expect("a probe");

  let _ = element.set_attribute("style", &format!("{property}:{}", css(color)));
  let _ = parent.append_child(&element);

  let color = computed(&element, property);

  element.remove();

  color
}

/// The computed background of `element`, serialized as [`probe`] serializes a color.
fn background(element: &Element) -> String {
  computed(element, "background-color")
}

/// Dispatches a cancelable, bubbling click at `target` with `ctrl` held and a click
/// count of `detail`; whether a handler prevented its default.
fn click(target: &Element, ctrl: bool, detail: i32) -> bool {
  let init = MouseEventInit::new();

  init.set_bubbles(true);
  init.set_cancelable(true);
  init.set_ctrl_key(ctrl);
  init.set_detail(detail);

  let event = MouseEvent::new_with_mouse_event_init_dict("click", &init).expect("a click");
  let _ = target.dispatch_event(&event);

  event.default_prevented()
}

/// Dispatches a click as [`click`] does with `Ctrl` held; whether it reached the window
/// with its default left to the browser, which the window then takes from it, so the
/// click opens no tab.
fn modified_click(target: &Element, detail: i32) -> bool {
  let reached = Rc::new(Cell::new(false));
  let keep = Closure::<dyn Fn(web_sys::Event)>::new({
    let reached = Rc::clone(&reached);

    move |event: web_sys::Event| {
      reached.set(!event.default_prevented());
      event.prevent_default();
    }
  });
  let window = web_sys::window().expect("a window");

  window
    .add_event_listener_with_callback("click", keep.as_ref().unchecked_ref())
    .expect("listen for the click");

  let _ = click(target, true, detail);

  window
    .remove_event_listener_with_callback("click", keep.as_ref().unchecked_ref())
    .expect("stop listening");

  reached.get()
}

/// Dispatches a cancelable, bubbling `keydown` of `key` at `target`; whether a handler
/// prevented its default.
fn key_down(target: &Element, key: &str) -> bool {
  let init = KeyboardEventInit::new();

  init.set_bubbles(true);
  init.set_cancelable(true);
  init.set_key(key);

  let event = KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).expect("a key");
  let _ = target.dispatch_event(&event);

  event.default_prevented()
}

/// `element` as the HTML element it is.
fn html(element: &Element) -> &HtmlElement {
  element.dyn_ref().expect("an HTML element")
}

/// The element `element`'s `aria-describedby` names, in order.
fn described_by(element: &Element) -> Vec<Element> {
  element
    .get_attribute("aria-describedby")
    .unwrap_or_default()
    .split_whitespace()
    .map(|id| {
      document()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("no element {id}"))
    })
    .collect()
}

/// The scheme an app put in force, returned to following the platform when dropped.
#[must_use]
struct Scheme;

impl Scheme {
  fn choose(scheme: ColorScheme) -> Self {
    media::set_scheme(Some(scheme));
    web::end_turn();

    Self
  }
}

impl Drop for Scheme {
  fn drop(&mut self) {
    media::set_scheme(None);
    web::end_turn();
  }
}

fn fixed(leaf: &StaticLeaf<Color>) -> Color {
  match leaf {
    StaticLeaf::Fixed(color) => *color,
    _ => panic!("a fixed color"),
  }
}

/// The default palette's card color under the light scheme.
fn card_light() -> Color {
  fixed(SimplePalette::default().card.definition().base())
}

/// The default palette's card color under the dark scheme.
fn card_dark() -> Color {
  let palette = SimplePalette::default();
  let (condition, leaf) = &palette.card.definition().variants()[0];

  assert_eq!(*condition, Media::Dark);

  fixed(leaf)
}

/// A static tree realizes its elements in order, a role as its semantic element, and a
/// layout view's anchors as no element of their own.
#[wasm_bindgen_test]
fn nested_views_realize_their_tree_in_order() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Container::new()
      .set_role(Role::Navigation)
      .set_child(Column::new().set_children((
        Text::new("first"),
        Row::new().set_children((Text::new("second"),)),
      )))
      .build()
  });

  let navigation = only_child(&root);

  assert_eq!(navigation.tag_name(), "NAV");
  assert_eq!(elements(&navigation).len(), 1);

  let column = only_child(&navigation);
  let content = elements(&column);

  assert_eq!(
    content.len(),
    2,
    "the column's two children and nothing else"
  );
  assert_eq!(content[0].tag_name(), "SPAN");
  assert_eq!(content[0].text_content().as_deref(), Some("first"));
  assert_eq!(content[1].tag_name(), "DIV");

  let row = elements(&content[1]);

  assert_eq!(row.len(), 1);
  assert_eq!(row[0].tag_name(), "SPAN");
  assert_eq!(row[0].text_content().as_deref(), Some("second"));
}

/// A click runs the handlers deepest first; a box that stops bubbling keeps the event
/// from its ancestors, and no box prevents the browser's default.
#[wasm_bindgen_test]
fn a_click_climbs_until_a_box_stops_it() {
  let (_guard, root) = fresh_root();
  let log = Rc::new(RefCell::new(Vec::new()));
  let note = |name: &'static str| {
    let log = Rc::clone(&log);

    move || {
      log.borrow_mut().push(name);
    }
  };

  let _ = mount({
    let outer = note("outer");
    let middle = note("middle");
    let inner = note("inner");

    move || {
      Container::new()
        .handle_click(outer)
        .set_child(
          Container::new()
            .handle_click(move || {
              middle();

              Bubbling::Stop
            })
            .set_child(Container::new().handle_click(inner)),
        )
        .build()
    }
  });

  let inner = only_child(&only_child(&only_child(&root)));
  let prevented = click(&inner, false, 1);

  assert_eq!(*log.borrow(), ["inner", "middle"]);
  assert!(!prevented, "no box states anything about the browser");
}

/// An edit of a text field's `<input>` commits the value into its binding.
#[wasm_bindgen_test]
fn an_edit_commits_into_the_binding() {
  let (_guard, root) = fresh_root();
  let value = Signal::new(String::new());

  let _ = mount(move || BasicTextField::new(TextEntry::new().bind_value(value)).build());

  let input = only_child(&root)
    .dyn_into::<HtmlInputElement>()
    .expect("an input");

  input.set_value("typed");
  dispatch(&input, "input");

  assert_eq!(value.get(), "typed");
}

/// A modal's `<dialog>` opens and closes with its bound signal, and a `cancel` asks the
/// binding to close it rather than closing on its own.
#[wasm_bindgen_test]
fn an_overlay_follows_its_signal_and_a_cancel_asks_it_to_close() {
  let (_guard, root) = fresh_root();
  let open = Signal::new(false);

  let _ = mount(move || {
    Container::new()
      .set_child(
        Overlay::modal()
          .bind_open(open)
          .set_child(Text::new("dialog")),
      )
      .build()
  });

  let dialog = root
    .query_selector("dialog")
    .expect("a valid selector")
    .expect("the modal");

  assert!(!dialog.has_attribute("open"));

  open.set(true);
  web::end_turn();

  assert!(dialog.has_attribute("open"));

  let init = web_sys::EventInit::new();

  init.set_cancelable(true);

  let cancel = web_sys::Event::new_with_event_init_dict("cancel", &init).expect("an event");
  let _ = dialog.dispatch_event(&cancel);

  web::end_turn();

  assert!(!open.get(), "the dismiss wrote the binding");
  assert!(!dialog.has_attribute("open"));
}

/// Keyed children keep the elements whose keys stay and move them, dropping only the
/// removed one.
#[wasm_bindgen_test]
fn keyed_children_keep_their_elements_through_a_removal_and_a_reorder() {
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

  let column = only_child(&root);
  let before = elements(&column);

  assert_eq!(before.len(), 3);

  items.set(vec![3, 1]);
  web::end_turn();

  let after = elements(&column);

  assert_eq!(after.len(), 2);
  assert_eq!(after[0], before[2], "the kept element moved, not rebuilt");
  assert_eq!(after[1], before[0]);
  assert_eq!(after[0].text_content().as_deref(), Some("3"));
  assert!(!before[1].is_connected(), "the removed one is gone");
}

/// A radio group's one tab stop follows the choice, and with nothing chosen it is the
/// first member in the tree, a member inserted first included.
#[wasm_bindgen_test]
fn a_radio_groups_tab_stop_follows_the_choice_in_tree_order() {
  let (_guard, root) = fresh_root();
  let chosen: Signal<Option<&str>> = Signal::new(None);
  let items = Signal::new(vec!["a", "b"]);

  let _ = mount(move || {
    BasicRadioGroup::new()
      .bind_chosen(chosen)
      .set_child(Column::new().derive_children(
        move || items.get(),
        |item| *item,
        |item| BasicRadio::new(Radio::new(Some(item))).set_label(item),
      ))
      .build()
  });

  let inputs = || {
    let found = root
      .query_selector_all("input[type=radio]")
      .expect("a valid selector");

    (0..found.length())
      .filter_map(|index| found.item(index))
      .map(|node| node.dyn_into::<HtmlInputElement>().expect("an input"))
      .collect::<Vec<_>>()
  };
  let stops = || {
    inputs()
      .iter()
      .map(|input| input.tab_index() >= 0)
      .collect::<Vec<_>>()
  };

  assert_eq!(stops(), [true, false]);

  inputs()[1].click();
  web::end_turn();

  assert_eq!(chosen.get(), Some("b"));
  assert_eq!(stops(), [false, true]);

  chosen.set(None);
  items.set(vec!["z", "a", "b"]);
  web::end_turn();

  assert_eq!(stops(), [true, false, false]);
}

/// The equal-footing gate: a layout view written outside the facade, against its
/// public API alone, realizes the same element the built-in it parallels does.
#[wasm_bindgen_test]
fn a_third_party_layout_view_matches_the_built_in() {
  #[must_use]
  struct Toolbar {
    base: LayoutBox<FlexChild>,
  }

  impl Toolbar {
    fn new() -> Self {
      Self {
        base: LayoutBox::new(Layout::Flex {
          axis: Axis::Horizontal,
          reversed: false,
        }),
      }
    }

    fn set_spacing(mut self, spacing: impl Into<skene_web::Spacing>) -> Self {
      self.base.style_mut().spacing = Some(spacing.into().into());

      self
    }
  }

  impl skene_web::AnyKind for Toolbar {}

  impl skene_foundation::View<skene_web::Active> for Toolbar {
    fn build(self) -> NodeId {
      View::build(self.base)
    }
  }

  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Column::new()
      .set_children((Toolbar::new().set_spacing(8), Row::new().set_spacing(8)))
      .build()
  });

  let both = elements(&only_child(&root));
  let (toolbar, row) = (&both[0], &both[1]);

  assert_eq!(toolbar.tag_name(), row.tag_name());
  assert_eq!(toolbar.get_attribute("class"), row.get_attribute("class"));

  for property in ["display", "flex-direction", "column-gap", "row-gap"] {
    assert_eq!(
      computed(toolbar, property),
      computed(row, property),
      "{property}"
    );
  }
}

const GUTTER: TokenRef<Length> = TokenRef::new("gutter");

/// A palette holding a gutter length.
#[derive(Debug, Clone, PartialEq)]
struct Gutters {
  /// The room between a row's children.
  gutter: skene_web::Token<Length>,
}

impl<V: From<skene_web::Definition<Length>> + 'static> TokenSet<V> for Gutters {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.gutter.define()]
  }
}

/// A row's layout properties reach its element: a spacing that is a token resolves
/// through the theme to the palette's length, an expression over it doubles it in a
/// reversed row, and a spacing per axis sets each gap.
#[wasm_bindgen_test]
fn layout_properties_reach_the_box_and_a_token_spacing_resolves() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Theme::new(Gutters {
      gutter: skene_web::Token::new(GUTTER.name().as_str(), 8.),
    })
    .set_child_builder(|| {
      Column::new().set_children((
        Row::new()
          .set_spacing(GUTTER)
          .set_arrangement(Arrangement::SpaceBetween)
          .set_item_alignment(ItemAlignment::Center),
        Row::reversed().set_spacing(Expr::<Length>::from(GUTTER) * 2.),
        Row::new().set_spacing(Spacing::new(2, 1)),
      ))
    })
    .build()
  });

  let rows = elements(&only_child(&only_child(&root)));
  let styles = |row: &Element, properties: &[&str]| {
    properties
      .iter()
      .map(|property| computed(row, property))
      .collect::<Vec<_>>()
  };

  assert_eq!(
    styles(
      &rows[0],
      &[
        "display",
        "flex-direction",
        "column-gap",
        "justify-content",
        "align-items"
      ]
    ),
    ["flex", "row", "8px", "space-between", "center"]
  );
  assert_eq!(
    styles(&rows[1], &["flex-direction", "column-gap"]),
    ["row-reverse", "16px"]
  );
  assert_eq!(styles(&rows[2], &["column-gap", "row-gap"]), ["2px", "1px"]);
}

/// The window's width, a pixel below it and a pixel above it.
fn around_the_viewport() -> (f32, f32) {
  let width = web_sys::window()
    .expect("a window")
    .inner_width()
    .expect("a width")
    .as_f64()
    .expect("a number");

  #[expect(
    clippy::cast_possible_truncation,
    reason = "a viewport width is precise enough in f32"
  )]
  let around = ((width - 1.) as f32, (width + 1.) as f32);

  around
}

/// A media condition on a layout's padding switches with the viewport, whether spelled
/// on the inset, on a plain spacing, or on a plain number: a condition the window's
/// width meets takes the variant, and one it falls short of keeps the base.
#[wasm_bindgen_test]
fn a_media_condition_on_padding_switches_with_the_viewport() {
  let (_guard, root) = fresh_root();
  let (below, above) = around_the_viewport();
  let column = move |threshold: f32| {
    Column::new()
      .set_spacing(Spacing::from(8).when(Media::min_width(threshold), 16))
      .set_padding(Inset::all(4).when(Media::min_width(threshold), Inset::all(12)))
      .set_children((
        Container::new()
          .set_width(40)
          .set_aspect_ratio(1f32.when(Media::min_width(threshold), 2.)),
        Container::new().set_height(1),
      ))
  };

  let _ = mount(move || {
    Column::new()
      .set_children((column(below), column(above)))
      .build()
  });

  let columns = elements(&only_child(&root));
  let read = |column: &Element| {
    (
      computed(column, "row-gap"),
      computed(column, "padding-top"),
      html(&elements(column)[0]).offset_height(),
    )
  };

  assert_eq!(
    read(&columns[0]),
    ("16px".to_owned(), "12px".to_owned(), 20),
    "the window is at least as wide"
  );
  assert_eq!(
    read(&columns[1]),
    ("8px".to_owned(), "4px".to_owned(), 40),
    "the window is narrower"
  );
}

/// A uniform grid declares one equal track per column, and its two-axis alignment lands
/// on both item alignments while its row arrangement places the rows.
#[wasm_bindgen_test]
fn a_uniform_grid_declares_equal_tracks_and_splits_its_alignment() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Container::new()
      .set_width(300)
      .set_child(
        Grid::uniform(3)
          .set_item_alignment(Alignment::new(ItemAlignment::End, ItemAlignment::Start))
          .set_row_arrangement(Arrangement::Center),
      )
      .build()
  });

  let grid = only_child(&only_child(&root));

  assert_eq!(
    [
      "display",
      "grid-template-columns",
      "justify-items",
      "align-items",
      "align-content"
    ]
    .map(|property| computed(&grid, property)),
    [
      "grid",
      "100px 100px 100px",
      "flex-end",
      "flex-start",
      "center"
    ]
  );
}

/// A stack's derived alignment writes both axes on the same element as its signal
/// changes.
#[wasm_bindgen_test]
fn a_stacks_derived_alignment_drives_both_axes() {
  let (_guard, root) = fresh_root();
  let alignment = Signal::new(Alignment::CENTER);

  let _ = mount(move || {
    Stack::new()
      .derive_item_alignment(move || alignment.get())
      .add_child(Text::new("x"))
      .build()
  });

  let stack = only_child(&root);
  let axes = |stack: &Element| {
    (
      computed(stack, "justify-items"),
      computed(stack, "align-items"),
    )
  };

  assert_eq!(axes(&stack), ("center".to_owned(), "center".to_owned()));

  alignment.set(Alignment::new(ItemAlignment::Start, ItemAlignment::End));
  web::end_turn();

  assert_eq!(only_child(&root), stack, "the same element");
  assert_eq!(
    axes(&stack),
    ("flex-start".to_owned(), "flex-end".to_owned())
  );
}

/// A page's routes, for the link test.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Page {
  Home,
  About,
}

impl skene_web::router::Route for Page {
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

/// A link is an anchor carrying its route's path, and a tab stop; a plain click or
/// `Enter` navigates the router and keeps the browser from navigating, and a modified
/// click, from a pointer or a key, leaves both to the browser.
#[wasm_bindgen_test]
fn a_link_carries_its_destination_and_navigates_the_router() {
  let (_guard, root) = fresh_root();
  let path = web_sys::window()
    .expect("a window")
    .location()
    .pathname()
    .expect("a path");
  let router = Rc::new(Cell::new(None));

  let _ = mount({
    let router = Rc::clone(&router);

    move || {
      router.set(Some(install_router(Page::Home)));

      Link::new(Page::About).set_child(Text::new("about")).build()
    }
  });

  let router = router.get().expect("a router");
  let link = only_child(&root);

  assert_eq!(link.tag_name(), "A");
  assert_eq!(link.get_attribute("href").as_deref(), Some("/about"));
  assert_eq!(link.text_content().as_deref(), Some("about"));
  assert!(
    link
      .clone()
      .dyn_into::<HtmlElement>()
      .expect("an HTML element")
      .tab_index()
      >= 0
  );
  assert!(click(&link, false, 1), "a plain click is the router's");
  assert_eq!(router.route(), Page::About);

  router.navigate(Page::Home);

  for detail in [1, 0] {
    assert!(
      modified_click(&link, detail),
      "a modified click is the browser's"
    );
    assert_eq!(router.route(), Page::Home);
  }

  assert!(key_down(&link, "Enter"), "Enter on a link is the router's");
  assert_eq!(router.route(), Page::About);

  web::push_path(&path);
}

/// A lazy subtree mounts its content once its future completes.
#[wasm_bindgen_test]
async fn a_lazy_subtree_mounts_once_its_future_completes() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Container::new()
      .set_child(AsyncView::new().set_child_loader(|| async { Text::new("loaded") }))
      .build()
  });

  let container = only_child(&root);

  assert!(elements(&container).is_empty(), "nothing before it loads");

  for _ in 0..4 {
    tick().await;
  }

  let content = elements(&container);

  assert_eq!(content.len(), 1);
  assert_eq!(content[0].text_content().as_deref(), Some("loaded"));
}

/// A derived title follows its signal into the document, and a stylesheet two owners
/// register is one element that stays while either lives.
#[wasm_bindgen_test]
fn a_derived_title_follows_its_signal_and_a_shared_stylesheet_stays_with_either_owner() {
  const HREF: &str = "data:text/css,";

  let (_guard, _root) = fresh_root();
  let first = Owner::new();
  let second = Owner::new();
  let title = Signal::new("Home".to_owned());
  let stylesheets = || {
    document()
      .head()
      .expect("a head")
      .query_selector_all(&format!("link[rel=stylesheet][href=\"{HREF}\"]"))
      .expect("a valid selector")
      .length()
  };

  first.enter(|| {
    window::derive_title(move || format!("{} — skene", title.get()));
    head::add_stylesheet(HREF);
  });
  second.enter(|| head::add_stylesheet(HREF));

  assert_eq!(document().title(), "Home — skene");
  assert_eq!(stylesheets(), 1);

  title.set("About".to_owned());
  web::end_turn();

  assert_eq!(document().title(), "About — skene");

  first.dispose();

  assert_eq!(stylesheets(), 1, "the second owner keeps it");

  second.dispose();

  assert_eq!(stylesheets(), 0);
}

/// A handle asked for its size only after its box built measures the box from then.
#[wasm_bindgen_test]
async fn a_handle_asked_for_its_size_late_measures_from_then() {
  let (_guard, _root) = fresh_root();
  let handle = NodeHandle::new();
  let node = mount(move || {
    Container::new()
      .set_width(64)
      .set_height(32)
      .bind_handle(handle)
      .build()
  });

  assert_eq!(handle.node(), Some(node));
  assert!(
    frames_until(|| handle.size() == Some(Size::new(64., 32.))).await,
    "measured as {:?}",
    handle.size()
  );
}

/// A handle bound to a box reports the box's measured size, tracked, and nothing
/// before the first measurement.
#[wasm_bindgen_test]
async fn a_size_handle_reports_the_measured_size() {
  let (_guard, _root) = fresh_root();
  let handle = NodeHandle::new();
  let seen = Rc::new(Cell::new(0));

  let _ = mount({
    let seen = Rc::clone(&seen);

    move || {
      Effect::new(move |_: Option<()>| {
        let _ = handle.size();

        seen.set(seen.get() + 1);
      });

      Container::new()
        .set_width(320)
        .set_height(200)
        .bind_handle(handle)
        .build()
    }
  });

  assert_eq!(handle.size(), None);
  assert_eq!(seen.get(), 1);
  assert!(
    frames_until(|| handle.size() == Some(Size::new(320., 200.))).await,
    "measured as {:?}",
    handle.size()
  );
  assert_eq!(seen.get(), 2, "the measurement notified the effect");
}

/// A nested theme replaces the palette beneath it and leaves the one above alone; each
/// consumer is inked by the nearest definition.
#[wasm_bindgen_test]
fn a_nested_theme_replaces_the_palette_beneath_it() {
  let (_guard, root) = fresh_root();
  let _scheme = Scheme::choose(ColorScheme::Light);
  let ink = Color::srgb(0.1, 0.9, 0.4);

  let _ = mount(move || {
    Theme::new(SimplePalette::default())
      .set_child_builder(move || {
        Column::new().set_children((
          Container::new().set_background(SimplePalette::current().card),
          Theme::new(SimplePalette::default().set_card(ink))
            .set_child_builder(|| Container::new().set_background(SimplePalette::CARD)),
        ))
      })
      .build()
  });

  let column = only_child(&only_child(&root));
  let content = elements(&column);
  let inner = only_child(&content[1]);

  assert_eq!(
    background(&content[0]),
    probe(&root, "background-color", card_light())
  );
  assert_eq!(background(&inner), probe(&root, "background-color", ink));
}

/// The scheme in force is the document's: `media::set_scheme` picks which value a
/// conditional token takes, the signal follows, and the element is re-inked in place;
/// `None` returns to following the platform.
#[wasm_bindgen_test]
fn the_scheme_follows_the_media_choice() {
  let (_guard, root) = fresh_root();
  let scheme = Scheme::choose(ColorScheme::Light);

  let _ = mount(|| {
    Theme::new(SimplePalette::default())
      .set_child_builder(|| Container::new().set_background(SimplePalette::CARD))
      .build()
  });

  let card = only_child(&only_child(&root));

  assert_eq!(media::scheme().get(), ColorScheme::Light);
  assert_eq!(
    background(&card),
    probe(&root, "background-color", card_light())
  );

  media::set_scheme(Some(ColorScheme::Dark));
  web::end_turn();

  assert_eq!(media::scheme().get(), ColorScheme::Dark);
  assert_eq!(media::chosen_scheme(), Some(ColorScheme::Dark));
  assert_eq!(
    document()
      .document_element()
      .and_then(|html| html.get_attribute("data-skene-scheme"))
      .as_deref(),
    Some("dark")
  );
  assert_eq!(
    background(&card),
    probe(&root, "background-color", card_dark())
  );
  assert_eq!(
    only_child(&only_child(&root)),
    card,
    "re-inked, not rebuilt"
  );

  drop(scheme);

  let preferred = if web_sys::window()
    .expect("a window")
    .match_media("(prefers-color-scheme: dark)")
    .ok()
    .flatten()
    .is_some_and(|query| query.matches())
  {
    (ColorScheme::Dark, card_dark())
  }
  else {
    (ColorScheme::Light, card_light())
  };

  assert_eq!(media::chosen_scheme(), None);
  assert_eq!(media::scheme().get(), preferred.0, "the platform's again");
  assert!(
    !document()
      .document_element()
      .is_some_and(|html| html.has_attribute("data-skene-scheme"))
  );
  assert_eq!(
    background(&card),
    probe(&root, "background-color", preferred.1)
  );
}

/// A derived scheme follows its signal the same way, and a `Media::Dark` watch agrees
/// with it.
#[wasm_bindgen_test]
fn a_derived_scheme_re_inks_without_a_rebuild() {
  let (_guard, root) = fresh_root();
  let _restore = Scheme::choose(ColorScheme::Light);
  let scheme = Signal::new(Some(ColorScheme::Light));
  let owner = Owner::new();
  let dark = owner.enter(|| media::watch(&Media::Dark));

  let _ = mount(move || {
    media::derive_scheme(move || scheme.get());

    Theme::new(SimplePalette::default())
      .set_child_builder(|| Container::new().set_background(SimplePalette::CARD))
      .build()
  });

  let card = only_child(&only_child(&root));

  assert!(!dark.get());
  assert_eq!(
    background(&card),
    probe(&root, "background-color", card_light())
  );

  scheme.set(Some(ColorScheme::Dark));
  web::end_turn();

  assert!(dark.get());
  assert_eq!(
    background(&card),
    probe(&root, "background-color", card_dark())
  );
  assert_eq!(
    only_child(&only_child(&root)),
    card,
    "re-inked, not rebuilt"
  );

  scheme.set(None);
  web::end_turn();
  owner.dispose();
}

/// The facade's typography inks the runs: a text under no theme in the default ink,
/// and a text under the Simple theme in the palette's foreground.
#[wasm_bindgen_test]
fn typography_inks_the_runs() {
  let (_guard, root) = fresh_root();
  let _scheme = Scheme::choose(ColorScheme::Light);

  let _ = mount(|| {
    assert_eq!(Typography::current(), Typography::default());

    Column::new()
      .set_children((
        Text::new("bare"),
        Theme::new(SimplePalette::default()).set_child_builder(|| Text::new("themed")),
      ))
      .build()
  });

  let content = elements(&only_child(&root));
  let bare = &content[0];
  let themed = only_child(&content[1]);
  let ink = fixed(Typography::default().ink.definition().base());
  let foreground = fixed(SimplePalette::default().foreground.definition().base());

  assert_eq!(computed(bare, "color"), probe(&root, "color", ink));
  assert_eq!(
    computed(&themed, "color"),
    probe(&root, "color", foreground)
  );
}

/// A border whose color is a token resolves through the theme, its width per edge, and
/// a transition reaches the element's declared transitions.
#[wasm_bindgen_test]
fn a_border_with_a_token_color_resolves_through_the_theme() {
  let (_guard, root) = fresh_root();
  let red = Color::srgb(1., 0., 0.);

  let _ = mount(move || {
    Theme::new(SimplePalette::default().set_card(red))
      .set_child_builder(|| {
        Container::new()
          .set_border_width(Inset::new(4, 1, 2, 3))
          .set_border_color(SimplePalette::CARD)
          .add_transition(Transition::new(Property::Background, 0.2))
      })
      .build()
  });

  let card = only_child(&only_child(&root));

  for (edge, width) in [
    ("top", "1px"),
    ("right", "2px"),
    ("bottom", "3px"),
    ("left", "4px"),
  ] {
    assert_eq!(
      computed(&card, &format!("border-{edge}-width")),
      width,
      "{edge}"
    );
    assert_eq!(
      computed(&card, &format!("border-{edge}-color")),
      probe(&root, &format!("border-{edge}-color"), red),
      "{edge}"
    );
  }

  assert_eq!(computed(&card, "transition-property"), "background-color");
  assert_eq!(computed(&card, "transition-duration"), "0.2s");
}

/// A group's compound media variant reaches the spans beneath it as the cascade
/// resolves it: the light scheme's color while the light scheme is in force, the base
/// under the dark one.
#[wasm_bindgen_test]
fn a_groups_compound_media_variant_reaches_its_spans() {
  let (_guard, root) = fresh_root();
  let scheme = Scheme::choose(ColorScheme::Light);
  let (red, green) = (Color::srgb(1., 0., 0.), Color::srgb(0., 1., 0.));

  let _ = mount(move || {
    RichText::new()
      .add_spans([
        Span::group([Span::new("x")]).set_color(Value::from(red).when(
          Condition::and([Media::min_width(1).into(), Media::Light.into()]),
          green,
        )),
      ])
      .build()
  });

  let span = root
    .query_selector("span span")
    .expect("query")
    .expect("the group's span");

  assert_eq!(span.text_content().as_deref(), Some("x"));
  assert_eq!(computed(&span, "color"), probe(&root, "color", green));

  drop(scheme);

  let _scheme = Scheme::choose(ColorScheme::Dark);

  assert_eq!(computed(&span, "color"), probe(&root, "color", red));
}

/// The `<input>` a text field mounted alone under `root` realizes.
fn input_of(root: &Element) -> HtmlInputElement {
  root
    .query_selector("input")
    .expect("query")
    .expect("an input")
    .dyn_into()
    .expect("an input element")
}

/// Reports `edit` at `node` as the reader's change of its text and selection, which its
/// toolkit shows already where `applied`, and answers with the verdict.
fn edited(node: NodeId, edit: &Edit, applied: bool) -> Verdict {
  report::<Web>(
    node,
    Report::Change {
      content: Content::Edit,
      proposed: ContentValue::Edit(edit),
      applied,
    },
  )
}

/// An `<input>`'s reported edit is answered with the truth: one its handler transforms
/// is replaced by what the handler kept, the selection beside it, and the `<input>`
/// shows the truth after the turn; one the truth holds stands.
#[wasm_bindgen_test]
fn a_reported_edit_is_answered_with_the_truth_when_the_handler_transforms_it() {
  let (_guard, root) = fresh_root();
  let text = Signal::new("ab".to_owned());
  let caret = Signal::new(TextSelection::collapsed(2));
  let node = mount(move || {
    BasicTextField::new(
      TextEntry::new()
        .derive_value(move || text.get())
        .handle_change(move |value| text.set(value.to_uppercase()))
        .bind_selection(caret),
    )
    .build()
  });
  let typed = Edit::new("abc").with_selection(TextSelection::collapsed(3));

  assert_eq!(
    edited(node, &typed, false),
    Verdict::Replace(Edit::new("ABC").with_selection(TextSelection::collapsed(3)))
  );
  assert_eq!(text.get(), "ABC");

  web::end_turn();

  assert_eq!(input_of(&root).value(), "ABC");

  let kept = Edit::new("ABCD").with_selection(TextSelection::collapsed(4));

  assert_eq!(edited(node, &kept, true), Verdict::Accept);
  assert_eq!(caret.get(), TextSelection::collapsed(4));
}

/// An edit the truth holds already is accepted without asking the handlers, and one
/// leaving a controlled selection unnamed is answered with the truth's.
#[wasm_bindgen_test]
fn a_reported_edit_the_truth_holds_asks_nothing() {
  let (_guard, _root) = fresh_root();
  let asked = Rc::new(RefCell::new(Vec::new()));
  let heard = Rc::clone(&asked);
  let node = mount(move || {
    BasicTextField::new(
      TextEntry::new()
        .set_value("ab")
        .handle_change(move |value| heard.borrow_mut().push(value.to_owned()))
        .set_selection(TextSelection::collapsed(1)),
    )
    .build()
  });
  let truth = Edit::new("ab").with_selection(TextSelection::collapsed(1));

  assert_eq!(edited(node, &truth, true), Verdict::Accept);
  assert!(asked.borrow().is_empty());
  assert_eq!(
    edited(node, &Edit::new("ab"), true),
    Verdict::Replace(truth)
  );
}

/// An entry deriving no text keeps no truth: its edit stands, its handler hearing it.
#[wasm_bindgen_test]
fn a_reported_edit_of_an_uncontrolled_entry_stands() {
  let (_guard, _root) = fresh_root();
  let heard = Rc::new(RefCell::new(Vec::new()));
  let log = Rc::clone(&heard);
  let node = mount(move || {
    BasicTextField::new(
      TextEntry::new().handle_change(move |value| log.borrow_mut().push(value.to_owned())),
    )
    .build()
  });

  assert_eq!(edited(node, &Edit::new("typed"), true), Verdict::Accept);
  assert_eq!(*heard.borrow(), ["typed"]);
}

/// A disabled entry hears no edit and answers each with what it holds, but one its
/// truth already holds.
#[wasm_bindgen_test]
fn a_disabled_entry_answers_its_edit_with_the_truth() {
  let (_guard, root) = fresh_root();
  let bound = Signal::new("12".to_owned());
  let node = mount(move || {
    BasicTextField::new(TextEntry::new().bind_value(bound))
      .set_disabled(true)
      .build()
  });

  assert!(input_of(&root).disabled());
  assert_eq!(
    edited(node, &Edit::new("123"), false),
    Verdict::Replace(Edit::new("12"))
  );
  assert_eq!(bound.get(), "12");
  assert_eq!(edited(node, &Edit::new("12"), true), Verdict::Accept);
}

/// The verdict is an edit's only write-back: the `<input>` is not reconciled with the
/// truth while the edit is reported.
#[wasm_bindgen_test]
fn a_reported_edit_is_not_reconciled() {
  let (_guard, root) = fresh_root();
  let node =
    mount(|| BasicTextField::new(TextEntry::new().set_value("12").handle_change(|_| {})).build());
  let input = input_of(&root);

  assert_eq!(input.value(), "12");

  // The `<input>` shows the reader's edit already.
  input.set_value("12a");

  assert_eq!(
    edited(node, &Edit::new("12a"), true),
    Verdict::Replace(Edit::new("12"))
  );

  web::end_turn();

  assert_eq!(input.value(), "12a", "nothing but the verdict writes back");
}

/// A checkbox is controlled: a platform toggle its handler rejects is undone on the
/// native input.
#[wasm_bindgen_test]
fn a_rejected_toggle_is_undone_on_the_platform() {
  let (_guard, root) = fresh_root();

  let _ =
    mount(|| BasicCheckbox::new(Toggle::new().set_checked(false).handle_toggle(|_| {})).build());

  let input = input_of(&root);

  input.set_checked(true);
  dispatch(&input, "change");
  web::end_turn();

  assert!(!input.checked(), "the rejected toggle is undone");
}

/// A slider's native `input` reports the value it shows: one the handler takes is
/// accepted and left shown, and one it refuses is undone on the `<input>` inside the
/// event.
#[wasm_bindgen_test]
fn a_sliders_native_value_is_answered_and_a_refused_one_undone() {
  let (_guard, root) = fresh_root();
  let bound = Signal::new(50.0f32);
  let asked = Rc::new(RefCell::new(Vec::new()));

  let _ = mount({
    let asked = Rc::clone(&asked);

    move || {
      Column::new()
        .set_children([
          BasicSlider::new(Slider::new().bind_value(bound)),
          BasicSlider::new(
            Slider::new()
              .set_value(20.)
              .handle_change(move |value| asked.borrow_mut().push(value)),
          ),
        ])
        .build()
    }
  });

  let inputs = root
    .query_selector_all("input[type=range]")
    .expect("query the ranges");
  let input = |index: u32| -> HtmlInputElement {
    inputs
      .item(index)
      .and_then(|node| node.dyn_into().ok())
      .expect("a range")
  };

  input(0).set_value("70");
  dispatch(&input(0), "input");
  web::end_turn();

  assert!(
    (bound.get() - 70.).abs() < f32::EPSILON,
    "the value is taken"
  );
  assert_eq!(input(0).value(), "70");

  input(1).set_value("30");
  dispatch(&input(1), "input");

  assert_eq!(*asked.borrow(), [30.], "the value is asked for");
  assert_eq!(input(1).value(), "20", "and undone inside the event");

  web::end_turn();

  assert_eq!(input(1).value(), "20");
}

/// The value of a color input adopted through the native door.
#[derive(Debug, Clone, PartialEq)]
struct Hex(Rc<str>);

impl ContentType for Hex {}

impl Controllable for Hex {
  fn holds(&self, proposed: &Self) -> bool {
    self == proposed
  }
}

/// The door's name for a color input's value.
const COLOR: ContentKey<Hex> = ContentKey::door("color");

/// A sink over a color input's `value`, counting the writes it shows.
struct ColorSink {
  input: HtmlInputElement,
  writes: Rc<Cell<usize>>,
}

impl ControlSink for ColorSink {
  fn read(&self, content: Content, read: &mut dyn FnMut(ContentValue<'_>)) {
    if content == COLOR.content() {
      read(COLOR.erase(&Hex(Rc::from(self.input.value()))));
    }
  }

  fn write(&self, _content: Content, value: ContentValue<'_>) {
    if let Some(Hex(color)) = COLOR.own(value) {
      self.writes.set(self.writes.get() + 1);
      self.input.set_value(&color);
    }
  }
}

/// Controls the value of `input`, the color input adopted as `node`, through a sink of
/// the test's own: its value controlled by `truth` and asked for through `ask`, the
/// sink's writes counted in `writes` and every verdict on a reader's change noted in
/// `verdicts`.
fn control_color(
  node: NodeId,
  input: &HtmlInputElement,
  truth: impl Fn() -> Hex + 'static,
  ask: impl Fn(&Hex) + 'static,
  writes: Rc<Cell<usize>>,
  verdicts: Rc<RefCell<Vec<Verdict>>>,
) {
  LowLevel::<Web>::register_sink(
    node,
    ColorSink {
      input: input.clone(),
      writes,
    },
  );
  Controller::new(COLOR, truth, ask).register::<Web>(node);

  let reported = input.clone();
  let on_input = Closure::<dyn Fn()>::new(move || {
    let proposed = Hex(Rc::from(reported.value()));
    let verdict = report::<Web>(
      node,
      Report::Change {
        content: COLOR.content(),
        proposed: COLOR.erase(&proposed),
        applied: true,
      },
    );

    verdicts.borrow_mut().push(verdict);
  });

  let _ = input.add_event_listener_with_callback("input", on_input.as_ref().unchecked_ref());

  on_input.forget();
}

/// A view of a test's own adopting a real `<input type=color>` it made through the
/// native door, its value controlled as [`control_color`] controls it.
fn color_well(
  truth: impl Fn() -> Hex + 'static,
  ask: impl Fn(&Hex) + 'static,
  writes: Rc<Cell<usize>>,
  verdicts: Rc<RefCell<Vec<Verdict>>>,
) -> NodeId {
  let input: HtmlInputElement = document()
    .create_element("input")
    .expect("an input")
    .dyn_into()
    .expect("an input element");

  input.set_type("color");

  let spec = NodeSpec::new(
    None,
    None,
    None,
    NodeFacts::Box(BoxFacts {
      layout: Layout::Document,
      container: None,
      transition_name: None,
      link: false,
      acting: false,
    }),
  );
  let node = LowLevel::<Web>::adopt(&spec, input.clone().into());

  control_color(node, &input, truth, ask, writes, verdicts);

  node
}

/// A view adopted through the native door is controlled under a built-in's contract
/// through its sink: a signal's write shows, a reader's change its toolkit applied is
/// accepted, a refused one is repaired to the truth, and a write of what the view
/// shows is skipped.
#[wasm_bindgen_test]
fn a_door_s_value_is_controlled_through_its_sink() {
  let (_guard, root) = fresh_root();
  let color = Signal::new(Hex(Rc::from("#000000")));
  let locked = Signal::new(false);
  let asked = Rc::new(RefCell::new(Vec::new()));
  let writes = Rc::new(Cell::new(0));
  let verdicts = Rc::new(RefCell::new(Vec::new()));
  let node = mount({
    let asked = Rc::clone(&asked);
    let writes = Rc::clone(&writes);
    let verdicts = Rc::clone(&verdicts);

    move || {
      color_well(
        move || color.get(),
        move |proposed| {
          asked.borrow_mut().push(proposed.clone());

          if !locked.get() {
            color.set(proposed.clone());
          }
        },
        writes,
        verdicts,
      )
    }
  });
  let input: HtmlInputElement = root
    .query_selector("input[type=color]")
    .expect("query the input")
    .and_then(|element| element.dyn_into().ok())
    .expect("the adopted input under the root");

  assert_eq!(
    Web::native(node),
    Some(input.clone().unchecked_into::<Element>()),
    "the node is realized by the view it adopted"
  );
  assert_eq!(writes.get(), 0, "the truth the input shows is not written");

  color.set(Hex(Rc::from("#ff0000")));
  web::end_turn();

  assert_eq!(input.value(), "#ff0000", "a signal's write shows");
  assert_eq!(writes.get(), 1);

  input.set_value("#00ff00");
  dispatch(&input, "input");
  web::end_turn();

  assert_eq!(
    *verdicts.borrow(),
    [Verdict::Accept],
    "the change is accepted"
  );
  assert_eq!(*asked.borrow(), [Hex(Rc::from("#00ff00"))]);
  assert_eq!(color.get(), Hex(Rc::from("#00ff00")), "and taken");
  assert_eq!(input.value(), "#00ff00");
  assert_eq!(
    writes.get(),
    1,
    "the truth the reader's change shows is not written"
  );

  locked.set(true);
  input.set_value("#0000ff");
  dispatch(&input, "input");

  assert_eq!(
    verdicts.borrow().last(),
    Some(&Verdict::Refuse),
    "the change is refused"
  );
  assert_eq!(input.value(), "#00ff00", "and repaired inside the event");
  assert_eq!(writes.get(), 2);

  web::end_turn();

  assert_eq!(input.value(), "#00ff00");
  assert_eq!(color.get(), Hex(Rc::from("#00ff00")));
  assert_eq!(writes.get(), 2, "the repair is written once");
}

/// A native view's element is made from its markup and handed, once adopted, to its
/// adopt handler, where a sink of the view's own controls its value under a built-in's
/// contract: a signal's write shows and a refused change is repaired to the truth.
#[wasm_bindgen_test]
fn a_native_view_is_adopted_with_its_attributes_and_controlled_through_its_sink() {
  let (_guard, root) = fresh_root();
  let color = Signal::new(Hex(Rc::from("#000000")));
  let writes = Rc::new(Cell::new(0));
  let verdicts = Rc::new(RefCell::new(Vec::new()));
  let handed = Rc::new(RefCell::new(None));
  let node = mount({
    let writes = Rc::clone(&writes);
    let verdicts = Rc::clone(&verdicts);
    let handed = Rc::clone(&handed);

    move || {
      web::NativeView::new("input")
        .add_attribute("type", "color")
        .add_attribute("aria-label", "Ink")
        .add_attribute("aria-label", "Paint")
        .handle_adopt(move |node, element| {
          *handed.borrow_mut() = Some(element.clone());
          control_color(
            node,
            element.unchecked_ref(),
            move || color.get(),
            |_| {},
            writes,
            verdicts,
          );
        })
        .build()
    }
  });
  let input: HtmlInputElement = root
    .query_selector("input")
    .expect("query the input")
    .and_then(|element| element.dyn_into().ok())
    .expect("the native view's input under the root");

  assert_eq!(input.type_(), "color", "made from its markup");
  assert_eq!(
    input.get_attribute("aria-label").as_deref(),
    Some("Paint"),
    "an attribute added again replaces the first"
  );
  assert_eq!(
    handed.borrow().as_ref(),
    Some(input.unchecked_ref::<Element>()),
    "the element in hand is the one adopted"
  );
  assert_eq!(Web::native(node), handed.borrow().clone());
  assert_eq!(writes.get(), 0, "the truth the input shows is not written");

  color.set(Hex(Rc::from("#ff0000")));
  web::end_turn();

  assert_eq!(input.value(), "#ff0000", "a signal's write shows");

  input.set_value("#00ff00");
  dispatch(&input, "input");

  assert_eq!(
    *verdicts.borrow(),
    [Verdict::Refuse],
    "the change is refused"
  );
  assert_eq!(input.value(), "#ff0000", "and repaired inside the event");
  assert_eq!(writes.get(), 2);
}

/// A radio choice the handler answers with another key is repaired by one write of the
/// group's chosen key: exactly the truth's radio is checked, neither the one the reader
/// chose nor the one chosen before.
#[wasm_bindgen_test]
fn a_refused_radio_choice_checks_exactly_the_truths_radio() {
  let (_guard, root) = fresh_root();
  let chosen = Signal::new("mint");
  let asked = Rc::new(RefCell::new(Vec::new()));

  let _ =
    mount({
      let asked = Rc::clone(&asked);

      move || {
        BasicRadioGroup::new()
          .derive_chosen(move || chosen.get())
          .handle_choose(move |value| {
            asked.borrow_mut().push(value);
            chosen.set("plum");
          })
          .set_child(Column::new().set_children(
            ["mint", "fig", "plum"].map(|flavor| BasicRadio::new(Radio::new(flavor))),
          ))
          .build()
      }
    });

  let radios = root
    .query_selector_all("input[type=radio]")
    .expect("query the radios");
  let checked = || {
    (0..radios.length())
      .filter_map(|index| radios.item(index)?.dyn_into::<HtmlInputElement>().ok())
      .map(|radio| radio.checked())
      .collect::<Vec<_>>()
  };
  let radio = |index: u32| -> HtmlInputElement {
    radios
      .item(index)
      .and_then(|node| node.dyn_into().ok())
      .expect("a radio")
  };

  assert_eq!(checked(), [true, false, false]);

  radio(1).click();

  assert_eq!(*asked.borrow(), ["fig"], "the choice was asked for");
  assert_eq!(
    checked(),
    [false, false, true],
    "the truth's radio alone, inside the event"
  );

  web::end_turn();

  assert_eq!(checked(), [false, false, true]);
  assert_eq!(chosen.get(), "plum");
}

/// A click on a native toggle that something cancels has its flip restored by the
/// browser once the dispatch ends, over the truth its handlers wrote; the checkbox then
/// shows the truth again.
#[wasm_bindgen_test]
async fn a_cancelled_click_shows_the_truth_its_handlers_wrote() {
  let (_guard, root) = fresh_root();
  let checked = Signal::new(false);

  let _ = mount(move || {
    BasicCheckbox::new(Toggle::new().bind_checked(checked))
      .handle_activate(move || checked.set(true))
      .build()
  });

  let input = input_of(&root);
  let cancel = Closure::<dyn Fn(web_sys::Event)>::new(|event: web_sys::Event| {
    event.prevent_default();
  });
  let window = web_sys::window().expect("a window");

  window
    .add_event_listener_with_callback("click", cancel.as_ref().unchecked_ref())
    .expect("listen for the click");
  input.click();
  window
    .remove_event_listener_with_callback("click", cancel.as_ref().unchecked_ref())
    .expect("stop listening");

  assert!(checked.get(), "the handler wrote the truth");
  assert!(!input.checked(), "the browser restored the cancelled flip");
  assert!(
    frames_until(|| input.checked()).await,
    "the checkbox shows the truth again"
  );
}

/// A key an input method is composing leaves the behavior alone and its default to the
/// platform; the same key composed acts.
#[wasm_bindgen_test]
fn a_composing_key_never_reaches_the_behavior() {
  let (_guard, root) = fresh_root();
  let bound = Signal::new(50.0f32);

  let _ = mount(move || BasicSlider::new(Slider::new().bind_value(bound)).build());

  let input = input_of(&root);
  let press = |composing: bool| {
    let init = KeyboardEventInit::new();

    init.set_bubbles(true);
    init.set_cancelable(true);
    init.set_key("ArrowRight");
    init.set_is_composing(composing);

    let event = KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).expect("a key");
    let _ = input.dispatch_event(&event);

    event.default_prevented()
  };

  assert!(!press(true), "a composing key keeps its default");
  assert!((bound.get() - 50.).abs() < f32::EPSILON);
  assert!(press(false));
  assert!((bound.get() - 51.).abs() < f32::EPSILON);
}

/// A text field is its `<input>`: its placeholder an attribute, no role of its own, its
/// change writing the bound signal and never reaching a box's click, and the signal
/// writing its value; an editor is a `<textarea>` showing its lines.
#[wasm_bindgen_test]
fn a_text_fields_change_writes_the_signal_and_stays_on_its_node() {
  let (_guard, root) = fresh_root();
  let bound = Signal::new(String::new());
  let clicks = Rc::new(Cell::new(0u32));
  let counted = Rc::clone(&clicks);

  let _ = mount(move || {
    Column::new()
      .set_children((
        Container::new()
          .handle_click(move || counted.set(counted.get() + 1))
          .set_child(BasicTextField::new(
            TextEntry::new().set_placeholder("Name").bind_value(bound),
          )),
        BasicTextEditor::new(TextEntry::new().set_lines(4)),
      ))
      .build()
  });

  let input = input_of(&root);

  assert_eq!(input.value(), "");
  assert_eq!(input.placeholder(), "Name");
  assert_eq!(input.get_attribute("role"), None);

  input.set_value("typed");
  dispatch(&input, "input");

  assert_eq!(bound.get(), "typed");
  assert_eq!(clicks.get(), 0, "the change is the field's alone");

  bound.set("fresh".to_owned());
  web::end_turn();

  assert_eq!(input.value(), "fresh");

  let editor = root
    .query_selector("textarea")
    .expect("query")
    .expect("the editor's textarea");

  assert_eq!(editor.get_attribute("rows").as_deref(), Some("4"));
}

/// A field whose secure flag flips keeps what it shows: its `type` switches between
/// `text` and `password` with the value and the caret where they were, and an edit made
/// while secure reports its text and selection as any field's does.
#[wasm_bindgen_test]
fn a_fields_secure_flag_keeps_its_value_and_caret() {
  let (_guard, root) = fresh_root();
  let text = Signal::new("secret".to_owned());
  let caret = Signal::new(TextSelection::collapsed(3));
  let secure = Signal::new(false);

  let _ = mount(move || {
    BasicTextField::new(
      TextEntry::new()
        .derive_secure(move || secure.get())
        .bind_value(text)
        .bind_selection(caret),
    )
    .build()
  });

  let input = input_of(&root);
  let range = |input: &HtmlInputElement| {
    (
      input.selection_start().expect("a selection API"),
      input.selection_end().expect("a selection API"),
    )
  };

  html(&input).focus().expect("focus the field");

  assert_eq!(input.type_(), "text");
  assert_eq!(range(&input), (Some(3), Some(3)));

  secure.set(true);
  web::end_turn();

  assert_eq!(input.type_(), "password");
  assert_eq!(input.value(), "secret");
  assert_eq!(range(&input), (Some(3), Some(3)), "the caret stays");

  input.set_value("secrets");
  input
    .set_selection_range(7, 7)
    .expect("a password input selects");
  dispatch(&input, "input");

  assert_eq!(text.get(), "secrets");
  assert_eq!(caret.get(), TextSelection::collapsed(7));

  secure.set(false);
  web::end_turn();

  assert_eq!(input.type_(), "text");
  assert_eq!(input.value(), "secrets");
  assert_eq!(range(&input), (Some(7), Some(7)));
}

/// A number's keyboard keeps the field a text input, the keyboard named by
/// `inputmode`, so an edit still reports its selection; a URL's switches the input to
/// its own type, which keeps the selection API too.
#[wasm_bindgen_test]
fn a_number_keyboard_keeps_a_text_input_reporting_its_selection() {
  let (_guard, root) = fresh_root();
  let text = Signal::new(String::new());
  let caret = Signal::new(TextSelection::collapsed(0));
  let keyboard = Signal::new(Keyboard::NUMBER);

  let _ = mount(move || {
    BasicTextField::new(
      TextEntry::new()
        .derive_keyboard(move || keyboard.get())
        .bind_value(text)
        .bind_selection(caret),
    )
    .build()
  });

  let input = input_of(&root);

  assert_eq!(input.type_(), "text");
  assert_eq!(input.get_attribute("inputmode").as_deref(), Some("numeric"));

  input.set_value("42");
  input
    .set_selection_range(1, 2)
    .expect("a text input selects");
  dispatch(&input, "input");

  assert_eq!(text.get(), "42");
  assert_eq!(caret.get(), TextSelection::new(1, 2));

  keyboard.set(Keyboard::URL.with_autofill(Autofill::Url));
  web::end_turn();

  assert_eq!(input.type_(), "url");
  assert_eq!(input.get_attribute("autocomplete").as_deref(), Some("url"));

  input.set_value("a.b");
  input
    .set_selection_range(0, 1)
    .expect("a URL input selects");
  dispatch(&input, "input");

  assert_eq!(text.get(), "a.b");
  assert_eq!(caret.get(), TextSelection::new(0, 1));
}

/// A text entry keeping its label inside its box.
struct InlineEntry;

impl Behavior for InlineEntry {
  fn native(&self) -> Option<NativeElement> {
    Some(NativeElement::TextField)
  }

  fn role(&self) -> Option<Role> {
    None
  }

  fn listens(&self) -> &'static [EventKind] {
    &[]
  }

  fn label_placement(&self) -> LabelPlacement {
    LabelPlacement::Trailing
  }
}

/// A behavior places its label: a text entry's sits beside its box, before it among its
/// siblings, a button's inside, and a third party's text entry may keep its label
/// inside instead.
#[wasm_bindgen_test]
fn a_behavior_places_its_label() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Column::new()
      .set_children((
        BasicTextField::new(TextEntry::new()).set_label("Name"),
        BasicButton::new(Activate::new()).set_label("Save"),
        Control::new(InlineEntry).set_label("Inline"),
      ))
      .build()
  });

  let column = only_child(&root);
  let siblings: Vec<String> = elements(&column)
    .iter()
    .map(|element| {
      format!(
        "{}:{}",
        element.tag_name(),
        element.text_content().unwrap_or_default()
      )
    })
    .collect();

  assert_eq!(siblings[0], "LABEL:Name", "{siblings:?}");
  assert_eq!(siblings[1], "INPUT:", "{siblings:?}");
  assert_eq!(
    siblings[2], "BUTTON:Save",
    "the button's caption is inside: {siblings:?}"
  );
  assert_eq!(
    siblings.len(),
    4,
    "the inline caption stands beside nothing: {siblings:?}"
  );
  assert!(
    siblings[3].ends_with("Inline"),
    "the inline caption is inside its box: {siblings:?}"
  );
}

/// A control's parts, body and label run across half an em apart unless the control
/// lays them out otherwise.
#[wasm_bindgen_test]
fn a_control_lays_out_its_row() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Column::new()
      .set_children((
        BasicButton::new(Activate::new()).set_label("a"),
        BasicButton::new(Activate::new())
          .set_axis(Axis::Vertical)
          .set_spacing(Spacing::all(3))
          .set_item_alignment(ItemAlignment::Start)
          .set_label("b"),
      ))
      .build()
  });

  let buttons = elements(&only_child(&root));
  let (plain, spaced) = (&buttons[0], &buttons[1]);
  let em = computed(plain, "font-size")
    .trim_end_matches("px")
    .parse::<f32>()
    .expect("a font size");

  assert_eq!(computed(plain, "flex-direction"), "row");
  assert_eq!(computed(plain, "column-gap"), format!("{}px", em / 2.));
  assert_eq!(computed(spaced, "flex-direction"), "column");
  assert_eq!(computed(spaced, "row-gap"), "3px");
  assert_eq!(computed(spaced, "align-items"), "flex-start");
}

/// The face reports what it opens: a listbox popup, and the listbox it controls.
#[wasm_bindgen_test]
fn the_face_reports_its_popup_and_the_list_it_controls() {
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
  let controlled = face
    .get_attribute("aria-controls")
    .expect("the face names the list it controls");
  let list = document()
    .get_element_by_id(&controlled)
    .expect("the controlled element");

  assert_eq!(
    face.get_attribute("aria-haspopup").as_deref(),
    Some("listbox")
  );
  assert_eq!(list.get_attribute("role").as_deref(), Some("listbox"));
  assert_eq!(
    list
      .query_selector_all("[role=option]")
      .expect("query")
      .length(),
    2
  );
}

/// A popup anchored to a bound box opens against the anchor's element: through CSS
/// anchor positioning where the engine has it, at the anchor's measured rectangle where
/// it does not; a derived placement re-places it while it stays open.
#[wasm_bindgen_test]
fn a_popup_anchored_to_a_bound_box_passes_the_anchors_sink() {
  let (_guard, root) = fresh_root();
  let open = Signal::new(false);
  let placement = Signal::new(Placement::Bottom);
  let anchor = NodeHandle::new();

  let _ = mount(move || {
    Column::new()
      .set_children((
        Container::new().set_height(120),
        Container::new()
          .bind_handle(anchor)
          .set_width(160)
          .set_height(24)
          .set_child(Text::new("anchor")),
        Overlay::popup()
          .set_anchor(anchor)
          .derive_placement(move || placement.get())
          .bind_open(open)
          .set_child(Text::new("popup")),
      ))
      .build()
  });

  let column = only_child(&root);
  let boxed = elements(&column)[1].clone();
  let popup = root
    .query_selector("[popover]")
    .expect("query")
    .expect("the popup");
  let is_open = || popup.matches(":popover-open").unwrap_or(false);

  assert!(
    anchor.node().is_some(),
    "the handle names the anchor's node"
  );
  assert_ne!(
    popup.tag_name().to_lowercase(),
    "dialog",
    "a popup is no modal"
  );
  assert!(!is_open());

  open.set(true);
  web::end_turn();

  assert!(is_open());

  let edges = |element: &Element| {
    let rect = element.get_bounding_client_rect();

    (rect.left(), rect.top(), rect.bottom())
  };
  let (anchor_left, anchor_top, anchor_bottom) = edges(&boxed);
  let (left, top, _) = edges(&popup);

  assert!(
    (top - anchor_bottom).abs() < 1. && (left - anchor_left).abs() < 1.,
    "below the anchor, start-aligned: popup {:?} anchor {:?}",
    (left, top),
    (anchor_left, anchor_bottom)
  );

  if web_sys::css::supports_with_value("position-area", "block-end").unwrap_or(false) {
    let name = html(&boxed)
      .style()
      .get_property_value("anchor-name")
      .unwrap();

    assert!(name.starts_with("--skene-anchor-"), "{name}");
    assert_eq!(
      html(&popup)
        .style()
        .get_property_value("position-anchor")
        .unwrap(),
      name,
      "the popup follows the anchor's sink through CSS anchor positioning"
    );
  }
  else {
    assert!(
      html(&popup)
        .style()
        .get_property_value("top")
        .unwrap()
        .ends_with("px"),
      "the measured rectangle stands in for the engine's"
    );
  }

  placement.set(Placement::Top);
  web::end_turn();

  let (_, _, bottom) = edges(&popup);

  assert!(is_open(), "re-placing keeps it open");
  assert!(
    (bottom - anchor_top).abs() < 1.,
    "above the anchor: popup bottom {bottom} anchor top {anchor_top}"
  );
}

/// A tooltip describes the element that takes focus within the view it wraps, not the
/// view's outer box: its `aria-describedby` names the bubble, a hint popover holding
/// the text, and no `title` stands in for it once live.
#[wasm_bindgen_test]
fn a_tooltip_describes_the_focus_holder_within_its_view() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Tooltip::new("Saves your work")
      .set_child(
        Container::new().set_child(
          Container::new()
            .set_focus_order(FocusOrder::Auto)
            .set_child(Text::new("Save")),
        ),
      )
      .build()
  });

  let outer = only_child(&root);
  let holder = outer
    .query_selector("[tabindex]")
    .expect("query")
    .expect("the focusable box");
  let bubbles = described_by(&holder);

  assert_eq!(bubbles.len(), 1, "the focus holder names the bubble");
  assert_eq!(bubbles[0].get_attribute("popover").as_deref(), Some("hint"));
  assert_eq!(
    bubbles[0].text_content().as_deref(),
    Some("Saves your work")
  );
  assert_eq!(
    outer.get_attribute("aria-describedby"),
    None,
    "the outer box does not"
  );
  assert_eq!(
    holder.get_attribute("title"),
    None,
    "no browser tooltip once live"
  );
  assert_eq!(
    root
      .query_selector_all("[aria-describedby]")
      .expect("query")
      .length(),
    1
  );
}

/// A derived text tooltip re-describes as its text changes and its report sits beside
/// the control's own through a rewrite of either; a view tooltip's bubble holds its view.
#[wasm_bindgen_test]
fn a_tooltips_report_sits_beside_the_controls_own() {
  let (_guard, root) = fresh_root();
  let disabled = Signal::new(false);
  let text = Signal::new("Saves".to_owned());

  let _ = mount(move || {
    Column::new()
      .set_children((
        Tooltip::derive(move || text.get()).set_child(
          BasicButton::new(Activate::new())
            .derive_disabled(move || disabled.get())
            .set_label("Save"),
        ),
        Tooltip::view(Text::new("rich")).set_child(Container::new().set_child(Text::new("plain"))),
      ))
      .build()
  });

  let described = root
    .query_selector_all("[aria-describedby]")
    .expect("query");

  assert_eq!(described.length(), 2);

  let element = |index: u32| -> Element {
    described
      .item(index)
      .expect("a described element")
      .dyn_into()
      .expect("an element")
  };
  let (button, plain) = (element(0), element(1));
  let bubble = described_by(&button);

  assert_eq!(bubble.len(), 1);
  assert_eq!(bubble[0].text_content().as_deref(), Some("Saves"));

  disabled.set(true);
  web::end_turn();

  assert!(
    button.has_attribute("disabled")
      || button.get_attribute("aria-disabled").as_deref() == Some("true"),
    "the control's own report"
  );
  assert_eq!(
    described_by(&button),
    bubble,
    "the description survives the control's rewrite"
  );

  text.set("Saves the document".to_owned());
  web::end_turn();

  assert_eq!(
    bubble[0].text_content().as_deref(),
    Some("Saves the document")
  );
  assert_eq!(described_by(&button), bubble);

  let rich = described_by(&plain);

  assert_eq!(rich.len(), 1, "a view tooltip names its bubble");
  assert_eq!(rich[0].text_content().as_deref(), Some("rich"));
}

/// A control's own description and a tooltip's fold into one `aria-describedby`, the
/// control's first, through a rewrite of the control's report.
#[wasm_bindgen_test]
fn a_controls_description_and_a_tooltips_fold_into_one_list() {
  let (_guard, root) = fresh_root();
  let disabled = Signal::new(false);

  let _ = mount(move || {
    let hint = Text::new("Required").build();

    Column::new()
      .set_children((
        AnyView::node(hint),
        Tooltip::new("Saves").set_child(
          BasicButton::new(Activate::new())
            .derive_disabled(move || disabled.get())
            .set_label("Save")
            .set_described_by(hint),
        ),
      ))
      .build()
  });

  let button = root
    .query_selector("[aria-describedby]")
    .expect("query")
    .expect("the described control");
  let texts = || {
    described_by(&button)
      .iter()
      .map(|element| element.text_content().unwrap_or_default())
      .collect::<Vec<_>>()
  };

  assert_eq!(texts(), ["Required", "Saves"]);

  disabled.set(true);
  web::end_turn();

  assert_eq!(
    texts(),
    ["Required", "Saves"],
    "through the control's rewrite"
  );
}

/// Whether the engine's own `matchMedia` matches `query`.
fn matches(query: &str) -> bool {
  web_sys::window()
    .expect("a window")
    .match_media(query)
    .ok()
    .flatten()
    .is_some_and(|list| list.matches())
}

/// Asserts that `signal` answers as the engine reports `on`, and that where the engine
/// reports `off`, the other answer, the signal holds false.
fn answers(signal: bool, on: &str, off: &str) {
  assert_eq!(signal, matches(on), "{on}");

  if matches(off) {
    assert!(!signal, "{off}");
  }
}

/// The web medium's own properties reach the engine: a box's corner radius and a run's
/// font size compute as set.
#[wasm_bindgen_test]
fn the_pixel_properties_reach_the_engine() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Container::new()
      .set_corner_radius(8)
      .set_child(Text::new("big").set_font_size(24.))
      .build()
  });

  let container = only_child(&root);

  assert_eq!(computed(&container, "border-top-left-radius"), "8px");
  assert_eq!(computed(&only_child(&container), "font-size"), "24px");
}

/// Reduced transparency and more contrast answer as the engine reports them: the
/// signals, a watch and a property's condition alike.
#[wasm_bindgen_test]
fn the_transparency_and_contrast_preferences_answer_as_the_engine_reports() {
  let (_guard, root) = fresh_root();
  let (red, green, blue) = (
    Color::srgb(1., 0., 0.),
    Color::srgb(0., 1., 0.),
    Color::srgb(0., 0., 1.),
  );
  let watched = Rc::new(Cell::new(None));

  let _ = mount({
    let watched = Rc::clone(&watched);

    move || {
      watched.set(Some(media::watch(&Media::HighContrast)));

      Container::new()
        .set_background(
          Value::from(red)
            .when(Condition::from(Media::ReducedTransparency), green)
            .when(Condition::from(Media::HighContrast), blue),
        )
        .build()
    }
  });

  let transparency = media::reduced_transparency().get();
  let contrast = media::high_contrast().get();

  answers(
    transparency,
    "(prefers-reduced-transparency: reduce)",
    "(prefers-reduced-transparency: no-preference)",
  );
  answers(
    contrast,
    "(prefers-contrast: more)",
    "(prefers-contrast: no-preference)",
  );
  assert_eq!(watched.get().expect("the watch").get(), contrast);

  let expected = if contrast {
    blue
  }
  else if transparency {
    green
  }
  else {
    red
  };

  assert_eq!(
    background(&only_child(&root)),
    probe(&root, "background-color", expected)
  );
}

/// The primary pointer's precision and whether it hovers answer as the engine reports
/// them: the signals, a watch and a property's condition alike.
#[wasm_bindgen_test]
fn the_pointer_precision_and_hover_answer_as_the_engine_reports() {
  let (_guard, root) = fresh_root();
  let (red, green, blue) = (
    Color::srgb(1., 0., 0.),
    Color::srgb(0., 1., 0.),
    Color::srgb(0., 0., 1.),
  );
  let watched = Rc::new(Cell::new(None));

  let _ = mount({
    let watched = Rc::clone(&watched);

    move || {
      watched.set(Some(media::watch(&Media::Hover)));

      Container::new()
        .set_background(
          Value::from(red)
            .when(
              Condition::from(Media::Pointer(PointerPrecision::Coarse)),
              green,
            )
            .when(
              Condition::from(Media::Pointer(PointerPrecision::Absent)),
              blue,
            ),
        )
        .build()
    }
  });

  let pointer = media::pointer().get();
  let hover = media::hover().get();

  assert_eq!(
    pointer == PointerPrecision::Fine,
    matches("(pointer: fine)")
  );
  assert_eq!(
    pointer == PointerPrecision::Coarse,
    matches("(pointer: coarse)")
  );

  if matches("(pointer: none)") {
    assert_eq!(pointer, PointerPrecision::Absent);
  }

  answers(hover, "(hover: hover)", "(hover: none)");
  assert_eq!(watched.get().expect("the watch").get(), hover);

  let expected = match pointer {
    PointerPrecision::Fine => red,
    PointerPrecision::Coarse => green,
    PointerPrecision::Absent => blue,
  };

  assert_eq!(
    background(&only_child(&root)),
    probe(&root, "background-color", expected)
  );
}

/// A length reading the safe area computes from the insets the engine reports, as the
/// signal does.
#[wasm_bindgen_test]
fn a_length_reads_the_safe_area() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    Container::new()
      .set_padding(Expr::<Length>::safe_area(Edge::Top) + 4)
      .build()
  });

  let reported = document().create_element("div").expect("a probe");

  let _ = reported.set_attribute(
    "style",
    "padding-top:env(safe-area-inset-top,0px);padding-bottom:env(safe-area-inset-bottom,0px)",
  );
  let _ = root.append_child(&reported);

  let inset = media::safe_area().get();

  assert_eq!(
    computed(&reported, "padding-top"),
    format!("{}px", inset.top)
  );
  assert_eq!(
    computed(&reported, "padding-bottom"),
    format!("{}px", inset.bottom)
  );
  assert_eq!(
    computed(&only_child(&root), "padding-top"),
    format!("{}px", inset.top + 4.)
  );

  reported.remove();
}

/// The focus events the engine fires anywhere in the document, each recorded with
/// whether the scene was borrowed as it fired, until the recorder drops.
struct FocusEvents {
  /// Each event's type and whether the scene was borrowed as it fired, in order.
  record: Rc<RefCell<Vec<(String, bool)>>>,
  /// The capturing listener on the window, removed on drop.
  listener: Closure<dyn Fn(web_sys::Event)>,
}

impl FocusEvents {
  /// The kinds of focus event the engine fires.
  const TYPES: [&str; 4] = ["focus", "blur", "focusin", "focusout"];

  /// Starts recording.
  fn new() -> Self {
    let record = Rc::new(RefCell::new(Vec::new()));
    let listener = Closure::<dyn Fn(web_sys::Event)>::new({
      let record = Rc::clone(&record);

      move |event: web_sys::Event| {
        record
          .borrow_mut()
          .push((event.type_(), skene_foundation::Scene::<Web>::is_borrowed()));
      }
    });
    let window = web_sys::window().expect("a window");

    for name in Self::TYPES {
      window
        .add_event_listener_with_callback_and_bool(name, listener.as_ref().unchecked_ref(), true)
        .expect("listen for a focus event");
    }

    Self { record, listener }
  }

  /// The events recorded so far.
  fn events(&self) -> Vec<(String, bool)> {
    self.record.borrow().clone()
  }

  /// Asserts that every event recorded so far fired with the scene released.
  fn assert_released(&self) {
    let events = self.events();

    assert!(
      events.iter().all(|(_, borrowed)| !borrowed),
      "an engine's focus event fired with the scene borrowed: {events:?}"
    );
  }
}

impl Drop for FocusEvents {
  fn drop(&mut self) {
    if let Some(window) = web_sys::window() {
      for name in Self::TYPES {
        let _ = window.remove_event_listener_with_callback_and_bool(
          name,
          self.listener.as_ref().unchecked_ref(),
          true,
        );
      }
    }
  }
}

/// A focus a handler asks for runs the engine's focus and blur events at once, each
/// firing with the scene released, and they reach the scene.
#[wasm_bindgen_test]
fn a_focus_from_a_handler_fires_its_events_with_the_scene_released() {
  let (_guard, root) = fresh_root();
  let log = Rc::new(RefCell::new(Vec::new()));
  let (first, second) = (NodeHandle::new(), NodeHandle::new());
  let note = |name: &'static str| {
    let log = Rc::clone(&log);

    move || log.borrow_mut().push(name)
  };

  let _ = mount({
    let (focused, blurred, reached) = (
      note("first focus"),
      note("first blur"),
      note("second focus"),
    );

    move || {
      Column::new()
        .set_children((
          Container::new()
            .set_focus_order(FocusOrder::Auto)
            .bind_handle(first)
            .handle_focus(move || {
              focused();
              second.focus();
            })
            .handle_blur(blurred),
          Container::new()
            .set_focus_order(FocusOrder::Auto)
            .bind_handle(second)
            .handle_focus(reached),
        ))
        .build()
    }
  });

  let boxes = elements(&only_child(&root));
  let events = FocusEvents::new();

  html(&boxes[0]).focus().expect("focus the first box");

  assert_eq!(*log.borrow(), ["first focus", "first blur", "second focus"]);
  assert!(
    events.events().iter().any(|(kind, _)| kind == "focusin"),
    "the engine fired its focus events"
  );
  events.assert_released();
  assert_eq!(document().active_element(), Some(boxes[1].clone()));
  assert_eq!(
    skene_foundation::Scene::<Web>::read(skene_foundation::Scene::focused),
    second.node()
  );
}

/// Moving the focused element and removing it run with the scene released: whatever
/// focus event the engine fires on the way fires with the scene released, the elements
/// land in the list's order, and once the focused node is removed nothing holds focus.
#[wasm_bindgen_test]
fn moving_and_removing_the_focused_element_run_with_the_scene_released() {
  let (_guard, root) = fresh_root();
  let order = Signal::new(vec![1, 2, 3]);
  let handles = [NodeHandle::new(), NodeHandle::new(), NodeHandle::new()];
  let log = Rc::new(RefCell::new(Vec::new()));

  let _ = mount({
    let log = Rc::clone(&log);

    move || {
      Column::new()
        .derive_children(
          move || order.get(),
          |item: &usize| *item,
          move |item| {
            let log = Rc::clone(&log);

            Container::new()
              .set_focus_order(FocusOrder::Auto)
              .bind_handle(handles[item - 1])
              .handle_blur(move || log.borrow_mut().push(item))
              .set_child(Text::new(item.to_string()))
          },
        )
        .build()
    }
  });

  let column = only_child(&root);
  let texts = || {
    elements(&column)
      .iter()
      .map(|element| element.text_content().unwrap_or_default())
      .collect::<Vec<_>>()
  };
  let element = |item: usize| {
    elements(&column)[texts()
      .iter()
      .position(|text| *text == item.to_string())
      .expect("the item")]
    .clone()
  };

  html(&element(3)).focus().expect("focus the last item");

  let events = FocusEvents::new();

  order.set(vec![3, 1, 2]);
  web::end_turn();

  assert_eq!(texts(), ["3", "1", "2"]);
  events.assert_released();

  html(&element(1))
    .focus()
    .expect("focus the moved-past item");

  let removed = handles[0].node().expect("the first item");

  assert_eq!(
    skene_foundation::Scene::<Web>::read(skene_foundation::Scene::focused),
    Some(removed)
  );

  order.set(vec![3, 2]);
  web::end_turn();

  assert_eq!(texts(), ["3", "2"]);
  events.assert_released();
  assert!(
    log.borrow().iter().all(|item| [1, 3].contains(item)),
    "only the items that held focus blur: {:?}",
    log.borrow()
  );
  assert!(log.borrow().iter().filter(|item| **item == 1).count() <= 1);
  assert!(!skene_foundation::Scene::<Web>::read(
    |scene| scene.contains(removed)
  ));
  assert_eq!(
    skene_foundation::Scene::<Web>::read(skene_foundation::Scene::focused),
    None,
    "the removed node held focus, and nothing holds it now"
  );

  html(&element(2)).focus().expect("focus a remaining item");

  assert_eq!(
    skene_foundation::Scene::<Web>::read(skene_foundation::Scene::focused),
    handles[1].node(),
    "focus still reaches the scene"
  );
}

/// The computed `property` a `<div>` under `parent` gets from the CSS `value`, so a
/// built-in's default compares as the engine resolves it.
fn resolved(parent: &Element, property: &str, value: &str) -> String {
  let element = document().create_element("div").expect("a probe");

  let _ = element.set_attribute("style", &format!("{property}:{value}"));
  let _ = parent.append_child(&element);

  let color = computed(&element, property);

  element.remove();

  color
}

/// The computed `property` a `<div>` under `parent` gets from the platform's color
/// `token`.
fn system(parent: &Element, property: &str, token: TokenRef<Color>) -> String {
  resolved(
    parent,
    property,
    &format!("var(--{})", token.name().as_str()),
  )
}

/// Every element under `parent` matching `selector`, in document order.
fn select(parent: &Element, selector: &str) -> Vec<Element> {
  let found = parent.query_selector_all(selector).expect("a selector");

  (0..found.length())
    .filter_map(|index| found.item(index)?.dyn_into().ok())
    .collect()
}

/// The built-in button is the page's own button, drawn with its defaults in the
/// platform's colors, and a setter its user calls replaces the default it overrides
/// while the rest stand.
#[wasm_bindgen_test]
fn a_buttons_defaults_apply_under_the_setters_its_user_calls() {
  let (_guard, root) = fresh_root();
  let red = Color::srgb(1., 0., 0.);
  let white = Color::srgb(1., 1., 1.);

  let _ = mount(move || {
    Row::new()
      .set_children((
        skene_web::Button::new("Save"),
        skene_web::Button::new("Stop")
          .set_background(red)
          .set_padding(Inset::all(3))
          .set_text_color(white),
      ))
      .build()
  });

  let buttons = select(&root, "button");
  let [save, stop] = buttons.as_slice()
  else {
    panic!("two buttons: {}", root.inner_html());
  };
  let label = |button: &Element| {
    button
      .query_selector("span")
      .ok()
      .flatten()
      .expect("a label")
  };

  assert_eq!(save.text_content().as_deref(), Some("Save"));
  assert_eq!(
    background(save),
    system(&root, "background-color", SystemColors::CONTROL)
  );
  assert_eq!(
    computed(&label(save), "color"),
    system(&root, "color", SystemColors::INK)
  );
  assert_eq!(computed(save, "padding-left"), "12px");
  assert_eq!(computed(save, "padding-top"), "4px");
  assert_eq!(computed(save, "border-top-width"), "2px");
  assert_eq!(computed(save, "border-top-left-radius"), "6px");
  assert_eq!(computed(save, "min-height"), "32px");
  assert_eq!(computed(save, "cursor"), "pointer");

  assert_eq!(background(stop), probe(&root, "background-color", red));
  assert_eq!(
    computed(&label(stop), "color"),
    probe(&root, "color", white)
  );
  assert_eq!(
    computed(stop, "padding-left"),
    "3px",
    "a padding its user sets"
  );
  assert_eq!(
    computed(stop, "border-top-left-radius"),
    "6px",
    "the defaults it leaves stand"
  );
}

/// A prominence the built-in button derives re-inks it in place: the accent's fill,
/// darkened, under the ink that sits on it, on the same element.
#[wasm_bindgen_test]
fn a_buttons_prominence_changes_its_fill_in_place() {
  let (_guard, root) = fresh_root();
  let prominent = Signal::new(false);

  let _ = mount(move || {
    skene_web::Button::new("Go")
      .derive_prominent(move || prominent.get())
      .build()
  });

  let button = only_child(&root);
  let label = only_child(&button);

  assert_eq!(button.tag_name(), "BUTTON");
  assert_eq!(
    background(&button),
    system(&root, "background-color", SystemColors::CONTROL)
  );

  prominent.set(true);
  web::end_turn();

  assert!(only_child(&root) == button, "the same button");
  assert_eq!(
    background(&button),
    resolved(
      &root,
      "background-color",
      &format!(
        "oklch(from var(--{}) calc(l - 0.1) c h / alpha)",
        SystemColors::ACCENT.name().as_str()
      )
    )
  );
  assert_ne!(
    background(&button),
    system(&root, "background-color", SystemColors::CONTROL)
  );
  assert_eq!(
    computed(&label, "color"),
    system(&root, "color", SystemColors::ON_ACCENT)
  );

  prominent.set(false);
  web::end_turn();

  assert_eq!(
    background(&button),
    system(&root, "background-color", SystemColors::CONTROL)
  );
}

/// The built-in checkbox draws its default mark beside the page's own input, filled
/// with the field's color and with the accent once checked, and a mark its user sets
/// takes the default's place.
#[wasm_bindgen_test]
fn a_checkboxs_default_mark_follows_it_and_a_mark_of_its_users_replaces_it() {
  let (_guard, root) = fresh_root();
  let checked = Signal::new(false);
  let red = Color::srgb(1., 0., 0.);

  let _ = mount(move || {
    Column::new()
      .set_children((
        skene_web::Checkbox::new()
          .set_label("Agree")
          .bind_checked(checked),
        skene_web::Checkbox::new().set_label("Mine").set_part(
          skene_web::Part::Mark,
          Container::new()
            .set_width(skene_web::Dimension::Length(10.))
            .set_height(skene_web::Dimension::Length(10.))
            .set_background(red),
        ),
      ))
      .build()
  });

  let labels = select(&root, "label");
  let [agree, mine] = labels.as_slice()
  else {
    panic!("two checkboxes: {}", root.inner_html());
  };
  let marks = |label: &Element| select(label, ":scope > div");
  let [mark] = marks(agree).try_into().expect("one mark");

  assert_eq!(computed(&mark, "width"), "18px");
  assert_eq!(
    background(&mark),
    system(&root, "background-color", SystemColors::FIELD)
  );

  checked.set(true);
  web::end_turn();

  assert_eq!(
    background(&mark),
    system(&root, "background-color", SystemColors::ACCENT)
  );

  let [own] = marks(mine).try_into().expect("its user's mark alone");

  assert_eq!(computed(&own, "width"), "10px");
  assert_eq!(background(&own), probe(&root, "background-color", red));
}

/// The built-in radio group lays its radios out in a column beneath its caption, each
/// radio's default mark beside the page's own radio button, ringed in the accent and
/// dotted while chosen, and a mark its user sets takes the default's place.
#[wasm_bindgen_test]
fn a_radio_groups_radios_sit_in_a_column_their_marks_following_the_choice() {
  let (_guard, root) = fresh_root();
  let size = Signal::new(1);
  let red = Color::srgb(1., 0., 0.);

  let _ = mount(move || {
    skene_web::RadioGroup::new()
      .set_label("Size")
      .bind_chosen(size)
      .set_children((
        skene_web::Radio::new(1).set_label("Small"),
        skene_web::Radio::new(2).set_label("Medium"),
        skene_web::Radio::new(3).set_label("Mine").set_part(
          skene_web::Part::Mark,
          Container::new()
            .set_width(skene_web::Dimension::Length(10.))
            .set_height(skene_web::Dimension::Length(10.))
            .set_background(red),
        ),
      ))
      .build()
  });

  let labels = select(&root, "label");
  let [small, medium, mine] = labels.as_slice()
  else {
    panic!("three radios: {}", root.inner_html());
  };
  let (first, second) = (
    small.get_bounding_client_rect(),
    medium.get_bounding_client_rect(),
  );

  assert!(
    second.top() >= first.bottom() + 7.5,
    "a column, spaced: {} then {}",
    first.bottom(),
    second.top()
  );
  assert!((second.left() - first.left()).abs() < 0.5, "at the start");
  assert!(
    select(small, "input[type=radio]").len() == 1,
    "the page's own radio button keeps it scriptless"
  );

  let marks = |label: &Element| select(label, ":scope > div");
  let [chosen] = marks(small).try_into().expect("one mark");
  let [other] = marks(medium).try_into().expect("one mark");
  let dot = |mark: &Element| only_child(mark);

  assert_eq!(computed(&chosen, "width"), "18px");
  assert_eq!(
    computed(&chosen, "border-top-color"),
    system(&root, "border-top-color", SystemColors::ACCENT)
  );
  assert_eq!(
    computed(&other, "border-top-color"),
    system(&root, "border-top-color", SystemColors::BORDER)
  );
  assert_eq!(
    background(&dot(&chosen)),
    system(&root, "background-color", SystemColors::ACCENT)
  );
  assert_eq!(
    background(&dot(&other)),
    probe(&root, "background-color", Color::TRANSPARENT)
  );

  size.set(2);
  web::end_turn();

  assert_eq!(
    background(&dot(&other)),
    system(&root, "background-color", SystemColors::ACCENT),
    "the dot follows the choice"
  );
  assert_eq!(
    computed(&chosen, "border-top-color"),
    system(&root, "border-top-color", SystemColors::BORDER)
  );

  let [own] = marks(mine).try_into().expect("its user's mark alone");

  assert_eq!(computed(&own, "width"), "10px");
  assert_eq!(background(&own), probe(&root, "background-color", red));
}

/// The built-in slider keeps the page's own range input and draws its track, its fill
/// and its thumb in the platform's colors, the thumb travelling the track less its own
/// width and the fill running to it as the value moves, on the same elements; a width
/// its user sets replaces the default one.
#[wasm_bindgen_test]
fn a_sliders_fill_and_thumb_follow_its_value_in_place() {
  let (_guard, root) = fresh_root();
  let volume = Signal::new(0.0f32);

  let _ = mount(move || {
    Column::new()
      .set_item_alignment(skene_web::prelude::ItemAlignment::Start)
      .set_children((
        skene_web::Slider::new()
          .set_label("Volume")
          .bind_value(volume),
        skene_web::Slider::new().set_width(skene_web::Dimension::Length(300.)),
      ))
      .build()
  });

  let labels = select(&root, "label");
  let [slider, wide] = labels.as_slice()
  else {
    panic!("two sliders: {}", root.inner_html());
  };

  assert_eq!(select(slider, "input[type=range]").len(), 1);
  assert_eq!(computed(slider, "width"), "160px");
  assert_eq!(computed(wide, "width"), "300px", "a width its user sets");

  let [track] = select(slider, ":scope > div")
    .try_into()
    .expect("the track alone");
  let [fill, thumb] = select(&track, ":scope > div > div > div")
    .try_into()
    .expect("the fill and the thumb");
  let rail = fill.parent_element().expect("the rail");
  let left = |element: &Element| element.get_bounding_client_rect().left();
  let right = |element: &Element| element.get_bounding_client_rect().right();
  let near = |a: f64, b: f64| (a - b).abs() < 0.5;

  assert_eq!(
    background(&rail),
    system(&root, "background-color", SystemColors::TRACK)
  );
  assert_eq!(
    background(&fill),
    system(&root, "background-color", SystemColors::ACCENT)
  );
  assert_eq!(
    background(&thumb),
    system(&root, "background-color", SystemColors::KNOB)
  );
  assert_eq!(computed(&rail, "overflow-x"), "hidden");
  assert!(
    near(left(&thumb), left(&track)),
    "at the minimum: the start"
  );
  assert!(
    near(right(&fill), left(&thumb)),
    "the fill runs to the thumb"
  );

  volume.set(50.);
  web::end_turn();

  let travel = track.get_bounding_client_rect().width() - 18.;

  assert!(
    near(left(&thumb) - left(&track), travel / 2.),
    "half way: {} of {travel}",
    left(&thumb) - left(&track)
  );
  assert!(near(right(&fill), left(&thumb)));
  assert!(near(left(&rail), left(&track)), "the rail keeps its place");

  volume.set(100.);
  web::end_turn();

  assert!(
    near(right(&thumb), right(&track)),
    "at the maximum: the end"
  );
  assert!(
    select(&track, ":scope > div > div > div")
      .first()
      .is_some_and(|first| *first == fill),
    "the same fill"
  );
}

/// The built-in field and editor are the page's own `<input>` and `<textarea>`, drawn
/// with their defaults in the platform's colors: the field's fill and outline, the
/// accent ring while focus shows, and the colors the value is selected in, which a setter
/// its user calls replaces.
#[wasm_bindgen_test]
fn a_text_fields_defaults_apply_and_its_selection_takes_the_platforms_colors() {
  let (_guard, root) = fresh_root();
  let red = Color::srgb(1., 0., 0.);
  let name = skene_web::reactive::Signal::new("Ada".to_owned());

  let _ = mount(move || {
    Column::new()
      .set_children((
        skene_web::TextField::new()
          .set_label("Name")
          .bind_value(name),
        skene_web::TextEditor::new()
          .set_label("Notes")
          .set_selection_background(red),
      ))
      .build()
  });

  let field = root
    .query_selector("input")
    .ok()
    .flatten()
    .expect("an input");
  let editor = root
    .query_selector("textarea")
    .ok()
    .flatten()
    .expect("a textarea");

  assert_eq!(field.unchecked_ref::<HtmlInputElement>().value(), "Ada");
  assert_eq!(editor.get_attribute("rows").as_deref(), Some("3"));

  for entry in [&field, &editor] {
    assert_eq!(
      background(entry),
      system(&root, "background-color", SystemColors::FIELD)
    );
    assert_eq!(
      computed(entry, "border-top-color"),
      system(&root, "color", SystemColors::BORDER)
    );
    assert_eq!(computed(entry, "border-top-width"), "1px");
    assert_eq!(computed(entry, "padding-left"), "8px");
    assert_eq!(computed(entry, "border-top-left-radius"), "6px");
    assert_eq!(computed(entry, "cursor"), "text");
  }

  assert_eq!(
    computed(&field, "--typography-selection-surface"),
    system(&root, "background-color", SystemColors::SELECTION),
    "the value is selected on the platform's selection color"
  );
  assert_eq!(
    computed(&field, "--typography-selection-ink"),
    system(&root, "color", SystemColors::INK)
  );
  assert_eq!(
    computed(&editor, "--typography-selection-surface"),
    probe(&root, "background-color", red),
    "a selection background its user sets"
  );

  html(&field).focus().expect("focus the field");

  assert_eq!(
    computed(&field, "border-top-color"),
    system(&root, "color", SystemColors::ACCENT),
    "the ring while focus shows"
  );
}

/// The built-in picker, drawn on a live page, takes its face's defaults and opens its
/// list in the platform's colors, the chosen option's name and the indicator on the
/// face and the chosen option's check alone showing; a choice renames the face in place.
#[wasm_bindgen_test]
fn a_pickers_face_and_list_take_their_defaults() {
  let (_guard, root) = fresh_root();
  let size = skene_web::reactive::Signal::new(2u8);

  let _ = mount(move || {
    skene_web::Picker::new()
      .set_label("Size")
      .bind_chosen(size)
      .set_options((
        skene_web::ListboxOption::new(1u8).set_label("Small"),
        skene_web::ListboxOption::new(2u8).set_label("Large"),
      ))
      .build()
  });

  assert!(
    root.query_selector("select").expect("query").is_none(),
    "a live page draws the picker outright"
  );

  let face = root
    .query_selector("[role=combobox]")
    .ok()
    .flatten()
    .expect("the face");

  // The list is promoted in place, inside the face's element: the face's own text is
  // its first two runs.
  let shown = |face: &Element| {
    select(face, "span")
      .iter()
      .take(2)
      .filter_map(|run| run.text_content())
      .collect::<String>()
  };

  assert_eq!(shown(&face), "Large\u{25be}");
  assert_eq!(
    background(&face),
    system(&root, "background-color", SystemColors::CONTROL)
  );
  assert_eq!(
    computed(&face, "border-top-color"),
    system(&root, "color", SystemColors::BORDER)
  );
  assert_eq!(computed(&face, "min-height"), "32px");

  html(&face).click();

  let options = select(&root, "[role=option]");
  let [small, large] = options.as_slice()
  else {
    panic!("two options: {}", root.inner_html());
  };
  let list = large
    .parent_element()
    .and_then(|column| column.parent_element())
    .expect("the list's box");

  assert_eq!(
    background(&list),
    system(&root, "background-color", SystemColors::FIELD)
  );
  assert_eq!(
    computed(&list, "border-top-color"),
    system(&root, "color", SystemColors::BORDER)
  );

  let check = |option: &Element| {
    let mark = option
      .query_selector("span")
      .ok()
      .flatten()
      .and_then(|span| span.parent_element())
      .expect("the option's mark");

    computed(&mark, "visibility")
  };

  assert_eq!(
    check(small),
    "hidden",
    "the check shows on the chosen option alone"
  );
  assert_eq!(check(large), "visible");

  html(small).click();

  assert_eq!(size.get(), 1, "a choice reaches the signal");
  assert_eq!(shown(&face), "Small\u{25be}");
}

/// The built-in listbox lays its options out in its list box, the chosen one's check
/// showing, and a list box its user sets takes the default's place.
#[wasm_bindgen_test]
fn a_listboxs_options_sit_in_its_list_box() {
  let (_guard, root) = fresh_root();
  let red = Color::srgb(1., 0., 0.);
  let fruit = skene_web::reactive::Signal::new(Some(2u8));
  let options = || {
    (
      skene_web::ListboxOption::new(1u8).set_label("Apple"),
      skene_web::ListboxOption::new(2u8).set_label("Banana"),
    )
  };

  let _ = mount(move || {
    Row::new()
      .set_children((
        skene_web::Listbox::new()
          .set_label("Fruit")
          .bind_chosen(fruit)
          .set_children(options()),
        skene_web::Listbox::<Option<u8>>::new()
          .set_label("Other")
          .set_list(Container::new().set_background(red))
          .set_children(options()),
      ))
      .build()
  });

  let listboxes = select(&root, "[role=listbox]");
  let [fruits, other] = listboxes.as_slice()
  else {
    panic!("two listboxes: {}", root.inner_html());
  };
  let list = |listbox: &Element| {
    listbox
      .query_selector("[role=option]")
      .ok()
      .flatten()
      .and_then(|option| option.parent_element())
      .and_then(|column| column.parent_element())
      .expect("the list's box")
  };

  assert_eq!(
    background(&list(fruits)),
    system(&root, "background-color", SystemColors::FIELD)
  );
  assert_eq!(computed(&list(fruits), "border-top-width"), "1px");
  assert_eq!(
    background(&list(other)),
    probe(&root, "background-color", red)
  );
  assert_eq!(computed(&list(other), "border-top-width"), "0px");

  let options = select(fruits, "[role=option]");
  let [apple, banana] = options.as_slice()
  else {
    panic!("two options: {}", fruits.inner_html());
  };
  let top = |element: &Element| element.get_bounding_client_rect().top();

  assert!(top(banana) > top(apple), "the options stand in a column");

  html(apple).click();

  assert_eq!(fruit.get(), Some(1));
  assert_eq!(
    apple.get_attribute("aria-selected").as_deref(),
    Some("true")
  );
}
