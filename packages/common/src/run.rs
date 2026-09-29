//! The run every text view is realized through.

use skene_core::{
  Animation,
  Color,
  Condition,
  DeclaredRunStyle,
  Definition,
  Definitions,
  Leaf,
  LinkPolicy,
  NodeId,
  NodeKind,
  Platform,
  RunDisplay,
  RunDynamics,
  RunKeyframe,
  RunProps,
  Scalar,
  Scene,
  StaticLeaf,
  StyleExtension,
  TokenName,
  Transition,
  Value,
};
use skene_reactive::Effect;

use crate::{
  realize::{bind_box_slot, bind_run_slot},
  typography,
};

/// What every text view declares of its run: selection, link and style.
#[must_use]
pub(crate) struct Run<P: Platform> {
  selectable: bool,
  link: Option<String>,
  link_policy: LinkPolicy,
  style: DeclaredRunStyle<P>,
  dynamics: RunDynamics<P::StyleExt>,
}

impl<P: Platform> Run<P> {
  /// An unselectable run with no link and nothing set.
  #[inline(always)]
  pub(crate) fn new() -> Self {
    Self {
      selectable: false,
      link: None,
      link_policy: LinkPolicy::new(),
      style: DeclaredRunStyle::<P>::default(),
      dynamics: RunDynamics::default(),
    }
  }

  /// The declared style, for a view to write its properties into.
  #[inline(always)]
  pub(crate) const fn style_mut(&mut self) -> &mut DeclaredRunStyle<P> {
    &mut self.style
  }

  /// Sets whether the reader can select and copy the text.
  #[inline(always)]
  pub(crate) const fn set_selectable(mut self, selectable: bool) -> Self {
    self.selectable = selectable;

    self
  }

  /// Makes the run a link to `destination`; a destination that would run script when
  /// followed, a `javascript:` URL, is never followed.
  #[inline(always)]
  pub(crate) fn set_link(mut self, destination: impl Into<String>) -> Self {
    self.link = Some(destination.into());

    self
  }

  /// Sets how the link opens and what it discloses.
  #[inline(always)]
  pub(crate) const fn set_link_policy(mut self, link_policy: LinkPolicy) -> Self {
    self.link_policy = link_policy;

    self
  }

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[inline(always)]
  pub(crate) fn set_transition(
    mut self,
    transition: Transition<<P::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    self.dynamics.transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub(crate) fn set_animation(mut self, animation: Animation<RunKeyframe<P>>) -> Self {
    self.dynamics.animations.push(animation);

    self
  }

  /// Realizes the run as a detached node with `display`.
  ///
  /// A property the run leaves unset references its typography token; a `group` also
  /// redefines the typography tokens of the properties it sets for the runs beneath it.
  pub(crate) fn build(self, display: RunDisplay, group: bool) -> NodeId {
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
    let annotation = Scene::<P>::with(Scene::take_pending_annotation);
    let (layout, layout_slots) = annotation.clone().map_or_else(
      || (None, Vec::new()),
      |annotation| {
        let (layout, layout_slots) = annotation.into_style().split_after(slots.len());

        (Some(Box::new(layout)), layout_slots)
      },
    );
    let props = RunProps {
      display,
      selectable: self.selectable,
      link: self.link,
      link_policy: self.link_policy,
      style,
      layout,
      dynamics: self.dynamics,
    };
    let sink = P::default().create_run(&props);
    let node = Scene::<P>::with(|scene| {
      let node = scene.create(NodeKind::Run, sink);

      if let Some(annotation) = annotation {
        scene.set_annotation(node, annotation);
      }

      node
    });

    for slot in slots {
      bind_run_slot::<P>(node, slot);
    }

    for slot in layout_slots {
      bind_box_slot::<P>(node, slot);
    }

    if let Some(style) = redefined {
      redefine_typography::<P>(node, style);
    }

    node
  }
}

/// A token to redefine, and the value redefining it.
type Redefinition<N> = (TokenName, Value<Color, Condition<N>>);

/// Defines the typography tokens of the properties `style` sets on `node`: once when
/// every leaf is static, else under a render effect following the derived leaves.
fn redefine_typography<P: Platform>(node: NodeId, style: DeclaredRunStyle<P>) {
  let entries: Vec<Redefinition<P::Scalar>> = [
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
      .map(|(name, value)| (*name, P::TokenValue::from(definition_of(value))))
      .collect::<Definitions<P::TokenValue>>()
  };

  if derived {
    Effect::render(move |_: Option<()>| {
      let definitions = define();

      Scene::<P>::with(|scene| scene.define_tokens(node, definitions));
    });
  }
  else {
    Scene::<P>::with(|scene| scene.define_tokens(node, define()));
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
    Leaf::Derived(compute) => compute(),
  }
}
