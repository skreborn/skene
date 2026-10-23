//! The one resolution of a node's style: the terminal environment and the node's
//! interaction feed the shared [`Resolver`], and the terminal reads the result back
//! into its own vocabulary.
//!
//! In a debug build every box, run and keyframe style also resolves through the node's
//! [`Record`](skene_foundation::Record), held equal to the typed resolution before the
//! terminal's own fill-ins.

use skene_foundation::{
  BoxStyle,
  Color,
  Condition,
  Conditional,
  Frame,
  KeptLeaf,
  Keyframe,
  ResolutionContext,
  Resolver,
  RunDynamics,
  RunStyle,
  Scene,
  Static,
  StaticLeaf,
  typography,
};

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
  fn with_resolver<R>(&self, operation: impl FnOnce(&Resolver<'_, u32, TerminalStyle>) -> R) -> R {
    self.context.with_resolver(operation)
  }

  /// A box keyframe with every leaf resolved at the node.
  pub(crate) fn resolve_box_keyframe(
    &self,
    keyframe: &BoxStyle<u32, Keyframe, TerminalStyle>,
  ) -> PlainBox {
    self.with_resolver(|resolver| {
      let style = resolver.box_keyframe(keyframe);

      if cfg!(debug_assertions) {
        record::hold(
          "box keyframe",
          keyframe,
          &style,
          &Frame::of_box(keyframe).resolve(resolver).boxed,
        );
      }

      style
    })
  }

  /// A run keyframe with every leaf resolved at the node.
  pub(crate) fn resolve_run_keyframe(
    &self,
    keyframe: &RunStyle<Keyframe, TerminalStyle>,
  ) -> PlainRun {
    self.with_resolver(|resolver| {
      let style = resolver.run_keyframe(keyframe);

      if cfg!(debug_assertions) {
        record::hold(
          "run keyframe",
          keyframe,
          &style,
          &Frame::of_run(keyframe).resolve(resolver).run,
        );
      }

      style
    })
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
    let record = self.records.record(self.handle)?;
    let props = &record.kind.box_record()?.props;
    let source: &BoxStyle<u32, Static<Condition<u32>>, TerminalStyle> = &props.style;
    let mut style = self.with_resolver(|resolver| {
      let style = resolver.box_style(source);

      if cfg!(debug_assertions) {
        record::hold(
          "box",
          source,
          &style,
          &record::of_box(source, &props.dynamics)
            .resolve(resolver)
            .boxed,
        );
      }

      style
    });

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
    let token = |token| Some(Conditional::new(KeptLeaf::Static(StaticLeaf::from(token))));
    let source = RunStyle::<Static<Condition<u32>>, TerminalStyle> {
      color: token(typography::INK),
      selection_color: token(typography::SELECTION_INK),
      selection_background: token(typography::SELECTION_SURFACE),
      ..RunStyle::default()
    };
    let mut style = self.with_resolver(|resolver| {
      let mut style = resolver.run_style(&source);

      if cfg!(debug_assertions) {
        record::hold(
          "entry run",
          &source,
          &style,
          &record::of_run(&source, &RunDynamics::default())
            .resolve(resolver)
            .run,
        );
      }

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
    let Kind::Run { props, .. } = &record.kind
    else {
      return None;
    };
    let mut style = self.with_resolver(|resolver| {
      let mut style = resolver.run_style(&props.style);

      if cfg!(debug_assertions) {
        record::hold(
          "run",
          &props.style,
          &style,
          &record::of_run(&props.style, &props.dynamics)
            .resolve(resolver)
            .run,
        );
      }

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

/// The record path of the resolution, held to the typed one in a debug build.
mod record {
  use std::{fmt::Debug, hash::Hasher};

  use rustc_hash::FxHasher;
  use skene_foundation::{
    BoxDynamics,
    BoxStyle,
    Condition,
    Fingerprint,
    Frame,
    Record,
    RunDynamics,
    RunStyle,
    Static,
  };

  use crate::TerminalStyle;

  /// The record of a box's `style` and `dynamics`, its motion held to reading back as
  /// `dynamics`.
  pub(super) fn of_box(
    style: &BoxStyle<u32, Static<Condition<u32>>, TerminalStyle>,
    dynamics: &BoxDynamics<u32, TerminalStyle>,
  ) -> Record<u32, TerminalStyle> {
    let mut record = Record::new();

    record.add_box(style);
    record.add_box_dynamics(dynamics);
    hold(
      "box's motion",
      dynamics,
      dynamics,
      &BoxDynamics {
        transitions: record.transitions().to_vec(),
        animations: record
          .animations()
          .iter()
          .map(|animation| animation.map_keyframes(Frame::box_keyframe))
          .collect(),
      },
    );

    record
  }

  /// The record of a run's `style` and `dynamics`, its motion held to reading back as
  /// `dynamics`.
  pub(super) fn of_run(
    style: &RunStyle<Static<Condition<u32>>, TerminalStyle>,
    dynamics: &RunDynamics<TerminalStyle>,
  ) -> Record<u32, TerminalStyle> {
    let mut record = Record::new();

    record.add_run(style);
    record.add_run_dynamics(dynamics);
    hold(
      "run's motion",
      dynamics,
      dynamics,
      &RunDynamics {
        transitions: record.transitions().to_vec(),
        animations: record
          .animations()
          .iter()
          .map(|animation| animation.map_keyframes(Frame::run_keyframe))
          .collect(),
      },
    );

    record
  }

  /// Holds what a `kind` style `source` resolved to through its record to what it
  /// resolved to typed, by their fingerprints, which compare lengths by their bits.
  pub(super) fn hold<T: Fingerprint + Debug>(
    kind: &str,
    source: &impl Debug,
    typed: &T,
    record: &T,
  ) {
    debug_assert_eq!(
      fingerprint(record),
      fingerprint(typed),
      "the {kind} {source:?} resolves through its record to {record:?}, typed to {typed:?}"
    );
  }

  /// A value's fingerprint.
  fn fingerprint(value: &impl Fingerprint) -> u64 {
    let mut hasher = FxHasher::default();

    value.fingerprint(&mut hasher);

    hasher.finish()
  }
}
