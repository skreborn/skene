//! Declarative style dynamics rendered to CSS: conditional variants, transitions, and
//! keyframe animations—the no-script styling contract of server-rendered output.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

use skene_gui::{
  Container,
  HasChildren,
  Text,
  core::{
    AlignItems,
    Animation,
    BoxStyle,
    Condition,
    Dimension,
    Easing,
    FlexWrap,
    GridPlacement,
    GridTrack,
    Inset,
    Line,
    Overflow,
    Position,
    RunStyle,
    StyleProperty,
    TrackLimit,
    Transition,
  },
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
    Container::new().set_when(Condition::MaxWidth(600.), BoxStyle::new().set_gap(4.))
  });

  let class = root_class(rendered.body());

  assert!(rendered.styles().contains(&format!(
    "@media (max-width: 600px){{.{class}{{column-gap:4px;row-gap:4px;}}}}"
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
      BoxStyle::new().set_gap(2.),
    )
  });

  let class = root_class(rendered.body());

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}:hover{{column-gap:2px;row-gap:2px;}}"))
  );

  assert!(rendered.styles().contains(&format!(
    "@media (prefers-color-scheme: dark){{.{class}:focus{{column-gap:2px;row-gap:2px;}}}}"
  )));
}

/// Overflow renders into the interned class.
#[test]
fn overflow_renders() {
  let rendered = render_to_string(|| Container::new().set_overflow(Overflow::Scroll));

  let class = root_class(rendered.body());

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}{{display:flow-root;overflow:scroll;}}"))
  );
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

/// The full flex item vocabulary compiles to its CSS properties, dimensions included.
#[test]
fn flex_item_properties_render() {
  let rendered = render_to_string(|| {
    Container::row()
      .set_flex_wrap(FlexWrap::Wrap)
      .set_width(Dimension::Percent(50.))
      .set_flex_basis(120.)
      .set_flex_grow(2.)
      .set_flex_shrink(0.)
      .set_align_self(AlignItems::Center)
      .set_margin(Inset {
        top: Dimension::Length(4.),
        right: Dimension::Auto,
        bottom: Dimension::Length(4.),
        left: Dimension::Auto,
      })
  });

  let class = root_class(rendered.body());
  let styles = rendered.styles();

  assert!(styles.contains(&format!(
    ".{class}{{align-self:center;display:flex;flex-basis:120px;flex-direction:row;\
     flex-grow:2;flex-shrink:0;flex-wrap:wrap;margin:4px auto 4px auto;width:50%;}}"
  )));
}

/// A positioned box renders its mode and inset.
#[test]
fn positioned_box_renders_inset() {
  let rendered = render_to_string(|| {
    Container::new()
      .set_position(Position::Absolute)
      .set_inset(Inset {
        top: Dimension::Length(0.),
        right: Dimension::Length(8.),
        bottom: Dimension::Auto,
        left: Dimension::Percent(10.),
      })
  });

  let class = root_class(rendered.body());

  assert!(rendered.styles().contains(&format!(
    ".{class}{{display:flow-root;inset:0px 8px auto 10%;position:absolute;}}"
  )));
}

/// Grid templates render taffy's full track vocabulary, and placements pin children.
#[test]
fn grid_tracks_and_placements_render() {
  let rendered = render_to_string(|| {
    Container::grid(1)
      .set_grid_template_columns([
        GridTrack::Length(100.),
        GridTrack::Fr(1.),
        GridTrack::Minmax(TrackLimit::Length(50.), TrackLimit::Fr(2.)),
        GridTrack::FitContent(200.),
      ])
      .set_grid_template_rows(GridTrack::Auto.repeated(2))
      .set_grid_auto_rows([GridTrack::MinContent])
      .set_children((Container::new()
        .set_grid_column(Line::between(1, 3))
        .set_grid_row(Line::span(2)),))
  });

  let styles = rendered.styles();

  assert!(styles.contains("grid-template-columns:100px 1fr minmax(50px, 2fr) fit-content(200px);"));
  assert!(styles.contains("grid-template-rows:auto auto;"));
  assert!(styles.contains("grid-auto-rows:min-content;"));
  assert!(styles.contains("grid-column:1 / 3;"));
  assert!(styles.contains("grid-row:auto / span 2;"));
}
