//! Resolved styles to the layout tree: the per-node restyle pass, the flattening of
//! anchors and scopes into their content, and the layout computation.
//!
//! Layout is taffy with `1.0 = 1 cell`. A node's taffy style is written only when the
//! style resolved for it differs from the one applied, so a keystroke relays out the
//! changed subtree and a hover change repaints without a relayout.

use std::{cell::OnceCell, ops::Range};

use rustc_hash::FxHashSet;
use skene_foundation::{
  Inset,
  Layout,
  Point,
  Rect,
  RunPlacement,
  Scene,
  Size,
  text::{Round, TextIndex},
};
use skene_layout::{
  Borders,
  BoxLayout,
  Leaf,
  Place,
  Revisit,
  TextFlow,
  taffy::{
    self,
    prelude::{AvailableSpace, length},
    style::{Display, Style},
  },
};

use crate::{
  Terminal,
  engine::{CellEngine, CellLaid, engine, paragraph_style},
  motion::{Clock, Presented},
  paint::{ShadowPaint, run_scale},
  records::{BoxCache, Field, Handle, Kind, PlainBox, PlainRun, Record, Records, RunCache},
  resolve::Env,
  text::{self, CellScale},
};

/// The character standing in for one cell of an inline box in its paragraph's text.
pub(crate) const INLINE_BOX: char = '\u{fffc}';

/// How far into its cell a point stands for the cell: just past its left edge, so a caret
/// placed at a cell goes to the cell's left edge, where a cluster's edge lies there, and
/// otherwise to the nearer edge of the cluster whose block holds it: before a cluster from
/// the cell its block starts at, after a wide cluster from its second cell.
const INTO_CELL: f32 = 1. / 16.;

/// What one restyle of a node decided.
struct Restyled {
  style: Style,
  leaf: Option<CellLeaf>,
}

/// What the layout measures a leaf by: a text leaf's flow, or a text entry's native
/// extent, its content box's cells and rows, as an `<input>`'s intrinsic size.
#[derive(Debug, PartialEq)]
pub(crate) enum CellLeaf {
  /// A text leaf, measured by its flow.
  Flow(Box<CellFlow>),
  /// A text entry, measured at its native extent.
  Entry(Size<f32>),
}

impl Leaf for CellLeaf {
  /// A flow's measure, or an entry's native extent in each dimension not `known`.
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    available: taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32> {
    match self {
      Self::Flow(flow) => flow.measure(known, available),
      Self::Entry(native) => Size::new(
        known.width.unwrap_or(native.width),
        known.height.unwrap_or(native.height),
      ),
    }
  }
}

/// One item of a text leaf: a run, its node, where its text ends in the leaf's and its
/// scale, or an inline box, its node and the cells and rows it laid out at.
#[derive(Debug, Clone, Copy, PartialEq)]
enum FlowItem {
  Run {
    node: Handle,
    end: usize,
    scale: CellScale,
  },
  Box {
    node: Handle,
    size: (usize, usize),
  },
}

/// What the layout measures a text leaf by: its runs and inline boxes laid out as one
/// paragraph through the cell engine, made once a measure, a paint or a hit first asks
/// and kept between frames. Two leaves are alike where their runs' nodes, texts and
/// scales, their boxes' nodes and sizes and the terminal's clustering are.
#[derive(Debug)]
pub(crate) struct CellFlow {
  text: String,
  items: Vec<FlowItem>,
  clustering: bool,
  flow: OnceCell<TextFlow<CellEngine>>,
}

impl PartialEq for CellFlow {
  fn eq(&self, other: &Self) -> bool {
    self.clustering == other.clustering && self.items == other.items && self.text == other.text
  }
}

impl CellFlow {
  /// An empty leaf.
  fn new() -> Self {
    Self {
      text: String::new(),
      items: Vec::new(),
      clustering: text::clustering(),
      flow: OnceCell::new(),
    }
  }

  /// Adds the run `node` holding `text` at `scale`.
  fn add_run(&mut self, node: Handle, text: &str, scale: CellScale) {
    self.text.push_str(text);
    self.items.push(FlowItem::Run {
      node,
      end: self.text.len(),
      scale,
    });
  }

  /// Adds the inline box `node` laid out `size` cells by rows.
  fn add_box(&mut self, node: Handle, size: (usize, usize)) {
    self.items.push(FlowItem::Box { node, size });
  }

  /// The leaf's flow, made once.
  pub(crate) fn flow(&self) -> &TextFlow<CellEngine> {
    self.flow.get_or_init(|| {
      let mut flow = TextFlow::new(engine(), CellScale::ONE.style(), paragraph_style());
      let mut start = 0;

      for item in &self.items {
        match *item {
          FlowItem::Run { node, end, scale } => {
            flow.add_run(node, &[], &self.text[start..end], scale.style());
            start = end;
          }
          FlowItem::Box {
            node,
            size: (cells, rows),
          } => flow.add_box(node, &[], Size::new(cells_of(cells), cells_of(rows))),
        }
      }

      flow
    })
  }

  /// The flow laid out `width` cells wide.
  pub(crate) fn laid(&self, width: usize) -> CellLaid {
    self.flow().layout_in(content(width)).0
  }
}

impl Leaf for CellFlow {
  /// The flow measured with a width floored to whole cells, at least one, as the terminal
  /// lays text out in cells.
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    available: taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32> {
    let floor = |width: f32| width.max(0.).floor().max(1.);
    let measured = self.flow().measure(
      taffy::geometry::Size {
        width: known.width.map(floor),
        height: known.height,
      },
      taffy::geometry::Size {
        width: match available.width {
          AvailableSpace::Definite(width) => AvailableSpace::Definite(floor(width)),
          space => space,
        },
        height: available.height,
      },
    );

    Size::new(known.width.unwrap_or(measured.width), measured.height)
  }
}

/// The content box a text node lays its flow out in, `width` cells wide, from its origin.
pub(crate) const fn content(width: usize) -> Rect<f32> {
  Rect::new(Point::ZERO, Size::new(cells_of(width), 0.))
}

/// The point standing for the cell (`column`, `row`) of a text node's flow.
pub(crate) const fn cell_point(column: usize, row: usize) -> Point<f32> {
  Point::new(cells_of(column) + INTO_CELL, cells_of(row) + 0.5)
}

/// `count` cells or rows as the layout counts them.
#[expect(
  clippy::cast_precision_loss,
  reason = "a count of cells stays far below the integers f32 holds exactly"
)]
pub(crate) const fn cells_of(count: usize) -> f32 {
  count as f32
}

/// One run of a text node's source: its node, the bytes of the source it holds, and the
/// byte of the flow its text starts at, `None` for an inline box, which the flow holds
/// as a box rather than as its placeholder.
#[derive(Debug, Clone)]
struct SourceRun {
  node: Handle,
  source: Range<usize>,
  flow: Option<usize>,
}

/// A text node's source text as its runs hold it, an inline box its placeholder, its
/// clusters, and where each run lies in it and in the flow the layout lays out: a
/// paragraph's leaf runs, or a run flowing in none.
#[derive(Debug)]
pub(crate) struct Source {
  pub(crate) text: String,
  pub(crate) index: TextIndex,
  runs: Vec<SourceRun>,
}

impl Source {
  /// The source of the text node `handle`.
  pub(crate) fn of(records: &Records, handle: Handle) -> Self {
    let runs = if is_paragraph(records, handle) {
      paragraph_runs(records, handle)
    }
    else {
      vec![handle]
    };
    let mut text = String::new();
    let mut flow = 0;
    let mut source = Vec::new();

    for run in runs {
      let Some(record) = records.record(run)
      else {
        continue;
      };
      let at = text.len();
      let boxed = is_inline_box(records, run);

      text.push_str(&record.text);
      source.push(SourceRun {
        node: run,
        source: at..text.len(),
        flow: (!boxed).then_some(flow),
      });

      if !boxed {
        flow += record.text.len();
      }
    }

    Self {
      index: TextIndex::new(&text),
      text,
      runs: source,
    }
  }

  /// The source's runs in order: each node and the bytes of the source it holds.
  pub(crate) fn runs(&self) -> impl Iterator<Item = (Handle, Range<usize>)> + '_ {
    self.runs.iter().map(|run| (run.node, run.source.clone()))
  }

  /// The cluster of the source a caret before byte `offset` of `node`'s text stands
  /// before; `None` for a node outside the source.
  pub(crate) fn cluster_of(&self, node: Handle, offset: usize) -> Option<usize> {
    let run = self.runs.iter().find(|run| run.node == node)?;
    let byte = (run.source.start + offset).min(run.source.end);

    Some(self.index.grapheme_at_byte(byte, Round::Down))
  }

  /// The run whose text holds byte `at` of the flow, and the cluster of the source it
  /// lies in.
  pub(crate) fn at_flow(&self, at: usize) -> Option<(Handle, usize)> {
    self.runs.iter().find_map(|run| {
      let start = run.flow?;
      let length = run.source.len();

      (start <= at && at < start + length).then(|| {
        (
          run.node,
          self
            .index
            .grapheme_at_byte(run.source.start + at - start, Round::Down),
        )
      })
    })
  }
}

/// The flow the text node `handle` lays out as, when it holds one.
pub(crate) fn flow_of(records: &Records, handle: Handle) -> Option<&CellFlow> {
  match records.tree.context(handle)? {
    CellLeaf::Flow(flow) => Some(flow.as_ref()),
    CellLeaf::Entry(_) => None,
  }
}

/// Whether `handle` lays its inline runs out as one paragraph: a flow box whose every
/// laid-out child is an inline text run or an inline box.
pub(crate) fn is_paragraph(records: &Records, handle: Handle) -> bool {
  if records.record(handle).and_then(Record::layout) != Some(Layout::Document) {
    return false;
  }

  let children = records.layout_children(handle);

  !children.is_empty()
    && children.iter().all(|child| {
      matches!(
        records.record(*child).map(|record| &record.kind),
        Some(Kind::Run { props, .. }) if props.placement != RunPlacement::Line
      )
    })
}

/// Whether `handle` is an inline box: a run laid out as a box among its paragraph's
/// text, its text one placeholder cluster as large as the box.
pub(crate) fn is_inline_box(records: &Records, handle: Handle) -> bool {
  matches!(
    records.record(handle).map(|record| &record.kind),
    Some(Kind::Run { props, .. }) if props.placement == RunPlacement::Embedded
  )
}

/// Whether `handle` is an inline run: text, a group or an inline box flowing in a
/// paragraph.
pub(crate) fn is_inline_run(records: &Records, handle: Handle) -> bool {
  matches!(
    records.record(handle).map(|record| &record.kind),
    Some(Kind::Run { props, .. }) if props.placement != RunPlacement::Line
  )
}

/// The paragraph the inline run `handle` flows in, through any groups around it.
pub(crate) fn enclosing_paragraph(records: &Records, handle: Handle) -> Option<Handle> {
  let mut current = records.layout_parent(handle)?;

  while !is_paragraph(records, current) {
    if !is_inline_run(records, current) {
      return None;
    }

    current = records.layout_parent(current)?;
  }

  Some(current)
}

/// The inline boxes of the paragraph `handle`, each with the cell its frame's top left
/// is in, relative to the paragraph's origin, against the last layout.
pub(crate) fn inline_boxes(records: &Records, handle: Handle) -> Vec<(Handle, (i64, i64))> {
  let Some(flow) = flow_of(records, handle)
  else {
    return Vec::new();
  };

  if flow
    .items
    .iter()
    .any(|item| matches!(item, FlowItem::Box { .. }))
  {
    let (width, _) = records.node_size(handle);

    return flow
      .flow()
      .place(content(usize::try_from(width).unwrap_or(1)))
      .into_iter()
      .filter(|(node, _)| is_inline_box(records, *node))
      .map(|(node, rect)| (node, (cell_of(rect.origin.x), cell_of(rect.origin.y))))
      .collect();
  }

  Vec::new()
}

/// The cell a layout coordinate of a flow falls in.
#[expect(
  clippy::cast_possible_truncation,
  reason = "a coordinate in cells stays far below i64::MAX"
)]
const fn cell_of(coordinate: f32) -> i64 {
  coordinate.round() as i64
}

/// Lays every inline box out on its own and records its size, which its placeholder
/// spans; `true` when a size changed, in which case its paragraph measures again.
pub(crate) fn measure_inline_boxes(records: &mut Records) -> bool {
  let mut changed = false;

  for handle in records.handles() {
    if !is_inline_box(records, handle) {
      continue;
    }

    records.tree.compute(
      handle,
      taffy::geometry::Size {
        width: AvailableSpace::MaxContent,
        height: AvailableSpace::MaxContent,
      },
    );

    let (width, height) = records.node_size(handle);
    let size = (
      usize::try_from(width).unwrap_or(1).max(1),
      usize::try_from(height).unwrap_or(1).max(1),
    );

    if let Some(Kind::Run { extent, .. }) =
      records.record_mut(handle).map(|record| &mut record.kind)
      && *extent != Some(size)
    {
      *extent = Some(size);
      records.queue_restyle(handle);
      changed = true;
    }
  }

  if changed {
    records.layout_dirty = true;
  }

  changed
}

/// The text of the paragraph `handle`: every inline run beneath it, in order.
pub(crate) fn paragraph_text(records: &Records, handle: Handle) -> String {
  let mut text = String::new();

  for run in paragraph_runs(records, handle) {
    if let Some(record) = records.record(run) {
      text.push_str(&record.text);
    }
  }

  text
}

/// The leaf runs of the paragraph `handle`, in order: a group run contributes its
/// runs, never its own (empty) text.
pub(crate) fn paragraph_runs(records: &Records, handle: Handle) -> Vec<Handle> {
  let mut runs = Vec::new();

  collect_runs(records, handle, &mut runs);

  runs
}

fn collect_runs(records: &Records, handle: Handle, runs: &mut Vec<Handle>) {
  for child in records.layout_children(handle) {
    let is_run = matches!(
      records.record(child).map(|record| &record.kind),
      Some(Kind::Run { .. })
    );

    if !is_run {
      continue;
    }

    if is_inline_box(records, child) || records.layout_children(child).is_empty() {
      runs.push(child);
    }
    else {
      collect_runs(records, child, runs);
    }
  }
}

/// Resolves the styles that may have moved and writes the taffy styles that changed;
/// `true` when any changed.
///
/// Every node resolves once the definitions, the environment or the display moved;
/// otherwise the nodes queued since the last restyle, those in motion, and those whose
/// last resolution read an interaction that has moved since, each with the paragraph
/// it flows in.
pub(crate) fn restyle(records: &mut Records) -> bool {
  let (handles, displayed) = revisited(records);
  let mut changed = false;

  records.motion_active = false;

  for handle in handles {
    let shown = match &displayed {
      Some(displayed) => displayed.contains(&handle),
      None => records.record(handle).is_some_and(|record| !record.hidden),
    };
    let Some(restyled) = restyle_one(records, handle, shown)
    else {
      continue;
    };

    if records.tree.set_context(handle, restyled.leaf) {
      changed = true;
    }

    if records.tree.set_style(handle, restyled.style) {
      records.set_style_calls += 1;
      changed = true;
    }
  }

  changed
}

/// The nodes the next restyle revisits, runs first, with the display where it revisits
/// every node.
fn revisited(records: &mut Records) -> (Vec<Handle>, Option<FxHashSet<Handle>>) {
  let interaction = Scene::<Terminal>::read(Scene::interaction_generation);
  let (mut handles, displayed) = match records.restyle.take(interaction) {
    Revisit::Every => (records.handles(), Some(records.displayed())),
    Revisit::Nodes(mut handles, moved) => {
      if moved {
        handles.extend(
          records
            .restyle
            .interactive()
            .filter(|handle| records.interaction_moved(*handle)),
        );
      }

      let paragraphs: Vec<Handle> = handles
        .iter()
        .filter(|handle| is_inline_run(records, **handle))
        .filter_map(|handle| enclosing_paragraph(records, *handle))
        .collect();

      handles.extend(paragraphs);
      handles.sort_unstable();
      handles.dedup();

      (handles, None)
    }
  };

  handles.sort_by_key(|handle| {
    !matches!(
      records.record(*handle).map(|record| &record.kind),
      Some(Kind::Run { .. })
    )
  });

  (handles, displayed)
}

/// Resolves `handle`'s style if stale or in motion, and returns the taffy style it maps
/// to; `None` for a node outside the layout. A node out of the display runs no motion
/// and forgets its presentation, so it shows afresh once `displayed` again.
fn restyle_one(records: &mut Records, handle: Handle, displayed: bool) -> Option<Restyled> {
  let generation = records.restyle.generation();
  let now = records.now();
  let reduced = records.reduced_motion;
  let record = records.record_mut(handle)?;

  if !record.kind.is_laid_out() {
    return None;
  }

  if !displayed {
    if !record.hidden {
      record.hidden = true;
      record.in_motion = false;
      record.kind.forget_presentation();
      records.restyle.read_interaction(handle, false);
    }

    return Some(taffy_style_of(records, handle));
  }

  record.hidden = false;

  let record = records.record(handle)?;
  let interaction = records.interaction(handle);
  let fresh = record.kind.resolved_under() == Some((generation, interaction));
  let in_motion = record.in_motion;

  if !fresh || in_motion {
    let Targets {
      resolved_box,
      resolved_run,
      resolved_layout,
      box_keyframes,
      run_keyframes,
      reads_interaction,
    } = targets(records, handle, fresh)?;

    if let Some(read) = reads_interaction {
      records.restyle.read_interaction(handle, read);
    }

    let record = records.record_mut(handle)?;
    let frame = Frame {
      generation,
      interaction,
      clock: Clock { now, reduced },
    };
    let reach = record.kind.box_style().map(ShadowPaint::band);
    // A node whose presentation came out different from the last one repaints.
    let (repaint, active) = match &mut record.kind {
      Kind::Run {
        kept,
        cache,
        motion,
        ..
      } => {
        let target = resolved_run.map(|target| (target, resolved_layout));

        present_run(kept, cache, motion, target, &run_keyframes, &frame)
      }
      kind => match kind.box_record_mut() {
        Some(inner) => present_box(inner, resolved_box, &box_keyframes, &frame),
        None => (false, false),
      },
    };

    record.in_motion = active;
    records.covers_stale |= record.kind.box_style().map(ShadowPaint::band) != reach;
    records.motion_active |= active;

    if active {
      records.restyle.keep_moving(handle);
    }

    if repaint {
      records.damage(handle);
    }
  }

  Some(taffy_style_of(records, handle))
}

/// What a restyle presents: the resolved box or run target, the wrapper layout of a
/// run, the keyframes of each, and whether the resolution read the node's interaction,
/// `None` where nothing resolved.
struct Targets {
  resolved_box: Option<PlainBox>,
  resolved_run: Option<PlainRun>,
  resolved_layout: PlainBox,
  box_keyframes: Vec<skene_foundation::Animation<PlainBox>>,
  run_keyframes: Vec<skene_foundation::Animation<PlainRun>>,
  reads_interaction: Option<bool>,
}

/// The targets of `handle`: a fresh node in motion samples its clock over the targets
/// and keyframes it already resolved; only a stale one resolves again.
fn targets(records: &mut Records, handle: Handle, fresh: bool) -> Option<Targets> {
  let record = records.record(handle)?;

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
          resolved_layout: PlainBox::default(),
          box_keyframes: cache.keyframes.clone(),
          run_keyframes: Vec::new(),
          reads_interaction: None,
        }
      }
    });
  }

  let env = Env::of(records, handle);
  let resolved_box = env.resolve_box();
  let resolved_run = env.resolve_run();
  let resolved_layout = env.resolve_run_layout();
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

  records.style_resolutions += 1;

  Some(Targets {
    resolved_box,
    resolved_run,
    resolved_layout,
    box_keyframes,
    run_keyframes,
    reads_interaction,
  })
}

/// What one restyle resolves under: the generation and interaction it is cached as,
/// and the clock its motion samples at.
struct Frame {
  generation: u64,
  interaction: crate::records::Interaction,
  clock: Clock,
}

/// Presents a run's resolution over its motion and caches it; `(repaint, active)`.
fn present_run(
  kept: &crate::records::KeptStyle,
  cache: &mut Option<RunCache>,
  motion: &mut crate::motion::RunMotion,
  resolved: Option<(PlainRun, PlainBox)>,
  keyframes: &[skene_foundation::Animation<PlainRun>],
  frame: &Frame,
) -> (bool, bool) {
  let mut active = false;
  let previous = cache.as_ref().map(|cache| &cache.target);
  let presented = resolved.map(|(target, layout)| {
    let Presented {
      style,
      active: running,
    } = motion.present(
      previous,
      target.clone(),
      kept.transitions(),
      keyframes,
      frame.clock,
    );

    active |= running;

    (style, target, layout)
  });
  let repaint =
    cache.as_ref().map(|cache| &cache.style) != presented.as_ref().map(|(style, _, _)| style);

  *cache = presented.map(|(style, target, layout)| RunCache {
    generation: frame.generation,
    interaction: frame.interaction,
    style,
    target,
    layout,
    keyframes: keyframes.to_vec(),
  });

  (repaint, active)
}

/// Presents a box's resolution over its motion and caches it; `(repaint, active)`.
fn present_box(
  inner: &mut crate::records::BoxRecord,
  resolved: Option<PlainBox>,
  keyframes: &[skene_foundation::Animation<PlainBox>],
  frame: &Frame,
) -> (bool, bool) {
  let Some(target) = resolved
  else {
    return (false, false);
  };
  let Presented {
    style,
    active: running,
  } = inner.motion.present(
    inner.cache.as_ref().map(|cache| &cache.target),
    target.clone(),
    inner.kept.transitions(),
    keyframes,
    frame.clock,
  );
  let repaint = inner
    .cache
    .as_ref()
    .is_none_or(|cache| cache.style != style);

  inner.cache = Some(BoxCache {
    generation: frame.generation,
    interaction: frame.interaction,
    style,
    target,
    keyframes: keyframes.to_vec(),
  });

  (repaint, running)
}

/// The taffy style `handle` takes from its resolved style and its place in the tree.
fn taffy_style_of(records: &Records, handle: Handle) -> Restyled {
  let record = records.record(handle).expect("a live record");
  let layout_parent = records.layout_parent(handle);
  let in_stack = layout_parent
    .and_then(|parent| records.record(parent))
    .and_then(Record::layout)
    == Some(Layout::Stack);
  let place = Place {
    in_stack,
    borders: Borders::Unit,
  };
  // The root's child fills the viewport, unless it sizes its own height.
  let fills_root = layout_parent == Some(records.root)
    && !record.is_overlay()
    && record
      .kind
      .box_style()
      .is_none_or(|style| style.height.is_none());

  match &record.kind {
    Kind::Run { props, cache, .. } => {
      let mut style = Style::default();
      let inline = props.placement != RunPlacement::Line;
      let has_children = !records.layout_children(handle).is_empty();

      // A wrapper's properties on a bare run: its flexibility, alignment, pinning and
      // grid placement, folded like a box's.
      if let Some(cache) = cache {
        cache.layout.fold_into(&mut style, place);
      }

      let text = if props.placement == RunPlacement::Embedded {
        // An inline box lays out on its own, at its content's size.
        style.display = Display::Flex;

        None
      }
      else if record.text.is_empty() && has_children {
        style.display = Display::Flex;

        None
      }
      else {
        let mut flow = CellFlow::new();

        flow.add_run(handle, &record.text, run_scale(records, handle));

        Some(flow)
      };

      place.apply(&mut style);

      if inline && has_children {
        style.flex_wrap = taffy::style::FlexWrap::Wrap;
      }

      Restyled {
        style,
        leaf: text.map(|flow| CellLeaf::Flow(Box::new(flow))),
      }
    }
    Kind::Anchor | Kind::Scope => Restyled {
      style: Style::default(),
      leaf: None,
    },
    Kind::Box(inner) => {
      let resolved = inner.style().cloned().unwrap_or_default();
      let paragraph = is_paragraph(records, handle);
      let mut style = resolved.taffy_style(inner.props.layout, place);

      if fills_root {
        style.flex_grow = 1.;
      }

      let leaf = paragraph.then(|| CellLeaf::Flow(Box::new(paragraph_flow(records, handle))));

      if paragraph {
        style.display = Display::Block;
      }

      Restyled { style, leaf }
    }
    Kind::Control { inner, field, .. } => {
      let resolved = inner.style().cloned().unwrap_or_default();
      let mut style = resolved.taffy_style(inner.props.layout, place);
      let leaf = field
        .as_ref()
        .map(|field| entry_leaf(&mut style, &resolved, field));

      if fills_root {
        style.flex_grow = 1.;
      }

      Restyled { style, leaf }
    }
    Kind::Overlay { inner, open, .. } => {
      let resolved = inner.style().cloned().unwrap_or_default();
      let mut style = resolved.taffy_style(inner.props.layout, place.detached());

      style.position = taffy::style::Position::Absolute;

      if !*open {
        style.display = Display::None;
      }

      Restyled { style, leaf: None }
    }
  }
}

/// The flow the paragraph `handle` lays out as: each leaf run's text at its scale, and
/// each inline box as large as it laid out on its own, one cell before it has.
fn paragraph_flow(records: &Records, handle: Handle) -> CellFlow {
  let mut flow = CellFlow::new();

  for run in paragraph_runs(records, handle) {
    let Some(record) = records.record(run)
    else {
      continue;
    };

    if is_inline_box(records, run) {
      let (cells, rows) = match &record.kind {
        Kind::Run {
          extent: Some(extent),
          ..
        } => *extent,
        _ => (1, 1),
      };

      flow.add_box(run, (cells, rows));
    }
    else {
      flow.add_run(run, &record.text, run_scale(records, run));
    }
  }

  flow
}

/// Hands every laid-out node its flattened children, where the tree changed since the
/// last layout.
pub(crate) fn rebuild_children(records: &mut Records) {
  if !records.restyle.take_structure() {
    return;
  }

  let handles = records.handles();

  for handle in handles {
    let laid_out = records
      .record(handle)
      .is_some_and(|record| record.kind.is_laid_out());

    if !laid_out || !records.tree.contains(handle) {
      continue;
    }

    let mut children = if is_paragraph(records, handle) {
      Vec::new()
    }
    else {
      records.layout_children(handle)
    };

    if handle == records.root {
      children.extend(
        records
          .overlays
          .iter()
          .copied()
          .filter(|overlay| records.is_reachable(*overlay)),
      );
    }

    records.tree.set_children(handle, children);
  }

  records.layout_dirty = true;
}

/// Computes the layout at `width` x `height`, the root sized to the terminal.
pub(crate) fn compute(records: &mut Records, width: u16, height: u16) {
  let root = records.root;
  let (width, height) = (f32::from(width), f32::from(height));

  records.tree.set_style(
    root,
    Style {
      display: Display::Flex,
      flex_direction: taffy::style::FlexDirection::Column,
      size: taffy::geometry::Size {
        width: length(width),
        height: length(height),
      },
      ..Style::default()
    },
  );
  records.tree.compute(
    root,
    taffy::geometry::Size {
      width: AvailableSpace::Definite(width),
      height: AvailableSpace::Definite(height),
    },
  );
  records.layout_generation += 1;
  records.covers_stale = true;
}

/// The edges the resolved style draws its border on: each of a width above zero, which
/// the terminal draws as one cell.
#[inline(always)]
pub(crate) fn border_edges(style: &PlainBox) -> Inset<bool> {
  let width = style.border_width.unwrap_or_default();

  Inset {
    top: width.top > 0,
    end: width.end > 0,
    bottom: width.bottom > 0,
    start: width.start > 0,
  }
}

/// The cells the resolved style's padding takes on each edge, a negative edge none, as
/// layout takes it.
pub(crate) fn padding_cells(style: &PlainBox) -> Inset<u32> {
  let padding = style.padding.unwrap_or_default();
  let cells = |edge: i32| u32::try_from(edge).unwrap_or(0);

  Inset {
    top: cells(padding.top),
    end: cells(padding.end),
    bottom: cells(padding.bottom),
    start: cells(padding.start),
  }
}

/// The leaf the entry `field` measures as, at its native extent: eight cells or one cell
/// past its value's widest line, and its lines tall; where `style` leaves it unsized, the
/// native size, its chrome included, is the least a layout stretching or shrinking it
/// leaves it.
fn entry_leaf(style: &mut Style, resolved: &PlainBox, field: &Field) -> CellLeaf {
  let chrome = chrome(resolved);
  let columns = (field.widest() + 1)
    .max(field.placeholder.as_deref().map_or(0, Field::widest_of))
    .max(8);
  let native = Size::new(
    cell(u32::try_from(columns).unwrap_or(u32::MAX)),
    cell(u32::from(field.lines)),
  );

  native_size(
    &mut style.size.width,
    &mut style.min_size.width,
    style.max_size.width,
    native.width + cell(chrome.start + chrome.end),
  );

  native_size(
    &mut style.size.height,
    &mut style.min_size.height,
    style.max_size.height,
    native.height + cell(chrome.top + chrome.bottom),
  );

  CellLeaf::Entry(native)
}

/// Lays an entry out along one axis at its native extent `native` where its `size` is
/// unset, as an `<input>`'s intrinsic size is: as its least, held to its `max` and raised
/// to its `min`, where each is a length or unset, and otherwise as its size, which its
/// `min` and `max` then bound.
fn native_size(
  size: &mut taffy::style::Dimension,
  min: &mut taffy::style::LengthPercentageAuto,
  max: taffy::style::LengthPercentageAuto,
  native: f32,
) {
  if !size.is_auto() {
    return;
  }

  let least = taffy::style::Dimension::from(*min).into_option();
  let most = taffy::style::Dimension::from(max).into_option();

  if (min.is_auto() || least.is_some()) && (max.is_auto() || most.is_some()) {
    let held = most.map_or(native, |most| native.min(most));

    *min = length(least.map_or(held, |least| held.max(least)));
  }
  else {
    *size = length(native);
  }
}

/// The cells the resolved style's border and padding take on each edge.
fn chrome(style: &PlainBox) -> Inset<u32> {
  let edges = border_edges(style);
  let padding = padding_cells(style);

  Inset {
    top: u32::from(edges.top) + padding.top,
    end: u32::from(edges.end) + padding.end,
    bottom: u32::from(edges.bottom) + padding.bottom,
    start: u32::from(edges.start) + padding.start,
  }
}

/// Whether the resolved style draws its border on any edge.
#[inline(always)]
pub(crate) fn has_border(style: &PlainBox) -> bool {
  border_edges(style) != Inset::default()
}

/// A cell length as layout units.
#[inline(always)]
pub(crate) const fn cell(value: u32) -> f32 {
  #[expect(
    clippy::cast_precision_loss,
    reason = "a cell count stays far below the integers f32 holds exactly"
  )]
  {
    value as f32
  }
}

#[cfg(test)]
mod properties {
  use proptest::prelude::*;
  use skene_foundation::{
    NodeKind,
    Scene,
    text::{Laid as _, TextIndex},
  };
  use skene_layout::{
    Leaf as _,
    taffy::{geometry::Size, prelude::AvailableSpace},
  };

  use super::{CellFlow, cells_of};
  use crate::{
    Terminal,
    engine::{drawn, whole, wrap_spans},
    text::{self, CellScale},
  };

  /// Text mostly of the clusters wrapping turns on: words, blanks, tabs, newlines, a
  /// hyphen, wide and combining clusters.
  fn arbitrary_text() -> impl Strategy<Value = String> {
    const PIECES: &[&str] = &[
      "a", "bc", "word", " ", "  ", "\t", "\n", "-", "日", "本語", "e\u{301}", "(x)",
    ];

    proptest::collection::vec(proptest::sample::select(PIECES), 0..24)
      .prop_map(|pieces| pieces.concat())
  }

  /// The flow of a run holding `text` at `scale`.
  fn flow(text: &str, scale: CellScale) -> CellFlow {
    let node = Scene::<Terminal>::with(|scene| scene.create(NodeKind::Run, None));
    let mut flow = CellFlow::new();

    flow.add_run(node, text, scale);

    flow
  }

  proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Any text at any width and scale measures as the lines it lays out into: as tall as
    /// they are, every cluster drawn on its line's rows, and as wide as the widest line
    /// drawn, or wider by blanks ending a line before a newline or at the text's end that
    /// take their room up to the width.
    #[test]
    fn a_measure_matches_its_wrapped_lines(
      text in arbitrary_text(),
      width in 0..40_usize,
      scale in 1..4_u8,
    ) {
      text::reset();

      let flow = flow(&text, CellScale::Whole(scale));
      let measured = flow.measure(
        Size { width: None, height: None },
        Size {
          width: AvailableSpace::Definite(cells_of(width)),
          height: AvailableSpace::MaxContent,
        },
      );
      let laid = flow.laid(width.max(1));
      let lines = laid.lines();
      let clusters = drawn(&laid, &text, &TextIndex::new(&text));
      let widest = clusters
        .iter()
        .map(|cluster| cluster.column + cluster.advance)
        .max()
        .unwrap_or(0);
      let rows: usize = lines.iter().map(|line| whole(line.height)).sum();
      let (cells, height) = (whole(measured.width), whole(measured.height));

      prop_assert_eq!(height, rows);
      prop_assert_eq!(rows, whole(laid.size().height));
      prop_assert!(cells == widest || (cells > widest && cells <= width.max(1)));

      for cluster in &clusters {
        prop_assert!(cluster.top + usize::from(scale) <= rows);
      }
    }

    /// Any text wraps at any width into lines that fit, but for a lone cluster too wide
    /// for one, and that leave out nothing but the blanks a line ends in and the
    /// newlines between paragraphs, an empty text into one empty line.
    #[test]
    fn a_wrap_fits_and_keeps_the_text(text in arbitrary_text(), width in 1..30_usize) {
      text::reset();

      let lines = wrap_spans(&text, width);
      let clusters: Vec<&str> = text::clusters(&text).map(|(_, cluster)| cluster).collect();
      let mut expected_start = 0;

      prop_assert!(!lines.is_empty());

      for (start, line) in &lines {
        let cells = text::cells(line);
        let count = text::count(line);

        prop_assert!(
          cells <= width || count == 1,
          "{:?} spans {} cells at width {}",
          line,
          cells,
          width
        );

        prop_assert!(*start >= expected_start);

        for skipped in &clusters[expected_start..*start] {
          prop_assert!(
            matches!(*skipped, " " | "\t" | "\n" | "\r\n"),
            "{skipped:?} was left out"
          );
        }

        prop_assert_eq!(&clusters[*start..start + count].concat(), line);

        expected_start = start + count;
      }

      for skipped in &clusters[expected_start..] {
        prop_assert!(matches!(*skipped, " " | "\t" | "\n"), "{skipped:?} was left out");
      }
    }
  }
}
