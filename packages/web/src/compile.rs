//! The property model as CSS: one rule template per node, and the custom-property
//! vocabulary the templates reference.
//!
//! The only place the web platform spells CSS for the property model. A property
//! compiles to one declaration (a shorthand where the property has components), except
//! an inset, which compiles to its block and inline halves; a fixed leaf renders as its
//! literal, a token as `var(--<name>)`, an expression as CSS math, and a derived slot
//! as `var(--skene-<property>)` for the base and `var(--skene-<property>-<i>)` for the
//! `i`-th variant, an inset's halves suffixed `-block` and `-inline` before the
//! variant's index. Variants group by their canonical condition into one rule per
//! conjunction, kept in each property's declaration order.

use std::{
  fmt::Write as _,
  hash::{Hash, Hasher},
};

use rustc_hash::{FxHashMap, FxHasher};
use skene_foundation::{
  Animation,
  Axis,
  BoxProps,
  BoxStyle,
  BoxValue,
  BoxVisitor,
  Condition,
  Conditional,
  ControlProps,
  Definitions,
  Fingerprint,
  GridFlow,
  IntoBoxValue,
  IntoRunValue,
  Keyframe,
  Layout,
  NativeElement,
  Overflow,
  OverlayProps,
  Placement,
  Plain,
  Platform,
  PlatformLeaf,
  Property,
  PropertyKey,
  RunPlacement,
  RunProps,
  RunStyle,
  RunValue,
  RunVisitor,
  Slot,
  Static,
  StaticBoxStyle,
  StaticLeaf,
  TextDecoration,
  TextDecorationStyle,
  TokenName,
  Wrap,
  typography,
  web::{WebProperty, WebStyle},
};

use super::{
  condition::{CLASS_PLACEHOLDER, Conjunction},
  expr::render_expr,
  motion::{NO_PREFERENCE, REDUCE, render_animation_values, render_keyframes, render_transitions},
  token::WebTokenValue,
};
use crate::declarations::{Declarations, Ident};

/// A platform over pixels with the pixel extension: the one the compiler serves.
pub(crate) trait PixelPlatform: Platform<Scalar = f32, StyleExt = WebStyle> {}

impl<P: Platform<Scalar = f32, StyleExt = WebStyle>> PixelPlatform for P {}

/// A property's key on the web.
pub(crate) type Key = PropertyKey<WebProperty>;

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
  /// The custom properties the slot is written to, one per property its target declares.
  #[must_use]
  pub(crate) fn custom_properties(&self) -> Vec<String> {
    slot_properties(self.property, self.position)
  }
}

/// The custom properties standing for the leaf at `position` of `key`, one per property
/// its target declares.
fn slot_properties(key: Key, position: usize) -> Vec<String> {
  let name = key.name().replace('_', "-");
  let index = match position {
    0 => String::new(),
    position => format!("-{position}"),
  };

  match target(key).halves {
    Some(_) => vec![
      format!("--skene-{name}-block{index}"),
      format!("--skene-{name}-inline{index}"),
    ],
    None => vec![format!("--skene-{name}{index}")],
  }
}

/// The CSS a property compiles to.
pub(crate) struct Target {
  /// The property a transition names, and the one declared unless the target has halves.
  pub(crate) name: &'static str,
  /// The block and inline halves declared in place of `name`, for an inset.
  halves: Option<[&'static str; 2]>,
  /// The pseudo-element the declarations style; empty for the element itself.
  pseudo: &'static str,
  /// The positioning declared beside the property, where it needs one.
  position: Option<&'static str>,
}

impl Target {
  /// The properties the target declares: its halves where it has them, else its name.
  const fn declared(&self) -> &[&'static str] {
    match &self.halves {
      Some(halves) => halves,
      None => std::slice::from_ref(&self.name),
    }
  }

  /// `declarations` with each property the target declares set to its text in `texts`,
  /// the shorthand its halves stand in for removed.
  fn declare(&self, declarations: Declarations, texts: &[String]) -> Declarations {
    let declarations = match self.halves {
      Some(_) => declarations.remove(self.name),
      None => declarations,
    };

    self
      .declared()
      .iter()
      .zip(texts)
      .fold(declarations, |declarations, (name, text)| {
        declarations.set(name, text.clone())
      })
  }
}

const fn plain(name: &'static str) -> Target {
  Target {
    name,
    halves: None,
    pseudo: "",
    position: None,
  }
}

/// A target declaring an inset as its block half and its inline half.
const fn halved(name: &'static str, block: &'static str, inline: &'static str) -> Target {
  Target {
    halves: Some([block, inline]),
    ..plain(name)
  }
}

/// The CSS property, pseudo-element and positioning `key` compiles to.
pub(crate) const fn target(key: Key) -> Target {
  match key {
    PropertyKey::Shared(property) => match property {
      Property::Width => plain("width"),
      Property::Height => plain("height"),
      Property::MinWidth => plain("min-width"),
      Property::MinHeight => plain("min-height"),
      Property::MaxWidth => plain("max-width"),
      Property::MaxHeight => plain("max-height"),
      Property::AspectRatio => plain("aspect-ratio"),
      Property::Margin => halved("margin", "margin-block", "margin-inline"),
      Property::Padding => halved("padding", "padding-block", "padding-inline"),
      Property::Offset => Target {
        position: Some("relative"),
        ..halved("inset", "inset-block", "inset-inline")
      },
      Property::Pinned => Target {
        position: Some("absolute"),
        ..halved("inset", "inset-block", "inset-inline")
      },
      Property::Background => plain("background-color"),
      Property::BorderWidth => halved("border-width", "border-block-width", "border-inline-width"),
      Property::BorderColor => plain("border-color"),
      Property::Cursor => plain("cursor"),
      Property::Spacing => plain("gap"),
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
      Property::GridAreas => plain("grid-template-areas"),
      Property::GridImplicitColumns => plain("grid-auto-columns"),
      Property::GridImplicitRows => plain("grid-auto-rows"),
      Property::GridFlow => plain("grid-auto-flow"),
      Property::GridRowPlacement => plain("grid-row"),
      Property::GridColumnPlacement => plain("grid-column"),
      Property::Overflow => plain("overflow"),
      Property::Hidden => plain("visibility"),
      Property::TextColor => plain("color"),
      Property::SelectionColor => Target {
        pseudo: "::selection",
        ..plain("color")
      },
      Property::SelectionBackground => Target {
        pseudo: "::selection",
        ..plain("background-color")
      },
      Property::Decoration => plain("text-decoration-line"),
      Property::DecorationStyle => plain("text-decoration-style"),
      Property::DecorationColor => plain("text-decoration-color"),
    },
    PropertyKey::Extension(property) => match property {
      WebProperty::CornerRadius => plain("border-radius"),
      WebProperty::FontSize => plain("font-size"),
      WebProperty::FontWeight => plain("font-weight"),
      WebProperty::LineHeight => plain("line-height"),
      WebProperty::LetterSpacing => plain("letter-spacing"),
    },
  }
}

/// A token reference: its custom property.
fn token_reference(name: TokenName) -> String {
  format!("var(--{name})")
}

/// The CSS texts of a box property's erased value, one per property its target
/// declares: an inset's block and inline halves, any other value whole.
pub(crate) fn render_box_value(value: &BoxValue<f32, WebStyle>) -> Vec<String> {
  let whole = match value {
    BoxValue::DimensionInset(inset) => return Declarations::render_dimension_inset(*inset).into(),
    BoxValue::Inset(inset) => {
      return vec![
        format!("{}px {}px", inset.top, inset.bottom),
        format!("{}px {}px", inset.start, inset.end),
      ];
    }
    BoxValue::Dimension(dimension) => Declarations::render_dimension(*dimension),
    BoxValue::Length(length) => format!("{}px", length.0),
    BoxValue::Number(value) | BoxValue::Extension(value) => value.to_string(),
    BoxValue::AspectRatio(ratio) => ratio.0.to_string(),
    BoxValue::Color(color) => Declarations::render_color(*color),
    BoxValue::Cursor(cursor) => Declarations::render_cursor(*cursor).to_owned(),
    BoxValue::Spacing(spacing) => format!("{}px {}px", spacing.vertical, spacing.horizontal),
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
    BoxValue::Template(template) => Declarations::render_template(template),
    BoxValue::Areas(areas) => Declarations::render_areas(areas),
    BoxValue::GridFlow(flow) => match flow {
      GridFlow::Rows => "row",
      GridFlow::Columns => "column",
      GridFlow::DenseRows => "row dense",
      GridFlow::DenseColumns => "column dense",
    }
    .to_owned(),
    BoxValue::Placement(line) => Declarations::render_grid_line(*line),
    BoxValue::Overflow(overflow) => {
      let keyword = |overflow| match overflow {
        Overflow::Visible => "visible",
        Overflow::Clip => "hidden",
        Overflow::Scroll => "scroll",
        Overflow::ScrollIfNeeded => "auto",
      };

      if overflow.horizontal == overflow.vertical {
        keyword(overflow.horizontal).to_owned()
      }
      else {
        format!(
          "{} {}",
          keyword(overflow.horizontal),
          keyword(overflow.vertical)
        )
      }
    }
    BoxValue::Hidden(hidden) => if *hidden { "hidden" } else { "inherit" }.to_owned(),
  };

  vec![whole]
}

/// The CSS text of a run property's erased value.
pub(crate) fn render_run_value(value: &RunValue<WebStyle>) -> String {
  match value {
    RunValue::Color(color) => Declarations::render_color(*color),
    RunValue::Length(length) => format!("{}px", length.0),
    RunValue::Decoration(decoration) => match decoration {
      TextDecoration::None => "none",
      TextDecoration::Underline => "underline",
      TextDecoration::Overline => "overline",
      TextDecoration::LineThrough => "line-through",
    }
    .to_owned(),
    RunValue::DecorationStyle(style) => match style {
      TextDecorationStyle::Solid => "solid",
      TextDecorationStyle::Double => "double",
      TextDecorationStyle::Dotted => "dotted",
      TextDecorationStyle::Dashed => "dashed",
      TextDecorationStyle::Wavy => "wavy",
    }
    .to_owned(),
    RunValue::Extension(value) => value.to_string(),
  }
}

/// The CSS texts a derived box leaf writes into the `count` custom properties of its
/// slot: a value rendered, a token or an expression referenced in each.
pub(crate) fn render_box_leaf(
  leaf: &StaticLeaf<BoxValue<f32, WebStyle>>,
  count: usize,
) -> Vec<String> {
  static_leaf_texts(leaf, count, |value| render_box_value(&value))
}

/// The CSS texts a derived run leaf writes into the `count` custom properties of its
/// slot: a value rendered, a token or an expression referenced in each.
pub(crate) fn render_run_leaf(leaf: &StaticLeaf<RunValue<WebStyle>>, count: usize) -> Vec<String> {
  static_leaf_texts(leaf, count, |value| vec![render_run_value(&value)])
}

/// The texts of a static leaf for `count` declarations: a fixed value through `fixed`,
/// a token or an expression referenced in each.
fn static_leaf_texts<T: Clone>(
  leaf: &StaticLeaf<T>,
  count: usize,
  fixed: impl FnOnce(T) -> Vec<String>,
) -> Vec<String> {
  match leaf {
    StaticLeaf::Fixed(value) => fixed(value.clone()),
    StaticLeaf::Token(name) => vec![token_reference(*name); count],
    StaticLeaf::Expr(expr) => vec![render_expr(expr.node()); count],
  }
}

/// The texts of a platform leaf of `key` at `position`, one per property its target
/// declares, recording a slot it references.
fn leaf_texts<T: Clone>(
  key: Key,
  position: usize,
  leaf: &PlatformLeaf<T>,
  slots: &mut Vec<SlotBinding>,
  fixed: impl FnOnce(T) -> Vec<String>,
) -> Vec<String> {
  match leaf {
    PlatformLeaf::Static(leaf) => static_leaf_texts(leaf, target(key).declared().len(), fixed),
    PlatformLeaf::Slot(slot) => {
      slots.push(SlotBinding {
        slot: *slot,
        property: key,
        position,
      });

      slot_properties(key, position)
        .into_iter()
        .map(|property| format!("var({property})"))
        .collect()
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
    value: &Conditional<PlatformLeaf<T>, Condition<f32>>,
    fixed: &impl Fn(T) -> Vec<String>,
  ) {
    let target = target(key);
    let Target {
      pseudo, position, ..
    } = target;
    let texts = leaf_texts(key, 0, value.base(), &mut self.slots, fixed);
    let base = target.declare(std::mem::take(self.base_of(pseudo)), &texts);
    let base = match position {
      Some(position) => base.set("position", position),
      None => base,
    };

    *self.base_of(pseudo) = base;

    let mut last = None;

    for (index, (condition, leaf)) in value.variants().iter().enumerate() {
      let texts = leaf_texts(key, index + 1, leaf, &mut self.slots, fixed);

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
        let declarations = target.declare(
          std::mem::take(&mut self.variants[group].declarations),
          &texts,
        );

        self.variants[group].declarations = match position {
          Some(position) => declarations.set("position", position),
          None => declarations,
        };
        last = Some(group);
      }
    }
  }

  /// Renders every rule after `prefix`: the base rules, the variants in order, then
  /// `trailer`.
  fn finish(self, mut out: String, trailer: &str) -> Compiled {
    for group in self.base.iter().chain(&self.variants) {
      if group.declarations.is_empty() {
        continue;
      }

      group
        .conjunction
        .emit(&mut out, None, group.pseudo, &group.declarations.render());
    }

    out.push_str(trailer);

    Compiled {
      template: out,
      slots: self.slots,
    }
  }
}

/// Feeds a static box style's properties to the rules.
struct BoxRules<'a>(&'a mut Rules);

impl BoxVisitor<f32, Static<Condition<f32>>, WebStyle> for BoxRules<'_> {
  fn value<T: IntoBoxValue<f32, WebStyle>>(
    &mut self,
    key: Key,
    value: &Option<Conditional<PlatformLeaf<T>, Condition<f32>>>,
  ) {
    if let Some(value) = value {
      self.0.property(key, value, &|fixed: T| {
        render_box_value(&fixed.into_box_value())
      });
    }
  }
}

/// Feeds a static run style's properties to the rules.
struct RunRules<'a>(&'a mut Rules);

impl RunVisitor<Static<Condition<f32>>, WebStyle> for RunRules<'_> {
  fn value<T: IntoRunValue<WebStyle>>(
    &mut self,
    key: Key,
    value: &Option<Conditional<PlatformLeaf<T>, Condition<f32>>>,
  ) {
    if let Some(value) = value {
      self.0.property(key, value, &|fixed: T| {
        vec![render_run_value(&fixed.into_run_value())]
      });
    }
  }
}

/// Renders a box keyframe as declarations.
struct KeyframeBox(Declarations);

impl BoxVisitor<f32, Keyframe, WebStyle> for KeyframeBox {
  fn value<T: IntoBoxValue<f32, WebStyle>>(&mut self, key: Key, value: &Option<StaticLeaf<T>>) {
    if let Some(leaf) = value {
      let target = target(key);
      let texts = static_leaf_texts(leaf, target.declared().len(), |fixed: T| {
        render_box_value(&fixed.into_box_value())
      });
      let declarations = target.declare(std::mem::take(&mut self.0), &texts);

      self.0 = match target.position {
        Some(position) => declarations.set("position", position),
        None => declarations,
      };
    }
  }
}

/// Renders a run keyframe as declarations.
struct KeyframeRun(Declarations);

impl RunVisitor<Keyframe, WebStyle> for KeyframeRun {
  fn value<T: IntoRunValue<WebStyle>>(&mut self, key: Key, value: &Option<StaticLeaf<T>>) {
    if let Some(leaf) = value {
      let target = target(key);
      let texts = static_leaf_texts(leaf, target.declared().len(), |fixed: T| {
        vec![render_run_value(&fixed.into_run_value())]
      });

      self.0 = target.declare(std::mem::take(&mut self.0), &texts);
    }
  }
}

fn box_keyframe_declarations(style: &BoxStyle<f32, Keyframe, WebStyle>) -> Declarations {
  let mut keyframe = KeyframeBox(Declarations::new());

  style.visit(&mut keyframe);

  keyframe.0
}

fn run_keyframe_declarations(style: &RunStyle<Keyframe, WebStyle>) -> Declarations {
  let mut keyframe = KeyframeRun(Declarations::new());

  style.visit(&mut keyframe);

  keyframe.0
}

/// Folds the motion into `base` and appends the `@keyframes` blocks to `out`; the
/// motion respecting reduced motion goes into the returned trailer, a rule on the
/// class under the no-preference media query, and under the reduce query each
/// animation respecting it rests on its final frame, so a reader preferring reduced
/// motion sees only the motion that opted out moving.
fn motion<S>(
  out: &mut String,
  base: Declarations,
  transitions: &[skene_foundation::Transition<WebProperty>],
  animations: &[Animation<S>],
  declarations: &impl Fn(&S) -> Declarations,
) -> (Declarations, String) {
  let mut base = base;
  let mut respecting = Declarations::new();
  let mut resting = Declarations::new();
  let insistent =
    |transition: &skene_foundation::Transition<WebProperty>| !transition.respects_reduced_motion;

  if !transitions.is_empty() {
    if transitions.iter().any(&insistent) {
      base = base.set(
        "transition",
        render_transitions(
          transitions
            .iter()
            .filter(|transition| insistent(transition)),
        ),
      );
    }

    if !transitions.iter().all(&insistent) {
      respecting = respecting.set("transition", render_transitions(transitions));
    }
  }

  if !animations.is_empty() {
    let names: Vec<String> = animations
      .iter()
      .map(|animation| render_keyframes(out, animation, declarations))
      .collect();
    let entries = || animations.iter().zip(names.iter().map(String::as_str));
    let insistent = |(animation, _): &(&Animation<S>, &str)| !animation.respects_reduced_motion;

    if entries().any(|entry| insistent(&entry)) {
      base = base.set(
        "animation",
        render_animation_values(entries().filter(insistent), |_| false),
      );
    }

    if !entries().all(|entry| insistent(&entry)) {
      respecting = respecting.set("animation", render_animation_values(entries(), |_| false));
      resting = resting.set(
        "animation",
        render_animation_values(entries(), |animation| animation.respects_reduced_motion),
      );
    }
  }

  let mut trailer = String::new();

  if !respecting.is_empty() {
    let _ = write!(
      trailer,
      "@media {NO_PREFERENCE}{{.{CLASS_PLACEHOLDER}{{{}}}}}",
      respecting.render()
    );
  }

  if !resting.is_empty() {
    let _ = write!(
      trailer,
      "@media {REDUCE}{{.{CLASS_PLACEHOLDER}{{{}}}}}",
      resting.render()
    );
  }

  (base, trailer)
}

/// The static display declarations for a [`Layout`].
fn layout_declarations(layout: Layout) -> Declarations {
  match layout {
    Layout::Document => Declarations::new().set("display", "flow-root"),
    Layout::Flex { axis, reversed } => Declarations::new().set("display", "flex").set(
      "flex-direction",
      match (axis, reversed) {
        (Axis::Horizontal, false) => "row",
        (Axis::Vertical, false) => "column",
        (Axis::Horizontal, true) => "row-reverse",
        (Axis::Vertical, true) => "column-reverse",
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

/// The base declarations a box starts from: its layout and its identity.
fn box_base<P: PixelPlatform>(props: &BoxProps<P>) -> Declarations {
  box_identity(props, layout_declarations(props.layout))
}

/// `base` with what names a box: when it names itself for container queries, its
/// containment—`size` when its height is set, else `inline-size`—and its
/// view-transition name.
fn box_identity<P: PixelPlatform>(props: &BoxProps<P>, mut base: Declarations) -> Declarations {
  if let Some(name) = props.container {
    let containment = if props.style.height.is_some() {
      "size"
    }
    else {
      "inline-size"
    };

    base = base.set("container", format!("{} / {containment}", name.0));
  }

  if let Some(name) = &props.transition_name {
    base = base.set("view-transition-name", Ident(name).to_string());
  }

  base
}

/// Compiles a box's props with `base` ahead of its own declarations.
fn compile_box_with<P: PixelPlatform>(props: &BoxProps<P>, base: Declarations) -> Compiled {
  let mut out = String::new();
  let (base, trailer) = motion(
    &mut out,
    base,
    &props.dynamics.transitions,
    &props.dynamics.animations,
    &box_keyframe_declarations,
  );
  let mut rules = Rules::new(base);

  props.style.visit(&mut BoxRules(&mut rules));

  rules.finish(out, &trailer)
}

/// The rule template of a box; a stack's places every child in its one cell.
pub(crate) fn compile_box<P: PixelPlatform>(props: &BoxProps<P>) -> Compiled {
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

/// The rule template of the per-child layout a wrapper gave a low-level node.
pub(crate) fn compile_layout<P: PixelPlatform>(layout: &StaticBoxStyle<P>) -> Compiled {
  let mut rules = Rules::new(Declarations::new());

  layout.visit(&mut BoxRules(&mut rules));

  rules.finish(String::new(), "")
}

/// The rule template of a control: its box's, over the control reset that strips the
/// native element's own chrome; a text entry's value is inked and selected by the
/// typography inherited at it.
pub(crate) fn compile_control<P: PixelPlatform>(props: &ControlProps<P>) -> Compiled {
  let entry = props.native.is_some_and(NativeElement::is_text_entry);
  let ink = if entry {
    token_reference(typography::INK.name())
  }
  else {
    "inherit".to_owned()
  };
  let mut compiled = compile_box_with(
    &props.box_props,
    box_base(&props.box_props)
      .set("appearance", "none")
      .set("font", "inherit")
      .set("color", ink)
      .set("text-align", "inherit"),
  );

  if entry {
    Conjunction::default().emit(
      &mut compiled.template,
      None,
      "::selection",
      &Declarations::new()
        .set(
          "background-color",
          token_reference(typography::SELECTION_SURFACE.name()),
        )
        .set("color", token_reference(typography::SELECTION_INK.name()))
        .render(),
    );
  }

  compiled
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
/// placement margins, its layout applying only while open so the platform's
/// `display: none` stands while closed, and the `::backdrop` rule of a modal with a
/// backdrop.
pub(crate) fn compile_overlay<P: PixelPlatform>(props: &OverlayProps<P>) -> Compiled {
  let base = box_identity(&props.box_props, Declarations::new())
    .set("border", "0 solid")
    .set("padding", "0")
    .set("background", "none")
    .set("color", "inherit")
    .set("overflow", "visible")
    .set("margin", placement_margin(props.placement));
  let mut compiled = compile_box_with(&props.box_props, base);

  Conjunction::default().emit(
    &mut compiled.template,
    None,
    ":where([open],:popover-open)",
    &layout_declarations(props.box_props.layout).render(),
  );

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

/// The rule template of a text run; a run on a line of its own lays out one line while
/// empty, through a rule of its class's specificity that precedes the class's own.
pub(crate) fn compile_run<P: PixelPlatform>(props: &RunProps<P>) -> Compiled {
  let mut base = match props.placement {
    RunPlacement::Line => Declarations::new().set("display", "block"),
    RunPlacement::Inline => Declarations::new(),
    RunPlacement::Embedded => Declarations::new().set("display", "inline-block"),
  };

  if props.selectable {
    base = base
      .set("-webkit-user-select", "text")
      .set("user-select", "text");
  }

  let mut out = String::new();
  let (base, trailer) = motion(
    &mut out,
    base,
    &props.dynamics.transitions,
    &props.dynamics.animations,
    &run_keyframe_declarations,
  );

  if props.placement == RunPlacement::Line {
    Conjunction::default().emit(
      &mut out,
      None,
      ":where(:empty)",
      &Declarations::new()
        .set("min-block-size", "var(--skene-line)")
        .render(),
    );
  }

  let mut rules = Rules::new(base);

  props.style.visit(&mut RunRules(&mut rules));

  if let Some(layout) = &props.layout {
    layout.visit(&mut BoxRules(&mut rules));
  }

  rules.finish(out, &trailer)
}

/// The text of a token definition's leaf.
fn definition_leaf(value: &WebTokenValue, position: usize) -> Option<String> {
  fn leaf<T: Clone>(
    definition: &skene_foundation::Definition<T, f32>,
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
      StaticLeaf::Token(name) => token_reference(*name),
      StaticLeaf::Expr(expr) => render_expr(expr.node()),
    })
  }

  match value {
    WebTokenValue::Color(definition) => leaf(definition, position, Declarations::render_color),
    WebTokenValue::Number(definition) => leaf(definition, position, |number| number.to_string()),
    WebTokenValue::Length(definition) => {
      leaf(definition, position, |length| format!("{}px", length.0))
    }
    WebTokenValue::Dimension(definition) => {
      leaf(definition, position, Declarations::render_dimension)
    }
  }
}

/// The media conditions of a definition's variants, in order.
fn definition_conditions(value: &WebTokenValue) -> Vec<skene_foundation::Media<f32>> {
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
    WebTokenValue::Length(definition) => definition
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

/// The `@property` rules registering the slot of `key` at `position`, one per custom
/// property it is written to; none inherits.
pub(crate) fn slot_registration(key: Key, position: usize) -> String {
  slot_properties(key, position)
    .into_iter()
    .fold(String::new(), |mut out, property| {
      let _ = write!(out, "@property {property}{{syntax:\"*\";inherits:false}}");

      out
    })
}

/// The `@property` registrations of every property's base slot, appended to the reset;
/// a variant's slot registers once a template uses it.
pub(crate) fn slot_registrations() -> String {
  struct Keys(Vec<Key>);

  impl BoxVisitor<f32, Plain, WebStyle> for Keys {
    fn value<T: IntoBoxValue<f32, WebStyle>>(&mut self, key: Key, _: &Option<T>) {
      self.0.push(key);
    }
  }

  impl RunVisitor<Plain, WebStyle> for Keys {
    fn value<T: IntoRunValue<WebStyle>>(&mut self, key: Key, _: &Option<T>) {
      self.0.push(key);
    }
  }

  let mut keys = Keys(Vec::new());

  BoxStyle::<f32, Plain, WebStyle>::default().visit(&mut keys);
  RunStyle::<Plain, WebStyle>::default().visit(&mut keys);
  keys.0.dedup();

  let mut out = String::new();

  for key in keys.0 {
    out.push_str(&slot_registration(key, 0));
  }

  out
}

/// The hash the class cache keys a box's compiled template on.
pub(crate) fn box_fingerprint<P: PixelPlatform>(props: &BoxProps<P>, reset: u8) -> u64 {
  let mut hasher = FxHasher::default();

  hasher.write_u8(reset);
  props.layout.hash(&mut hasher);
  props.container.hash(&mut hasher);
  props.transition_name.hash(&mut hasher);
  props.style.fingerprint(&mut hasher);
  props.dynamics.fingerprint(&mut hasher);

  hasher.finish()
}

/// The hash the class cache keys a control's compiled template on: its box's and its
/// kind's.
pub(crate) fn control_fingerprint<P: PixelPlatform>(props: &ControlProps<P>) -> u64 {
  let mut hasher = FxHasher::default();

  hasher.write_u64(box_fingerprint(&props.box_props, 1));
  props.native.hash(&mut hasher);

  hasher.finish()
}

/// The hash the class cache keys an overlay's compiled template on.
pub(crate) fn overlay_fingerprint<P: PixelPlatform>(props: &OverlayProps<P>) -> u64 {
  let mut hasher = FxHasher::default();

  hasher.write_u64(box_fingerprint(&props.box_props, 2));
  props.modal.hash(&mut hasher);
  (props.placement as u8).hash(&mut hasher);
  props.backdrop.fingerprint(&mut hasher);

  hasher.finish()
}

/// The hash the class cache keys a low-level node's compiled layout on.
pub(crate) fn layout_fingerprint<P: PixelPlatform>(layout: &StaticBoxStyle<P>) -> u64 {
  let mut hasher = FxHasher::default();

  hasher.write_u8(3);
  layout.fingerprint(&mut hasher);

  hasher.finish()
}

/// The hash the class cache keys a run's compiled template on.
pub(crate) fn run_fingerprint<P: PixelPlatform>(props: &RunProps<P>) -> u64 {
  let mut hasher = FxHasher::default();

  (props.placement as u8).hash(&mut hasher);
  props.selectable.hash(&mut hasher);
  props.style.fingerprint(&mut hasher);
  props.layout.is_some().hash(&mut hasher);

  if let Some(layout) = &props.layout {
    layout.fingerprint(&mut hasher);
  }

  props.dynamics.fingerprint(&mut hasher);

  hasher.finish()
}
