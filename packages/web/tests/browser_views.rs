//! The web facade's views realized in a real browser: the elements they build, the
//! styles the engine computes for them, the events that reach them, and the signals
//! their bindings write.
//!
//! Run in a headless browser:
//! `cargo test -p skene-web --target wasm32-unknown-unknown --test browser_views`.
#![cfg(skene_web_platform = "browser")]

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_foundation::{
  Axis,
  BasicPickerBase,
  ColorScheme,
  ColumnBase,
  ContainerBase,
  ControlBase,
  DecorationBase,
  Edit,
  EventKind,
  FlexBase,
  FlexChild,
  ItemAlignment,
  LabelPlacement,
  Layout,
  Media,
  NativeElement,
  OverlayBase,
  RowBase,
  Size,
  Spacing,
  StaticLeaf,
  TextBase,
  ThemeBase,
  Verdict,
  report_edit,
};
use skene_simple_web::prelude::*;
use skene_web::{
  Activate,
  AsyncView,
  AsyncViewBase,
  BasicButton,
  BasicCheckbox,
  BasicListboxOption,
  BasicPicker,
  BasicRadio,
  BasicRadioGroup,
  BasicSlider,
  BasicTextEditor,
  BasicTextField,
  Behavior,
  Bubbling,
  Color,
  Column,
  Container,
  Control,
  HasChild,
  HasChildren,
  LayoutBox,
  Link,
  LinkBase,
  ListboxOption,
  NodeHandle,
  NodeId,
  Overlay,
  Radio,
  RadioGroup,
  Role,
  Row,
  Slider,
  Text,
  TextEntry,
  TextSelection,
  Theme,
  ThemedIn,
  Toggle,
  Typography,
  View,
  head,
  install_router,
  media,
  platform::{self, Declarations, Web, mount},
  reactive::{Effect, Get, Owner, Set, Signal},
  testing::{computed, dispatch, document, frames_until, fresh_root, only_child, tick},
  wasm_bindgen::JsCast,
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

/// The computed `property` a `<div>` under `parent` gets from `<property>: <color>`, so
/// colors compare as the engine serializes them.
fn probe(parent: &Element, property: &str, color: Color) -> String {
  let element = document().create_element("div").expect("a probe");

  let _ = element.set_attribute(
    "style",
    &format!("{property}:{}", Declarations::render_color(color)),
  );
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

/// The scheme an app put in force, returned to following the platform when dropped.
#[must_use]
struct Scheme;

impl Scheme {
  fn choose(scheme: ColorScheme) -> Self {
    media::set_scheme(Some(scheme));
    platform::end_turn();

    Self
  }
}

impl Drop for Scheme {
  fn drop(&mut self) {
    media::set_scheme(None);
    platform::end_turn();
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
  platform::end_turn();

  assert!(dialog.has_attribute("open"));

  let init = web_sys::EventInit::new();

  init.set_cancelable(true);

  let cancel = web_sys::Event::new_with_event_init_dict("cancel", &init).expect("an event");
  let _ = dialog.dispatch_event(&cancel);

  platform::end_turn();

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
  platform::end_turn();

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
  let chosen: Signal<Option<String>> = Signal::new(None);
  let items = Signal::new(vec!["a", "b"]);

  let _ = mount(move || {
    BasicRadioGroup::new(RadioGroup::new().bind_chosen(chosen))
      .set_child(Column::new().derive_children(
        move || items.get(),
        |item| *item,
        |item| BasicRadio::new(Radio::new(item)).set_label(item),
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
  platform::end_turn();

  assert_eq!(chosen.get().as_deref(), Some("b"));
  assert_eq!(stops(), [false, true]);

  chosen.set(None);
  items.set(vec!["z", "a", "b"]);
  platform::end_turn();

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
      !click(&link, true, detail),
      "a modified click is the browser's"
    );
    assert_eq!(router.route(), Page::Home);
  }

  assert!(key_down(&link, "Enter"), "Enter on a link is the router's");
  assert_eq!(router.route(), Page::About);

  platform::push_path(&path);
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
  platform::end_turn();

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
  platform::end_turn();

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
  platform::end_turn();

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
  platform::end_turn();
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

/// The `<input>` a text field mounted alone under `root` realizes.
fn input_of(root: &Element) -> HtmlInputElement {
  root
    .query_selector("input")
    .expect("query")
    .expect("an input")
    .dyn_into()
    .expect("an input element")
}

/// A drawn entry's edit is answered with the truth: one its handler transforms is
/// replaced by what the handler kept, the selection beside it, and one the truth holds
/// stands.
#[wasm_bindgen_test]
fn report_edit_answers_with_the_truth_when_the_handler_transforms_it() {
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
    report_edit::<Web>(node, &typed, false),
    Verdict::Replace(Edit::new("ABC").with_selection(TextSelection::collapsed(3)))
  );
  assert_eq!(text.get(), "ABC");

  platform::end_turn();

  assert_eq!(input_of(&root).value(), "ABC");

  let kept = Edit::new("ABCD").with_selection(TextSelection::collapsed(4));

  assert_eq!(report_edit::<Web>(node, &kept, true), Verdict::Accept);
  assert_eq!(caret.get(), TextSelection::collapsed(4));
}

/// An edit the truth holds already is accepted without asking the handlers, and one
/// leaving a controlled selection unnamed is answered with the truth's.
#[wasm_bindgen_test]
fn report_edit_of_what_the_truth_holds_asks_nothing() {
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

  assert_eq!(report_edit::<Web>(node, &truth, true), Verdict::Accept);
  assert!(asked.borrow().is_empty());
  assert_eq!(
    report_edit::<Web>(node, &Edit::new("ab"), true),
    Verdict::Replace(truth)
  );
}

/// An entry deriving no text keeps no truth: its edit stands, its handler hearing it.
#[wasm_bindgen_test]
fn report_edit_accepts_an_uncontrolled_entry() {
  let (_guard, _root) = fresh_root();
  let heard = Rc::new(RefCell::new(Vec::new()));
  let log = Rc::clone(&heard);
  let node = mount(move || {
    BasicTextField::new(
      TextEntry::new().handle_change(move |value| log.borrow_mut().push(value.to_owned())),
    )
    .build()
  });

  assert_eq!(
    report_edit::<Web>(node, &Edit::new("typed"), true),
    Verdict::Accept
  );
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
    report_edit::<Web>(node, &Edit::new("123"), false),
    Verdict::Replace(Edit::new("12"))
  );
  assert_eq!(bound.get(), "12");
  assert_eq!(
    report_edit::<Web>(node, &Edit::new("12"), true),
    Verdict::Accept
  );
}

/// The verdict is an edit's only write-back: the `<input>` is not reconciled with the
/// truth while the edit is reported.
#[wasm_bindgen_test]
fn report_edit_does_not_reconcile() {
  let (_guard, root) = fresh_root();
  let node =
    mount(|| BasicTextField::new(TextEntry::new().set_value("12").handle_change(|_| {})).build());
  let input = input_of(&root);

  assert_eq!(input.value(), "12");

  // The `<input>` shows the reader's edit already.
  input.set_value("12a");

  assert_eq!(
    report_edit::<Web>(node, &Edit::new("12a"), true),
    Verdict::Replace(Edit::new("12"))
  );

  platform::end_turn();

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
  platform::end_turn();

  assert!(!input.checked(), "the rejected toggle is undone");
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
  platform::end_turn();

  assert_eq!(input.value(), "fresh");

  let editor = root
    .query_selector("textarea")
    .expect("query")
    .expect("the editor's textarea");

  assert_eq!(editor.get_attribute("rows").as_deref(), Some("4"));
}

/// A text entry keeping its label inside its box.
struct InlineEntry;

impl Behavior<Web> for InlineEntry {
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
  let value = Signal::new("a".to_owned());

  let _ = mount(move || {
    BasicPicker::new()
      .bind_chosen(value)
      .set_label("Size")
      .set_options(
        ["a", "b"].map(|value| BasicListboxOption::new(ListboxOption::new(value)).set_label(value)),
      )
      .set_face(|shown| Text::derive(move || shown.get()))
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
