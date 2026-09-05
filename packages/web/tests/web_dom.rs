//! The browser leaf over the new model, against a real DOM: tokens and palettes
//! through the theme, derived slots beside variants, controls over their native
//! inputs, delegated events through core's dispatch, the environment's signals, and
//! hydration of the server leaf's markup.
//!
//! Run in a headless browser:
//! `cargo test -p skene-web --target wasm32-unknown-unknown --test web_dom`.
#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use std::{cell::Cell, rc::Rc};

use skene_core::{
  Color,
  ColorScheme,
  Condition,
  Driver,
  HasChildren,
  Leaf,
  Media,
  State,
  Token,
  TokenName,
  TokenRef,
  TokenSet,
  Value,
};
use skene_reactive::{Get, Set, Signal};
use skene_simple::{SimpleButton, SimpleCheckbox, SimplePicker};
use skene_views::{Container, RawCheckbox, Text, Theme, Toggle, View};
use skene_web::{Web, WebTokenValue};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, KeyboardEvent, KeyboardEventInit};

wasm_bindgen_test_configure!(run_in_browser);

type Box_ = Container<Web>;

/// A component library's palette: a card color answering the scheme, and a gutter.
#[derive(Debug, Clone, PartialEq)]
struct Palette {
  card: Token<Color, f32>,
  gutter: Token<f32, f32>,
}

impl Palette {
  const CARD: TokenRef<Color> = TokenRef::new("test-card");

  fn with_card(card: Color) -> Self {
    Self {
      card: Token::new("test-card", card),
      ..Self::default()
    }
  }
}

impl Default for Palette {
  fn default() -> Self {
    Self {
      card: Token::new("test-card", white()).define_when(Media::Dark, black()),
      gutter: Token::new("test-gutter", 8.),
    }
  }
}

impl TokenSet<WebTokenValue> for Palette {
  fn definitions(&self) -> Vec<(TokenName, WebTokenValue)> {
    vec![self.card.define(), self.gutter.define()]
  }
}

fn white() -> Color {
  Color::srgb(1., 1., 1.)
}

fn black() -> Color {
  Color::srgb(0., 0., 0.)
}

fn red() -> Color {
  Color::srgb(1., 0., 0.)
}

fn blue() -> Color {
  Color::srgb(0., 0., 1.)
}

/// A color as the driver renders it.
fn css(color: Color) -> String {
  let [lightness, chroma, hue, alpha] = color.to_oklch();

  format!("oklch({lightness} {chroma} {hue} / {alpha})")
}

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
    skene_web::uninstall();
    self.0.remove();
  }
}

/// A connected root `<div>`, so computed styles resolve.
fn connected_root() -> Element {
  let root = document().create_element("div").expect("a root");

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect the root");

  root
}

/// Installs the driver rooted at a fresh connected `<div>`.
fn fresh_root() -> (Installed, Element) {
  let root = connected_root();

  skene_web::install(root.clone());

  (Installed(root.clone()), root)
}

/// The single view element built directly under the root.
fn only_child(root: &Element) -> Element {
  root.first_element_child().expect("a built child")
}

fn computed(element: &Element, property: &str) -> String {
  web_sys::window()
    .expect("a window")
    .get_computed_style(element)
    .expect("computed style")
    .expect("a computed style")
    .get_property_value(property)
    .expect("a property value")
}

/// The computed background a `<div>` under `root` gets from the declaration
/// `background-color: <value>`, so colors compare as the engine serializes them.
fn probe(root: &Element, value: &str) -> String {
  let element = document().create_element("div").expect("a probe");

  let _ = element.set_attribute("style", &format!("background-color:{value}"));
  let _ = root.append_child(&element);

  let color = computed(&element, "background-color");

  element.remove();

  color
}

/// The text of every `<style>` the driver owns, in document order.
fn sheet_text() -> String {
  let styles = document()
    .query_selector_all("style[data-skene-style]")
    .expect("query the styles");

  (0..styles.length())
    .filter_map(|index| styles.item(index))
    .filter_map(|style| style.text_content())
    .collect()
}

/// How many `<style>` elements the driver owns.
fn sheet_count() -> u32 {
  document()
    .query_selector_all("style[data-skene-style]")
    .expect("query the styles")
    .length()
}

/// The first `skene-` class of `element`.
fn class_of(element: &Element) -> String {
  element
    .get_attribute("class")
    .unwrap_or_default()
    .split(' ')
    .find(|class| class.starts_with("skene-"))
    .expect("a styled element")
    .to_owned()
}

fn html(element: &Element) -> HtmlElement {
  element.clone().dyn_into().expect("an HTML element")
}

/// The markup the server leaf would have produced for the tree under `root`: every
/// element marked in document order, which is creation order for a static tree.
fn as_served(root: &Element) -> String {
  let elements = root.query_selector_all("*").expect("query the elements");

  for index in 0..elements.length() {
    if let Some(element) = elements
      .item(index)
      .and_then(|node| node.dyn_into::<Element>().ok())
    {
      let _ = element.set_attribute("data-h", &(index + 1).to_string());
    }
  }

  root.inner_html()
}

/// Installs the server's `<style>` block holding `text` in the head.
fn serve_styles(text: &str) -> Element {
  let block = document().create_element("style").expect("a style");

  let _ = block.set_attribute("id", "skene-styles");
  block.set_text_content(Some(text));
  document()
    .head()
    .expect("a head")
    .append_child(&block)
    .expect("append the block");

  block
}

/// A token color under a theme is the element's computed background.
#[wasm_bindgen_test]
fn a_token_color_resolves_through_the_theme() {
  let (_guard, root) = fresh_root();

  let _ = skene_web::mount(|| {
    Theme::<Web, _>::new(Palette::default())
      .set_child_builder(|| Box_::new().set_color(Palette::CARD))
      .build()
  });

  let scope = only_child(&root);
  let card = only_child(&scope);

  assert_eq!(computed(&scope, "display"), "contents");
  assert_eq!(
    computed(&card, "background-color"),
    probe(&root, &css(white()))
  );
  assert!(
    sheet_text().contains(&format!(
      ".{}{{background-color:var(--test-card);",
      class_of(&card)
    )),
    "{}",
    sheet_text()
  );
}

/// A palette write re-inks the consumer in place: the same element, a signal inside
/// keeping its value.
#[wasm_bindgen_test]
fn a_palette_write_reinks_without_replacing_the_element() {
  let (_guard, root) = fresh_root();
  let card = Signal::new(white());
  let inner: Rc<Cell<Option<Signal<i32>>>> = Rc::new(Cell::new(None));

  let _ = skene_web::mount({
    let inner = Rc::clone(&inner);

    move || {
      Theme::<Web, _>::derive(move || Palette::with_card(card.get()))
        .set_child_builder(move || {
          let count = Signal::new(1);

          inner.set(Some(count));

          Box_::new()
            .set_color(Palette::CARD)
            .set_child(Text::<Web>::derive(move || count.get().to_string()))
        })
        .build()
    }
  });

  let before = only_child(&only_child(&root));
  let count = inner.get().expect("the inner signal");

  count.set(5);

  assert_eq!(before.text_content().as_deref(), Some("5"));

  card.set(red());

  let after = only_child(&only_child(&root));

  assert!(before == after, "the consumer was re-inked, not rebuilt");
  assert_eq!(
    computed(&after, "background-color"),
    probe(&root, &css(red()))
  );
  assert_eq!(count.get(), 5);
  assert_eq!(after.text_content().as_deref(), Some("5"));
}

/// A derived base writes its slot and a hover variant on the same property is a later
/// rule over the slot reference, so it still wins.
#[wasm_bindgen_test]
fn a_derived_color_stays_below_its_hover_variant() {
  let (_guard, root) = fresh_root();
  let color = Signal::new(white());

  let _ = skene_web::mount(move || {
    Box_::new()
      .set_color(
        Value::<Color, Condition<f32>>::new(Leaf::Derived(Rc::new(move || color.get())))
          .when(State::Hovered.into(), red()),
      )
      .build()
  });

  let element = only_child(&root);
  let class = class_of(&element);

  assert_eq!(
    computed(&element, "background-color"),
    probe(&root, &css(white()))
  );

  color.set(blue());

  assert_eq!(
    computed(&element, "background-color"),
    probe(&root, &css(blue()))
  );

  let sheet = sheet_text();
  let base = sheet
    .find(&format!(".{class}{{background-color:var(--skene-color);"))
    .expect("the base rule references the slot");
  let hover = sheet
    .find(&format!(
      ".{class}:hover{{background-color:{};}}",
      css(red())
    ))
    .expect("the hover rule holds the variant");

  assert!(
    base < hover,
    "the variant is emitted after the base: {sheet}"
  );
  assert_eq!(
    html(&element)
      .style()
      .get_property_value("--skene-color")
      .unwrap(),
    css(blue())
  );
}

/// A checkbox is a label over its native input; one click on the input flips the
/// bound signal once, and the mark's color follows the control's state.
#[wasm_bindgen_test]
fn a_checkbox_flips_once_per_click() {
  let (_guard, root) = fresh_root();
  let checked = Signal::new(false);
  let flips = Rc::new(Cell::new(0));

  let _ = skene_web::mount({
    let flips = Rc::clone(&flips);

    move || {
      SimpleCheckbox::<Web>::new()
        .set_label("Agree")
        .derive_checked(move || checked.get())
        .handle_toggle(move |next| {
          flips.set(flips.get() + 1);
          checked.set(next);
        })
        .build()
    }
  });

  let label = only_child(&root);

  assert_eq!(label.tag_name(), "LABEL");

  let input = only_child(&label);

  assert_eq!(input.tag_name(), "INPUT");
  assert_eq!(input.get_attribute("type").as_deref(), Some("checkbox"));

  let input: HtmlInputElement = input.dyn_into().expect("an input");
  let mark = input.next_element_sibling().expect("the mark");
  let card = probe(&root, "var(--simple-card)");
  let primary = probe(&root, "var(--simple-primary-surface)");

  assert_ne!(card, primary);
  assert_eq!(computed(&mark, "background-color"), card);

  input.click();

  assert_eq!(flips.get(), 1, "one press is one flip");
  assert!(checked.get());
  assert!(input.checked());
  assert_eq!(computed(&mark, "background-color"), primary);

  let caption = mark.next_element_sibling().expect("the caption");

  assert_eq!(caption.text_content().as_deref(), Some("Agree"));
  html(&caption).click();

  assert_eq!(flips.get(), 2, "a click on the caption is one flip too");
  assert!(!checked.get());
  assert!(!input.checked());
  assert_eq!(computed(&mark, "background-color"), card);
}

/// A click on a native toggle is observed, not acted on: the platform flipped it once,
/// the control's own observer and the container above it each hear one click per
/// press, whether the press lands on the input or on its caption.
#[wasm_bindgen_test]
fn a_native_toggle_click_is_observed_once_per_press() {
  let (_guard, root) = fresh_root();
  let checked = Signal::new(false);
  let observed = Rc::new(Cell::new(0));
  let container_clicks = Rc::new(Cell::new(0));

  let _ = skene_web::mount({
    let observed = Rc::clone(&observed);
    let container_clicks = Rc::clone(&container_clicks);

    move || {
      Box_::new()
        .handle_click(move || container_clicks.set(container_clicks.get() + 1))
        .set_child(
          RawCheckbox::<Web>::new(Toggle::new().bind_checked(checked))
            .set_label("Agree")
            .handle_click(move || observed.set(observed.get() + 1)),
        )
        .build()
    }
  });

  let container = only_child(&root);
  let label = only_child(&container);
  let input: HtmlInputElement = only_child(&label).dyn_into().expect("an input");

  input.click();

  assert_eq!(observed.get(), 1, "the observer hears the press once");
  assert_eq!(
    container_clicks.get(),
    1,
    "the click climbs to the container once"
  );
  assert!(checked.get(), "the platform's flip reached the signal");
  assert!(input.checked(), "the platform's flip stands");

  // A press on the label reaches the root twice: at the label, then synthesized at
  // the input.
  html(&label).click();

  assert_eq!(observed.get(), 2, "a press on the caption is one click too");
  assert_eq!(container_clicks.get(), 2);
  assert!(!checked.get());
  assert!(!input.checked());
}

/// A button owns its click: the container around it never sees it, while a click on
/// the container itself does reach it.
#[wasm_bindgen_test]
fn a_button_click_does_not_reach_the_container() {
  let (_guard, root) = fresh_root();
  let container_clicks = Rc::new(Cell::new(0));
  let button_clicks = Rc::new(Cell::new(0));

  let _ = skene_web::mount({
    let container_clicks = Rc::clone(&container_clicks);
    let button_clicks = Rc::clone(&button_clicks);

    move || {
      Box_::new()
        .handle_click(move || container_clicks.set(container_clicks.get() + 1))
        .set_child(
          SimpleButton::<Web>::new("Go")
            .handle_click(move || button_clicks.set(button_clicks.get() + 1)),
        )
        .build()
    }
  });

  let container = only_child(&root);
  let button = only_child(&container);

  assert_eq!(button.tag_name(), "BUTTON");
  html(&button).click();

  assert_eq!(button_clicks.get(), 1);
  assert_eq!(container_clicks.get(), 0);

  html(&container).click();

  assert_eq!(container_clicks.get(), 1);
  assert_eq!(button_clicks.get(), 1);
}

/// A part's focus-visible variant anchors on the control's native input.
#[wasm_bindgen_test]
fn a_focus_visible_variant_anchors_on_the_input() {
  let (_guard, root) = fresh_root();

  let _ = skene_web::mount(|| SimpleCheckbox::<Web>::new().set_label("Agree").build());

  let label = only_child(&root);
  let input: HtmlInputElement = only_child(&label).dyn_into().expect("an input");
  let mark = input.next_element_sibling().expect("the mark");
  let class = class_of(&mark);
  let sheet = sheet_text();

  assert!(
    sheet.contains(&format!(
      ":where(:is(input,textarea):focus-visible) ~ .{class}"
    )),
    "{sheet}"
  );
  assert!(
    sheet.contains(&format!(
      ":where([data-skene-control]:is(:focus-visible)) .{class}"
    )),
    "{sheet}"
  );

  let ring = probe(&root, "var(--simple-ring)");
  let border = probe(&root, "var(--simple-border)");

  assert_ne!(ring, border);
  assert_eq!(computed(&mark, "border-top-color"), border);
  input.focus().expect("focus the input");

  assert_eq!(
    document().active_element().as_ref(),
    Some(input.unchecked_ref())
  );

  if input.matches(":focus-visible").unwrap_or(false) {
    assert_eq!(computed(&mark, "border-top-color"), ring);
  }
}

/// The environment reports the window: the viewport signal is its inner size, a
/// width watch answers as its media query does, and the scheme composes the app's
/// choice with the platform's preference.
#[wasm_bindgen_test]
fn media_reports_the_window() {
  let (_guard, _root) = fresh_root();
  let window = web_sys::window().expect("a window");
  let width = window.inner_width().unwrap().as_f64().unwrap();
  let height = window.inner_height().unwrap().as_f64().unwrap();
  let media = Web.media();

  assert!((f64::from(media.viewport.get().width) - width).abs() < f64::EPSILON);
  assert!((f64::from(media.viewport.get().height) - height).abs() < f64::EPSILON);

  #[expect(clippy::cast_possible_truncation)]
  let below = (width - 1.) as f32;
  #[expect(clippy::cast_possible_truncation)]
  let above = (width + 1.) as f32;

  assert!(skene_web::watch_media(&Media::MinWidth(below)).get());
  assert!(!skene_web::watch_media(&Media::MinWidth(above)).get());
  assert!(
    skene_web::watch_media(&Media::or([Media::MinWidth(above), Media::MaxWidth(above)])).get()
  );
  assert!(skene_web::media_query(&format!("(min-width: {below}px)")).get());
  assert!(!skene_web::media_query(&format!("(min-width: {above}px)")).get());

  let platform_dark = window
    .match_media("(prefers-color-scheme: dark)")
    .unwrap()
    .is_some_and(|list| list.matches());
  let platform = if platform_dark {
    ColorScheme::Dark
  }
  else {
    ColorScheme::Light
  };
  let dark = skene_web::watch_media(&Media::Dark);

  assert_eq!(media.scheme.get(), platform);
  assert_eq!(dark.get(), platform_dark);

  skene_web::set_scheme(Some(ColorScheme::Dark));

  assert_eq!(media.scheme.get(), ColorScheme::Dark);
  assert!(dark.get());
  assert_eq!(
    document()
      .document_element()
      .unwrap()
      .get_attribute("data-skene-scheme")
      .as_deref(),
    Some("dark")
  );
  assert_eq!(skene_web::stored_scheme(), Some(ColorScheme::Dark));

  skene_web::set_scheme(None);

  assert_eq!(media.scheme.get(), platform);
  assert_eq!(skene_web::stored_scheme(), None);
}

/// A page the server leaf rendered hydrates with no mismatch: its elements are
/// adopted in place and its one `<style>` block is the only stylesheet.
#[wasm_bindgen_test]
fn a_served_page_hydrates_without_mismatch() {
  let app = || {
    Theme::<Web, _>::new(Palette::default()).set_child_builder(|| {
      Box_::new()
        .set_color(Palette::CARD)
        .set_child(Text::<Web>::new("Hi"))
    })
  };
  let (markup, styles) = {
    let (_guard, root) = fresh_root();

    let _ = skene_web::mount(move || app().build());

    (as_served(&root), sheet_text())
  };

  assert!(markup.contains("data-h=\"1\""), "{markup}");

  let block = serve_styles(&styles);
  let root = connected_root();

  root.set_inner_html(&markup);

  let served_card = only_child(&only_child(&root));

  skene_web::hydrate(root.clone(), "", app);

  let guard = Installed(root.clone());
  let card = only_child(&only_child(&root));

  assert_eq!(skene_web::hydration_mismatches(), 0);
  assert!(served_card == card, "the card was adopted, not rebuilt");
  assert_eq!(card.get_attribute("data-h"), None);
  assert_eq!(card.text_content().as_deref(), Some("Hi"));
  assert_eq!(sheet_count(), 1, "the server's block is the one stylesheet");
  assert!(block.has_attribute("data-skene-style"));
  assert_eq!(
    computed(&card, "background-color"),
    probe(&root, &css(white()))
  );

  drop(guard);

  assert!(!block.is_connected());
}

/// An input the reader ticked before the client arrived reports its live state into
/// the bound signal on adoption.
#[wasm_bindgen_test]
fn a_ticked_input_reports_on_hydration() {
  let app = |checked: Signal<bool>| {
    move || RawCheckbox::<Web>::new(Toggle::new().bind_checked(checked)).set_label("Agree")
  };
  let (markup, styles) = {
    let (_guard, root) = fresh_root();

    let _ = skene_web::mount(move || app(Signal::new(false))().build());

    (as_served(&root), sheet_text())
  };

  let _block = serve_styles(&styles);
  let root = connected_root();

  root.set_inner_html(&markup);

  let input: HtmlInputElement = only_child(&only_child(&root)).dyn_into().expect("an input");

  assert!(!input.has_attribute("checked"), "{markup}");
  input.set_checked(true);

  let checked = Signal::new(false);

  skene_web::hydrate(root.clone(), "", app(checked));

  let _guard = Installed(root.clone());

  assert_eq!(skene_web::hydration_mismatches(), 0);
  assert!(checked.get(), "the reader got there first");
  assert!(input.checked());

  checked.set(false);

  assert!(!input.checked());
}

/// Presses `key` on `element`, as the keyboard would: a bubbling `keydown`.
fn press(element: &Element, key: &str) {
  let init = KeyboardEventInit::new();

  init.set_key(key);
  init.set_bubbles(true);
  init.set_cancelable(true);

  let event =
    KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).expect("a key event");

  let _ = element.dispatch_event(&event);
}

/// A picker's face is a combobox over a manual popover: `ArrowDown` opens it with
/// focus on the chosen option, the arrows move focus, and `Enter` chooses, closes and
/// returns focus to the face with the signal written.
#[wasm_bindgen_test]
fn a_picker_opens_and_chooses_with_the_keyboard() {
  let (_guard, root) = fresh_root();
  let value = Signal::new("md".to_owned());

  let _ = skene_web::mount(move || {
    SimplePicker::<Web>::new()
      .set_options([("sm", "Small"), ("md", "Medium"), ("lg", "Large")])
      .bind_value(value)
      .build()
  });

  let face = only_child(&root);

  assert_eq!(face.get_attribute("role").as_deref(), Some("combobox"));
  assert_eq!(
    face.get_attribute("aria-expanded").as_deref(),
    Some("false")
  );

  let popup = face.last_element_child().expect("the popup");

  assert_eq!(popup.get_attribute("popover").as_deref(), Some("manual"));
  assert!(!popup.matches(":popover-open").unwrap_or(true));

  let options = popup
    .query_selector_all("[role=option]")
    .expect("the options");

  assert_eq!(options.length(), 3);

  let option = |index: u32| -> Element {
    options
      .item(index)
      .expect("an option")
      .dyn_into()
      .expect("an element")
  };

  assert_eq!(
    option(1).get_attribute("aria-selected").as_deref(),
    Some("true")
  );
  html(&face).focus().expect("focus the face");
  press(&face, "ArrowDown");

  assert!(popup.matches(":popover-open").unwrap_or(false));
  assert_eq!(face.get_attribute("aria-expanded").as_deref(), Some("true"));
  assert_eq!(
    document().active_element().as_ref(),
    Some(option(1).unchecked_ref())
  );

  press(&option(1), "ArrowUp");

  assert_eq!(
    document().active_element().as_ref(),
    Some(option(0).unchecked_ref())
  );

  press(&option(0), "Enter");

  assert_eq!(value.get(), "sm");
  assert!(!popup.matches(":popover-open").unwrap_or(true));
  assert_eq!(
    option(0).get_attribute("aria-selected").as_deref(),
    Some("true")
  );
  assert_eq!(
    document().active_element().as_ref(),
    Some(face.unchecked_ref())
  );
}
