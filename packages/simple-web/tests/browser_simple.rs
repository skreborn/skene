//! The Simple library in a real browser: every view resolves the palette's tokens
//! through the one theme mechanism, the controls' parts follow the control, and a
//! third-party palette works under the same theme, read as the engine computes each
//! element's style.
//!
//! Run in a headless browser:
//! `cargo test-web --test browser_simple`.

#![cfg(all(skene_platform = "web", not(skene_server)))]

use std::{cell::RefCell, rc::Rc};

use skene_simple_web::prelude::*;
use skene_web::{
  Media,
  SliderRange,
  StaticLeaf,
  Token,
  TokenName,
  TokenRef,
  TokenSet,
  platform::web::{self, Declarations, WebTokenValue, mount},
  prelude::*,
  testing::{Installed, computed, dispatch, document, frames_until, fresh_root, only_child},
  wasm_bindgen::JsCast,
};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{Element, HtmlElement, HtmlInputElement, KeyboardEvent, KeyboardEventInit};

wasm_bindgen_test_configure!(run_in_browser);

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

/// The fixed color `token` is defined as: under the dark scheme when `dark`, else its
/// base.
fn defined(token: &Token<Color>, dark: bool) -> Color {
  let definition = token.definition();
  let leaf = if dark {
    definition
      .variants()
      .iter()
      .find_map(|(media, leaf)| (*media == Media::Dark).then_some(leaf))
      .unwrap_or(definition.base())
  }
  else {
    definition.base()
  };

  match leaf {
    StaticLeaf::Fixed(color) => *color,
    StaticLeaf::Token(_) | StaticLeaf::Expr(_) => panic!("the default palette holds fixed colors"),
  }
}

fn light(token: &Token<Color>) -> Color {
  defined(token, false)
}

fn dark(token: &Token<Color>) -> Color {
  defined(token, true)
}

/// The element children of `parent`, in order.
fn elements(parent: &Element) -> Vec<Element> {
  let children = parent.children();

  (0..children.length())
    .filter_map(|index| children.item(index))
    .collect()
}

/// `element` as the HTML element it is.
fn html(element: &Element) -> HtmlElement {
  element.clone().dyn_into().expect("an HTML element")
}

/// `element` as the input it is.
fn input(element: &Element) -> HtmlInputElement {
  element.clone().dyn_into().expect("an input")
}

/// The computed `property` a `<div>` under the page's body gets from
/// `<property>: <color>`, so colors compare as the engine serializes them.
fn probe(property: &str, color: Color) -> String {
  let element = document().create_element("div").expect("a probe");

  let _ = element.set_attribute(
    "style",
    &format!("{property}:{}", Declarations::render_color(color)),
  );
  let _ = document().body().expect("a body").append_child(&element);

  let color = computed(&element, property);

  element.remove();

  color
}

/// The computed background of `element`.
fn background(element: &Element) -> String {
  computed(element, "background-color")
}

/// The computed ink of `element`.
fn ink(element: &Element) -> String {
  computed(element, "color")
}

/// The computed border of `element`: its top edge's width and color, every edge being
/// drawn alike.
fn border(element: &Element) -> (String, String) {
  (
    computed(element, "border-top-width"),
    computed(element, "border-top-color"),
  )
}

/// A border `width` pixels wide in `color`, as [`border`] reads one.
fn drawn(width: u8, color: Color) -> (String, String) {
  (format!("{width}px"), probe("border-top-color", color))
}

/// The background of `element` once its transition settles on `expected`, or as it
/// stands after a hundred frames.
async fn settled(element: &Element, expected: &str) -> String {
  let _ = frames_until(|| background(element) == expected).await;

  background(element)
}

/// The view `view` builds under a default Simple theme on a fresh root, the guard of
/// the root and the element the view realizes, the theme realizing a scope of its own.
fn themed<V: View + 'static>(view: impl FnOnce() -> V + 'static) -> (Installed, Element) {
  let (guard, root) = fresh_root();

  let _ = mount(|| {
    Theme::new(SimplePalette::default())
      .set_child_builder(view)
      .build()
  });

  let element = only_child(&only_child(&root));

  (guard, element)
}

/// Dispatches a cancelable, bubbling `keydown` of `key` at `target`.
fn press(target: &Element, key: &str) {
  let init = KeyboardEventInit::new();

  init.set_bubbles(true);
  init.set_cancelable(true);
  init.set_key(key);

  let event = KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).expect("a key");
  let _ = target.dispatch_event(&event);

  web::end_turn();
}

/// Whether `element` is the document's focused element.
fn is_focused(element: &Element) -> bool {
  document().active_element().as_ref() == Some(element)
}

/// Whether `element` is disabled as the engine reports it.
fn is_disabled(element: &Element) -> bool {
  element.matches(":disabled").expect("a selector")
}

/// A button's surface is the primary look's, focusing the control paints the focus
/// surface on the control itself, its label is inked in the look's ink, a filled look
/// draws a transparent hairline, and an icon button takes its accessible name.
#[wasm_bindgen_test]
async fn a_buttons_surface_and_focus_resolve_the_primary_look_on_the_control() {
  let _scheme = Scheme::choose(ColorScheme::Light);
  let palette = SimplePalette::default();
  let (_guard, button) = themed(|| {
    Column::new().set_children((
      SimpleButton::new("Save"),
      SimpleButton::new("")
        .set_accessible_name("Close")
        .set_child(Container::new()),
    ))
  });
  let [button, icon] = &elements(&button)[..]
  else {
    panic!("two buttons");
  };
  let label = only_child(button);

  assert_eq!(button.tag_name(), "BUTTON");
  assert_eq!(button.get_attribute("aria-label").as_deref(), Some("Save"));
  assert_eq!(label.text_content().as_deref(), Some("Save"));
  assert_eq!(
    icon.get_attribute("aria-label").as_deref(),
    Some("Close"),
    "an icon button takes its accessible name"
  );

  let surface = probe("background-color", light(&palette.primary.surface));

  assert_eq!(settled(button, &surface).await, surface);
  assert_eq!(ink(&label), probe("color", light(&palette.primary.ink)));
  assert_eq!(border(button), drawn(1, Color::TRANSPARENT));

  html(button).focus().expect("focus the button");

  assert!(button.matches(":focus-visible").expect("a selector"));

  let focus = probe("background-color", light(&palette.primary.focus));

  assert_eq!(settled(button, &focus).await, focus);
}

/// A derived variant redefines the look on the button's own element: the surface and
/// the border follow the signal and the element is never rebuilt.
#[wasm_bindgen_test]
async fn a_derived_variant_redefines_the_look_without_a_rebuild() {
  let _scheme = Scheme::choose(ColorScheme::Light);
  let palette = SimplePalette::default();
  let variant = Signal::new(SimpleVariant::Primary);
  let (_guard, button) =
    themed(move || SimpleButton::new("Go").derive_variant(move || variant.get()));
  let primary = probe("background-color", light(&palette.primary.surface));

  assert_eq!(settled(&button, &primary).await, primary);

  variant.set(SimpleVariant::Outline);
  web::end_turn();

  let outline = probe("background-color", Color::TRANSPARENT);

  assert_eq!(settled(&button, &outline).await, outline);
  assert_eq!(
    border(&button),
    drawn(1, light(palette.outline.border.as_ref().unwrap()))
  );

  variant.set(SimpleVariant::Destructive);
  web::end_turn();

  let destructive = probe("background-color", light(&palette.destructive.surface));

  assert_eq!(settled(&button, &destructive).await, destructive);
  assert_eq!(border(&button), drawn(1, Color::TRANSPARENT));
  assert!(button.is_connected(), "re-inked, not rebuilt");
}

/// A checkbox's mark sits on the card surface and fills with the primary surface, its
/// border and its glyph following the control's checked state after a click, and its
/// border is the ring while the control's input is focused.
#[wasm_bindgen_test]
fn a_checkbox_mark_follows_the_checked_state() {
  let _scheme = Scheme::choose(ColorScheme::Light);
  let palette = SimplePalette::default();
  let bound = Signal::new(false);
  let (_guard, checkbox) = themed(move || {
    SimpleCheckbox::new()
      .set_label("Remember me")
      .bind_checked(bound)
  });
  let [field, mark, caption] = &elements(&checkbox)[..]
  else {
    panic!("the input, the mark and the caption");
  };
  let field = input(field);
  let check = only_child(mark);
  let glyph = only_child(&check);

  assert_eq!(checkbox.tag_name(), "LABEL");
  assert_eq!(field.type_(), "checkbox");
  assert_eq!(
    field.get_attribute("aria-label").as_deref(),
    Some("Remember me")
  );
  assert_eq!(caption.text_content().as_deref(), Some("Remember me"));
  assert_eq!(
    background(mark),
    probe("background-color", light(&palette.card))
  );
  assert_eq!(border(mark), drawn(1, light(&palette.border)));
  assert_eq!(
    computed(&check, "visibility"),
    "hidden",
    "the unchecked glyph is hidden"
  );

  field.click();
  web::end_turn();

  assert!(bound.get());
  assert_eq!(
    background(mark),
    probe("background-color", light(&palette.primary.surface))
  );
  assert_eq!(border(mark), drawn(1, light(&palette.primary.surface)));
  assert_eq!(computed(&check, "visibility"), "visible");
  assert_eq!(ink(&glyph), probe("color", light(&palette.primary.ink)));

  field.focus().expect("focus the input");

  assert!(field.matches(":focus-visible").expect("a selector"));
  assert_eq!(border(mark), drawn(1, light(&palette.ring)));
}

/// A radio's ring shows its dot only while the group's choice is its value.
#[wasm_bindgen_test]
fn a_radio_ring_follows_the_groups_choice() {
  let _scheme = Scheme::choose(ColorScheme::Light);
  let palette = SimplePalette::default();
  let chosen: Signal<Option<String>> = Signal::new(None);
  let (_guard, group) = themed(move || {
    SimpleRadioGroup::new()
      .bind_chosen(chosen)
      .set_child(Column::new().set_children((
        SimpleRadio::new("a").set_label("A"),
        SimpleRadio::new("b").set_label("B"),
      )))
  });
  let members = elements(&only_child(&group));
  let field = |index: usize| input(&elements(&members[index])[0]);
  let dot = |index: usize| only_child(&elements(&members[index])[1]);
  let transparent = probe("background-color", Color::TRANSPARENT);

  assert_eq!(group.get_attribute("role").as_deref(), Some("radiogroup"));
  assert_eq!(field(0).type_(), "radio");
  assert_eq!(background(&dot(0)), transparent);

  field(1).click();
  web::end_turn();

  assert_eq!(chosen.get().as_deref(), Some("b"));
  assert_eq!(
    background(&dot(1)),
    probe("background-color", light(&palette.primary.surface))
  );
  assert_eq!(background(&dot(0)), transparent);
}

/// A radio group without its child builds an empty group, as a control without a body
/// does: the group's element, named and reporting its role, holding its caption alone.
#[wasm_bindgen_test]
fn a_radio_group_without_its_child_builds_an_empty_group() {
  let (_guard, group) = themed(|| {
    SimpleRadioGroup::new()
      .set_label("Size")
      .bind_chosen(Signal::new(None))
  });
  let content = elements(&group);

  assert_eq!(group.get_attribute("role").as_deref(), Some("radiogroup"));
  assert_eq!(group.get_attribute("aria-label").as_deref(), Some("Size"));
  assert_eq!(content.len(), 1, "{}", group.outer_html());
  assert_eq!(content[0].text_content().as_deref(), Some("Size"));
}

/// A slider's fill and its thumb's pinned edge are the fraction of the range the bound
/// value sits at, and the thumb's border becomes the ring when the control's input is
/// focused.
#[wasm_bindgen_test]
fn a_slider_fill_follows_the_bound_value_and_its_thumb_rings_when_focused() {
  let _scheme = Scheme::choose(ColorScheme::Light);
  let palette = SimplePalette::default();
  let bound = Signal::new(50.);
  let (_guard, slider) = themed(move || {
    SimpleSlider::new()
      .set_range(SliderRange::new(0., 200., 1.))
      .bind_value(bound)
  });
  let [field, face] = &elements(&slider)[..]
  else {
    panic!("the input and the slider's face");
  };
  let field = input(field);
  let stack = only_child(face);
  let [track, lane, rail] = &elements(&stack)[..]
  else {
    panic!("the track, the fill's lane and the thumb's rail");
  };
  let fill = only_child(lane);
  let travel = only_child(rail);
  let thumb = only_child(&travel);
  let thumb_extent = 16.;
  let fill_fraction = || {
    fill.get_bounding_client_rect().width()
      / (lane.get_bounding_client_rect().width() - thumb_extent)
  };
  let thumb_fraction = || {
    let travel = travel.get_bounding_client_rect();

    (thumb.get_bounding_client_rect().left() - travel.left()) / travel.width()
  };

  assert_eq!(field.type_(), "range");
  assert_eq!(field.value(), "50");
  assert!(
    track.get_bounding_client_rect().width() > 100.,
    "a laid-out track"
  );
  assert!((fill_fraction() - 0.25).abs() < 0.01, "{}", fill_fraction());
  assert!(
    (thumb_fraction() - 0.25).abs() < 0.01,
    "{}",
    thumb_fraction()
  );
  assert_eq!(
    background(&fill),
    probe("background-color", light(&palette.primary.surface))
  );
  assert_eq!(
    background(track),
    probe("background-color", light(&palette.secondary.surface))
  );

  bound.set(150.);
  web::end_turn();

  assert_eq!(field.value(), "150");
  assert!((fill_fraction() - 0.75).abs() < 0.01, "{}", fill_fraction());
  assert!(
    (thumb_fraction() - 0.75).abs() < 0.01,
    "{}",
    thumb_fraction()
  );
  assert_eq!(border(&thumb), drawn(2, light(&palette.background)));

  field.focus().expect("focus the input");

  assert!(field.matches(":focus-visible").expect("a selector"));
  assert_eq!(border(&thumb), drawn(2, light(&palette.ring)));
}

/// A text field's border is the ring only while it is focused, an edit commits into its
/// binding, and an editor wears the same chrome over more lines.
#[wasm_bindgen_test]
fn a_text_fields_border_is_the_ring_only_while_focused() {
  let _scheme = Scheme::choose(ColorScheme::Light);
  let palette = SimplePalette::default();
  let bound = Signal::new(String::new());
  let (_guard, column) = themed(move || {
    Column::new().set_children((
      SimpleTextField::new()
        .set_placeholder("Name")
        .bind_value(bound),
      SimpleTextEditor::new().set_lines(6),
    ))
  });
  let [field, editor] = &elements(&column)[..]
  else {
    panic!("the field and the editor");
  };
  let entry = input(field);

  assert_eq!(field.get_attribute("placeholder").as_deref(), Some("Name"));
  assert_eq!(border(field), drawn(1, light(&palette.border)));

  entry.focus().expect("focus the field");

  assert_eq!(border(field), drawn(1, light(&palette.ring)));

  entry.blur().expect("blur the field");

  assert_eq!(border(field), drawn(1, light(&palette.border)));

  entry.set_value("typed");
  dispatch(field, "input");

  assert_eq!(bound.get(), "typed");
  assert_eq!(editor.tag_name(), "TEXTAREA");
  assert_eq!(editor.get_attribute("rows").as_deref(), Some("6"));
  assert_eq!(border(editor), drawn(1, light(&palette.border)));
}

/// Under the dark scheme every token of the default palette resolves its dark value,
/// on a box and on a button alike.
#[wasm_bindgen_test]
async fn the_default_palette_answers_the_dark_scheme() {
  let scheme = Scheme::choose(ColorScheme::Light);
  let palette = SimplePalette::default();
  let (_guard, card) = themed(|| {
    SimpleCard::new().set_children((
      Container::new().set_background(SimplePalette::BACKGROUND),
      SimpleButton::new("Save"),
    ))
  });
  let [plain, button] = &elements(&only_child(&card))[..]
  else {
    panic!("the box and the button");
  };

  assert_eq!(
    background(&card),
    probe("background-color", light(&palette.card))
  );

  drop(scheme);

  let _scheme = Scheme::choose(ColorScheme::Dark);
  let surface = probe("background-color", dark(&palette.primary.surface));

  assert_eq!(
    background(&card),
    probe("background-color", dark(&palette.card))
  );
  assert_eq!(border(&card), drawn(1, dark(&palette.border)));
  assert_eq!(
    background(plain),
    probe("background-color", dark(&palette.background))
  );
  assert_eq!(settled(button, &surface).await, surface);
}

/// A text with no color under the Simple theme is inked in the palette's foreground
/// through the typography alias, and a link in the palette's link color.
#[wasm_bindgen_test]
fn text_under_the_simple_theme_inks_in_the_palettes_foreground() {
  let _scheme = Scheme::choose(ColorScheme::Light);
  let palette = SimplePalette::default();
  let (_guard, column) = themed(|| {
    Column::new().set_children((
      Text::new("plain"),
      Text::new("link").set_link("https://example.com"),
    ))
  });
  let [plain, link] = &elements(&column)[..]
  else {
    panic!("the plain text and the link");
  };

  assert_eq!(link.tag_name(), "A");
  assert_eq!(ink(plain), probe("color", light(&palette.foreground)));
  assert_eq!(ink(link), probe("color", light(&palette.link)));
}

/// With no theme above, reading the palette yields the default and defines it at the
/// root, so a box referencing a token and a Simple view alike resolve it.
#[wasm_bindgen_test]
fn the_default_palette_stands_in_at_the_root_without_a_theme() {
  let _scheme = Scheme::choose(ColorScheme::Light);
  let (_guard, root) = fresh_root();
  let palette = SimplePalette::default();
  let read = Rc::new(RefCell::new(None));

  let _ = mount({
    let read = Rc::clone(&read);

    move || {
      *read.borrow_mut() = Some(SimplePalette::current());

      Column::new()
        .set_children((
          Container::new().set_background(SimplePalette::CARD),
          SimpleCard::new(),
        ))
        .build()
    }
  });

  let [plain, card] = &elements(&only_child(&root))[..]
  else {
    panic!("the box and the card");
  };
  let color = probe("background-color", light(&palette.card));

  assert_eq!(read.borrow().as_ref(), Some(&palette));
  assert_eq!(background(plain), color);
  assert_eq!(background(card), color);
}

/// A component library's palette: one accent token, built from the same public pieces.
#[derive(Debug, Clone, PartialEq)]
struct AcmePalette {
  accent: Token<Color>,
}

impl AcmePalette {
  const ACCENT: TokenRef<Color> = TokenRef::new("acme-accent");
}

impl Default for AcmePalette {
  fn default() -> Self {
    Self {
      accent: Token::new(Self::ACCENT.name().as_str(), Color::srgb(1., 0., 0.))
        .define_when(Media::Dark, Color::srgb(0., 0., 1.)),
    }
  }
}

impl TokenSet<WebTokenValue> for AcmePalette {
  fn definitions(&self) -> Vec<(TokenName, WebTokenValue)> {
    vec![self.accent.define()]
  }
}

/// A third-party palette nests under the Simple theme exactly as the Simple palette
/// does: its theme defines it, `current` reads it, a box resolves it under both
/// schemes, and the Simple views beneath still resolve the Simple palette.
#[wasm_bindgen_test]
async fn a_third_party_palette_works_under_theme_like_the_simple_one() {
  let scheme = Scheme::choose(ColorScheme::Light);
  let palette = SimplePalette::default();
  let (_guard, acme) = themed(|| {
    Theme::new(AcmePalette::default()).set_child_builder(|| {
      assert_eq!(AcmePalette::current(), AcmePalette::default());

      SimpleCard::new().set_children((
        Container::new().set_background(AcmePalette::ACCENT),
        SimpleButton::new("Save"),
      ))
    })
  });
  let card = only_child(&acme);
  let [accent, button] = &elements(&only_child(&card))[..]
  else {
    panic!("the accent box and the button");
  };
  let surface = probe("background-color", light(&palette.primary.surface));

  assert_eq!(
    background(accent),
    probe("background-color", Color::srgb(1., 0., 0.))
  );
  assert_eq!(
    background(&card),
    probe("background-color", light(&palette.card))
  );
  assert_eq!(settled(button, &surface).await, surface);

  drop(scheme);

  let _scheme = Scheme::choose(ColorScheme::Dark);

  assert_eq!(
    background(accent),
    probe("background-color", Color::srgb(0., 0., 1.))
  );
  assert_eq!(
    background(&card),
    probe("background-color", dark(&palette.card))
  );
}

/// A picker wears the field chrome and rings while focused; its indicator follows the
/// open state; its list surface is on the card; the chosen row fills in the primary
/// look with its check and caption inked to match, and the focused row highlights.
#[wasm_bindgen_test]
fn a_pickers_face_rings_and_its_rows_follow_focus_and_the_choice() {
  let _scheme = Scheme::choose(ColorScheme::Light);
  let palette = SimplePalette::default();
  let value = Signal::new("md".to_owned());
  let (_guard, picker) = themed(move || {
    SimplePicker::new()
      .set_options([("sm", "Small"), ("md", "Medium"), ("lg", "Large")])
      .bind_chosen(value)
  });
  let [face, popup] = &elements(&picker)[..]
  else {
    panic!("the face and the popup");
  };
  let [shown, indicator] = &elements(face)[..]
  else {
    panic!("the shown label and the indicator");
  };
  let listbox = only_child(popup);
  let surface = only_child(&only_child(&listbox));
  let rows = elements(&only_child(&surface));
  let check = |index: usize| elements(&rows[index])[0].clone();
  let glyph = |index: usize| only_child(&check(index));
  let caption = |index: usize| elements(&rows[index])[1].clone();
  let transparent = probe("background-color", Color::TRANSPARENT);
  let chosen = probe("background-color", light(&palette.primary.surface));

  assert_eq!(picker.get_attribute("role").as_deref(), Some("combobox"));
  assert_eq!(shown.text_content().as_deref(), Some("Medium"));
  assert_eq!(
    background(&picker),
    probe("background-color", light(&palette.card))
  );
  assert_eq!(border(&picker), drawn(1, light(&palette.border)));
  assert_eq!(
    ink(indicator),
    probe("color", light(&palette.muted_foreground))
  );

  assert_eq!(listbox.get_attribute("role").as_deref(), Some("listbox"));
  assert_eq!(
    background(&surface),
    probe("background-color", light(&palette.card))
  );
  assert_eq!(
    rows[1].get_attribute("aria-label").as_deref(),
    Some("Medium")
  );
  assert_eq!(background(&rows[1]), chosen);
  assert_eq!(ink(&glyph(1)), probe("color", light(&palette.primary.ink)));
  assert_eq!(
    ink(&caption(1)),
    probe("color", light(&palette.primary.ink))
  );
  assert_eq!(background(&rows[0]), transparent);
  assert_eq!(computed(&check(1), "visibility"), "visible");
  assert_eq!(
    computed(&check(0), "visibility"),
    "hidden",
    "an unchosen check is hidden"
  );
  assert_eq!(ink(&caption(0)), probe("color", light(&palette.foreground)));

  html(&picker).focus().expect("focus the face");

  assert!(picker.matches(":focus-visible").expect("a selector"));
  assert_eq!(border(&picker), drawn(1, light(&palette.ring)));

  press(&picker, "ArrowDown");

  assert!(popup.matches(":popover-open").expect("a selector"));
  assert!(is_focused(&rows[1]));
  assert_eq!(ink(indicator), probe("color", light(&palette.foreground)));
  assert_eq!(
    border(&picker),
    drawn(1, light(&palette.ring)),
    "open, it rings"
  );
  assert_eq!(background(&rows[0]), transparent);

  press(&rows[1], "ArrowUp");

  assert!(is_focused(&rows[0]));
  assert_eq!(
    background(&rows[0]),
    probe("background-color", light(&palette.secondary.surface))
  );
  assert_eq!(background(&rows[1]), chosen);

  press(&rows[0], "Enter");

  assert_eq!(value.get(), "sm");
  assert!(!popup.matches(":popover-open").expect("a selector"));
  assert_eq!(shown.text_content().as_deref(), Some("Small"));
  assert!(is_focused(&picker), "focus returns to the face");

  html(&picker).blur().expect("blur the face");

  assert_eq!(background(&rows[0]), chosen);
  assert_eq!(background(&rows[1]), transparent);
}

/// A disabled button takes the palette's disabled surface and ink whatever its variant,
/// a bordered look keeping the palette's border; enabled again, it returns to its look.
#[wasm_bindgen_test]
async fn a_disabled_button_takes_the_disabled_surface_and_ink() {
  let _scheme = Scheme::choose(ColorScheme::Light);
  let palette = SimplePalette::default();
  let disabled = Signal::new(true);
  let (_guard, button) = themed(move || {
    SimpleButton::new("Save")
      .set_variant(SimpleVariant::Outline)
      .derive_disabled(move || disabled.get())
  });
  let label = only_child(&button);
  let surface = probe("background-color", light(&palette.disabled_surface));

  assert!(is_disabled(&button));
  assert_eq!(settled(&button, &surface).await, surface);
  assert_eq!(ink(&label), probe("color", light(&palette.disabled_ink)));
  assert_eq!(border(&button), drawn(1, light(&palette.border)));

  disabled.set(false);
  web::end_turn();

  let outline = probe("background-color", light(&palette.outline.surface));

  assert!(!is_disabled(&button));
  assert_eq!(settled(&button, &outline).await, outline);
  assert_eq!(ink(&label), probe("color", light(&palette.outline.ink)));
}
