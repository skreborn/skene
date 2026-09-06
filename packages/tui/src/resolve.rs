//! The one resolution of a node's style: the terminal environment and the node's
//! interaction feed the shared [`Resolver`], and the terminal reads the result back
//! into its own vocabulary.

use skene_core::{
  Annotation,
  BoxStyle,
  Color,
  ColorScheme,
  Condition,
  ContainerName,
  Interaction,
  Keyframe,
  NodeId,
  Resolver,
  RunStyle,
  Scene,
  Size,
  Static,
};

use crate::{
  BOLD_WEIGHT,
  BorderGlyphs,
  TerminalExt,
  Tui,
  store::{Handle, Kind, PlainBox, PlainRun, Store},
};

/// The font size expressions in `em` evaluate against: one cell.
const EM: f32 = 1.;

/// What one node's properties resolve against: the environment, the node's
/// interaction, and the named containers above it.
pub(crate) struct Env<'a> {
  store: &'a Store,
  handle: Handle,
  node: Option<NodeId>,
  viewport: Size<u32>,
  scheme: ColorScheme,
  reduced_motion: bool,
  interaction: Interaction,
  containers: Vec<(ContainerName, Option<Size<u32>>)>,
}

impl<'a> Env<'a> {
  /// The environment of `handle` in `store`.
  pub(crate) fn of(store: &'a Store, handle: Handle) -> Self {
    Self {
      store,
      handle,
      node: store.record(handle).and_then(|record| record.node),
      viewport: store.viewport(),
      scheme: store.scheme(),
      reduced_motion: store.reduced_motion,
      interaction: store.interaction(handle),
      containers: store.containers_above(handle),
    }
  }

  /// Runs `operation` with the resolver of this environment, reading tokens through
  /// the scene and slots through the store.
  fn with_resolver<R>(&self, operation: impl FnOnce(&Resolver<'_, u32, TerminalExt>) -> R) -> R {
    let node = self.node;
    let token = move |name| Scene::<Tui>::with(|scene| scene.resolve_token(node?, name));
    let slot = |slot| self.store.record(self.handle)?.slots.get(&slot).cloned();

    operation(&Resolver {
      viewport: self.viewport,
      scheme: self.scheme,
      reduced_motion: self.reduced_motion,
      interaction: self.interaction,
      containers: &self.containers,
      em: EM,
      token: &token,
      slot: &slot,
    })
  }

  /// A box keyframe with every leaf resolved at the node.
  pub(crate) fn resolve_box_keyframe(
    &self,
    keyframe: &BoxStyle<u32, Keyframe, TerminalExt>,
  ) -> PlainBox {
    self.with_resolver(|resolver| resolver.box_keyframe(keyframe))
  }

  /// A run keyframe with every leaf resolved at the node.
  pub(crate) fn resolve_run_keyframe(
    &self,
    keyframe: &RunStyle<Keyframe, TerminalExt>,
  ) -> PlainRun {
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

  /// The box style of the record, every property resolved; with its border's ink.
  pub(crate) fn resolve_box(&self) -> Option<(PlainBox, Option<Color>)> {
    let record = self.store.record(self.handle)?;
    let source: &BoxStyle<u32, Static<Condition<u32>>, TerminalExt> =
      &record.kind.box_record()?.props.style;
    let (mut style, border_ink) = self.with_resolver(|resolver| {
      let style = resolver.box_style(source);
      let border_ink = style
        .border
        .as_ref()
        .and_then(|border| resolver.color_leaf(&border.color));

      (style, border_ink)
    });

    if style.ext.border_glyphs.is_none()
      && style.ext.corner_radius.is_some_and(|radius| radius > 0.)
    {
      style.ext.border_glyphs = Some(BorderGlyphs::ROUNDED);
    }

    Some((style, border_ink))
  }

  /// The run style of the record, every property resolved.
  pub(crate) fn resolve_run(&self) -> Option<PlainRun> {
    let record = self.store.record(self.handle)?;
    let Kind::Run { props, .. } = &record.kind
    else {
      return None;
    };
    let mut style = self.with_resolver(|resolver| resolver.run_style(&props.style));

    if style.ext.bold.is_none() {
      style.ext.bold = style.ext.weight.map(|weight| weight >= BOLD_WEIGHT);
    }

    Some(style)
  }
}
