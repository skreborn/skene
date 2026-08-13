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
  BoxProp,
  BoxProps,
  Composable,
  Condition,
  ControlProps,
  Easing,
  FlexDirection,
  GridFlow,
  Layout,
  Overflow,
  OverlayProps,
  Placement,
  RunDisplay,
  RunProp,
  RunProps,
  StyleProperty,
  Transition,
  Wrap,
};

use crate::Declarations;

/// The class-name placeholder inside a rule template, replaced once the interner assigns
/// the content-addressed name.
pub(crate) const CLASS_PLACEHOLDER: char = '\u{1}';

/// The static display declarations for a [`Layout`].
#[inline(always)]
fn layout_declarations(layout: Layout) -> Declarations {
  match layout {
    Layout::Flow => Declarations::new().set("display", "flow-root"),
    Layout::Flex { direction } => Declarations::new().set("display", "flex").set(
      "flex-direction",
      match direction {
        FlexDirection::Row => "row",
        FlexDirection::Column => "column",
        FlexDirection::RowReverse => "row-reverse",
        FlexDirection::ColumnReverse => "column-reverse",
      },
    ),
    Layout::Grid => Declarations::new().set("display", "grid"),
    // One track each way: `1fr` fills a sized box and hugs an unsized one, and the
    // zero minimum lets a scrolling or shrinking child fit.
    Layout::Single => Declarations::new()
      .set("display", "grid")
      .set("grid-template-columns", "minmax(0, 1fr)")
      .set("grid-template-rows", "minmax(0, 1fr)"),
    // The stack is the containing block its pinned children position against.
    Layout::Stack => Declarations::new()
      .set("display", "grid")
      .set("position", "relative"),
  }
}

/// Folds one box property into `declarations`.
#[expect(clippy::too_many_lines)]
fn apply_box_prop(declarations: Declarations, prop: &BoxProp) -> Declarations {
  match prop {
    BoxProp::Width(value) => declarations.set("width", Declarations::render_dimension(*value)),
    BoxProp::Height(value) => declarations.set("height", Declarations::render_dimension(*value)),
    BoxProp::MinWidth(value) => {
      declarations.set("min-width", Declarations::render_dimension(*value))
    }
    BoxProp::MinHeight(value) => {
      declarations.set("min-height", Declarations::render_dimension(*value))
    }
    BoxProp::MaxWidth(value) => {
      declarations.set("max-width", Declarations::render_dimension(*value))
    }
    BoxProp::MaxHeight(value) => {
      declarations.set("max-height", Declarations::render_dimension(*value))
    }
    BoxProp::AspectRatio(value) => declarations.set("aspect-ratio", value.to_string()),
    BoxProp::Margin(value) => {
      declarations.set("margin", Declarations::render_dimension_inset(*value))
    }
    BoxProp::Padding(value) => declarations.set(
      "padding",
      format!(
        "{}px {}px {}px {}px",
        value.top, value.right, value.bottom, value.left,
      ),
    ),
    BoxProp::Offset(value) => declarations
      .set("position", "relative")
      .set("inset", Declarations::render_dimension_inset(*value)),
    BoxProp::Pinned(value) => declarations
      .set("position", "absolute")
      .set("inset", Declarations::render_dimension_inset(*value)),
    BoxProp::Color(value) => {
      declarations.set("background-color", Declarations::render_color(*value))
    }
    BoxProp::Border(value) => declarations
      .set("border-width", format!("{}px", value.width))
      .set("border-style", "solid")
      .set("border-color", Declarations::render_color(value.color)),
    BoxProp::BorderRadius(value) => declarations.set("border-radius", format!("{value}px")),
    BoxProp::Cursor(value) => declarations.set("cursor", Declarations::render_cursor(*value)),
    BoxProp::Spacing(value) => declarations
      .set("column-gap", format!("{}px", value.column))
      .set("row-gap", format!("{}px", value.row)),
    BoxProp::Wrap(value) => declarations.set(
      "flex-wrap",
      match value {
        Wrap::None => "nowrap",
        Wrap::Forward => "wrap",
        Wrap::Reverse => "wrap-reverse",
      },
    ),
    BoxProp::Flex(value) => declarations
      .set("flex-basis", Declarations::render_dimension(value.initial))
      .set("flex-grow", value.grow_factor.to_string())
      .set("flex-shrink", value.shrink_factor.to_string()),
    BoxProp::Arrangement(value) => {
      declarations.set("justify-content", Declarations::render_arrangement(*value))
    }
    BoxProp::ItemAlignment(value) => {
      declarations.set("align-items", Declarations::render_item_alignment(*value))
    }
    BoxProp::LineArrangement(value) => {
      declarations.set("align-content", Declarations::render_arrangement(*value))
    }
    BoxProp::ItemHorizontalAlignment(value) => {
      declarations.set("justify-items", Declarations::render_item_alignment(*value))
    }
    BoxProp::SelfAlignment(value) => {
      declarations.set("align-self", Declarations::render_item_alignment(*value))
    }
    BoxProp::SelfHorizontalAlignment(value) => {
      declarations.set("justify-self", Declarations::render_item_alignment(*value))
    }
    BoxProp::GridColumns(value) => {
      declarations.set("grid-template-columns", Declarations::render_tracks(value))
    }
    BoxProp::GridRows(value) => {
      declarations.set("grid-template-rows", Declarations::render_tracks(value))
    }
    BoxProp::GridImplicitColumns(value) => {
      declarations.set("grid-auto-columns", Declarations::render_tracks(value))
    }
    BoxProp::GridImplicitRows(value) => {
      declarations.set("grid-auto-rows", Declarations::render_tracks(value))
    }
    BoxProp::GridFlow(value) => declarations.set(
      "grid-auto-flow",
      match value {
        GridFlow::Rows => "row",
        GridFlow::Columns => "column",
        GridFlow::DenseRows => "row dense",
        GridFlow::DenseColumns => "column dense",
      },
    ),
    BoxProp::GridRowPlacement(value) => {
      declarations.set("grid-row", Declarations::render_grid_line(*value))
    }
    BoxProp::GridColumnPlacement(value) => {
      declarations.set("grid-column", Declarations::render_grid_line(*value))
    }
    BoxProp::Overflow(value) => declarations.set(
      "overflow",
      match value {
        Overflow::Visible => "visible",
        Overflow::Hidden => "hidden",
        Overflow::Scroll => "scroll",
        Overflow::Auto => "auto",
      },
    ),
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

  if let Some(value) = style.aspect_ratio {
    declarations = apply_box_prop(declarations, &BoxProp::AspectRatio(value));
  }

  if let Some(value) = style.margin {
    declarations = apply_box_prop(declarations, &BoxProp::Margin(value));
  }

  if let Some(value) = style.padding {
    declarations = apply_box_prop(declarations, &BoxProp::Padding(value));
  }

  if let Some(value) = style.offset {
    declarations = apply_box_prop(declarations, &BoxProp::Offset(value));
  }

  if let Some(value) = style.pinned {
    declarations = apply_box_prop(declarations, &BoxProp::Pinned(value));
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

  if let Some(value) = style.wrap {
    declarations = apply_box_prop(declarations, &BoxProp::Wrap(value));
  }

  if let Some(value) = style.flex {
    declarations = apply_box_prop(declarations, &BoxProp::Flex(value));
  }

  if let Some(value) = style.arrangement {
    declarations = apply_box_prop(declarations, &BoxProp::Arrangement(value));
  }

  if let Some(value) = style.item_alignment {
    declarations = apply_box_prop(declarations, &BoxProp::ItemAlignment(value));
  }

  if let Some(value) = style.line_arrangement {
    declarations = apply_box_prop(declarations, &BoxProp::LineArrangement(value));
  }

  if let Some(value) = style.item_horizontal_alignment {
    declarations = apply_box_prop(declarations, &BoxProp::ItemHorizontalAlignment(value));
  }

  if let Some(value) = style.self_alignment {
    declarations = apply_box_prop(declarations, &BoxProp::SelfAlignment(value));
  }

  if let Some(value) = style.self_horizontal_alignment {
    declarations = apply_box_prop(declarations, &BoxProp::SelfHorizontalAlignment(value));
  }

  if let Some(value) = &style.grid_columns {
    declarations = apply_box_prop(declarations, &BoxProp::GridColumns(value.clone()));
  }

  if let Some(value) = &style.grid_rows {
    declarations = apply_box_prop(declarations, &BoxProp::GridRows(value.clone()));
  }

  if let Some(value) = &style.grid_implicit_columns {
    declarations = apply_box_prop(declarations, &BoxProp::GridImplicitColumns(value.clone()));
  }

  if let Some(value) = &style.grid_implicit_rows {
    declarations = apply_box_prop(declarations, &BoxProp::GridImplicitRows(value.clone()));
  }

  if let Some(value) = style.grid_flow {
    declarations = apply_box_prop(declarations, &BoxProp::GridFlow(value));
  }

  if let Some(value) = style.grid_row_placement {
    declarations = apply_box_prop(declarations, &BoxProp::GridRowPlacement(value));
  }

  if let Some(value) = style.grid_column_placement {
    declarations = apply_box_prop(declarations, &BoxProp::GridColumnPlacement(value));
  }

  if let Some(value) = style.overflow {
    declarations = apply_box_prop(declarations, &BoxProp::Overflow(value));
  }

  declarations
}

/// The complete rule template for a box: its layout and fixed properties, plus variants,
/// transitions and animations.
pub(crate) fn box_template(props: &BoxProps) -> String {
  template(
    fold_box_style(layout_declarations(props.layout), &props.style),
    "",
    &props.variants,
    &props.transitions,
    &props.animations,
    |style| fold_box_style(Declarations::new(), style),
  )
}

/// The reset an overlay's element carries: the user-agent dialog/popover chrome
/// stripped, the top-layer fixed positioning and auto margins (which center it) left
/// intact for the placement margins to override.
const OVERLAY_RESET: &str = "border:none;padding:0;background:none;color:inherit;overflow:visible;";

/// The margin declaration placing an overlay against the viewport: `auto` centers an
/// axis, `0` pins an edge.
fn placement_margin(placement: Placement) -> &'static str {
  match placement {
    Placement::Center => "auto",
    Placement::Top => "0 auto auto",
    Placement::TopStart => "0 auto auto 0",
    Placement::TopEnd => "0 0 auto auto",
    Placement::Bottom => "auto auto 0",
    Placement::BottomStart => "auto auto 0 0",
    Placement::BottomEnd => "auto 0 0 auto",
  }
}

/// The complete rule template for an overlay: its reset, its placement margins, its
/// content style and variants, and—for a modal with a backdrop—the `::backdrop` rule.
pub(crate) fn overlay_template(props: &OverlayProps) -> String {
  let base = fold_box_style(
    Declarations::new().set("margin", placement_margin(props.placement)),
    &props.style,
  );
  let mut out = template(base, OVERLAY_RESET, &props.variants, &[], &[], |style| {
    fold_box_style(Declarations::new(), style)
  });

  if let Some(color) = props.backdrop {
    push_rule(
      &mut out,
      &[],
      "::backdrop",
      &Declarations::new().set("background-color", Declarations::render_color(color)),
    );
  }

  out
}

/// The reset every form control carries in its own class: no native chrome, no
/// browser-imposed spacing or typography—an unstyled element the author styles from
/// zero. Focus outlines are deliberately left intact.
pub(crate) const CONTROL_RESET: &str =
  "appearance:none;background:none;border:none;margin:0;padding:0;font:inherit;color:inherit;";

/// The toggle controls' reset: the shared one, except the border stays—the default
/// chrome draws the checkbox box and radio ring with it, in the inherited ink.
pub(crate) const TOGGLE_RESET: &str = "appearance:none;background:none;border:1px solid currentColor;margin:0;padding:0;\
   font:inherit;color:inherit;";

/// The select control's reset: the shared one, upgraded to the customizable-select
/// rendering (`appearance: base-select`) where the browser supports it.
pub(crate) const SELECT_RESET: &str = "appearance:none;appearance:base-select;background:none;border:none;margin:0;padding:0;\
   font:inherit;color:inherit;";

/// The complete rule template for a form control: its reset, then its inherited text
/// style and its static style—so every control carries a class, and author declarations
/// override the reset in place. With `feedback`, the built-in hover and focus tints
/// follow the variants, mixing the border toward the given base color while the author
/// sets no border (`currentColor` for a toggle, whose chrome border is that already).
pub(crate) fn control_template(
  control: &ControlProps,
  reset: &'static str,
  feedback: Option<&'static str>,
) -> String {
  let base = fold_box_style(
    fold_run_style(Declarations::new(), &control.text),
    &control.style,
  );
  let mut out = template(
    base,
    reset,
    &control.variants,
    &control.transitions,
    &control.animations,
    |style| fold_box_style(Declarations::new(), style),
  );

  if let Some(border_base) = feedback {
    push_interaction_rules(
      &mut out,
      &control.variants,
      &control.style.border.map_or_else(
        || border_base.to_owned(),
        |border| Declarations::render_color(border.color),
      ),
    );
  }

  let selection = fold_selection(&control.text);

  if !selection.is_empty() {
    push_rule(&mut out, &[], "::selection", &selection);
  }

  out
}

/// Appends the built-in interaction feedback: a slight ink wash over the background
/// (two, three, and four 32nds for hover, focus, and both) under a stronger ink mix
/// into the border (four, six, and eight sixteenths)—each state emitted only while no
/// variant of the control conditions on it, so authored styling wins over the default.
fn push_interaction_rules(
  out: &mut String,
  variants: &[(Condition, skene_core::BoxStyle)],
  border: &str,
) {
  use skene_core::Composable;

  let authored_hover = variants
    .iter()
    .any(|(condition, _)| condition.involves(&|leaf| matches!(leaf, Condition::Hovered)));
  let authored_focus = variants.iter().any(|(condition, _)| {
    condition.involves(&|leaf| matches!(leaf, Condition::Focused | Condition::FocusVisible))
  });

  let mut tint = |states: &[&'static str], wash_32nds: u16, border_16ths: u16| {
    let wash = format!(
      "color-mix(in srgb, currentColor {}%, transparent)",
      f32::from(wash_32nds) * 100. / 32.,
    );
    let edge = format!(
      "color-mix(in srgb, currentColor {}%, {border})",
      f32::from(border_16ths) * 100. / 16.,
    );

    push_rule(
      out,
      states,
      "",
      &Declarations::new()
        .set(
          "background-image",
          format!("linear-gradient({wash}, {wash})"),
        )
        .set("border-color", edge),
    );
  };

  if !authored_hover {
    tint(&[":hover"], 2, 4);
  }

  if !authored_focus {
    tint(&[":focus-visible"], 3, 6);
  }

  if !authored_hover && !authored_focus {
    tint(&[":focus-visible", ":hover"], 4, 8);
  }
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

/// The declarations of a run style's `::selection` pseudo-element.
fn fold_selection(style: &skene_core::RunStyle) -> Declarations {
  let mut declarations = Declarations::new();

  if let Some(value) = style.selection_color {
    declarations = declarations.set("color", Declarations::render_color(value));
  }

  if let Some(value) = style.selection_background {
    declarations = declarations.set("background-color", Declarations::render_color(value));
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

  let mut base = fold_run_style(display, &props.style);

  if props.selectable {
    base = base
      .set("-webkit-user-select", "text")
      .set("user-select", "text");
  }

  let selection = fold_selection(&props.style);

  if base.is_empty()
    && selection.is_empty()
    && props
      .variants
      .iter()
      .all(|(_, style)| *style == skene_core::RunStyle::default())
    && props.transitions.is_empty()
    && props.animations.is_empty()
  {
    return None;
  }

  let mut out = template(
    base,
    "",
    &props.variants,
    &props.transitions,
    &props.animations,
    |style| fold_run_style(Declarations::new(), style),
  );

  if !selection.is_empty() {
    push_rule(&mut out, &[], "::selection", &selection);
  }

  push_variant_rules(&mut out, "::selection", &props.variants, &fold_selection);

  Some(out)
}

/// The declarations for one text-run property alone, for an inline override.
#[inline(always)]
pub(crate) fn run_prop_declarations(prop: &RunProp) -> Declarations {
  apply_run_prop(Declarations::new(), prop)
}

/// The rule template for a raw declaration set—the escape-hatch styling path.
pub(crate) fn declarations_template(declarations: &Declarations) -> String {
  let mut out = String::new();

  push_rule(&mut out, &[], "", declarations);

  out
}

/// Appends one rule: `.<placeholder><states…><pseudo>{declarations}`.
fn push_rule(
  out: &mut String,
  states: &[&'static str],
  pseudo: &'static str,
  declarations: &Declarations,
) {
  out.push('.');
  out.push(CLASS_PLACEHOLDER);

  for state in states {
    out.push_str(state);
  }

  out.push_str(pseudo);
  out.push('{');
  out.push_str(&declarations.render());
  out.push('}');
}

/// Assembles the rule template: `@keyframes` blocks, the base rule (`reset` first, so
/// author declarations override it in place; transitions and animations folded in),
/// then each variant's rules—one per disjunctive conjunction of its condition—in
/// declaration order.
fn template<S>(
  mut base: Declarations,
  reset: &'static str,
  variants: &[(Condition, S)],
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

  out.push('.');
  out.push(CLASS_PLACEHOLDER);
  out.push('{');
  out.push_str(reset);
  out.push_str(&base.render());
  out.push('}');

  push_variant_rules(&mut out, "", variants, &style_declarations);

  out
}

/// Appends each variant's rules—one per disjunctive conjunction of its condition, each
/// targeting `pseudo` after the state selectors—in declaration order. Variants whose
/// declarations are empty add nothing.
fn push_variant_rules<S>(
  out: &mut String,
  pseudo: &'static str,
  variants: &[(Condition, S)],
  style_declarations: &impl Fn(&S) -> Declarations,
) {
  for (condition, style) in variants {
    let declarations = style_declarations(style);

    if declarations.is_empty() {
      continue;
    }

    for term in condition.disjunctive_leaves() {
      let mut media: Vec<String> = term.iter().copied().filter_map(media_feature).collect();
      let mut states: Vec<&'static str> = term.iter().copied().filter_map(state_selector).collect();

      // Canonical order, so the same condition set always renders the same template.
      media.sort_unstable();
      states.sort_unstable();

      if media.is_empty() {
        push_rule(out, &states, pseudo, &declarations);
      }
      else {
        out.push_str("@media ");
        out.push_str(&media.join(" and "));
        out.push('{');
        push_rule(out, &states, pseudo, &declarations);
        out.push('}');
      }
    }
  }
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
    StyleProperty::AspectRatio => "aspect-ratio",
    StyleProperty::Margin => "margin",
    StyleProperty::Padding => "padding",
    StyleProperty::Inset => "inset",
    StyleProperty::Background => "background-color",
    StyleProperty::BorderColor => "border-color",
    StyleProperty::BorderWidth => "border-width",
    StyleProperty::BorderRadius => "border-radius",
    StyleProperty::Spacing => "gap",
    StyleProperty::Flex => "flex",
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
