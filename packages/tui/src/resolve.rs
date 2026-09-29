//! The one resolution of a node's style: the terminal environment and the node's
//! interaction feed the shared [`Resolver`], and the terminal reads the result back
//! into its own vocabulary.

use skene_common::typography;
use skene_core::{
  Annotation,
  BoxStyle,
  Color,
  Condition,
  Conditional,
  ContainerName,
  Environment,
  Interaction,
  Keyframe,
  NodeId,
  PlatformLeaf,
  Resolved,
  Resolver,
  RunStyle,
  Scene,
  Size,
  Static,
  StaticLeaf,
  TokenName,
};

use crate::{
  BOLD_WEIGHT,
  BorderGlyphs,
  TokenValue,
  Tui,
  TuiExt,
  store::{Handle, Kind, PlainBox, PlainRun, Store},
  text,
};

/// The font size an `em` evaluates against where no typography defines one, or where the
/// terminal draws every size at one cell: one cell.
const EM: f32 = 1.;

/// What one node's properties resolve against: the environment, the node's
/// interaction, and the named containers above it.
pub(crate) struct Env<'a> {
  /// The records the environment reads.
  store: &'a Store,
  /// The node's record.
  handle: Handle,
  /// The scene node the record realizes, or `None` for the root.
  node: Option<NodeId>,
  /// The reader's environment.
  environment: Environment<u32>,
  /// The node's interaction.
  interaction: Interaction,
  /// The named containers above the node, nearest first, with their sizes.
  containers: Vec<(ContainerName, Option<Size<u32>>)>,
}

impl<'a> Env<'a> {
  /// The environment of `handle` in `store`.
  pub(crate) fn of(store: &'a Store, handle: Handle) -> Self {
    Self {
      store,
      handle,
      node: store.record(handle).and_then(|record| record.node),
      environment: store.environment(),
      interaction: store.interaction(handle),
      containers: store.containers_above(handle),
    }
  }

  /// The environment a token's definition resolves in: the defining node's, else the
  /// root's.
  fn of_scope(store: &'a Store, scope: Option<NodeId>) -> Option<Self> {
    let handle = match scope {
      Some(node) => Scene::<Tui>::with(|scene| scene.record(node).map(|record| *record.sink()))?,
      None => store.root,
    };

    Some(Self::of(store, handle))
  }

  /// The definition of `name` in force here, with the node defining it.
  fn scoped_token(&self, name: TokenName) -> Option<(Option<NodeId>, TokenValue)> {
    match self.node {
      Some(node) => Scene::<Tui>::with(|scene| scene.resolve_scoped_token(node, name)),
      None => Scene::<Tui>::with(|scene| scene.root_definitions().get(name).cloned())
        .map(|value| (None, value)),
    }
  }

  /// The value of the token `name` in force here, resolved where it is defined.
  fn token(&self, name: TokenName) -> Option<Resolved<u32>> {
    Resolver::<u32, TuiExt>::nested(|| {
      let (scope, definition) = self.scoped_token(name)?;

      Self::of_scope(self.store, scope)?.with_resolver(|resolver| resolver.definition(&definition))
    })
  }

  /// The font size in force here, in cells: the typography's size, resolved where it
  /// is defined, where the terminal proves text sizing, else [`EM`].
  fn font_size(&self) -> f32 {
    if !text::sizing() {
      return EM;
    }

    Resolver::<u32, TuiExt>::nested(|| {
      let (scope, definition) = self.scoped_token(typography::SIZE.name())?;

      Self::of_scope(self.store, scope)?
        .with_resolver_at(EM, |resolver| resolver.definition(&definition))
    })
    .and_then(|resolved| match resolved {
      Resolved::Length(scale) => Some(scale * EM),
      _ => None,
    })
    .unwrap_or(EM)
  }

  /// Runs `operation` with the resolver of this environment, reading tokens where they
  /// are defined and slots through the store.
  fn with_resolver<R>(&self, operation: impl FnOnce(&Resolver<'_, u32, TuiExt>) -> R) -> R {
    self.with_resolver_at(self.font_size(), operation)
  }

  /// Runs `operation` with the resolver of this environment, an `em` evaluating against
  /// `em`.
  fn with_resolver_at<R>(
    &self,
    em: f32,
    operation: impl FnOnce(&Resolver<'_, u32, TuiExt>) -> R,
  ) -> R {
    let token = |name| self.token(name);
    let slot = |slot| self.store.record(self.handle)?.slots.get(&slot).cloned();

    operation(&Resolver {
      environment: self.environment,
      interaction: self.interaction,
      containers: &self.containers,
      em,
      token: &token,
      slot: &slot,
    })
  }

  /// A box keyframe with every leaf resolved at the node.
  pub(crate) fn resolve_box_keyframe(
    &self,
    keyframe: &BoxStyle<u32, Keyframe, TuiExt>,
  ) -> PlainBox {
    self.with_resolver(|resolver| resolver.box_keyframe(keyframe))
  }

  /// A run keyframe with every leaf resolved at the node.
  pub(crate) fn resolve_run_keyframe(&self, keyframe: &RunStyle<Keyframe, TuiExt>) -> PlainRun {
    self.with_resolver(|resolver| resolver.run_keyframe(keyframe))
  }

  /// The per-child layout properties a wrapper annotated the node with, resolved into
  /// an otherwise empty box style.
  pub(crate) fn resolve_run_layout(&self) -> PlainBox {
    let mut layout = PlainBox::default();
    let Some(annotation) = self
      .node
      .and_then(|node| Scene::<Tui>::with(|scene| scene.record(node)?.annotation().cloned()))
    else {
      return layout;
    };
    let Annotation {
      flex,
      self_alignment,
      self_horizontal_alignment,
      pinned,
      grid_row_placement,
      grid_column_placement,
    } = annotation;

    self.with_resolver(|resolver| {
      layout.flex = flex
        .as_ref()
        .and_then(|value| resolver.declared_value(value));
      layout.self_alignment = self_alignment
        .as_ref()
        .and_then(|value| resolver.declared_value(value));
      layout.self_horizontal_alignment = self_horizontal_alignment
        .as_ref()
        .and_then(|value| resolver.declared_value(value));
      layout.pinned = pinned
        .as_ref()
        .and_then(|value| resolver.declared_value(value));
      layout.grid_row_placement = grid_row_placement
        .as_ref()
        .and_then(|value| resolver.declared_value(value));
      layout.grid_column_placement = grid_column_placement
        .as_ref()
        .and_then(|value| resolver.declared_value(value));
    });

    layout
  }

  /// The box style of the record, every property resolved.
  pub(crate) fn resolve_box(&self) -> Option<PlainBox> {
    let record = self.store.record(self.handle)?;
    let source: &BoxStyle<u32, Static<Condition<u32>>, TuiExt> =
      &record.kind.box_record()?.props.style;
    let mut style = self.with_resolver(|resolver| resolver.box_style(source));

    if style.ext.border_glyphs.is_none()
      && style.ext.corner_radius.is_some_and(|radius| radius.0 > 0.)
    {
      style.ext.border_glyphs = Some(BorderGlyphs::ROUNDED);
    }

    Some(style)
  }

  /// The run style a text entry's value paints with: the typography inherited at the
  /// entry, every property resolved.
  pub(crate) fn resolve_entry_run(&self) -> PlainRun {
    let token = |token| {
      Some(Conditional::new(PlatformLeaf::Static(StaticLeaf::from(
        token,
      ))))
    };
    let style = RunStyle::<Static<Condition<u32>>, TuiExt> {
      color: token(typography::INK),
      selection_color: token(typography::SELECTION_INK),
      selection_background: token(typography::SELECTION_SURFACE),
      ..RunStyle::default()
    };
    let mut style = self.with_resolver(|resolver| {
      let mut style = resolver.run_style(&style);

      style.ext.weight = resolver.keyframe_run_value(&StaticLeaf::from(typography::WEIGHT));

      style
    });

    style.ext.bold = style.ext.weight.map(|weight| weight >= BOLD_WEIGHT);

    style
  }

  /// The colors a link paints in while its focus shows: the typography's focused-link
  /// ink and surface in force at the run, each `None` where unset or transparent.
  pub(crate) fn resolve_link_focus(&self) -> (Option<Color>, Option<Color>) {
    self.with_resolver(|resolver| {
      (
        resolver.color_leaf(&StaticLeaf::from(typography::LINK_FOCUS_INK)),
        resolver.color_leaf(&StaticLeaf::from(typography::LINK_FOCUS_SURFACE)),
      )
    })
  }

  /// The run style of the record, every property resolved.
  pub(crate) fn resolve_run(&self) -> Option<PlainRun> {
    let record = self.store.record(self.handle)?;
    let Kind::Run { props, .. } = &record.kind
    else {
      return None;
    };
    let mut style = self.with_resolver(|resolver| {
      let mut style = resolver.run_style(&props.style);

      // A size or weight the run leaves unset is the typography's.
      if style.ext.size.is_none() {
        style.ext.size = resolver.keyframe_run_value(&StaticLeaf::from(typography::SIZE));
      }

      if style.ext.weight.is_none() {
        style.ext.weight = resolver.keyframe_run_value(&StaticLeaf::from(typography::WEIGHT));
      }

      style
    });

    if style.ext.bold.is_none() {
      style.ext.bold = style.ext.weight.map(|weight| weight >= BOLD_WEIGHT);
    }

    Some(style)
  }
}
