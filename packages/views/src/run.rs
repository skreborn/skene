//! The run every text view is realized through.

use skene_core::{
  Color,
  LinkPolicy,
  RunDisplay,
  next::{
    Condition,
    DeclaredRunStyle,
    Definition,
    Definitions,
    Driver,
    Leaf,
    NodeId,
    NodeKind,
    RunDynamics,
    RunProps,
    Scalar,
    Scene,
    StaticLeaf,
    TokenName,
    Value,
  },
};
use skene_reactive::Effect;

use crate::{realize::bind_run_slot, typography};

/// What every text view declares of its run: selection, link and style.
#[must_use]
pub struct Run<D: Driver> {
  selectable: bool,
  link: Option<String>,
  link_policy: LinkPolicy,
  style: DeclaredRunStyle<D>,
}

impl<D: Driver> Run<D> {
  /// An unselectable run with no link and nothing set.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      selectable: false,
      link: None,
      link_policy: LinkPolicy::new(),
      style: DeclaredRunStyle::<D>::default(),
    }
  }

  /// The declared style, for a view to write its properties into.
  #[inline(always)]
  pub const fn style_mut(&mut self) -> &mut DeclaredRunStyle<D> {
    &mut self.style
  }

  /// Sets whether the reader can select and copy the text.
  #[inline(always)]
  pub const fn set_selectable(mut self, selectable: bool) -> Self {
    self.selectable = selectable;

    self
  }

  /// Makes the run a link to `destination`.
  #[inline(always)]
  pub fn set_link(mut self, destination: impl Into<String>) -> Self {
    self.link = Some(destination.into());

    self
  }

  /// Sets how the link opens and what it discloses.
  #[inline(always)]
  pub const fn set_link_policy(mut self, link_policy: LinkPolicy) -> Self {
    self.link_policy = link_policy;

    self
  }

  /// Realizes the run as a detached node with `display`.
  ///
  /// A property the run leaves unset references its typography token; a `group` also
  /// redefines the typography tokens of the properties it sets for the runs beneath it.
  pub fn build(self, display: RunDisplay, group: bool) -> NodeId {
    let mut style = self.style;
    let redefined = group.then(|| style.clone());
    let ink = if self.link.is_some() {
      typography::LINK_INK
    }
    else {
      typography::INK
    };

    style.color.get_or_insert_with(|| Value::from(ink));
    style
      .selection_color
      .get_or_insert_with(|| Value::from(typography::SELECTION_INK));
    style
      .selection_background
      .get_or_insert_with(|| Value::from(typography::SELECTION_SURFACE));

    let (style, slots) = style.split();
    let props = RunProps {
      display,
      selectable: self.selectable,
      link: self.link,
      link_policy: self.link_policy,
      style,
      dynamics: RunDynamics::default(),
    };
    let sink = D::default().create_run(&props);
    let node = Scene::<D>::with(|scene| scene.create(NodeKind::Run, sink));

    for slot in slots {
      bind_run_slot::<D>(node, slot);
    }

    if let Some(style) = redefined {
      redefine_typography::<D>(node, style);
    }

    node
  }
}

/// A token to redefine, and the value redefining it.
type Redefinition<N> = (TokenName, Value<Color, Condition<N>>);

/// Defines the typography tokens of the properties `style` sets on `node`: once when
/// every leaf is static, else under a render effect following the derived leaves.
fn redefine_typography<D: Driver>(node: NodeId, style: DeclaredRunStyle<D>) {
  let entries: Vec<Redefinition<D::Scalar>> = [
    (typography::INK, style.color),
    (typography::SELECTION_INK, style.selection_color),
    (typography::SELECTION_SURFACE, style.selection_background),
  ]
  .into_iter()
  .filter_map(|(token, value)| value.map(|value| (token.name(), value)))
  .collect();

  if entries.is_empty() {
    return;
  }

  let derived = entries.iter().any(|(_, value)| !value.is_static());
  let define = move || {
    entries
      .iter()
      .map(|(name, value)| (*name, D::TokenValue::from(definition_of(value))))
      .collect::<Definitions<D::TokenValue>>()
  };

  if derived {
    Effect::render(move |_: Option<()>| {
      let definitions = define();

      Scene::<D>::with(|scene| scene.define_tokens(node, definitions));
    });
  }
  else {
    Scene::<D>::with(|scene| scene.define_tokens(node, define()));
  }
}

/// The definition a value redefines a token as: its static leaves as they are, its
/// derived leaves as the value they compute now, and its media variants alone.
fn definition_of<N: Scalar>(value: &Value<Color, Condition<N>>) -> Definition<Color, N> {
  let mut definition = Definition::new(static_leaf(value.base()));

  for (condition, leaf) in value.variants() {
    if let Condition::Media(media) = condition {
      definition = definition.when(media.clone(), static_leaf(leaf));
    }
  }

  definition
}

fn static_leaf(leaf: &Leaf<Color>) -> StaticLeaf<Color> {
  match leaf {
    Leaf::Static(leaf) => leaf.clone(),
    Leaf::Derived(compute) => StaticLeaf::Fixed(compute()),
  }
}
