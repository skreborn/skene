//! Declarative style dynamics rendered to CSS: conditional variants, transitions, and
//! keyframe animations—the no-script styling contract of server-rendered output.

#![cfg(skene_web_platform = "server")]

use skene_core::{
  Animation,
  Axes,
  BoxStyle,
  Color,
  Dimension,
  Easing,
  GridAreas,
  GridName,
  GridPlacement,
  GridTemplate,
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
};
use skene_simple::web::prelude::*;
use skene_web::{
  Column,
  Condition,
  Container,
  Flexible,
  Grid,
  GridCell,
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
  WebStyle,
  When,
  reactive::Signal,
  render_to_string,
};

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
fn rounded(radius: f32) -> BoxStyle<f32, Keyframe, WebStyle> {
  let mut style: BoxStyle<f32, Keyframe, WebStyle> = BoxStyle::default();

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

/// An overflow per axis renders as the two-value shorthand, horizontal first.
#[test]
fn overflow_renders_per_axis() {
  let rendered = render_to_string(|| {
    Container::new().set_overflow(Axes::new(Overflow::Hidden, Overflow::Scroll))
  });

  let class = root_class(rendered.body());

  assert!(rendered.styles().contains(&format!(
    ".{class}{{display:grid;grid-template-columns:1fr;grid-template-rows:1fr;overflow:hidden scroll;}}"
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

/// An adaptive track renders as an auto-filled repetition, and a flexible minimum as
/// `auto`, since CSS rejects a flexible one.
#[test]
fn adaptive_tracks_and_flexible_minimums_render() {
  let rendered = render_to_string(|| {
    Grid::adaptive(120.).set_rows(vec![GridTrack::Minmax(
      TrackLimit::Flex(1.),
      TrackLimit::Length(40.),
    )])
  });

  let styles = rendered.styles();

  assert!(
    styles.contains("grid-template-columns:repeat(auto-fill, minmax(120px, 1fr));"),
    "{styles}"
  );
  assert!(
    styles.contains("grid-template-rows:minmax(auto, 40px);"),
    "{styles}"
  );
}

/// Named areas render as a quoted row per grid row, and a child's placement by an area's
/// name or a line's as CSS's named lines.
#[test]
fn named_areas_and_placements_render() {
  const HEAD: GridName = GridName::new("head");
  const MAIN: GridName = GridName::new("main");

  let rendered = render_to_string(|| {
    Grid::new()
      .set_areas(GridAreas::new(["head head", ". main"]))
      .set_children((GridCell::new()
        .set_row(HEAD)
        .set_column(Line {
          start: GridPlacement::Named(MAIN, 2),
          end: GridPlacement::NamedSpan(MAIN, 1),
        })
        .set_child(Container::new()),))
  });

  let styles = rendered.styles();

  assert!(
    styles.contains("grid-template-areas:\"head head\" \". main\";"),
    "{styles}"
  );
  assert!(styles.contains("grid-row:head / head;"), "{styles}");
  assert!(
    styles.contains("grid-column:2 main / span main;"),
    "{styles}"
  );
}

/// A template renders each line's names in one bracketed group before the track the
/// line starts, and the end line's after the last track.
#[test]
fn a_templates_line_names_render() {
  const FULL: GridName = GridName::new("full");
  const CONTENT: GridName = GridName::new("content");
  const WIDE: GridName = GridName::new("wide");

  let rendered = render_to_string(|| {
    Grid::new().set_columns(
      GridTemplate::new()
        .add_line(FULL)
        .add_track(GridTrack::Flex(1.))
        .add_line(CONTENT)
        .add_line(WIDE)
        .add_track(GridTrack::Length(600.))
        .add_track(GridTrack::Flex(1.))
        .add_line(FULL),
    )
  });

  let styles = rendered.styles();

  assert!(
    styles.contains("grid-template-columns:[full] 1fr [content wide] 600px 1fr [full];"),
    "{styles}"
  );
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
      let mut start: BoxStyle<f32, Keyframe, WebStyle> = BoxStyle::default();
      let mut end: BoxStyle<f32, Keyframe, WebStyle> = BoxStyle::default();

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

/// The view-transition opt-in emits the cross-document rule for its render and every
/// later one on the thread, and a named box carries its `view-transition-name`; without
/// the opt-in the rule is absent.
#[test]
fn view_transitions_opt_in_reaches_the_stylesheet() {
  let rendered = render_to_string(|| {
    skene_web::view_transitions::enable();

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

  let later = render_to_string(Container::new);

  assert!(
    later.styles().contains("@view-transition"),
    "the opt-in holds for every later render on the thread"
  );

  let elsewhere = std::thread::spawn(|| render_to_string(Container::new).styles().to_owned())
    .join()
    .expect("a render on its own thread");

  assert!(!elsewhere.contains("@view-transition"));
}

/// The index of the `}` closing the block `text` opens at `open`.
fn closing(text: &str, open: usize) -> usize {
  let mut depth = 0;

  for (index, character) in text[open..].char_indices() {
    match character {
      '{' | '(' => depth += 1,
      '}' | ')' => {
        depth -= 1;

        if depth == 0 {
          return open + index;
        }
      }
      _ => {}
    }
  }

  panic!("an unclosed block in {text}");
}

/// The parts of `list` between its commas outside parentheses.
fn top_level_parts(list: &str) -> Vec<&str> {
  let mut parts = Vec::new();
  let mut depth = 0;
  let mut start = 0;

  for (index, character) in list.char_indices() {
    match character {
      '(' => depth += 1,
      ')' => depth -= 1,
      ',' if depth == 0 => {
        parts.push(list[start..index].trim());
        start = index + 1;
      }
      _ => {}
    }
  }

  parts.push(list[start..].trim());
  parts
}

/// Every selector of the rules in `block`, the at-rules they sit in opened and the
/// keyframes skipped.
fn rule_selectors(block: &str, found: &mut Vec<String>) {
  let mut rest = block;

  while let Some(open) = rest.find('{') {
    let prelude = rest[..open].trim();
    let close = closing(rest, open);

    if prelude.starts_with('@') {
      if !prelude.starts_with("@keyframes") {
        rule_selectors(&rest[open + 1..close], found);
      }
    }
    else {
      found.extend(top_level_parts(prelude).into_iter().map(str::to_owned));
    }

    rest = &rest[close + 1..];
  }
}

/// Every selector of the rules in the style layers of `styles`.
fn layered_selectors(styles: &str) -> Vec<String> {
  let mut found = Vec::new();
  let mut rest = styles;

  while let Some(start) = rest.find("@layer skene.style{") {
    let open = start + "@layer skene.style".len();
    let close = closing(rest, open);

    rule_selectors(&rest[open + 1..close], &mut found);
    rest = &rest[close + 1..];
  }

  found
}

/// Where the name starting at `start` in `characters` ends.
fn name_end(characters: &[char], start: usize) -> usize {
  characters[start..]
    .iter()
    .position(|character| !(character.is_alphanumeric() || *character == '-' || *character == '_'))
    .map_or(characters.len(), |length| start + length)
}

/// The specificity of `selector` as ids, classes and types: `:where()` counts nothing,
/// `:is()`, `:not()` and `:has()` count their most specific argument, and a
/// pseudo-element, which styles another box, is set aside.
fn specificity(selector: &str) -> (u32, u32, u32) {
  let characters: Vec<char> = selector.chars().collect();
  let mut total = (0, 0, 0);
  let mut index = 0;

  while index < characters.len() {
    match characters[index] {
      '#' => {
        total.0 += 1;
        index = name_end(&characters, index + 1);
      }
      '.' => {
        total.1 += 1;
        index = name_end(&characters, index + 1);
      }
      '[' => {
        total.1 += 1;
        index += characters[index..]
          .iter()
          .position(|character| *character == ']')
          .expect("a closed attribute selector")
          + 1;
      }
      ':' if characters.get(index + 1) == Some(&':') => index = name_end(&characters, index + 2),
      ':' => {
        let end = name_end(&characters, index + 1);
        let name: String = characters[index + 1..end].iter().collect();

        if characters.get(end) == Some(&'(') {
          let text: String = characters[end..].iter().collect();
          let close = end + text[..=closing(&text, 0)].chars().count() - 1;
          let argument: String = characters[end + 1..close].iter().collect();

          match name.as_str() {
            "where" => {}
            "is" | "not" | "has" => {
              let most = top_level_parts(&argument)
                .into_iter()
                .map(specificity)
                .max()
                .unwrap_or_default();

              total = (total.0 + most.0, total.1 + most.1, total.2 + most.2);
            }
            _ => total.1 += 1,
          }

          index = close + 1;
        }
        else {
          total.1 += 1;
          index = end;
        }
      }
      character if character.is_alphabetic() => {
        total.2 += 1;
        index = name_end(&characters, index);
      }
      _ => index += 1,
    }
  }

  total
}

/// No compiled rule outranks a class: states, anchors and the scheme sit in `:where()`,
/// so the order of the rules alone decides between variants, and every rule sits in the
/// style layer, above the reset and beneath an app's own CSS.
#[test]
fn every_rule_has_a_class_specificity() {
  let rendered = render_to_string(|| {
    let states = [
      State::Hovered,
      State::Focused,
      State::FocusVisible,
      State::Pressed,
      State::Checked,
      State::Indeterminate,
      State::Disabled,
      State::Invalid,
      State::Open,
    ];
    let mut width = skene_core::Value::from(Dimension::Length(10.));

    for (index, state) in (0u8..).zip(states) {
      width = width.when(
        Condition::from(state),
        Dimension::Length(f32::from(index) + 11.),
      );
    }

    let width = width.when(
      Condition::and([
        Condition::from(State::Hovered),
        Condition::from(State::Checked),
      ]),
      Dimension::Length(30.),
    );

    Column::new().set_children((
      Container::new().set_width(width).set_color(
        Color::srgb(0.1, 0.2, 0.3)
          .when(Media::Dark, Color::srgb(0.7, 0.8, 0.9))
          .when(Media::MinWidth(600.), Color::srgb(0.4, 0.5, 0.6)),
      ),
      Text::new("hovered")
        .set_color(Color::srgb(0., 0., 0.).when(State::Hovered, Color::srgb(1., 0., 0.))),
      SimpleButton::new("Save"),
      SimpleCheckbox::new()
        .bind_checked(Signal::new(false))
        .set_label("checked"),
      SimpleRadioGroup::new()
        .bind_chosen(Signal::new(None))
        .set_label("group")
        .set_child(Row::new().set_children((
          SimpleRadio::new("one").set_label("one"),
          SimpleRadio::new("two").set_label("two"),
        ))),
      SimpleSlider::new()
        .set_label("slider")
        .bind_value(Signal::new(0.5)),
      SimpleTextField::new().bind_value(Signal::new(String::new())),
      SimplePicker::new()
        .set_label("picker")
        .set_options([("a", "A"), ("b", "B")])
        .bind_value(Signal::new("a".to_owned())),
      SimpleTooltip::new("described").set_child(SimpleButton::new("Described")),
    ))
  });
  let selectors = layered_selectors(rendered.styles());

  assert_eq!(specificity("label[data-skene-control]"), (0, 1, 1));
  assert_eq!(specificity(".a:hover::selection"), (0, 2, 0));
  assert_eq!(specificity(".a:is(:hover,#b) > *"), (1, 1, 0));
  assert_eq!(specificity(":where(input.b:checked) ~ .a"), (0, 1, 0));
  assert!(selectors.len() > 50, "{}", rendered.styles());

  for selector in selectors {
    assert_eq!(specificity(&selector), (0, 1, 0), "{selector}");
  }
}

/// A derived variant's slot is registered not to inherit, as every base slot is.
#[test]
fn a_derived_variants_slot_is_registered() {
  let rendered = render_to_string(|| {
    Container::new().set_color(
      skene_core::Value::from(Color::srgb(1., 0., 0.))
        .derive_when(Condition::from(State::Hovered), || Color::srgb(0., 0., 1.)),
    )
  });

  assert!(
    rendered
      .styles()
      .contains("@property --skene-color-1{syntax:\"*\";inherits:false}"),
    "{}",
    rendered.styles()
  );
}
