//! The presentation pass: the nodes whose style may have moved, each resolved where its
//! resolution is stale, presented over its motion at one reading of the frame clock, and
//! cached under what it resolved against.

use rustc_hash::FxHashSet;
use skene_foundation::{Animation, Interaction, NodeId, Scene};

use super::{
  BoxCache,
  BoxRecord,
  Drawn,
  Env,
  Kind,
  PlainBox,
  PlainRun,
  Presented,
  RunCache,
  Runtime,
  Sample,
};
use crate::Revisit;

/// What one node presents under in a pass: the generation and the interaction its
/// resolution is cached as, and the clock reading its motion presents at.
struct Pass {
  generation: u64,
  interaction: Interaction,
  sample: Sample,
}

/// What a node presents: the resolved box or run target, the wrapper layout of a run,
/// the keyframes of each, and whether the resolution read the node's interaction,
/// `None` where nothing resolved.
struct Targets<P: Drawn> {
  resolved_box: Option<PlainBox<P>>,
  resolved_run: Option<PlainRun<P>>,
  resolved_layout: PlainBox<P>,
  box_keyframes: Vec<Animation<PlainBox<P>>>,
  run_keyframes: Vec<Animation<PlainRun<P>>>,
  reads_interaction: Option<bool>,
}

impl<P: Drawn> Runtime<P> {
  /// Presents every node whose style may have moved, as
  /// [`write_styles`](Self::write_styles) states, and returns each such node holding a
  /// record, displayed or not, for the layout tree to take.
  pub(super) fn present(&mut self) -> Vec<NodeId> {
    let (mut nodes, displayed) = self.revisited();
    let generation = self.restyle.generation();
    let sample = Sample {
      now: self.now(),
      reduced: self.window().reported().reduced_motion,
    };

    self.motion_active = false;

    nodes.retain(|node| {
      let shown = match &displayed {
        Some(displayed) => displayed.contains(node),
        None => self.record(*node).is_some_and(|record| !record.hidden),
      };

      self.present_one(*node, shown, generation, sample).is_some()
    });

    nodes
  }

  /// The nodes the next pass revisits, with the display where it revisits every node.
  fn revisited(&mut self) -> (Vec<NodeId>, Option<FxHashSet<NodeId>>) {
    let interaction = Scene::<P>::read(Scene::interaction_generation);
    match self.restyle.take(interaction) {
      Revisit::Every => (self.handles(), Some(self.displayed())),
      Revisit::Nodes(mut nodes, moved) => {
        if moved {
          nodes.extend(
            self
              .restyle
              .interactive()
              .filter(|node| self.interaction_moved(*node)),
          );
        }

        let paragraphs: Vec<NodeId> = nodes
          .iter()
          .filter(|node| self.is_inline_run(**node))
          .filter_map(|node| self.enclosing_paragraph(*node))
          .collect();

        nodes.extend(paragraphs);
        nodes.sort_unstable();
        nodes.dedup();

        (nodes, None)
      }
    }
  }

  /// Presents `node` if its resolution is stale or it is in motion, where `displayed`;
  /// `None` for a node without a record, or one whose fresh resolution was never
  /// cached.
  fn present_one(
    &mut self,
    node: NodeId,
    displayed: bool,
    generation: u64,
    sample: Sample,
  ) -> Option<()> {
    let record = self.record_mut(node)?;

    if !displayed {
      if !record.hidden {
        record.hidden = true;
        record.in_motion = false;
        record.kind.forget_presentation();
        self.restyle.read_interaction(node, false);
      }

      return Some(());
    }

    record.hidden = false;

    let record = self.record(node)?;
    let interaction = self.interaction(node);
    let fresh = record.kind.resolved_under() == Some((generation, interaction));

    if fresh && !record.in_motion {
      return Some(());
    }

    let mut targets = self.targets(node, fresh)?;

    if let Some(read) = targets.reads_interaction.take() {
      self.restyle.read_interaction(node, read);
    }

    let record = self.record_mut(node)?;
    let pass = Pass {
      generation,
      interaction,
      sample,
    };
    let reach = record.kind.box_style().map(P::reach);
    // A node whose presentation came out different from the last one repaints.
    let (repaint, active) = record.kind.present(targets, &pass);

    record.in_motion = active;
    self.geometry_stale |= record.kind.box_style().map(P::reach) != reach;
    self.motion_active |= active;

    if active {
      self.restyle.keep_moving(node);
    }

    if repaint {
      self.damage(node);
    }

    Some(())
  }

  /// The targets of `node`: a fresh node in motion samples its clock over the targets
  /// and keyframes it already resolved; only a stale one resolves again.
  fn targets(&mut self, node: NodeId, fresh: bool) -> Option<Targets<P>> {
    let record = self.record(node)?;

    if fresh {
      return Some(match &record.kind {
        Kind::Run { cache, .. } => {
          let cache = cache.as_ref()?;

          Targets {
            resolved_box: None,
            resolved_run: Some(cache.target.clone()),
            resolved_layout: cache.layout.clone(),
            box_keyframes: Vec::new(),
            run_keyframes: cache.keyframes.clone(),
            reads_interaction: None,
          }
        }
        kind => {
          let cache = kind.box_record()?.cache.as_ref()?;

          Targets {
            resolved_box: Some(cache.target.clone()),
            resolved_run: None,
            resolved_layout: PlainBox::<P>::default(),
            box_keyframes: cache.keyframes.clone(),
            run_keyframes: Vec::new(),
            reads_interaction: None,
          }
        }
      });
    }

    let env = Env::of(self, node);
    let resolved_box = env.resolve_box();
    let (resolved_run, resolved_layout) = env
      .resolve_run()
      .map_or((None, PlainBox::<P>::default()), |(run, layout)| {
        (Some(run), layout)
      });
    let box_keyframes = record
      .kind
      .box_record()
      .map(|inner| {
        inner
          .kept
          .animations()
          .iter()
          .map(|animation| animation.map_keyframes(|frame| env.resolve_box_keyframe(frame)))
          .collect::<Vec<_>>()
      })
      .unwrap_or_default();
    let run_keyframes = match &record.kind {
      Kind::Run { kept, .. } => kept
        .animations()
        .iter()
        .map(|animation| animation.map_keyframes(|frame| env.resolve_run_keyframe(frame)))
        .collect::<Vec<_>>(),
      _ => Vec::new(),
    };
    let reads_interaction = Some(env.reads_interaction());

    self.style_resolutions += 1;

    Some(Targets {
      resolved_box,
      resolved_run,
      resolved_layout,
      box_keyframes,
      run_keyframes,
      reads_interaction,
    })
  }

  /// The interaction state `node`'s conditions resolve against now, as the scene
  /// resolves it.
  fn interaction(&self, node: NodeId) -> Interaction {
    let node = self.record(node).map(|record| record.node);

    Scene::<P>::with(|scene| node.map_or_else(Interaction::default, |node| scene.interaction(node)))
  }

  /// Whether `node`'s interaction moved since its style was resolved; `false` for a
  /// node presenting nothing.
  fn interaction_moved(&self, node: NodeId) -> bool {
    self
      .record(node)
      .and_then(|record| record.kind.resolved_under())
      .is_some_and(|(_, resolved)| resolved != self.interaction(node))
  }
}

impl<P: Drawn> Kind<P> {
  /// Presents the node's resolution in `targets` over its motion at `pass` and caches
  /// it; `(repaint, active)`.
  fn present(&mut self, targets: Targets<P>, pass: &Pass) -> (bool, bool) {
    let Self::Run {
      kept,
      cache,
      motion,
      ..
    } = self
    else {
      return self.box_record_mut().map_or((false, false), |inner| {
        inner.present(targets.resolved_box, targets.box_keyframes, pass)
      });
    };
    let mut active = false;
    let previous = cache.as_ref().map(|cache| &cache.target);
    let presented = targets.resolved_run.map(|target| {
      let Presented {
        style,
        active: running,
      } = motion.present(
        previous,
        target.clone(),
        kept.transitions(),
        &targets.run_keyframes,
        pass.sample,
      );

      active |= running;

      (style, target)
    });
    let repaint =
      cache.as_ref().map(|cache| &cache.style) != presented.as_ref().map(|(style, _)| style);

    *cache = presented.map(|(style, target)| RunCache {
      generation: pass.generation,
      interaction: pass.interaction,
      style,
      target,
      layout: targets.resolved_layout,
      keyframes: targets.run_keyframes,
    });

    (repaint, active)
  }
}

impl<P: Drawn> BoxRecord<P> {
  /// Presents the box's resolution over its motion at `pass` and caches it;
  /// `(repaint, active)`.
  fn present(
    &mut self,
    resolved: Option<PlainBox<P>>,
    keyframes: Vec<Animation<PlainBox<P>>>,
    pass: &Pass,
  ) -> (bool, bool) {
    let Some(target) = resolved
    else {
      return (false, false);
    };
    let Presented {
      style,
      active: running,
    } = self.motion.present(
      self.cache.as_ref().map(|cache| &cache.target),
      target.clone(),
      self.kept.transitions(),
      &keyframes,
      pass.sample,
    );
    let repaint = self.cache.as_ref().is_none_or(|cache| cache.style != style);

    self.cache = Some(BoxCache {
      generation: pass.generation,
      interaction: pass.interaction,
      style,
      target,
      keyframes,
    });

    (repaint, running)
  }
}
