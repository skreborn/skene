//! The server leaf over the new model: the string tree serialized against the real
//! sink, its classes content-addressed and its stylesheet real CSS.
//!
//! Run under a WASI runtime:
//! `cargo test -p skene-web --target wasm32-wasip2 --test web_ssr`.
#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene_core::{
  Color,
  Condition,
  ContainerName,
  ContainerQuery,
  Dimension,
  Expr,
  HasChildren,
  Media,
  State,
  StaticLeaf,
  Token,
  TokenName,
  TokenRef,
  TokenSet,
  When,
};
use skene_reactive::{Get, Set, signal};
use skene_views::{Container, RawCheckbox, Text, Theme, Toggle};
use skene_web::{Web, WebTokenValue, render_to_string};

type Box_ = Container<Web>;

/// A component library's palette: a card color answering the scheme, and a gutter.
#[derive(Debug, Clone, PartialEq)]
struct Palette {
  card: Token<Color, f32>,
  gutter: Token<f32, f32>,
}

impl Palette {
  const CARD: TokenRef<Color> = TokenRef::new("simple-card");
  const GUTTER: TokenRef<f32> = TokenRef::new("simple-gutter");

  fn with_card(card: Color) -> Self {
    Self {
      card: Token::new("simple-card", card),
      ..Self::default()
    }
  }
}

impl Default for Palette {
  fn default() -> Self {
    Self {
      card: Token::new("simple-card", white()).define_when(Media::Dark, black()),
      gutter: Token::new("simple-gutter", 8.),
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

/// A color as the driver renders it.
fn css(color: Color) -> String {
  let [lightness, chroma, hue, alpha] = color.to_oklch();

  format!("oklch({lightness} {chroma} {hue} / {alpha})")
}

/// Every class named in `body`, in document order.
fn classes(body: &str) -> Vec<String> {
  body
    .match_indices("class=\"")
    .map(|(start, _)| {
      let rest = &body[start + 7..];

      rest[..rest.find('"').expect("class end")].to_owned()
    })
    .collect()
}

/// The first `skene-` class in `body`.
fn first_class(body: &str) -> String {
  classes(body)
    .into_iter()
    .find(|class| class.starts_with("skene-"))
    .expect("a styled element")
}

const DEFERRING: &str = ":where(:root:not([data-skene-scheme=\"light\"]))";
const CHOSEN: &str = ":where(:root[data-skene-scheme=\"dark\"])";

/// A token leaf renders as a reference to its custom property and never as a literal.
#[test]
fn a_token_renders_as_a_custom_property_reference() {
  let rendered = render_to_string(|| {
    Theme::<Web, _>::new(Palette::default()).set_child_builder(|| {
      Box_::new()
        .set_color(Palette::CARD)
        .set_corner_radius(Palette::GUTTER)
    })
  });
  let class = classes(rendered.body())[1].clone();

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}{{background-color:var(--simple-card);")),
    "{}",
    rendered.styles()
  );
  assert!(
    rendered
      .styles()
      .contains("border-radius:calc(var(--simple-gutter) * 1px);"),
    "a number token read by a pixel property scales to pixels: {}",
    rendered.styles()
  );
  assert!(
    !rendered
      .styles()
      .split(".skene-")
      .last()
      .unwrap()
      .contains(&css(white())),
    "the consumer's class holds no literal: {}",
    rendered.styles()
  );
}

/// A theme scope defines its tokens on a class of its own, the dark variant under the
/// preference query guarded by the deferring root and under the chosen root outright.
#[test]
fn a_theme_scope_defines_its_tokens_under_both_scheme_guards() {
  let rendered = render_to_string(|| {
    Theme::<Web, _>::new(Palette::default())
      .set_child_builder(|| Box_::new().set_color(Palette::CARD))
  });
  let body = rendered.body();
  let styles = rendered.styles();
  let scope = first_class(body);

  assert!(
    body.starts_with("<div data-h=\"1\""),
    "the scope is the first element: {body}"
  );
  assert!(
    body.contains("style=\"display:contents;\""),
    "and stays out of layout: {body}"
  );
  assert!(
    styles.contains(&format!(
      ".{scope}{{--simple-card:{};--simple-gutter:8;}}",
      css(white())
    )),
    "{styles}"
  );
  assert!(
    styles.contains(&format!(
      "@media (prefers-color-scheme: dark){{{DEFERRING} .{scope}{{--simple-card:{};}}}}",
      css(black())
    )),
    "{styles}"
  );
  assert!(
    styles.contains(&format!(
      "{CHOSEN} .{scope}{{--simple-card:{};}}",
      css(black())
    )),
    "{styles}"
  );
  assert!(
    styles.contains(
      "@property --simple-card{syntax:\"<color>\";inherits:true;initial-value:transparent}"
    ),
    "the token registers its type once: {styles}"
  );
  assert_eq!(styles.matches("@property --simple-card{").count(), 1);
}

/// A color expression renders as `color-mix()` and relative color.
#[test]
fn a_color_expression_renders_as_css_color_functions() {
  let rendered = render_to_string(|| {
    Box_::new()
      .set_color(Expr::from(Palette::CARD).mix(red(), 0.25).with_alpha(0.5))
      .set_child(Text::<Web>::new("x"))
  });

  assert!(
    rendered.styles().contains(&format!(
      "background-color:oklch(from color-mix(in oklab, var(--simple-card), {} 25%) l c h / 0.5);",
      css(red())
    )),
    "{}",
    rendered.styles()
  );
}

/// A container condition renders an `@container` block, and the named box publishes
/// itself as a container.
#[test]
fn a_container_condition_renders_an_at_container_block() {
  let rendered = render_to_string(|| {
    Box_::new()
      .set_container_name(ContainerName("sidebar"))
      .set_child(Box_::new().set_color(white().when(
        Condition::from(Media::Container {
          name: Some(ContainerName("sidebar")),
          query: ContainerQuery::MinWidth(300.),
        }),
        black(),
      )))
  });
  let styles = rendered.styles();
  let inner = classes(rendered.body())[1].clone();

  assert!(
    styles.contains("container:sidebar / inline-size;"),
    "{styles}"
  );
  assert!(
    styles.contains(&format!(
      "@container sidebar (min-width: 300px){{.{inner}{{background-color:{};}}}}",
      css(black())
    )),
    "{styles}"
  );
}

/// A derived leaf is a slot: the class references its custom property and the value
/// computed on the server is written inline, so a variant still outranks it.
#[test]
fn a_derived_color_is_a_slot_reference_with_its_value_inline() {
  let rendered = render_to_string(|| {
    let (color, _) = signal(red());

    Box_::new()
      .derive_color(move || color.get())
      .set_child(Text::<Web>::new("x"))
  });
  let class = first_class(rendered.body());

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}{{background-color:var(--skene-color);")),
    "{}",
    rendered.styles()
  );
  assert!(
    rendered
      .body()
      .contains(&format!("style=\"--skene-color:{};\"", css(red()))),
    "{}",
    rendered.body()
  );
  assert!(
    rendered
      .styles()
      .contains("@property --skene-color{syntax:\"*\";inherits:false}"),
    "the slot is registered uninherited: {}",
    rendered.styles()
  );
}

/// A derived leaf naming a token writes the token's reference inline, so the page's
/// theme answers it with no script.
#[test]
fn a_derived_token_is_a_slot_reference_written_inline() {
  let rendered = render_to_string(|| {
    Theme::<Web, _>::new(Palette::default()).set_child_builder(|| {
      let (plain, _) = signal(false);

      Box_::new().derive_color(move || {
        if plain.get() {
          StaticLeaf::from(red())
        }
        else {
          StaticLeaf::from(Palette::CARD)
        }
      })
    })
  });

  assert!(
    rendered
      .body()
      .contains("style=\"--skene-color:var(--simple-card);\""),
    "{}",
    rendered.body()
  );
}

/// A checkbox is a label wrapping its native input first, then its parts, and a part's
/// checked variant anchors on that input.
#[test]
fn a_checkbox_is_a_label_over_a_native_input_and_its_parts() {
  let rendered = render_to_string(|| {
    let (checked, set_checked) = signal(true);

    RawCheckbox::<Web>::new(
      Toggle::new()
        .derive_checked(move || checked.get())
        .handle_toggle(move |next| set_checked.set(next)),
    )
    .set_part(
      skene_views::Part::Mark,
      Box_::new().set_color(white().when(Condition::from(State::Checked), black())),
    )
    .set_label("Agree")
  });
  let body = rendered.body();
  let styles = rendered.styles();

  assert!(
    body.starts_with("<label data-h=\"1\" data-skene-control"),
    "{body}"
  );
  assert!(
    body.contains("<input data-h=\"2\" type=\"checkbox\" class=\"skene-native\""),
    "{body}"
  );
  assert!(
    body.contains("checked") && body.contains("tabindex=\"0\""),
    "the input carries the state and the focus: {body}"
  );

  let input_end = body.find("</label>").expect("the label closes");
  let mark = body.find("<div").expect("the mark part");
  let label_text = body.find(">Agree<").expect("the label text");

  assert!(mark < label_text && label_text < input_end);

  let mark_class = classes(&body[mark..])[0].clone();

  assert!(
    styles.contains(&format!(":where(input:checked ~ *) .{mark_class}")),
    "{styles}"
  );
  assert!(
    styles.contains(&format!(":where(input:checked) ~ .{mark_class}")),
    "{styles}"
  );
  assert!(
    styles.contains(&format!(
      ".{mark_class}{{background-color:{};}}",
      css(black())
    )),
    "{styles}"
  );
}

/// Two boxes styled by the same token under two palettes intern one class, since a
/// consumer emits the name alone.
#[test]
fn two_palettes_share_one_consumer_class() {
  let rendered = render_to_string(|| {
    Theme::<Web, _>::new(Palette::default()).set_child_builder(|| {
      Box_::new().set_color(Palette::CARD).set_child(
        Theme::<Web, _>::new(Palette::with_card(red()))
          .set_child_builder(|| Box_::new().set_color(Palette::CARD)),
      )
    })
  });
  let consumers: Vec<String> = classes(rendered.body())
    .into_iter()
    .filter(|class| {
      rendered
        .styles()
        .contains(&format!(".{class}{{background-color"))
    })
    .collect();

  assert_eq!(consumers.len(), 2, "{}", rendered.body());
  assert_eq!(consumers[0], consumers[1]);
  assert_eq!(
    rendered
      .styles()
      .matches(&format!(".{}{{", consumers[0]))
      .count(),
    1
  );
}

/// Classes are content-addressed and rules follow interning order: two renders agree
/// byte for byte, and interning the same styles in another order changes the order of
/// the rules but never a name.
#[test]
fn classes_are_deterministic_and_rules_follow_interning_order() {
  let app = || {
    Box_::new()
      .set_color(white())
      .set_child(Box_::new().set_width(Dimension::Length(10.)))
  };
  let first = render_to_string(app);
  let second = render_to_string(app);

  assert_eq!(first.styles(), second.styles());
  assert_eq!(first.body(), second.body());

  let reversed = render_to_string(|| {
    Box_::new()
      .set_width(Dimension::Length(10.))
      .set_child(Box_::new().set_color(white()))
  });
  let forward = classes(first.body());
  let backward = classes(reversed.body());

  assert_eq!(forward[0], backward[1]);
  assert_eq!(forward[1], backward[0]);

  let position = |styles: &str, class: &str| styles.find(&format!(".{class}{{")).unwrap();

  assert!(position(first.styles(), &forward[0]) < position(first.styles(), &forward[1]));
  assert!(position(reversed.styles(), &backward[0]) < position(reversed.styles(), &backward[1]));
}

/// Every slot registers once, and the registrations precede the rules.
#[test]
fn slot_registrations_appear_once() {
  let rendered = render_to_string(|| Box_::new().set_child(Text::<Web>::new("x")));
  let styles = rendered.styles();

  assert_eq!(styles.matches("@property --skene-width{").count(), 1);
  assert_eq!(styles.matches("@property --skene-text-color{").count(), 1);
  assert_eq!(styles.matches("@property --skene-font-size{").count(), 1);
  let class = first_class(rendered.body());

  assert!(styles.find("@property").unwrap() < styles.find(&format!(".{class}{{")).unwrap());
}
