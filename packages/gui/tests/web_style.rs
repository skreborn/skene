//! Declarative style dynamics rendered to CSS: conditional variants, transitions, and
//! keyframe animations—the no-script styling contract of server-rendered output.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene_gui::{
  Container,
  HasChildren,
  Text,
  core::{Animation, BoxStyle, Condition, Easing, RunStyle, StyleProperty, Transition},
  render_to_string,
};

/// The class assigned to the first (only) top-level element of a render.
fn root_class(body: &str) -> String {
  let start = body.find("class=\"").expect("a class") + 7;
  let end = body[start..].find('"').expect("a closing quote") + start;

  body[start..end].to_owned()
}

/// A hover variant renders as a `:hover` rule on the same class.
#[test]
fn state_variant_renders_a_pseudo_class_rule() {
  let rendered = render_to_string(|| {
    Container::new()
      .set_width(10.)
      .set_when(Condition::Hovered, BoxStyle::new().set_width(20.))
  });

  let class = root_class(rendered.body());

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}{{display:flow-root;width:10px;}}"))
  );

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}:hover{{width:20px;}}"))
  );
}

/// A media variant wraps its rule in an `@media` block.
#[test]
fn media_variant_renders_an_at_media_block() {
  let rendered = render_to_string(|| {
    Container::new().set_when(Condition::MaxWidth(600.), BoxStyle::new().set_spacing(4.))
  });

  let class = root_class(rendered.body());

  assert!(rendered.styles().contains(&format!(
    "@media (max-width: 600px){{.{class}{{gap:4px;}}}}"
  )));
}

/// Combined conditions nest: media wraps, states suffix, all on one rule.
#[test]
fn combined_conditions_nest_media_around_states() {
  let rendered = render_to_string(|| {
    Container::new().set_when(
      Condition::and([Condition::Hovered, Condition::DarkMode]),
      BoxStyle::new().set_border_radius(8.),
    )
  });

  let class = root_class(rendered.body());

  assert!(rendered.styles().contains(&format!(
    "@media (prefers-color-scheme: dark){{.{class}:hover{{border-radius:8px;}}}}"
  )));
}

/// An `or` condition emits one rule per alternative; a nested `and` expands into its
/// own conjunction.
#[test]
fn or_composition_emits_a_rule_per_alternative() {
  let rendered = render_to_string(|| {
    Container::new().set_when(
      Condition::or([
        Condition::Hovered,
        Condition::and([Condition::DarkMode, Condition::Focused]),
      ]),
      BoxStyle::new().set_spacing(2.),
    )
  });

  let class = root_class(rendered.body());

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}:hover{{gap:2px;}}"))
  );

  assert!(rendered.styles().contains(&format!(
    "@media (prefers-color-scheme: dark){{.{class}:focus{{gap:2px;}}}}"
  )));
}

/// Transitions render into the base rule's `transition` declaration.
#[test]
fn transitions_render_into_the_base_rule() {
  let rendered = render_to_string(|| {
    Container::new()
      .set_width(10.)
      .set_transition(Transition::new(StyleProperty::Background, 0.3))
      .set_transition(
        Transition::new(StyleProperty::Width, 1.)
          .set_delay(0.5)
          .set_easing(Easing::EaseInOut),
      )
  });

  assert!(
    rendered
      .styles()
      .contains("transition:background-color 0.3s ease 0s, width 1s ease-in-out 0.5s;")
  );
}

/// An animation renders a content-addressed `@keyframes` block plus the `animation`
/// declaration referencing it.
#[test]
fn animations_render_keyframes_and_reference_them() {
  let rendered = render_to_string(|| {
    Container::new().set_animation(
      Animation::new(
        2.,
        [
          (0., BoxStyle::new().set_border_radius(0.)),
          (100., BoxStyle::new().set_border_radius(16.)),
        ],
      )
      .set_infinite()
      .set_alternate(),
    )
  });

  let styles = rendered.styles();
  let start = styles
    .find("@keyframes skene-k-")
    .expect("a keyframes block")
    + 11;
  let end = styles[start..].find('{').expect("a body") + start;
  let name = &styles[start..end];

  assert!(styles.contains(&format!(
    "@keyframes {name}{{0%{{border-radius:0px;}}100%{{border-radius:16px;}}}}"
  )));

  assert!(styles.contains(&format!(
    "animation:{name} 2s ease 0s infinite alternate both;"
  )));
}

/// Text runs carry variants too, and identical dynamic styles intern to one class.
#[test]
fn run_variants_intern_and_render() {
  let rendered = render_to_string(|| {
    Container::new().set_children((
      Text::new("a").set_when(Condition::Hovered, RunStyle::new().set_font_weight(700.)),
      Text::new("b").set_when(Condition::Hovered, RunStyle::new().set_font_weight(700.)),
    ))
  });

  let styles = rendered.styles();
  let hover_rules = styles.matches(":hover{font-weight:700;}").count();

  assert_eq!(hover_rules, 1);
}

/// Identical full styles still collapse to one class; a differing variant splits them.
#[test]
fn variants_participate_in_content_addressing() {
  let rendered = render_to_string(|| {
    Container::new().set_children((
      Container::new()
        .set_width(10.)
        .set_when(Condition::Hovered, BoxStyle::new().set_width(20.)),
      Container::new()
        .set_width(10.)
        .set_when(Condition::Hovered, BoxStyle::new().set_width(20.)),
      Container::new()
        .set_width(10.)
        .set_when(Condition::Hovered, BoxStyle::new().set_width(30.)),
    ))
  });

  let body = rendered.body();
  let mut classes: Vec<String> = Vec::new();
  let mut rest = body;

  while let Some(index) = rest.find("class=\"") {
    let start = index + 7;
    let end = rest[start..].find('"').expect("a closing quote") + start;

    classes.push(rest[start..end].to_owned());
    rest = &rest[end..];
  }

  // The outer box plus three children; the two identical children share one class.
  assert_eq!(classes.len(), 4);
  assert_eq!(classes[1], classes[2]);
  assert_ne!(classes[1], classes[3]);
}
