//! Typed properties to CSS: the web realization of the driver vocabulary.
//!
//! One property maps to the same declaration text whether it is folded into a class or
//! applied alone as an inline override, so a value renders identically on both paths and
//! on both leaves. A node's full static style renders to a **rule template**: the
//! complete CSS for one class—base declarations, conditional variants as
//! pseudo-classes and `@media` blocks, transitions, and content-addressed `@keyframes`—
//! with [`CLASS_PLACEHOLDER`] standing in for the class name until the interner assigns
//! one.

use std::{
  fmt::Write as _,
  hash::{Hash, Hasher},
};

use rustc_hash::FxHasher;
use skene_core::{
  Animation,
  Axis,
  BoxProp,
  BoxProps,
  Condition,
  Easing,
  InputProps,
  Layout,
  RunDisplay,
  RunProp,
  RunProps,
  StyleProperty,
  Transition,
};

use crate::Declarations;

/// The class-name placeholder inside a rule template, replaced once the interner assigns
/// the content-addressed name.
pub(crate) const CLASS_PLACEHOLDER: char = '\u{1}';

/// The static display declarations for a [`Layout`].
#[inline(always)]
fn layout_declarations(layout: Layout) -> Declarations {
  match layout {
    Layout::Block => Declarations::new().set("display", "flow-root"),
    Layout::Flex { direction } => Declarations::new().set("display", "flex").set(
      "flex-direction",
      match direction {
        Axis::Horizontal => "row",
        Axis::Vertical => "column",
      },
    ),
    Layout::Grid { columns } => Declarations::new()
      .set("display", "grid")
      .set("grid-template-columns", format!("repeat({columns}, 1fr)")),
    Layout::Stack => Declarations::new().set("display", "grid"),
  }
}

/// Folds one box property into `declarations`.
fn apply_box_prop(declarations: Declarations, prop: &BoxProp) -> Declarations {
  match prop {
    BoxProp::Width(value) => declarations.set("width", format!("{value}px")),
    BoxProp::Height(value) => declarations.set("height", format!("{value}px")),
    BoxProp::MinWidth(value) => declarations.set("min-width", format!("{value}px")),
    BoxProp::MinHeight(value) => declarations.set("min-height", format!("{value}px")),
    BoxProp::MaxWidth(value) => declarations.set("max-width", format!("{value}px")),
    BoxProp::MaxHeight(value) => declarations.set("max-height", format!("{value}px")),
    BoxProp::Padding(value) => declarations.set(
      "padding",
      format!(
        "{}px {}px {}px {}px",
        value.top, value.right, value.bottom, value.left,
      ),
    ),
    BoxProp::Color(value) => {
      declarations.set("background-color", Declarations::render_color(*value))
    }
    BoxProp::Border(value) => declarations
      .set("border-width", format!("{}px", value.width))
      .set("border-style", "solid")
      .set("border-color", Declarations::render_color(value.color)),
    BoxProp::BorderRadius(value) => declarations.set("border-radius", format!("{value}px")),
    BoxProp::Cursor(value) => declarations.set("cursor", Declarations::render_cursor(*value)),
    BoxProp::Spacing(value) => declarations.set("gap", format!("{value}px")),
    BoxProp::MainAxisAlignment(value) => {
      declarations.set("justify-content", Declarations::render_main_axis(*value))
    }
    BoxProp::CrossAxisAlignment(value) => {
      declarations.set("align-items", Declarations::render_cross_axis(*value))
    }
  }
}

/// Folds a box style's fixed properties into `declarations`.
fn fold_box_style(mut declarations: Declarations, style: &skene_core::BoxStyle) -> Declarations {
  if let Some(value) = style.width {
    declarations = apply_box_prop(declarations, &BoxProp::Width(value));
  }

  if let Some(value) = style.height {
    declarations = apply_box_prop(declarations, &BoxProp::Height(value));
  }

  if let Some(value) = style.min_width {
    declarations = apply_box_prop(declarations, &BoxProp::MinWidth(value));
  }

  if let Some(value) = style.min_height {
    declarations = apply_box_prop(declarations, &BoxProp::MinHeight(value));
  }

  if let Some(value) = style.max_width {
    declarations = apply_box_prop(declarations, &BoxProp::MaxWidth(value));
  }

  if let Some(value) = style.max_height {
    declarations = apply_box_prop(declarations, &BoxProp::MaxHeight(value));
  }

  if let Some(value) = style.padding {
    declarations = apply_box_prop(declarations, &BoxProp::Padding(value));
  }

  if let Some(value) = style.color {
    declarations = apply_box_prop(declarations, &BoxProp::Color(value));
  }

  if let Some(value) = style.border {
    declarations = apply_box_prop(declarations, &BoxProp::Border(value));
  }

  if let Some(value) = style.border_radius {
    declarations = apply_box_prop(declarations, &BoxProp::BorderRadius(value));
  }

  if let Some(value) = style.cursor {
    declarations = apply_box_prop(declarations, &BoxProp::Cursor(value));
  }

  if let Some(value) = style.spacing {
    declarations = apply_box_prop(declarations, &BoxProp::Spacing(value));
  }

  if let Some(value) = style.main_axis_alignment {
    declarations = apply_box_prop(declarations, &BoxProp::MainAxisAlignment(value));
  }

  if let Some(value) = style.cross_axis_alignment {
    declarations = apply_box_prop(declarations, &BoxProp::CrossAxisAlignment(value));
  }

  declarations
}

/// The complete rule template for a box: its layout and fixed properties, plus variants,
/// transitions and animations.
pub(crate) fn box_template(props: &BoxProps) -> String {
  template(
    fold_box_style(layout_declarations(props.layout), &props.style),
    &props.variants,
    &props.transitions,
    &props.animations,
    |style| fold_box_style(Declarations::new(), style),
  )
}

/// The complete rule template for an input, or `None` when it is entirely unstyled.
pub(crate) fn input_template(props: &InputProps) -> Option<String> {
  let base = fold_box_style(Declarations::new(), &props.style);

  if base.is_empty()
    && props
      .variants
      .iter()
      .all(|(_, style)| *style == skene_core::BoxStyle::default())
    && props.transitions.is_empty()
    && props.animations.is_empty()
  {
    return None;
  }

  Some(template(
    base,
    &props.variants,
    &props.transitions,
    &props.animations,
    |style| fold_box_style(Declarations::new(), style),
  ))
}

/// The declarations for one box property alone, for an inline override.
#[inline(always)]
pub(crate) fn box_prop_declarations(prop: &BoxProp) -> Declarations {
  apply_box_prop(Declarations::new(), prop)
}

/// Folds one text-run property into `declarations`.
fn apply_run_prop(declarations: Declarations, prop: &RunProp) -> Declarations {
  match prop {
    RunProp::Color(value) => declarations.set("color", Declarations::render_color(*value)),
    RunProp::FontSize(value) => declarations.set("font-size", format!("{value}px")),
    RunProp::FontWeight(value) => declarations.set("font-weight", value.to_string()),
  }
}

/// Folds a run style's fixed properties into `declarations`.
fn fold_run_style(mut declarations: Declarations, style: &skene_core::RunStyle) -> Declarations {
  if let Some(value) = style.color {
    declarations = apply_run_prop(declarations, &RunProp::Color(value));
  }

  if let Some(value) = style.font_size {
    declarations = apply_run_prop(declarations, &RunProp::FontSize(value));
  }

  if let Some(value) = style.font_weight {
    declarations = apply_run_prop(declarations, &RunProp::FontWeight(value));
  }

  declarations
}

/// The complete rule template for a text run, or `None` when it is entirely unstyled (an
/// unstyled inline run carries no class).
pub(crate) fn run_template(props: &RunProps) -> Option<String> {
  let display = match props.display {
    RunDisplay::Block => Declarations::new().set("display", "block"),
    RunDisplay::Inline => Declarations::new(),
    RunDisplay::InlineBox => Declarations::new().set("display", "inline-block"),
  };

  let base = fold_run_style(display, &props.style);

  if base.is_empty()
    && props
      .variants
      .iter()
      .all(|(_, style)| *style == skene_core::RunStyle::default())
    && props.transitions.is_empty()
    && props.animations.is_empty()
  {
    return None;
  }

  Some(template(
    base,
    &props.variants,
    &props.transitions,
    &props.animations,
    |style| fold_run_style(Declarations::new(), style),
  ))
}

/// The declarations for one text-run property alone, for an inline override.
#[inline(always)]
pub(crate) fn run_prop_declarations(prop: &RunProp) -> Declarations {
  apply_run_prop(Declarations::new(), prop)
}

/// The rule template for a raw declaration set—the escape-hatch styling path.
pub(crate) fn declarations_template(declarations: &Declarations) -> String {
  let mut out = String::new();

  push_rule(&mut out, &[], declarations);

  out
}

/// Appends one rule: `.<placeholder><states…>{declarations}`.
fn push_rule(out: &mut String, states: &[&'static str], declarations: &Declarations) {
  out.push('.');
  out.push(CLASS_PLACEHOLDER);

  for state in states {
    out.push_str(state);
  }

  out.push('{');
  out.push_str(&declarations.render());
  out.push('}');
}

/// Assembles the rule template: `@keyframes` blocks, the base rule (with transitions and
/// animations folded in), then each variant rule in declaration order.
fn template<S>(
  mut base: Declarations,
  variants: &[(Vec<Condition>, S)],
  transitions: &[Transition],
  animations: &[Animation<S>],
  style_declarations: impl Fn(&S) -> Declarations,
) -> String {
  let mut out = String::new();

  if !transitions.is_empty() {
    base = base.set("transition", render_transitions(transitions));
  }

  if !animations.is_empty() {
    let mut names = Vec::new();

    for animation in animations {
      names.push(render_keyframes(&mut out, animation, &style_declarations));
    }

    base = base.set("animation", render_animation_values(animations, &names));
  }

  push_rule(&mut out, &[], &base);

  for (conditions, style) in variants {
    let declarations = style_declarations(style);

    if declarations.is_empty() {
      continue;
    }

    let mut media: Vec<String> = conditions.iter().filter_map(media_feature).collect();
    let mut states: Vec<&'static str> = conditions.iter().filter_map(state_selector).collect();

    // Canonical order, so the same condition set always renders the same template.
    media.sort_unstable();
    states.sort_unstable();

    if media.is_empty() {
      push_rule(&mut out, &states, &declarations);
    }
    else {
      out.push_str("@media ");
      out.push_str(&media.join(" and "));
      out.push('{');
      push_rule(&mut out, &states, &declarations);
      out.push('}');
    }
  }

  out
}

/// The media feature a condition renders to, when it is an environment condition.
fn media_feature(condition: &Condition) -> Option<String> {
  match condition {
    Condition::MinWidth(value) => Some(format!("(min-width: {value}px)")),
    Condition::MaxWidth(value) => Some(format!("(max-width: {value}px)")),
    Condition::MinHeight(value) => Some(format!("(min-height: {value}px)")),
    Condition::MaxHeight(value) => Some(format!("(max-height: {value}px)")),
    Condition::DarkMode => Some("(prefers-color-scheme: dark)".to_owned()),
    Condition::LightMode => Some("(prefers-color-scheme: light)".to_owned()),
    Condition::ReducedMotion => Some("(prefers-reduced-motion: reduce)".to_owned()),
    _ => None,
  }
}

/// The pseudo-class a condition renders to, when it is an interaction state.
fn state_selector(condition: &Condition) -> Option<&'static str> {
  match condition {
    Condition::Hovered => Some(":hover"),
    Condition::Focused => Some(":focus"),
    Condition::FocusVisible => Some(":focus-visible"),
    Condition::Pressed => Some(":active"),
    Condition::Disabled => Some(":disabled"),
    _ => None,
  }
}

/// The CSS property name a [`StyleProperty`] transitions.
fn style_property_name(property: StyleProperty) -> &'static str {
  match property {
    StyleProperty::All => "all",
    StyleProperty::Width => "width",
    StyleProperty::Height => "height",
    StyleProperty::MinWidth => "min-width",
    StyleProperty::MinHeight => "min-height",
    StyleProperty::MaxWidth => "max-width",
    StyleProperty::MaxHeight => "max-height",
    StyleProperty::Padding => "padding",
    StyleProperty::Background => "background-color",
    StyleProperty::BorderColor => "border-color",
    StyleProperty::BorderWidth => "border-width",
    StyleProperty::BorderRadius => "border-radius",
    StyleProperty::Spacing => "gap",
    StyleProperty::TextColor => "color",
    StyleProperty::FontSize => "font-size",
    StyleProperty::FontWeight => "font-weight",
  }
}

/// A timing curve's CSS text.
fn render_easing(easing: Easing) -> String {
  match easing {
    Easing::Linear => "linear".to_owned(),
    Easing::Ease => "ease".to_owned(),
    Easing::EaseIn => "ease-in".to_owned(),
    Easing::EaseOut => "ease-out".to_owned(),
    Easing::EaseInOut => "ease-in-out".to_owned(),
    Easing::CubicBezier(x1, y1, x2, y2) => format!("cubic-bezier({x1}, {y1}, {x2}, {y2})"),
  }
}

/// The `transition` declaration value for a set of transitions.
fn render_transitions(transitions: &[Transition]) -> String {
  let mut out = String::new();

  for (index, transition) in transitions.iter().enumerate() {
    if index > 0 {
      out.push_str(", ");
    }

    out.push_str(style_property_name(transition.property));

    let _ = write!(
      out,
      " {}s {} {}s",
      transition.duration,
      render_easing(transition.easing),
      transition.delay,
    );
  }

  out
}

/// Appends one animation's `@keyframes` block, named by the hash of its body so the name
/// is identical on every leaf, and returns that name.
fn render_keyframes<S>(
  out: &mut String,
  animation: &Animation<S>,
  style_declarations: &impl Fn(&S) -> Declarations,
) -> String {
  let mut body = String::new();

  for (percent, style) in &animation.keyframes {
    let _ = write!(body, "{percent}%");

    body.push('{');
    body.push_str(&style_declarations(style).render());
    body.push('}');
  }

  let name = {
    let mut hasher = FxHasher::default();

    body.hash(&mut hasher);

    format!("skene-k-{:x}", hasher.finish())
  };

  out.push_str("@keyframes ");
  out.push_str(&name);
  out.push('{');
  out.push_str(&body);
  out.push('}');

  name
}

/// The `animation` declaration value for a set of animations and their keyframes names.
fn render_animation_values<S>(animations: &[Animation<S>], names: &[String]) -> String {
  let mut out = String::new();

  for (index, animation) in animations.iter().enumerate() {
    if index > 0 {
      out.push_str(", ");
    }

    let _ = write!(
      out,
      "{} {}s {} {}s {} {} both",
      names[index],
      animation.duration,
      render_easing(animation.easing),
      animation.delay,
      match animation.iterations {
        Some(count) => count.to_string(),
        None => "infinite".to_owned(),
      },
      if animation.alternate {
        "alternate"
      }
      else {
        "normal"
      },
    );
  }

  out
}
