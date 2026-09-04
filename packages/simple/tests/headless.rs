//! The Simple library on the host driver: every view resolves the palette's tokens
//! through the one theme mechanism, the controls' parts follow the control, and a
//! third-party palette works under the same theme.

use skene_core::{
  Color,
  ColorScheme,
  Dimension,
  KeyEvent,
  Modifiers,
  next::{
    Border,
    BoxValue,
    HasChildren,
    Media,
    NoProperty,
    Part,
    Property,
    PropertyKey,
    Slot,
    StaticLeaf,
    Token,
    TokenName,
    TokenRef,
    TokenSet,
    View,
  },
};
use skene_headless::{
  Headless,
  HeadlessTokenValue,
  SlotValue,
  accessible_name,
  children,
  control_props,
  is_focused,
  is_open,
  part,
  reset,
  resolve_border,
  resolve_color,
  root_definitions,
  set_hovered,
  set_scheme,
  slot_value,
  text,
  trigger_blur,
  trigger_change,
  trigger_click,
  trigger_focus,
  trigger_key_down,
};
use skene_reactive::{Get, Owner, Set, Signal};
use skene_simple::next::SimpleVariant;
use skene_views::ThemedIn;

type SimplePalette = skene_simple::next::SimplePalette<f32>;
type SimpleButton = skene_simple::next::SimpleButton<Headless>;
type SimpleCard = skene_simple::next::SimpleCard<Headless>;
type SimpleCheckbox = skene_simple::next::SimpleCheckbox<Headless>;
type SimpleRadioGroup = skene_simple::next::SimpleRadioGroup<Headless>;
type SimpleRadio = skene_simple::next::SimpleRadio<Headless>;
type SimpleSlider = skene_simple::next::SimpleSlider<Headless>;
type SimpleTextField = skene_simple::next::SimpleTextField<Headless>;
type SimpleTextEditor = skene_simple::next::SimpleTextEditor<Headless>;
type SimplePicker = skene_simple::next::SimplePicker<Headless>;
type Container = skene_views::Container<Headless>;
type Column = skene_views::Column<Headless>;
type Text = skene_views::Text<Headless>;
type Theme<P, C = skene_views::NoChild> = skene_views::Theme<Headless, P, C>;

const COLOR: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Color);
const TEXT_COLOR: PropertyKey<NoProperty> = PropertyKey::Shared(Property::TextColor);

/// The fixed color `token` is defined as: under the dark scheme when `dark`, else its
/// base.
fn defined(token: &Token<Color, f32>, dark: bool) -> Color {
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

fn light(token: &Token<Color, f32>) -> Color {
  defined(token, false)
}

fn dark(token: &Token<Color, f32>) -> Color {
  defined(token, true)
}

/// The one view built under a default Simple theme.
fn themed<V: View<Headless>>(
  owner: Owner,
  view: impl FnOnce() -> V + 'static,
) -> skene_core::next::NodeId {
  let theme = owner.enter(|| {
    Theme::new(SimplePalette::default())
      .set_child_builder(view)
      .build()
  });

  children(theme)[0]
}

/// A button's surface is the primary look's, hovering paints the hover surface,
/// focusing the control paints the focus surface on the control itself, its label is
/// inked in the look's ink, and a filled look draws a transparent hairline.
#[test]
fn a_buttons_surface_hover_and_focus_resolve_the_primary_look_on_the_control() {
  reset();

  let owner = Owner::new();
  let palette = SimplePalette::default();
  let button = themed(owner, || SimpleButton::new("Save"));
  let label = children(button)[0];

  assert_eq!(accessible_name(button), Some("Save".to_owned()));
  assert_eq!(part(label), Some(Part::Label));
  assert_eq!(
    resolve_color(button, COLOR),
    Some(light(&palette.primary.surface))
  );
  assert_eq!(
    resolve_color(label, TEXT_COLOR),
    Some(light(&palette.primary.ink))
  );
  assert_eq!(
    resolve_border(button),
    Some(Border::new(1., Color::TRANSPARENT))
  );

  set_hovered(button, true);

  assert_eq!(
    resolve_color(button, COLOR),
    Some(light(&palette.primary.hover))
  );

  set_hovered(button, false);
  trigger_focus(button);

  assert_eq!(
    resolve_color(button, COLOR),
    Some(light(&palette.primary.focus))
  );

  owner.dispose();
}

/// A derived variant redefines the look on the button's own node: the surface and the
/// border follow the signal and the node is never rebuilt.
#[test]
fn a_derived_variant_redefines_the_look_without_a_rebuild() {
  reset();

  let owner = Owner::new();
  let palette = SimplePalette::default();
  let variant = Signal::new(SimpleVariant::Primary);
  let button = themed(owner, move || {
    SimpleButton::new("Go").derive_variant(move || variant.get())
  });

  assert_eq!(
    resolve_color(button, COLOR),
    Some(light(&palette.primary.surface))
  );

  variant.set(SimpleVariant::Outline);

  assert_eq!(resolve_color(button, COLOR), Some(Color::TRANSPARENT));
  assert_eq!(
    resolve_border(button),
    Some(Border::new(
      1.,
      light(palette.outline.border.as_ref().unwrap())
    ))
  );

  variant.set(SimpleVariant::Destructive);

  assert_eq!(
    resolve_color(button, COLOR),
    Some(light(&palette.destructive.surface))
  );
  assert_eq!(
    resolve_border(button),
    Some(Border::new(1., Color::TRANSPARENT))
  );

  owner.dispose();
}

/// A checkbox's mark sits on the card surface and fills with the primary surface, its
/// border and its glyph following the control's checked state after a click.
#[test]
fn a_checkbox_mark_follows_the_checked_state() {
  reset();

  let owner = Owner::new();
  let palette = SimplePalette::default();
  let bound = Signal::new(false);
  let checkbox = themed(owner, move || {
    SimpleCheckbox::new()
      .set_label("Remember me")
      .bind_checked(bound)
  });
  let mark = children(checkbox)[0];
  let glyph = children(mark)[1];

  assert_eq!(part(mark), Some(Part::Mark));
  assert_eq!(accessible_name(checkbox), Some("Remember me".to_owned()));
  assert_eq!(resolve_color(mark, COLOR), Some(light(&palette.card)));
  assert_eq!(
    resolve_border(mark),
    Some(Border::new(1., light(&palette.border)))
  );
  assert_eq!(resolve_color(glyph, TEXT_COLOR), Some(Color::TRANSPARENT));

  let _clicked = trigger_click(checkbox);

  assert!(bound.get());
  assert_eq!(
    resolve_color(mark, COLOR),
    Some(light(&palette.primary.surface))
  );
  assert_eq!(
    resolve_border(mark),
    Some(Border::new(1., light(&palette.primary.surface)))
  );
  assert_eq!(
    resolve_color(glyph, TEXT_COLOR),
    Some(light(&palette.primary.ink))
  );

  trigger_focus(checkbox);

  assert_eq!(
    resolve_border(mark),
    Some(Border::new(1., light(&palette.ring)))
  );

  owner.dispose();
}

/// A radio's ring shows its dot only while the group's choice is its value.
#[test]
fn a_radio_ring_follows_the_groups_choice() {
  reset();

  let owner = Owner::new();
  let palette = SimplePalette::default();
  let chosen: Signal<Option<String>> = Signal::new(None);
  let group = themed(owner, move || {
    SimpleRadioGroup::new()
      .bind_chosen(chosen)
      .set_child(Column::new().set_children((
        SimpleRadio::new("a").set_label("A"),
        SimpleRadio::new("b").set_label("B"),
      )))
  });
  let column = children(group)[0];
  let member = |index: usize| children(column)[index + 1];
  let dot = |index: usize| children(children(member(index))[0])[1];

  assert_eq!(part(children(member(0))[0]), Some(Part::Mark));
  assert_eq!(resolve_color(dot(0), COLOR), Some(Color::TRANSPARENT));

  let _clicked = trigger_click(member(1));

  assert_eq!(chosen.get().as_deref(), Some("b"));
  assert_eq!(
    resolve_color(dot(1), COLOR),
    Some(light(&palette.primary.surface))
  );
  assert_eq!(resolve_color(dot(0), COLOR), Some(Color::TRANSPARENT));

  owner.dispose();
}

/// A slider's fill is the fraction of the range the bound value sits at, and its
/// thumb's border becomes the ring when the control is focused.
#[test]
fn a_slider_fill_follows_the_bound_value_and_its_thumb_rings_when_focused() {
  reset();

  let owner = Owner::new();
  let palette = SimplePalette::default();
  let bound = Signal::new(50.);
  let slider = themed(owner, move || {
    SimpleSlider::new().set_max(200.).bind_value(bound)
  });
  let stack = children(children(slider)[0])[1];
  let track = children(stack)[1];
  let fill = children(track)[1];
  let thumb = children(children(stack)[2])[1];
  let fill_width = || match slot_value(fill, Slot(0)) {
    Some(SlotValue::Box(BoxValue::Dimension(Dimension::Fraction(fraction)))) => fraction,
    other => panic!("the fill's width is a derived fraction, not {other:?}"),
  };

  assert_eq!(part(track), Some(Part::Track));
  assert_eq!(part(fill), Some(Part::Fill));
  assert_eq!(part(thumb), Some(Part::Thumb));
  assert!((fill_width() - 0.25).abs() < 1e-6);
  assert_eq!(
    resolve_color(fill, COLOR),
    Some(light(&palette.primary.surface))
  );

  bound.set(150.);

  assert!((fill_width() - 0.75).abs() < 1e-6);
  assert_eq!(
    resolve_border(thumb),
    Some(Border::new(2., light(&palette.background)))
  );

  trigger_focus(slider);

  assert_eq!(
    resolve_border(thumb),
    Some(Border::new(2., light(&palette.ring)))
  );

  owner.dispose();
}

/// A text field's border is the ring only while it is focused, and an editor wears the
/// same chrome over more lines.
#[test]
fn a_text_fields_border_is_the_ring_only_while_focused() {
  reset();

  let owner = Owner::new();
  let palette = SimplePalette::default();
  let bound = Signal::new(String::new());
  let field = themed(owner, move || {
    SimpleTextField::new()
      .set_placeholder("Name")
      .bind_value(bound)
  });
  let editor = themed(owner, || SimpleTextEditor::new().set_lines(6));

  assert_eq!(
    control_props(field).and_then(|props| props.placeholder),
    Some("Name".to_owned())
  );
  assert_eq!(
    resolve_border(field),
    Some(Border::new(1., light(&palette.border)))
  );

  trigger_focus(field);

  assert_eq!(
    resolve_border(field),
    Some(Border::new(1., light(&palette.ring)))
  );

  trigger_blur(field);

  assert_eq!(
    resolve_border(field),
    Some(Border::new(1., light(&palette.border)))
  );

  trigger_change(field, "typed");

  assert_eq!(bound.get(), "typed");
  assert_eq!(control_props(editor).map(|props| props.lines), Some(6));
  assert_eq!(
    resolve_border(editor),
    Some(Border::new(1., light(&palette.border)))
  );

  owner.dispose();
}

/// Under the dark scheme every token of the default palette resolves its dark value,
/// on a box and on a button alike.
#[test]
fn the_default_palette_answers_the_dark_scheme() {
  reset();

  let owner = Owner::new();
  let palette = SimplePalette::default();
  let card = themed(owner, || {
    SimpleCard::new().set_children((
      Container::new().set_color(SimplePalette::BACKGROUND),
      SimpleButton::new("Save"),
    ))
  });
  let column = children(card)[1];
  let background = children(column)[1];
  let button = children(column)[2];

  assert_eq!(resolve_color(card, COLOR), Some(light(&palette.card)));

  set_scheme(ColorScheme::Dark);

  assert_eq!(resolve_color(card, COLOR), Some(dark(&palette.card)));
  assert_eq!(
    resolve_border(card),
    Some(Border::new(1., dark(&palette.border)))
  );
  assert_eq!(
    resolve_color(background, COLOR),
    Some(dark(&palette.background))
  );
  assert_eq!(
    resolve_color(button, COLOR),
    Some(dark(&palette.primary.surface))
  );

  owner.dispose();
}

/// A text with no color under the Simple theme is inked in the palette's foreground
/// through the typography alias, and a link in the palette's link color.
#[test]
fn text_under_the_simple_theme_inks_in_the_palettes_foreground() {
  reset();

  let owner = Owner::new();
  let palette = SimplePalette::default();
  let column = themed(owner, || {
    Column::new().set_children((
      Text::new("plain"),
      Text::new("link").set_link("https://example.com"),
    ))
  });
  let plain = children(column)[1];
  let link = children(column)[2];

  assert_eq!(
    resolve_color(plain, TEXT_COLOR),
    Some(light(&palette.foreground))
  );
  assert_eq!(resolve_color(link, TEXT_COLOR), Some(light(&palette.link)));

  owner.dispose();
}

/// With no theme above, reading the palette yields the default and defines it at the
/// root, so a box referencing a token and a Simple view alike resolve it.
#[test]
fn the_default_palette_stands_in_at_the_root_without_a_theme() {
  reset();

  let owner = Owner::new();
  let palette = SimplePalette::default();
  let (read, box_node, card) = owner.enter(|| {
    let read = <SimplePalette as ThemedIn<Headless>>::current();

    (
      read,
      Container::new().set_color(SimplePalette::CARD).build(),
      SimpleCard::new().build(),
    )
  });

  assert_eq!(read, palette);
  assert!(root_definitions().get(SimplePalette::CARD.name()).is_some());
  assert_eq!(resolve_color(box_node, COLOR), Some(light(&palette.card)));
  assert_eq!(resolve_color(card, COLOR), Some(light(&palette.card)));

  owner.dispose();
}

/// A component library's palette: one accent token, built from the same public pieces.
#[derive(Debug, Clone, PartialEq)]
struct AcmePalette {
  accent: Token<Color, f32>,
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

impl TokenSet<HeadlessTokenValue> for AcmePalette {
  fn definitions(&self) -> Vec<(TokenName, HeadlessTokenValue)> {
    vec![self.accent.define()]
  }
}

/// A third-party palette nests under the Simple theme exactly as the Simple palette
/// does: its theme defines it, `current` reads it, a box resolves it under both
/// schemes, and the Simple views beneath still resolve the Simple palette.
#[test]
fn a_third_party_palette_works_under_theme_like_the_simple_one() {
  reset();

  let owner = Owner::new();
  let palette = SimplePalette::default();
  let acme = themed(owner, || {
    Theme::new(AcmePalette::default()).set_child_builder(|| {
      assert_eq!(
        <AcmePalette as ThemedIn<Headless>>::current(),
        AcmePalette::default()
      );

      SimpleCard::new().set_children((
        Container::new().set_color(AcmePalette::ACCENT),
        SimpleButton::new("Save"),
      ))
    })
  });
  let card = children(acme)[0];
  let column = children(card)[1];
  let accent = children(column)[1];
  let button = children(column)[2];

  assert_eq!(resolve_color(accent, COLOR), Some(Color::srgb(1., 0., 0.)));
  assert_eq!(resolve_color(card, COLOR), Some(light(&palette.card)));
  assert_eq!(
    resolve_color(button, COLOR),
    Some(light(&palette.primary.surface))
  );

  set_scheme(ColorScheme::Dark);

  assert_eq!(resolve_color(accent, COLOR), Some(Color::srgb(0., 0., 1.)));
  assert_eq!(resolve_color(card, COLOR), Some(dark(&palette.card)));

  owner.dispose();
}

/// A picker wears the field chrome and rings while focused; its indicator is the face
/// part and follows the open state; its list surface is the list part on the card; the
/// chosen row fills in the primary look with its check and caption inked to match, and
/// the focused row highlights.
#[test]
fn a_pickers_face_rings_and_its_rows_follow_focus_and_the_choice() {
  reset();

  let owner = Owner::new();
  let palette = SimplePalette::default();
  let value = Signal::new("md".to_owned());
  let picker = themed(owner, move || {
    SimplePicker::new()
      .set_options([("sm", "Small"), ("md", "Medium"), ("lg", "Large")])
      .bind_value(value)
  });
  let face = children(picker)[0];
  let shown = children(face)[1];
  let indicator = children(face)[2];
  let overlay = children(picker)[1];
  let listbox = children(overlay)[1];
  let surface = children(children(listbox)[0])[1];
  let column = children(surface)[1];
  let row = |index: usize| children(column)[index + 1];
  let check = |index: usize| children(row(index))[0];
  let caption = |index: usize| children(row(index))[1];
  let key = |key: &str| KeyEvent {
    key: key.to_owned(),
    modifiers: Modifiers::default(),
  };

  assert_eq!(text(shown), Some("Medium".to_owned()));
  assert_eq!(part(indicator), Some(Part::Face));
  assert_eq!(resolve_color(picker, COLOR), Some(light(&palette.card)));
  assert_eq!(
    resolve_border(picker),
    Some(Border::new(1., light(&palette.border)))
  );
  assert_eq!(
    resolve_color(indicator, TEXT_COLOR),
    Some(light(&palette.muted_foreground))
  );

  trigger_focus(picker);

  assert_eq!(
    resolve_border(picker),
    Some(Border::new(1., light(&palette.ring)))
  );

  assert_eq!(part(surface), Some(Part::List));
  assert_eq!(resolve_color(surface, COLOR), Some(light(&palette.card)));
  assert_eq!(part(check(1)), Some(Part::Item));
  assert_eq!(part(caption(1)), Some(Part::Label));
  assert_eq!(accessible_name(row(1)), Some("Medium".to_owned()));
  assert_eq!(
    resolve_color(row(1), COLOR),
    Some(light(&palette.primary.surface))
  );
  assert_eq!(
    resolve_color(check(1), TEXT_COLOR),
    Some(light(&palette.primary.ink))
  );
  assert_eq!(
    resolve_color(caption(1), TEXT_COLOR),
    Some(light(&palette.primary.ink))
  );
  assert_eq!(resolve_color(row(0), COLOR), Some(Color::TRANSPARENT));
  assert_eq!(
    resolve_color(check(0), TEXT_COLOR),
    Some(Color::TRANSPARENT)
  );
  assert_eq!(
    resolve_color(caption(0), TEXT_COLOR),
    Some(light(&palette.foreground))
  );

  let _ = trigger_key_down(picker, &key("ArrowDown"));

  assert!(is_open(overlay));
  assert!(is_focused(row(1)));
  assert_eq!(
    resolve_color(indicator, TEXT_COLOR),
    Some(light(&palette.foreground))
  );

  let _ = trigger_key_down(row(1), &key("ArrowUp"));

  assert!(is_focused(row(0)));
  assert_eq!(
    resolve_color(row(0), COLOR),
    Some(light(&palette.secondary.surface))
  );
  assert_eq!(
    resolve_color(row(1), COLOR),
    Some(light(&palette.primary.surface))
  );

  let _ = trigger_key_down(row(0), &key("Enter"));

  assert_eq!(value.get(), "sm");
  assert!(!is_open(overlay));
  assert_eq!(text(shown), Some("Small".to_owned()));
  assert_eq!(
    resolve_color(row(0), COLOR),
    Some(light(&palette.primary.surface))
  );
  assert_eq!(resolve_color(row(1), COLOR), Some(Color::TRANSPARENT));

  owner.dispose();
}
