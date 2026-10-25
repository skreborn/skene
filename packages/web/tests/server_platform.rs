//! The server leaf over the new model: the string tree serialized against the real
//! sink, its classes content-addressed and its stylesheet real CSS.
//!
//! Run on the server leaf: `cargo test-server --test server_platform`.
#![cfg(all(skene_platform = "web", skene_server))]

use std::{cell::Cell, rc::Rc, time::Duration};

use skene_foundation::{
  BasicCheckbox,
  BasicTextEditor,
  Behavior,
  Color,
  Condition,
  Container,
  ContainerBase,
  ContainerName,
  ContainerQuery,
  Control,
  ControlState,
  DecorationBase,
  DecorationWithCornerRadius,
  EventKind,
  Expr,
  Flexibility,
  Flexible,
  HasChild,
  HasChildren,
  Length,
  Media,
  NativeElement,
  Role,
  Row,
  State,
  StaticLeaf,
  Text,
  TextEntry,
  Theme,
  Toggle,
  Token,
  TokenName,
  TokenRef,
  TokenSet,
  When,
};
use skene_reactive::{Get, Set, signal};
use skene_web::{
  Platform,
  platform::web::{NativeView, Web, WebTokenValue, render_to_string},
  schedule,
};

type Box_ = Container<Web>;

/// A component library's palette: a card color answering the scheme, and a gutter.
#[derive(Debug, Clone, PartialEq)]
struct Palette {
  card: Token<Color, f32>,
  gutter: Token<Length, f32>,
}

impl Palette {
  const CARD: TokenRef<Color> = TokenRef::new("simple-card");
  const GUTTER: TokenRef<Length> = TokenRef::new("simple-gutter");

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
      gutter: Token::new("simple-gutter", 8),
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

/// A color as the platform renders it.
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
        .set_background(Palette::CARD)
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
      .contains("border-radius:var(--simple-gutter);"),
    "a length token read by a radius is referenced as it is: {}",
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
      .set_child_builder(|| Box_::new().set_background(Palette::CARD))
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
      ".{scope}{{--simple-card:{};--simple-gutter:8px;}}",
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
      .set_background(Expr::from(Palette::CARD).mix(red(), 0.25).with_alpha(0.5))
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
      .set_child(Box_::new().set_background(white().when(
        Condition::from(Media::Container {
          name: Some(ContainerName("sidebar")),
          query: ContainerQuery::min_width(300),
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
      .derive_background(move || color.get())
      .set_child(Text::<Web>::new("x"))
  });
  let class = first_class(rendered.body());

  assert!(
    rendered.styles().contains(&format!(
      ".{class}{{background-color:var(--skene-background);"
    )),
    "{}",
    rendered.styles()
  );
  assert!(
    rendered
      .body()
      .contains(&format!("style=\"--skene-background:{};\"", css(red()))),
    "{}",
    rendered.body()
  );
  assert!(
    rendered
      .styles()
      .contains("@property --skene-background{syntax:\"*\";inherits:false}"),
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

      Box_::new().derive_background(move || {
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
      .contains("style=\"--skene-background:var(--simple-card);\""),
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

    BasicCheckbox::<Web>::new(
      Toggle::new()
        .derive_checked(move || checked.get())
        .handle_toggle(move |next| set_checked.set(next)),
    )
    .set_part(
      skene_foundation::Part::Mark,
      Box_::new().set_background(white().when(Condition::from(State::Checked), black())),
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
    body.contains(
      "<input data-h=\"2\" type=\"checkbox\" aria-label=\"Agree\" class=\"skene-native\""
    ),
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
    styles.contains(&format!(
      ":where(input.skene-native:checked ~ *) .{mark_class}"
    )),
    "{styles}"
  );
  assert!(
    styles.contains(&format!(
      ":where(input.skene-native:checked) ~ .{mark_class}"
    )),
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

/// A native view's placeholder carries the layout its wrapper gave it in its class.
#[test]
fn a_native_view_takes_its_wrappers_layout() {
  let rendered = render_to_string(|| {
    Row::<Web>::new().add_child(
      Flexible::<Web>::new()
        .set_flexibility(Flexibility::grow(3.))
        .set_child(NativeView::new("canvas")),
    )
  });
  let body = rendered.body();
  let raw = body
    .rfind("<canvas")
    .expect("the native view's placeholder");
  let class = classes(&body[raw..])[0].clone();

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}{{flex:3 1 auto;}}")),
    "{}",
    rendered.styles()
  );
}

/// A behavior standing for a set that is only partly checked.
struct Partial;

impl Behavior for Partial {
  fn native(&self) -> Option<NativeElement> {
    Some(NativeElement::Checkbox)
  }

  fn role(&self) -> Option<Role> {
    Some(Role::CheckBox)
  }

  fn listens(&self) -> &'static [EventKind] {
    &[]
  }

  fn state(&self) -> ControlState {
    ControlState {
      checked: skene_foundation::CheckState::Mixed,
      ..ControlState::DEFAULT
    }
  }
}

/// A partly checked checkbox's input carries its state without script, and a part's
/// indeterminate variant anchors on that input.
#[test]
fn an_indeterminate_checkbox_carries_its_state_in_markup() {
  let rendered = render_to_string(|| {
    Control::<Web, Partial>::new(Partial)
      .set_part(
        skene_foundation::Part::Mark,
        Box_::new().set_background(white().when(Condition::from(State::Indeterminate), black())),
      )
      .set_label("Some")
  });
  let body = rendered.body();
  let styles = rendered.styles();

  assert!(body.contains("data-skene-indeterminate"), "{body}");
  assert!(!body.contains(" indeterminate"), "{body}");
  assert!(
    styles.contains("input.skene-native:is(:indeterminate, [data-skene-indeterminate])"),
    "{styles}"
  );
}

/// A text editor whose value starts with a newline keeps it through the parser, which
/// drops a textarea's first newline.
#[test]
fn a_text_editor_keeps_a_leading_newline() {
  let rendered = render_to_string(|| {
    BasicTextEditor::<Web>::new(TextEntry::new().set_lines(3).set_value("\nsecond"))
  });
  let body = rendered.body();

  assert!(body.contains(">\n\nsecond</textarea>"), "{body}");
}

/// Two boxes styled by the same token under two palettes intern one class.
#[test]
fn two_palettes_share_one_consumer_class() {
  let rendered = render_to_string(|| {
    Theme::<Web, _>::new(Palette::default()).set_child_builder(|| {
      Box_::new().set_background(Palette::CARD).set_child(
        Theme::<Web, _>::new(Palette::with_card(red()))
          .set_child_builder(|| Box_::new().set_background(Palette::CARD)),
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
      .set_background(white())
      .set_child(Box_::new().set_width(10))
  };
  let first = render_to_string(app);
  let second = render_to_string(app);

  assert_eq!(first.styles(), second.styles());
  assert_eq!(first.body(), second.body());

  let reversed = render_to_string(|| {
    Box_::new()
      .set_width(10)
      .set_child(Box_::new().set_background(white()))
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

/// The server leaf is the web platform, told apart by `IS_SERVER`, and both answer as
/// consts.
#[test]
fn the_server_leaf_is_the_web_platform() {
  const _: () = assert!(matches!(Platform::CURRENT, Platform::Web) && Platform::IS_SERVER);

  assert_eq!(Platform::CURRENT, Platform::Web);
  assert_eq!((Platform::IS_SERVER, Platform::IS_APPLE), (true, false));
  assert_eq!(Platform::CURRENT.name(), "web");
}

/// A server render has no clock: an action scheduled while it builds never runs.
#[test]
fn a_server_render_never_runs_a_scheduled_action() {
  let ran = Rc::new(Cell::new(false));
  let rendered = render_to_string({
    let ran = Rc::clone(&ran);

    move || {
      let _ = schedule(Duration::ZERO, move || ran.set(true));

      Box_::new()
    }
  });

  assert!(rendered.body().contains("skene-"), "{}", rendered.body());
  assert!(!ran.get());
}
