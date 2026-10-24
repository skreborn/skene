//! The one resolution of a node's style: the terminal environment and the node's
//! interaction feed the shared [`Resolver`], the node's style kept whole resolves
//! through the shared [`Keep::Nothing`](skene_foundation::Keep::Nothing) entry
//! ([`CompiledStyle::resolve`](skene_foundation::CompiledStyle::resolve)), and the
//! terminal fills in what its own vocabulary takes from the typography.

use skene_foundation::{Color, Frame, ResolutionContext, Resolver, Scene, StaticLeaf, typography};

use crate::{
  BOLD_WEIGHT,
  BorderGlyphs,
  Terminal,
  TerminalStyle,
  records::{Handle, Kind, PlainBox, PlainRun, Records},
};

/// What one node's properties resolve against, with the records it reads.
pub(crate) struct Env<'a> {
  /// The records the environment reads.
  records: &'a Records,
  /// The node's record.
  handle: Handle,
  /// The environment, the node's interaction and the named containers above it.
  context: ResolutionContext<'a, Terminal, Records>,
}

impl<'a> Env<'a> {
  /// The environment of `handle` in `records`.
  pub(crate) fn of(records: &'a Records, handle: Handle) -> Self {
    Self {
      records,
      handle,
      context: ResolutionContext::of(records, handle)
        .unwrap_or_else(|| ResolutionContext::root(records)),
    }
  }

  /// Whether a resolution in this environment has read the node's interaction.
  #[inline(always)]
  pub(crate) const fn reads_interaction(&self) -> bool {
    self.context.read_interaction()
  }

  /// Runs `operation` with the resolver of this environment, reading tokens where they
  /// are defined and slots through the records.
  fn with_resolver<R>(&self, operation: impl FnOnce(&Resolver<'_, i32, TerminalStyle>) -> R) -> R {
    self.context.with_resolver(operation)
  }

  /// A box keyframe of the node's style with every leaf resolved at the node.
  pub(crate) fn resolve_box_keyframe(&self, frame: &Frame<i32, TerminalStyle>) -> PlainBox {
    self.with_resolver(|resolver| frame.resolve(resolver).boxed)
  }

  /// A run keyframe of the node's style with every leaf resolved at the node.
  pub(crate) fn resolve_run_keyframe(&self, frame: &Frame<i32, TerminalStyle>) -> PlainRun {
    self.with_resolver(|resolver| frame.resolve(resolver).run)
  }

  /// The per-child layout properties a wrapper annotated the node with, resolved into
  /// an otherwise empty box style.
  pub(crate) fn resolve_run_layout(&self) -> PlainBox {
    Scene::<Terminal>::read(|scene| scene.record(self.handle)?.annotation().cloned())
      .map(|annotation| self.with_resolver(|resolver| annotation.resolve(resolver)))
      .unwrap_or_default()
  }

  /// The box style of the record, every property resolved.
  pub(crate) fn resolve_box(&self) -> Option<PlainBox> {
    let kept = &self.records.record(self.handle)?.kind.box_record()?.style;
    let mut style = self.with_resolver(|resolver| kept.resolve(resolver).boxed);

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
    let mut style = self.with_resolver(|resolver| {
      let mut style = PlainRun {
        color: resolver.keyframe_run_value(&StaticLeaf::from(typography::INK)),
        selection_color: resolver.keyframe_run_value(&StaticLeaf::from(typography::SELECTION_INK)),
        selection_background: resolver
          .keyframe_run_value(&StaticLeaf::from(typography::SELECTION_SURFACE)),
        ..PlainRun::default()
      };

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
    let record = self.records.record(self.handle)?;
    let Kind::Run { style: kept, .. } = &record.kind
    else {
      return None;
    };
    let mut style = self.with_resolver(|resolver| {
      let mut style = kept.resolve(resolver).run;

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
