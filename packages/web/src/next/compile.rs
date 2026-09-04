//! The property model as CSS: one rule template per node, and the custom-property
//! vocabulary the templates reference.
//!
//! The only place the web driver spells CSS for the new model. A property compiles to
//! one declaration (a shorthand where the property has components, so a single custom
//! property can carry a derived value); a fixed leaf renders as its literal, a token as
//! `var(--<name>)`—a number token read by a pixel-measured property as
//! `calc(var(--<name>) * 1px)`—an expression as CSS math, and a derived slot as
//! `var(--skene-<property>)` for the base and `var(--skene-<property>-<i>)` for the
//! `i`-th variant. Variants group by their canonical condition into one rule per
//! conjunction, kept in each property's declaration order.

use std::hash::{Hash, Hasher};

use rustc_hash::{FxHashMap, FxHasher};
use skene_core::{
  Animation,
  FlexDirection,
  GridFlow,
  Layout,
  Overflow,
  Placement,
  RunDisplay,
  Wrap,
  next::{
    BoxProps,
    BoxStyle,
    BoxValue,
    BoxVisitor,
    Condition,
    Conditional,
    ControlProps,
    Definitions,
    Driver,
    DriverLeaf,
    Fingerprint,
    IntoBoxValue,
    IntoRunValue,
    OverlayProps,
    PixelExt,
    PixelProperty,
    Plain,
    Property,
    PropertyKey,
    RunProps,
    RunStyle,
    RunValue,
    RunVisitor,
    Slot,
    Static,
    StaticLeaf,
    TokenName,
  },
};

use super::{
  condition::{CLASS_PLACEHOLDER, Conjunction},
  expr::render_expr,
  motion::{render_animation_values, render_keyframes, render_transitions},
  token::WebTokenValue,
};
use crate::Declarations;

/// A driver over pixels with the pixel extension: the one the compiler serves.
pub(crate) trait PixelDriver: Driver<Scalar = f32, StyleExt = PixelExt> {}

impl<D: Driver<Scalar = f32, StyleExt = PixelExt>> PixelDriver for D {}

/// A property's key on the web.
pub(crate) type Key = PropertyKey<PixelProperty>;

/// What a node's props compiled to: the rule template and the slots the class
/// references.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub(crate) struct Compiled {
  /// The rule template, the class name as [`CLASS_PLACEHOLDER`].
  pub template: String,
  /// The property and leaf position behind each slot, indexed by slot.
  pub slots: Vec<SlotBinding>,
}

/// The property and leaf position a slot writes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub(crate) struct SlotBinding {
  /// The slot.
  pub slot: Slot,
  /// The property the slot belongs to.
  pub property: Key,
  /// The leaf's position: `0` the base, `i` the `i`-th variant.
  pub position: usize,
}

impl SlotBinding {
  /// The custom property the slot is written to.
  #[must_use]
  pub(crate) fn custom_property(&self) -> String {
    slot_property(self.property, self.position)
  }
}

/// The custom property standing for the leaf at `position` of `key`.
fn slot_property(key: Key, position: usize) -> String {
  let name = key.name().replace('_', "-");

  if position == 0 {
    format!("--skene-{name}")
  }
  else {
    format!("--skene-{name}-{position}")
  }
}

/// How a numeric value of a property reads.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Unit {
  /// Logical pixels.
  Pixels,
  /// A plain number.
  Plain,
}

/// The CSS a property compiles to.
struct Target {
  name: &'static str,
  pseudo: &'static str,
  unit: Unit,
  position: Option<&'static str>,
}

const fn plain(name: &'static str) -> Target {
  Target {
    name,
    pseudo: "",
    unit: Unit::Plain,
    position: None,
  }
}

const fn pixels(name: &'static str) -> Target {
  Target {
    name,
    pseudo: "",
    unit: Unit::Pixels,
    position: None,
  }
}

/// The CSS property, pseudo-element, unit and positioning `key` compiles to.
const fn target(key: Key) -> Target {
  match key {
    PropertyKey::Shared(property) => match property {
      Property::Width => plain("width"),
      Property::Height => plain("height"),
      Property::MinWidth => plain("min-width"),
      Property::MinHeight => plain("min-height"),
      Property::MaxWidth => plain("max-width"),
      Property::MaxHeight => plain("max-height"),
      Property::AspectRatio => plain("aspect-ratio"),
      Property::Margin => plain("margin"),
      Property::Padding => pixels("padding"),
      Property::Offset => Target {
        position: Some("relative"),
        ..plain("inset")
      },
      Property::Pinned => Target {
        position: Some("absolute"),
        ..plain("inset")
      },
      Property::Color => plain("background-color"),
      Property::Border => plain("border"),
      Property::Cursor => plain("cursor"),
      Property::Spacing => pixels("gap"),
      Property::Wrap => plain("flex-wrap"),
      Property::Flex => plain("flex"),
      Property::Arrangement => plain("justify-content"),
      Property::ItemAlignment => plain("align-items"),
      Property::LineArrangement => plain("align-content"),
      Property::ItemHorizontalAlignment => plain("justify-items"),
      Property::SelfAlignment => plain("align-self"),
      Property::SelfHorizontalAlignment => plain("justify-self"),
      Property::GridColumns => plain("grid-template-columns"),
      Property::GridRows => plain("grid-template-rows"),
      Property::GridImplicitColumns => plain("grid-auto-columns"),
      Property::GridImplicitRows => plain("grid-auto-rows"),
      Property::GridFlow => plain("grid-auto-flow"),
      Property::GridRowPlacement => plain("grid-row"),
      Property::GridColumnPlacement => plain("grid-column"),
      Property::Overflow => plain("overflow"),
      Property::TextColor => plain("color"),
      Property::SelectionColor => Target {
        pseudo: "::selection",
        ..plain("color")
      },
      Property::SelectionBackground => Target {
        pseudo: "::selection",
        ..plain("background-color")
      },
    },
    PropertyKey::Extension(property) => match property {
      PixelProperty::CornerRadius => pixels("border-radius"),
      PixelProperty::FontSize => pixels("font-size"),
      PixelProperty::FontWeight => plain("font-weight"),
      PixelProperty::LineHeight => plain("line-height"),
      PixelProperty::LetterSpacing => pixels("letter-spacing"),
    },
  }
}

/// A number in the property's unit.
fn number(unit: Unit, value: f32) -> String {
  match unit {
    Unit::Pixels => format!("{value}px"),
    Unit::Plain => value.to_string(),
  }
}

/// A token reference read by a property: its custom property, scaled to pixels where
/// the property measures in them.
fn token_reference(unit: Unit, name: TokenName) -> String {
  match unit {
    Unit::Pixels => format!("calc(var(--{name}) * 1px)"),
    Unit::Plain => format!("var(--{name})"),
  }
}

/// A color leaf's text: a literal, a token or an expression.
fn color_leaf(leaf: &StaticLeaf<skene_core::Color>) -> String {
  match leaf {
    StaticLeaf::Fixed(color) => Declarations::render_color(*color),
    StaticLeaf::Token(reference) => format!("var(--{})", reference.name()),
    StaticLeaf::Expr(expr) => render_expr(expr.node()),
  }
}

/// The CSS text of a box property's erased value.
pub(crate) fn render_box_value(key: Key, value: &BoxValue<f32, PixelExt>) -> String {
  let unit = target(key).unit;

  match value {
    BoxValue::Dimension(dimension) => Declarations::render_dimension(*dimension),
    BoxValue::Number(value) | BoxValue::Extension(value) => number(unit, *value),
    BoxValue::DimensionInset(inset) => Declarations::render_dimension_inset(*inset),
    BoxValue::Inset(inset) => format!(
      "{}px {}px {}px {}px",
      inset.top, inset.right, inset.bottom, inset.left
    ),
    BoxValue::Color(color) => Declarations::render_color(*color),
    BoxValue::Border(border) => {
      format!("{}px solid {}", border.width, color_leaf(&border.color))
    }
    BoxValue::Cursor(cursor) => Declarations::render_cursor(*cursor).to_owned(),
    BoxValue::Spacing(spacing) => format!("{}px {}px", spacing.row, spacing.column),
    BoxValue::Wrap(wrap) => match wrap {
      Wrap::None => "nowrap",
      Wrap::Forward => "wrap",
      Wrap::Reverse => "wrap-reverse",
    }
    .to_owned(),
    BoxValue::Flexibility(flex) => format!(
      "{} {} {}",
      flex.grow_factor,
      flex.shrink_factor,
      Declarations::render_dimension(flex.initial),
    ),
    BoxValue::Arrangement(arrangement) => Declarations::render_arrangement(*arrangement).to_owned(),
    BoxValue::ItemAlignment(alignment) => {
      Declarations::render_item_alignment(*alignment).to_owned()
    }
    BoxValue::Tracks(tracks) => Declarations::render_tracks(tracks),
    BoxValue::GridFlow(flow) => match flow {
      GridFlow::Rows => "row",
      GridFlow::Columns => "column",
      GridFlow::DenseRows => "row dense",
      GridFlow::DenseColumns => "column dense",
    }
    .to_owned(),
    BoxValue::Placement(line) => Declarations::render_grid_line(*line),
    BoxValue::Overflow(overflow) => match overflow {
      Overflow::Visible => "visible",
      Overflow::Hidden => "hidden",
      Overflow::Scroll => "scroll",
      Overflow::Auto => "auto",
    }
    .to_owned(),
  }
}

/// The CSS text of a run property's erased value.
pub(crate) fn render_run_value(key: Key, value: &RunValue<PixelExt>) -> String {
  match value {
    RunValue::Color(color) => Declarations::render_color(*color),
    RunValue::Extension(value) => number(target(key).unit, *value),
  }
}

/// The text of a driver leaf of `key` at `position`, recording a slot it references.
fn leaf_text<T: Clone>(
  key: Key,
  position: usize,
  leaf: &DriverLeaf<T>,
  slots: &mut Vec<SlotBinding>,
  fixed: impl FnOnce(T) -> String,
) -> String {
  match leaf {
    DriverLeaf::Static(StaticLeaf::Fixed(value)) => fixed(value.clone()),
    DriverLeaf::Static(StaticLeaf::Token(reference)) => {
      token_reference(target(key).unit, reference.name())
    }
    DriverLeaf::Static(StaticLeaf::Expr(expr)) => render_expr(expr.node()),
    DriverLeaf::Slot(slot) => {
      slots.push(SlotBinding {
        slot: *slot,
        property: key,
        position,
      });

      format!("var({})", slot_property(key, position))
    }
  }
}

/// One rule in the making: its conjunction, its pseudo-element and its declarations.
struct Group {
  conjunction: Conjunction,
  pseudo: &'static str,
  declarations: Declarations,
}

/// The rules of one template, gathered property by property.
struct Rules {
  base: Vec<Group>,
  variants: Vec<Group>,
  latest: FxHashMap<(Conjunction, &'static str), usize>,
  slots: Vec<SlotBinding>,
}

impl Rules {
  fn new(base: Declarations) -> Self {
    Self {
      base: vec![Group {
        conjunction: Conjunction::default(),
        pseudo: "",
        declarations: base,
      }],
      variants: Vec::new(),
      latest: FxHashMap::default(),
      slots: Vec::new(),
    }
  }

  /// The base declarations of `pseudo`.
  fn base_of(&mut self, pseudo: &'static str) -> &mut Declarations {
    let index = self
      .base
      .iter()
      .position(|group| group.pseudo == pseudo)
      .unwrap_or_else(|| {
        self.base.push(Group {
          conjunction: Conjunction::default(),
          pseudo,
          declarations: Declarations::new(),
        });

        self.base.len() - 1
      });

    &mut self.base[index].declarations
  }

  /// Adds one property: its base leaf to the base rule and each variant to the rule of
  /// its conjunction, reusing a conjunction's rule only while that keeps the property's
  /// variants in declaration order.
  fn property<T: Clone>(
    &mut self,
    key: Key,
    value: &Conditional<DriverLeaf<T>, Condition<f32>>,
    fixed: &impl Fn(T) -> String,
  ) {
    let Target {
      name,
      pseudo,
      position,
      ..
    } = target(key);
    let text = leaf_text(key, 0, value.base(), &mut self.slots, fixed);
    let base = std::mem::take(self.base_of(pseudo)).set(name, text);
    let base = match position {
      Some(position) => base.set("position", position),
      None => base,
    };

    *self.base_of(pseudo) = base;

    let mut last = None;

    for (index, (condition, leaf)) in value.variants().iter().enumerate() {
      let text = leaf_text(key, index + 1, leaf, &mut self.slots, fixed);

      for conjunction in Conjunction::of_condition(condition) {
        let lookup = (conjunction, pseudo);
        let group = match self.latest.get(&lookup) {
          Some(&group) if last.is_none_or(|last| group > last) => group,
          _ => {
            self.variants.push(Group {
              conjunction: lookup.0.clone(),
              pseudo,
              declarations: Declarations::new(),
            });

            let group = self.variants.len() - 1;

            self.latest.insert(lookup, group);

            group
          }
        };
        let declarations =
          std::mem::take(&mut self.variants[group].declarations).set(name, text.clone());

        self.variants[group].declarations = match position {
          Some(position) => declarations.set("position", position),
          None => declarations,
        };
        last = Some(group);
      }
    }
  }

  /// Renders every rule after `prefix`: the base rules, then the variants in order.
  fn finish(self, mut out: String) -> Compiled {
    for group in self.base.iter().chain(&self.variants) {
      if group.declarations.is_empty() {
        continue;
      }

      group
        .conjunction
        .emit(&mut out, None, group.pseudo, &group.declarations.render());
    }

    Compiled {
      template: out,
      slots: self.slots,
    }
  }
}

/// Feeds a static box style's properties to the rules.
struct BoxRules<'a>(&'a mut Rules);

impl BoxVisitor<f32, Static<Condition<f32>>, PixelExt> for BoxRules<'_> {
  fn value<T: IntoBoxValue<f32, PixelExt>>(
    &mut self,
    key: Key,
    value: &Option<Conditional<DriverLeaf<T>, Condition<f32>>>,
  ) {
    if let Some(value) = value {
      self.0.property(key, value, &|fixed: T| {
        render_box_value(key, &fixed.into_box_value())
      });
    }
  }
}

/// Feeds a static run style's properties to the rules.
struct RunRules<'a>(&'a mut Rules);

impl RunVisitor<Static<Condition<f32>>, PixelExt> for RunRules<'_> {
  fn value<T: IntoRunValue<PixelExt>>(
    &mut self,
    key: Key,
    value: &Option<Conditional<DriverLeaf<T>, Condition<f32>>>,
  ) {
    if let Some(value) = value {
      self.0.property(key, value, &|fixed: T| {
        render_run_value(key, &fixed.into_run_value())
      });
    }
  }
}

/// Renders a plain box style as declarations: a keyframe.
struct PlainBox(Declarations);

impl BoxVisitor<f32, Plain, PixelExt> for PlainBox {
  fn value<T: IntoBoxValue<f32, PixelExt>>(&mut self, key: Key, value: &Option<T>) {
    if let Some(value) = value {
      let Target { name, position, .. } = target(key);
      let declarations = std::mem::take(&mut self.0)
        .set(name, render_box_value(key, &value.clone().into_box_value()));

      self.0 = match position {
        Some(position) => declarations.set("position", position),
        None => declarations,
      };
    }
  }
}

/// Renders a plain run style as declarations: a keyframe.
struct PlainRun(Declarations);

impl RunVisitor<Plain, PixelExt> for PlainRun {
  fn value<T: IntoRunValue<PixelExt>>(&mut self, key: Key, value: &Option<T>) {
    if let Some(value) = value {
      self.0 = std::mem::take(&mut self.0).set(
        target(key).name,
        render_run_value(key, &value.clone().into_run_value()),
      );
    }
  }
}

fn plain_box_declarations(style: &BoxStyle<f32, Plain, PixelExt>) -> Declarations {
  let mut plain = PlainBox(Declarations::new());

  style.visit(&mut plain);

  plain.0
}

fn plain_run_declarations(style: &RunStyle<Plain, PixelExt>) -> Declarations {
  let mut plain = PlainRun(Declarations::new());

  style.visit(&mut plain);

  plain.0
}

/// Folds the motion into `base` and appends the `@keyframes` blocks to `out`.
fn motion<S>(
  out: &mut String,
  base: Declarations,
  transitions: &[skene_core::Transition],
  animations: &[Animation<S>],
  declarations: &impl Fn(&S) -> Declarations,
) -> Declarations {
  let mut base = base;

  if !transitions.is_empty() {
    base = base.set("transition", render_transitions(transitions));
  }

  if !animations.is_empty() {
    let names: Vec<String> = animations
      .iter()
      .map(|animation| render_keyframes(out, animation, declarations))
      .collect();

    base = base.set("animation", render_animation_values(animations, &names));
  }

  base
}

/// The static display declarations for a [`Layout`].
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
    Layout::Single => Declarations::new()
      .set("display", "grid")
      .set("grid-template-columns", "1fr")
      .set("grid-template-rows", "1fr"),
    Layout::Stack => Declarations::new()
      .set("display", "grid")
      .set("position", "relative"),
  }
}

/// The base declarations a box starts from: its layout and, when it names itself for
/// container queries, its containment—`size` when its height is set, else
/// `inline-size`.
fn box_base<D: PixelDriver>(props: &BoxProps<D>) -> Declarations {
  let mut base = layout_declarations(props.layout);

  if let Some(name) = props.container {
    let containment = if props.style.height.is_some() {
      "size"
    }
    else {
      "inline-size"
    };

    base = base.set("container", format!("{} / {containment}", name.0));
  }

  base
}

/// Compiles a box's props with `reset` and `base` ahead of its own declarations.
fn compile_box_with<D: PixelDriver>(props: &BoxProps<D>, base: Declarations) -> Compiled {
  let mut out = String::new();
  let base = motion(
    &mut out,
    base,
    &props.dynamics.transitions,
    &props.dynamics.animations,
    &plain_box_declarations,
  );
  let mut rules = Rules::new(base);

  props.style.visit(&mut BoxRules(&mut rules));

  rules.finish(out)
}

/// The rule template of a box; a stack's places every child in its one cell.
pub(crate) fn compile_box<D: PixelDriver>(props: &BoxProps<D>) -> Compiled {
  let mut compiled = compile_box_with(props, box_base(props));

  if props.layout == Layout::Stack {
    Conjunction::default().emit(
      &mut compiled.template,
      None,
      ">*",
      &Declarations::new().set("grid-area", "1 / 1").render(),
    );
  }

  compiled
}

/// The rule template of a control: its box's, over the control reset that strips the
/// native element's own chrome.
pub(crate) fn compile_control<D: PixelDriver>(props: &ControlProps<D>) -> Compiled {
  compile_box_with(
    &props.box_props,
    box_base(&props.box_props)
      .set("appearance", "none")
      .set("font", "inherit")
      .set("color", "inherit")
      .set("text-align", "inherit"),
  )
}

/// The margin declaration placing an overlay against the viewport: `auto` centers an
/// axis, `0` pins an edge.
const fn placement_margin(placement: Placement) -> &'static str {
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

/// The rule template of an overlay: its box's over the top-layer reset and its
/// placement margins, and the `::backdrop` rule of a modal with a backdrop.
pub(crate) fn compile_overlay<D: PixelDriver>(props: &OverlayProps<D>) -> Compiled {
  let base = box_base(&props.box_props)
    .set("border", "none")
    .set("padding", "0")
    .set("background", "none")
    .set("color", "inherit")
    .set("overflow", "visible")
    .set("margin", placement_margin(props.placement));
  let mut compiled = compile_box_with(&props.box_props, base);

  if let Some(color) = props.backdrop {
    Conjunction::default().emit(
      &mut compiled.template,
      None,
      "::backdrop",
      &Declarations::new()
        .set("background-color", Declarations::render_color(color))
        .render(),
    );
  }

  compiled
}

/// The rule template of a text run.
pub(crate) fn compile_run<D: PixelDriver>(props: &RunProps<D>) -> Compiled {
  let mut base = match props.display {
    RunDisplay::Block => Declarations::new().set("display", "block"),
    RunDisplay::Inline => Declarations::new(),
    RunDisplay::InlineBox => Declarations::new().set("display", "inline-block"),
  };

  if props.selectable {
    base = base
      .set("-webkit-user-select", "text")
      .set("user-select", "text");
  }

  let mut out = String::new();
  let base = motion(
    &mut out,
    base,
    &props.dynamics.transitions,
    &props.dynamics.animations,
    &plain_run_declarations,
  );
  let mut rules = Rules::new(base);

  props.style.visit(&mut RunRules(&mut rules));

  rules.finish(out)
}

/// The text of a token definition's leaf.
fn definition_leaf(value: &WebTokenValue, position: usize) -> Option<String> {
  fn leaf<T: Clone>(
    definition: &skene_core::next::Definition<T, f32>,
    position: usize,
    fixed: impl FnOnce(T) -> String,
  ) -> Option<String> {
    let leaf = if position == 0 {
      definition.base()
    }
    else {
      &definition.variants().get(position - 1)?.1
    };

    Some(match leaf {
      StaticLeaf::Fixed(value) => fixed(value.clone()),
      StaticLeaf::Token(reference) => format!("var(--{})", reference.name()),
      StaticLeaf::Expr(expr) => render_expr(expr.node()),
    })
  }

  match value {
    WebTokenValue::Color(definition) => leaf(definition, position, Declarations::render_color),
    WebTokenValue::Number(definition) => leaf(definition, position, |number| number.to_string()),
    WebTokenValue::Dimension(definition) => {
      leaf(definition, position, Declarations::render_dimension)
    }
  }
}

/// The media conditions of a definition's variants, in order.
fn definition_conditions(value: &WebTokenValue) -> Vec<skene_core::next::Media<f32>> {
  match value {
    WebTokenValue::Color(definition) => definition
      .variants()
      .iter()
      .map(|(media, _)| media.clone())
      .collect(),
    WebTokenValue::Number(definition) => definition
      .variants()
      .iter()
      .map(|(media, _)| media.clone())
      .collect(),
    WebTokenValue::Dimension(definition) => definition
      .variants()
      .iter()
      .map(|(media, _)| media.clone())
      .collect(),
  }
}

/// Whether every definition is a single leaf under no condition.
pub(crate) fn all_unconditional(definitions: &Definitions<WebTokenValue>) -> bool {
  definitions
    .iter()
    .all(|(_, value)| definition_conditions(value).is_empty())
}

/// The inline custom-property declarations of unconditional definitions.
pub(crate) fn inline_definitions(
  definitions: &Definitions<WebTokenValue>,
) -> Vec<(String, String)> {
  definitions
    .iter()
    .filter_map(|(name, value)| Some((format!("--{name}"), definition_leaf(value, 0)?)))
    .collect()
}

/// The rule template defining `definitions` on `subject`: the bases in one rule, each
/// variant under its media block, the scheme emitted twice.
fn compile_definitions_on(subject: &str, definitions: &Definitions<WebTokenValue>) -> String {
  let mut base = Vec::new();
  let mut variants: Vec<(Conjunction, Vec<(String, String)>)> = Vec::new();
  let mut latest: FxHashMap<Conjunction, usize> = FxHashMap::default();

  for (name, value) in definitions.iter() {
    let property = format!("--{name}");

    if let Some(text) = definition_leaf(value, 0) {
      base.push((property.clone(), text));
    }

    let mut last = None;

    for (index, media) in definition_conditions(value).iter().enumerate() {
      let Some(text) = definition_leaf(value, index + 1)
      else {
        continue;
      };

      for conjunction in Conjunction::of_media(media) {
        let group = match latest.get(&conjunction) {
          Some(&group) if last.is_none_or(|last| group > last) => group,
          _ => {
            variants.push((conjunction.clone(), Vec::new()));
            latest.insert(conjunction, variants.len() - 1);

            variants.len() - 1
          }
        };

        variants[group].1.push((property.clone(), text.clone()));
        last = Some(group);
      }
    }
  }

  let mut out = String::new();
  let render = |entries: &[(String, String)]| {
    let mut text = String::new();

    for (property, value) in entries {
      text.push_str(property);
      text.push(':');
      text.push_str(value);
      text.push(';');
    }

    text
  };

  out.push_str(subject);
  out.push('{');
  out.push_str(&render(&base));
  out.push('}');

  for (conjunction, entries) in &variants {
    conjunction.emit(&mut out, Some(subject), "", &render(entries));
  }

  out
}

/// The rule template of a scope's token definitions.
pub(crate) fn compile_definitions(definitions: &Definitions<WebTokenValue>) -> String {
  compile_definitions_on(&format!(".{CLASS_PLACEHOLDER}"), definitions)
}

/// The rule template of the root's token definitions.
pub(crate) fn compile_root_definitions(definitions: &Definitions<WebTokenValue>) -> String {
  compile_definitions_on(":root", definitions)
}

/// The `@property` registrations of every slot, appended to the reset: a slot is a
/// per-node value the node's own class reads, so it does not inherit.
pub(crate) fn slot_registrations() -> String {
  struct Keys(Vec<Key>);

  impl BoxVisitor<f32, Plain, PixelExt> for Keys {
    fn value<T: IntoBoxValue<f32, PixelExt>>(&mut self, key: Key, _: &Option<T>) {
      self.0.push(key);
    }
  }

  impl RunVisitor<Plain, PixelExt> for Keys {
    fn value<T: IntoRunValue<PixelExt>>(&mut self, key: Key, _: &Option<T>) {
      self.0.push(key);
    }
  }

  let mut keys = Keys(Vec::new());

  BoxStyle::<f32, Plain, PixelExt>::default().visit(&mut keys);
  RunStyle::<Plain, PixelExt>::default().visit(&mut keys);
  keys.0.dedup();

  let mut out = String::new();

  for key in keys.0 {
    out.push_str("@property ");
    out.push_str(&slot_property(key, 0));
    out.push_str("{syntax:\"*\";inherits:false}");
  }

  out
}

/// The hash the class cache keys a box's compiled template on.
pub(crate) fn box_fingerprint<D: PixelDriver>(props: &BoxProps<D>, reset: u8) -> u64 {
  let mut hasher = FxHasher::default();

  hasher.write_u8(reset);
  props.layout.hash(&mut hasher);
  props.container.hash(&mut hasher);
  props.style.fingerprint(&mut hasher);
  props.dynamics.fingerprint(&mut hasher);

  hasher.finish()
}

/// The hash the class cache keys an overlay's compiled template on.
pub(crate) fn overlay_fingerprint<D: PixelDriver>(props: &OverlayProps<D>) -> u64 {
  let mut hasher = FxHasher::default();

  hasher.write_u64(box_fingerprint(&props.box_props, 2));
  props.modal.hash(&mut hasher);
  (props.placement as u8).hash(&mut hasher);
  props.backdrop.fingerprint(&mut hasher);

  hasher.finish()
}

/// The hash the class cache keys a run's compiled template on.
pub(crate) fn run_fingerprint<D: PixelDriver>(props: &RunProps<D>) -> u64 {
  let mut hasher = FxHasher::default();

  (props.display as u8).hash(&mut hasher);
  props.selectable.hash(&mut hasher);
  props.style.fingerprint(&mut hasher);
  props.dynamics.fingerprint(&mut hasher);

  hasher.finish()
}
