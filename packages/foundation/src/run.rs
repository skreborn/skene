//! The run every text view is realized through.

use std::rc::Rc;

use skene_reactive::Effect;

use crate::{
  Animation,
  Condition,
  DeclaredRunStyle,
  Definition,
  Definitions,
  FocusOrder,
  Leaf,
  NodeId,
  NodeKind,
  Realization,
  RunDynamics,
  RunKeyframe,
  RunPlacement,
  RunProps,
  Scalar,
  Scene,
  StaticLeaf,
  StyleExtension,
  TokenName,
  TokenRef,
  Tokenable,
  Transition,
  TypeMetrics,
  Value,
  realize::{bind_box_slot, bind_run_slot},
  typography,
};

/// What every text view declares of its run: selection, link and style.
///
/// A text view holds one and reaches its surface through [`HasRun`](crate::HasRun),
/// realizing it with [`build`](Self::build) or, for a group, [`build_group`](Self::build_group).
#[must_use]
pub struct Run<P: Realization> {
  selectable: bool,
  link: Option<String>,
  style: DeclaredRunStyle<P>,
  dynamics: RunDynamics<P::StyleExt>,
  realizations: Vec<Realize<P>>,
}

type Realize<P> = Box<dyn FnOnce(NodeId, &RunProps<P>)>;

impl<P: Realization> std::fmt::Debug for Run<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Run").finish_non_exhaustive()
  }
}

impl<P: Realization> Default for Run<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Run<P> {
  /// An unselectable run with no link and nothing set.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      selectable: false,
      link: None,
      style: DeclaredRunStyle::<P>::default(),
      dynamics: RunDynamics::default(),
      realizations: Vec::new(),
    }
  }

  /// The declared style, for a view to write its properties into.
  #[inline(always)]
  pub const fn style_mut(&mut self) -> &mut DeclaredRunStyle<P> {
    &mut self.style
  }

  /// Sets whether the reader can select and copy the text.
  #[inline(always)]
  pub const fn set_selectable(mut self, selectable: bool) -> Self {
    self.selectable = selectable;

    self
  }

  /// Makes the run a link to `destination`; a destination that would run script when
  /// followed, a `javascript:` URL, is never followed.
  pub fn set_link(mut self, destination: impl Into<String>) -> Self {
    self.link = Some(destination.into());

    self
  }

  /// Adds `realization`, called with the run's node and props once the platform has
  /// realized the run, after the realizations added before it.
  pub fn add_realization(
    mut self,
    realization: impl FnOnce(NodeId, &RunProps<P>) + 'static,
  ) -> Self {
    self.realizations.push(Box::new(realization));

    self
  }

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[inline(always)]
  pub fn add_transition(
    mut self,
    transition: Transition<<P::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    self.dynamics.transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn add_animation(mut self, animation: Animation<RunKeyframe<P>>) -> Self {
    self.dynamics.animations.push(animation);

    self
  }

  /// Realizes the run as a detached node with `display`, each property it leaves unset
  /// referencing its typography token.
  pub fn build(self, placement: RunPlacement) -> NodeId {
    self.realize(placement, Vec::new())
  }

  /// Realizes the run as a detached group with `display`: as [`build`](Self::build)
  /// does, redefining for the runs beneath it the typography tokens of the text
  /// properties it sets, its type metrics among them, and a link group setting no color
  /// the ink as the link ink.
  pub fn build_group(self, placement: RunPlacement) -> NodeId
  where
    P::StyleExt: TypeMetrics,
  {
    let style = &self.style;
    let ink = match (&style.color, &self.link) {
      (None, Some(_)) => Some(Value::from(typography::LINK_INK)),
      (color, _) => color.clone(),
    };
    let redefinitions = [
      Redefinition::of(typography::INK, ink),
      Redefinition::of(typography::SELECTION_INK, style.selection_color.clone()),
      Redefinition::of(
        typography::SELECTION_SURFACE,
        style.selection_background.clone(),
      ),
      Redefinition::of(typography::SIZE, P::StyleExt::font_size(&style.ext)),
      Redefinition::of(typography::WEIGHT, P::StyleExt::font_weight(&style.ext)),
      Redefinition::of(
        typography::LINE_HEIGHT,
        P::StyleExt::line_height(&style.ext),
      ),
      Redefinition::of(
        typography::LETTER_SPACING,
        P::StyleExt::letter_spacing(&style.ext),
      ),
    ]
    .into_iter()
    .flatten()
    .collect();

    self.realize(placement, redefinitions)
  }

  /// Realizes the run with `display`, defining `redefinitions` on it.
  fn realize(
    self,
    placement: RunPlacement,
    redefinitions: Vec<Redefinition<P::TokenValue>>,
  ) -> NodeId {
    let mut style = self.style;
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
    let links = self.link.is_some();
    let props = RunProps {
      placement,
      selectable: self.selectable,
      link: self.link,
      style,
      layout,
      dynamics: self.dynamics,
    };
    let node = Scene::<P>::with(|scene| scene.create(NodeKind::Run, annotation));

    P::create_run(node, &props);

    for realization in self.realizations {
      realization(node, &props);
    }

    if links {
      Scene::<P>::with(|scene| scene.note_focus_order(node, Some(FocusOrder::Auto)));
    }

    for slot in slots {
      bind_run_slot::<P>(node, slot);
    }

    for slot in layout_slots {
      bind_box_slot::<P>(node, slot);
    }

    Redefinition::define_all::<P>(node, redefinitions);

    node
  }
}

/// A typography token a group redefines: its name, its definition as of now, and
/// whether a derived leaf changes it.
struct Redefinition<V> {
  name: TokenName,
  define: Rc<dyn Fn() -> V>,
  derived: bool,
}

impl<V: 'static> Redefinition<V> {
  /// `token` redefined as `value`; `None` where the run leaves `value` unset.
  fn of<T: Tokenable + Clone, N: Scalar>(
    token: TokenRef<T>,
    value: Option<Value<T, Condition<N>>>,
  ) -> Option<Self>
  where
    V: From<Definition<T, N>>,
  {
    let value = value?;

    Some(Self {
      name: token.name(),
      derived: !value.is_static(),
      define: Rc::new(move || V::from(definition_of(&value))),
    })
  }

  /// Defines `redefinitions` on `node`: once when none is derived, else under a render
  /// effect following the derived leaves.
  fn define_all<P: Realization<TokenValue = V>>(node: NodeId, redefinitions: Vec<Self>) {
    if redefinitions.is_empty() {
      return;
    }

    let derived = redefinitions
      .iter()
      .any(|redefinition| redefinition.derived);
    let define = move || {
      redefinitions
        .iter()
        .map(|redefinition| (redefinition.name, (redefinition.define)()))
        .collect::<Definitions<V>>()
    };

    if derived {
      Effect::render(move |_: Option<()>| {
        let definitions = define();

        Scene::<P>::with(|scene| scene.define_tokens(node, definitions));
      });
    }
    else {
      let definitions = define();

      Scene::<P>::with(|scene| scene.define_tokens(node, definitions));
    }
  }
}

/// The definition a value redefines a token as: its static leaves as they are, its
/// derived leaves as the value they compute now, and the variants testing the
/// environment alone, compound ones included.
fn definition_of<T: Clone, N: Scalar>(value: &Value<T, Condition<N>>) -> Definition<T, N> {
  let mut definition = Definition::new(static_leaf(value.base()));

  for (condition, leaf) in value.variants() {
    if let Some(media) = condition.to_media() {
      definition = definition.when(media, static_leaf(leaf));
    }
  }

  definition
}

fn static_leaf<T: Clone>(leaf: &Leaf<T>) -> StaticLeaf<T> {
  match leaf {
    Leaf::Static(leaf) => leaf.clone(),
    Leaf::Derived(compute) => compute(),
  }
}
