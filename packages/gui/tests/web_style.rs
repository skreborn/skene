//! Declarative style dynamics rendered to CSS: conditional variants, transitions, and
//! keyframe animations—the no-script styling contract of server-rendered output.

#![cfg(skene_leaf = "server")]

use skene_gui::{
  Condition,
  Container,
  Flexible,
  Grid,
  GridCell,
  GuiExt,
  HasChildren,
  Inset,
  Media,
  Pinned,
  Row,
  Stack,
  State,
  Text,
  Theme,
  ThemedIn,
  When,
  core::{
    Animation,
    BoxStyle,
    Color,
    Dimension,
    Easing,
    GridTrack,
    ItemAlignment,
    Keyframe,
    Line,
    Overflow,
    Property,
    StaticLeaf,
    TrackLimit,
    Transition,
    Wrap,
  },
  render_to_string,
};
use skene_simple::gui::prelude::*;

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

/// The class assigned to the first (only) top-level element of a render.
fn root_class(body: &str) -> String {
  classes(body).into_iter().next().expect("a class")
}

/// The declarations of the rule `selector` opens in `styles`.
fn rule<'a>(styles: &'a str, selector: &str) -> &'a str {
  let start = styles.find(&format!("{selector}{{")).expect("the rule") + selector.len() + 1;
  let rest = &styles[start..];

  &rest[..rest.find('}').expect("rule end")]
}

/// The root selector under which the platform's preference still decides.
const DEFERRING: &str = ":where(:root:not([data-skene-scheme=\"light\"]))";
/// The root selector under which the app has chosen the dark scheme outright.
const CHOSEN: &str = ":where(:root[data-skene-scheme=\"dark\"])";

/// A color as the CSS the platform renders it to.
fn css_color(color: Color) -> String {
  let [lightness, chroma, hue, alpha] = color.to_oklch();

  format!("oklch({lightness} {chroma} {hue} / {alpha})")
}

/// The fixed color a token's leaf holds.
fn fixed(leaf: &StaticLeaf<Color>) -> Color {
  match leaf {
    StaticLeaf::Fixed(color) => *color,
    _ => panic!("a fixed color"),
  }
}

/// A plain box style rounding its corners by `radius`, for a keyframe.
fn rounded(radius: f32) -> BoxStyle<f32, Keyframe, GuiExt> {
  let mut style: BoxStyle<f32, Keyframe, GuiExt> = BoxStyle::default();

  style.ext.corner_radius = Some(radius.into());

  style
}

/// A hover variant renders as a `:hover` rule on the same class, adding no specificity.
#[test]
fn state_variant_renders_a_pseudo_class_rule() {
  let rendered = render_to_string(|| {
    Container::new().set_width(
      Dimension::Length(10.).when(Condition::from(State::Hovered), Dimension::Length(20.)),
    )
  });

  let class = root_class(rendered.body());

  assert!(rendered.styles().contains(&format!(
    ".{class}{{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;width:10px;}}"
  )));

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}:where(:hover){{width:20px;}}"))
  );
}

/// A media variant wraps its rule in an `@media` block.
#[test]
fn media_variant_renders_an_at_media_block() {
  let rendered = render_to_string(|| {
    Container::new().set_corner_radius(0f32.when(Condition::from(Media::MaxWidth(600.)), 4.))
  });

  let class = root_class(rendered.body());

  assert!(rendered.styles().contains(&format!(
    "@media (max-width: 600px){{.{class}{{border-radius:4px;}}}}"
  )));
}

/// Combined conditions nest: media wraps, states suffix, all on one rule.
#[test]
fn combined_conditions_nest_media_around_states() {
  let rendered = render_to_string(|| {
    Container::new().set_corner_radius(0f32.when(
      Condition::and([State::Hovered.into(), Media::Dark.into()]),
      8.,
    ))
  });

  let class = root_class(rendered.body());

  assert!(
    rendered.styles().contains(&format!(
      "@media (prefers-color-scheme: dark){{{DEFERRING} .{class}:where(:hover){{border-radius:8px;}}}}"
    )),
    "{}",
    rendered.styles()
  );
}

/// An `or` condition emits one rule per alternative; a nested `and` expands into its
/// own conjunction.
#[test]
fn or_composition_emits_a_rule_per_alternative() {
  let rendered = render_to_string(|| {
    Container::new().set_corner_radius(0f32.when(
      Condition::or([
        Media::MaxWidth(600.).into(),
        Condition::and([Media::Dark.into(), State::Hovered.into()]),
      ]),
      2.,
    ))
  });

  let class = root_class(rendered.body());

  assert!(
    rendered.styles().contains(&format!(
      "@media (max-width: 600px){{.{class}{{border-radius:2px;}}}}"
    )),
    "{}",
    rendered.styles()
  );

  assert!(
    rendered.styles().contains(&format!(
      "@media (prefers-color-scheme: dark){{{DEFERRING} .{class}:where(:hover){{border-radius:2px;}}}}"
    )),
    "{}",
    rendered.styles()
  );
}

/// Overflow renders into the interned class.
#[test]
fn overflow_renders() {
  let rendered = render_to_string(|| Container::new().set_overflow(Overflow::Scroll));

  let class = root_class(rendered.body());

  assert!(rendered.styles().contains(&format!(
    ".{class}{{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;overflow:scroll;}}"
  )));
}

/// Transitions render into the base rule's `transition` declaration.
#[test]
fn transitions_render_into_the_base_rule() {
  let rendered = render_to_string(|| {
    Container::new()
      .set_width(Dimension::Length(10.))
      .set_transition(Transition::new(Property::Color, 0.3))
      .set_transition(
        Transition::new(Property::Width, 1.)
          .set_delay(0.5)
          .set_easing(Easing::EaseInOut),
      )
  });

  assert!(
    rendered
      .styles()
      .contains("transition:background-color 0.3s ease 0s, width 1s ease-in-out 0.5s;"),
    "{}",
    rendered.styles()
  );
}

/// An animation renders a content-addressed `@keyframes` block plus the `animation`
/// declaration referencing it.
#[test]
fn animations_render_keyframes_and_reference_them() {
  let rendered = render_to_string(|| {
    Container::new().set_animation(
      Animation::new(2., [(0., rounded(0.)), (100., rounded(16.))])
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

  assert!(
    styles.contains(&format!(
      "@keyframes {name}{{0%{{border-radius:0px;}}100%{{border-radius:16px;}}}}"
    )),
    "{styles}"
  );

  assert!(
    styles.contains(&format!(
      "animation:{name} 2s ease 0s infinite alternate both;"
    )),
    "{styles}"
  );
}

/// Text runs carry variants too, and identical dynamic styles intern to one class.
#[test]
fn run_variants_intern_and_render() {
  let rendered = render_to_string(|| {
    Row::new().set_children((
      Text::new("a").set_font_weight(400f32.when(Condition::from(State::Hovered), 700.)),
      Text::new("b").set_font_weight(400f32.when(Condition::from(State::Hovered), 700.)),
    ))
  });

  let styles = rendered.styles();
  let hover_rules = styles.matches(":where(:hover){font-weight:700;}").count();

  assert_eq!(hover_rules, 1, "{styles}");
}

/// Identical full styles still collapse to one class; a differing variant splits them.
#[test]
fn variants_participate_in_content_addressing() {
  let rendered = render_to_string(|| {
    Row::new().set_children((
      Container::new().set_width(
        Dimension::Length(10.).when(Condition::from(State::Hovered), Dimension::Length(20.)),
      ),
      Container::new().set_width(
        Dimension::Length(10.).when(Condition::from(State::Hovered), Dimension::Length(20.)),
      ),
      Container::new().set_width(
        Dimension::Length(10.).when(Condition::from(State::Hovered), Dimension::Length(30.)),
      ),
    ))
  });

  let classes = classes(rendered.body());

  // The outer box plus three children; the two identical children share one class.
  assert_eq!(classes.len(), 4);
  assert_eq!(classes[1], classes[2]);
  assert_ne!(classes[1], classes[3]);
}

/// The flex vocabulary compiles end to end: the container's wrap and the wrapped
/// child's flexibility, alignment override, fraction width, and auto margins, all in
/// the child's own class.
#[test]
fn flex_properties_render() {
  let rendered = render_to_string(|| {
    Row::new()
      .set_wrap(Wrap::Forward)
      .set_children((Flexible::new()
        .set_grow_factor(2.)
        .set_shrink_factor(0.)
        .set_initial(120.)
        .set_item_alignment(ItemAlignment::Center)
        .set_child(
          Container::new()
            .set_width(Dimension::Fraction(0.5))
            .set_margin(Inset {
              top: Dimension::Length(4.),
              right: Dimension::Auto,
              bottom: Dimension::Length(4.),
              left: Dimension::Auto,
            }),
        ),))
  });

  let class = root_class(rendered.body());
  let styles = rendered.styles();

  assert!(styles.contains(&format!(
    ".{class}{{display:flex;flex-direction:row;flex-wrap:wrap;}}"
  )));
  assert!(
    styles.contains("margin:4px auto 4px auto;width:50%;"),
    "{styles}"
  );
  assert!(styles.contains("flex:2 0 120px;"), "{styles}");
  assert!(styles.contains("align-self:center;"), "{styles}");
}

/// A pinned stack child renders out of flow against the stack's edges.
#[test]
fn pinned_child_renders_out_of_flow() {
  let rendered = render_to_string(|| {
    Stack::new().set_children((Pinned::new()
      .set_edges(Inset {
        top: Dimension::Length(0.),
        right: Dimension::Length(8.),
        bottom: Dimension::Auto,
        left: Dimension::Fraction(0.1),
      })
      .set_child(Container::new()),))
  });

  let styles = rendered.styles();

  assert!(
    styles.contains("inset:0px 8px auto 10%;position:absolute;"),
    "{styles}"
  );
}

/// A stack's children share its one grid cell, so they overlap.
#[test]
fn stack_children_share_one_grid_cell() {
  let rendered =
    render_to_string(|| Stack::new().set_children((Text::new("under"), Text::new("over"))));
  let class = root_class(rendered.body());

  assert!(
    rendered
      .styles()
      .contains(&format!(".{class}>*{{grid-area:1 / 1;}}")),
    "{}",
    rendered.styles()
  );
}

/// Grid templates render taffy's full track vocabulary, and placements pin children.
#[test]
fn grid_tracks_and_placements_render() {
  let rendered = render_to_string(|| {
    Grid::new()
      .set_columns(vec![
        GridTrack::Length(100.),
        GridTrack::Flex(1.),
        GridTrack::Minmax(TrackLimit::Length(50.), TrackLimit::Flex(2.)),
        GridTrack::FitContent(200.),
      ])
      .set_rows(GridTrack::Auto.repeated(2).collect::<Vec<_>>())
      .set_implicit_rows(vec![GridTrack::MinContent])
      .set_children((GridCell::new()
        .set_column(Line::between(1, 3))
        .set_row(Line::span(2))
        .set_child(Container::new()),))
  });

  let styles = rendered.styles();

  assert!(styles.contains("grid-template-columns:100px 1fr minmax(50px, 2fr) fit-content(200px);"));
  assert!(styles.contains("grid-template-rows:auto auto;"));
  assert!(styles.contains("grid-auto-rows:min-content;"));
  assert!(styles.contains("grid-column:1 / 3;"), "{styles}");
  assert!(styles.contains("grid-row:auto / span 2;"), "{styles}");
}

/// A theme's scheme is real CSS: a consumer emits the token's name alone, and the
/// theme's scope carries the light value on its class and the dark one inside a
/// `prefers-color-scheme` block, so a server-rendered page answers the reader's
/// preference with no bundle loaded at all.
#[test]
fn a_theme_following_the_platform_renders_a_color_scheme_query() {
  let rendered = render_to_string(|| {
    Theme::new(SimplePalette::default())
      .set_child_builder(|| Container::new().set_color(SimplePalette::current().card))
  });
  let classes = classes(rendered.body());
  let (scope, consumer) = (&classes[0], &classes[1]);
  let card = SimplePalette::default().card;
  let light = css_color(fixed(card.definition().base()));
  let dark = css_color(fixed(&card.definition().variants()[0].1));
  let styles = rendered.styles();

  assert!(
    rule(styles, &format!(".{consumer}")).contains("background-color:var(--simple-card);"),
    "the consumer names the token: {styles}"
  );
  assert!(
    rule(styles, &format!(".{scope}")).contains(&format!("--simple-card:{light};")),
    "the light value is the scope's own: {styles}"
  );
  assert!(
    rule(
      styles,
      &format!("@media (prefers-color-scheme: dark){{{DEFERRING} .{scope}")
    )
    .contains(&format!("--simple-card:{dark};")),
    "{styles}"
  );
  assert!(
    rule(styles, &format!("{CHOSEN} .{scope}")).contains(&format!("--simple-card:{dark};")),
    "and the same value under a scheme the app chooses at runtime: {styles}"
  );
}

/// A scheme variant renders twice: the platform's preference, deferring to a choice the
/// app may have written on the root, and that choice. Both root selectors sit in
/// `:where()`, so neither adds specificity and the cascade decides what it always did.
#[test]
fn a_scheme_variant_renders_the_preference_and_the_choice() {
  let rendered = render_to_string(|| {
    Container::new().set_corner_radius(0f32.when(Condition::from(Media::Dark), 8.))
  });
  let class = root_class(rendered.body());

  assert!(rendered.styles().contains(&format!(
    "@media (prefers-color-scheme: dark){{{DEFERRING} .{class}{{border-radius:8px;}}}}"
  )));

  assert!(
    rendered
      .styles()
      .contains(&format!("{CHOSEN} .{class}{{border-radius:8px;}}"))
  );
}

/// A scheme condition combined with a state keeps the state on both rules, so a chosen
/// scheme and a hover compose exactly as the preference and a hover do.
#[test]
fn a_scheme_variant_keeps_its_states_on_both_rules() {
  let rendered = render_to_string(|| {
    Container::new().set_corner_radius(0f32.when(
      Condition::and([State::Hovered.into(), Media::Dark.into()]),
      4.,
    ))
  });
  let class = root_class(rendered.body());

  assert!(rendered.styles().contains(&format!(
    "@media (prefers-color-scheme: dark){{{DEFERRING} .{class}:where(:hover){{border-radius:4px;}}}}"
  )));

  assert!(rendered.styles().contains(&format!(
    "{CHOSEN} .{class}:where(:hover){{border-radius:4px;}}"
  )));
}

/// A token naming no condition renders one value and no query: nothing is left for the
/// platform to answer, and a palette says so per property.
#[test]
fn an_unconditional_token_renders_no_color_scheme_query() {
  let flat = Color::srgb(0.2, 0.4, 0.8);
  let rendered = render_to_string(move || {
    Theme::new(SimplePalette::default().set_card(flat))
      .set_child_builder(|| Container::new().set_color(SimplePalette::current().card))
  });
  let scope = root_class(rendered.body());
  let styles = rendered.styles();

  assert!(
    rule(styles, &format!(".{scope}")).contains(&format!("--simple-card:{};", css_color(flat))),
    "{styles}"
  );
  assert!(
    !rule(
      styles,
      &format!("@media (prefers-color-scheme: dark){{{DEFERRING} .{scope}")
    )
    .contains("--simple-card:"),
    "the dark block leaves the flat token alone: {styles}"
  );
}

/// The base rule of `class` in `styles`: the declarations between its braces.
fn base_rule<'a>(styles: &'a str, class: &str) -> &'a str {
  let start = styles
    .find(&format!(".{class}{{"))
    .expect("the class's base rule")
    + class.len()
    + 2;
  let rest = &styles[start..];

  &rest[..rest.find('}').expect("rule end")]
}

/// A transition respecting reduced motion lands under the no-preference media query;
/// one that opted out stays in the base rule, and the query then carries both.
#[test]
fn reduced_motion_gates_the_motion_that_respects_it() {
  let rendered = render_to_string(|| {
    Container::new()
      .set_transition(Transition::new(Property::Color, 0.3))
      .set_transition(Transition::new(Property::Width, 1.).set_respect_reduced_motion(false))
  });
  let class = root_class(rendered.body());
  let styles = rendered.styles();

  assert!(
    base_rule(styles, &class).contains("transition:width 1s ease 0s;"),
    "{styles}"
  );
  assert!(
    styles.contains(&format!(
      "@media (prefers-reduced-motion: no-preference){{.{class}{{transition:background-color \
       0.3s ease 0s, width 1s ease 0s;}}}}"
    )),
    "{styles}"
  );

  let respecting =
    render_to_string(|| Container::new().set_transition(Transition::new(Property::Color, 0.3)));
  let class = root_class(respecting.body());

  assert!(
    !base_rule(respecting.styles(), &class).contains("transition"),
    "{}",
    respecting.styles()
  );
  assert!(
    respecting.styles().contains(&format!(
      "@media (prefers-reduced-motion: no-preference){{.{class}{{transition:background-color \
       0.3s ease 0s;}}}}"
    )),
    "{}",
    respecting.styles()
  );
}

/// A keyframe holding a token renders its `var()`, so a palette or scheme change moves
/// the animation's frames.
#[test]
fn a_token_keyframe_renders_its_variable() {
  let rendered = render_to_string(|| {
    Theme::new(SimplePalette::default()).set_child_builder(|| {
      let mut start: BoxStyle<f32, Keyframe, GuiExt> = BoxStyle::default();
      let mut end: BoxStyle<f32, Keyframe, GuiExt> = BoxStyle::default();

      start.color = Some(SimplePalette::CARD.into());
      end.color = Some(Color::srgb(1., 0., 0.).into());

      Container::new().set_animation(Animation::new(2., [(0., start), (100., end)]))
    })
  });
  let styles = rendered.styles();

  assert!(
    styles.contains("0%{background-color:var(--simple-card);}"),
    "{styles}"
  );
  assert!(
    styles.contains("@property --simple-card{syntax:\"<color>\""),
    "{styles}"
  );
}

/// The view-transition opt-in emits the cross-document rule, and a named box carries
/// its `view-transition-name`; without the opt-in the rule is absent.
#[test]
fn view_transitions_opt_in_reaches_the_stylesheet() {
  let rendered = render_to_string(|| {
    skene_gui::view_transitions::enable();

    Container::new().set_transition_name("hero")
  });
  let class = root_class(rendered.body());

  assert!(
    rendered
      .styles()
      .contains("@view-transition{navigation:auto}"),
    "{}",
    rendered.styles()
  );
  assert!(
    base_rule(rendered.styles(), &class).contains("view-transition-name:hero;"),
    "{}",
    rendered.styles()
  );

  let plain = render_to_string(Container::new);

  assert!(!plain.styles().contains("@view-transition"));
}
