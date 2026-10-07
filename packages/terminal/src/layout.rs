//! Resolved styles to the layout tree: the per-node restyle pass, the flattening of
//! anchors and scopes into their content, and the layout computation.
//!
//! Layout is taffy with `1.0 = 1 cell`. A node's taffy style is written only when the
//! style resolved for it differs from the one applied, so a keystroke relays out the
//! changed subtree and a hover change repaints without a relayout.

use std::cell::OnceCell;

use rustc_hash::FxHashSet;
use skene_foundation::{Inset, Layout, RunPlacement, Scene, Size};
use skene_layout::{
  Borders,
  BoxLayout,
  Place,
  taffy::{
    self,
    prelude::{AvailableSpace, length},
    style::{Display, Style},
  },
};

use crate::{
  Terminal,
  motion::{Clock, Presented},
  paint::{ShadowPaint, Shaped, line_holding, paragraph_scales, run_scale, sized_rows, wrap_sized},
  records::{BoxCache, Handle, Kind, PlainBox, PlainRun, Record, Records, RunCache},
  resolve::Env,
  text::{self, Scales},
};

/// The character standing in for one cell of an inline box in its paragraph's text.
pub(crate) const INLINE_BOX: char = '\u{fffc}';

/// What one restyle of a node decided.
struct Restyled {
  style: Style,
  text: Option<Measured>,
}

/// What the layout measures a text leaf by: its text, the scale of each stretch of it,
/// and its extents at its narrowest and widest, measured once a layout first asks.
#[derive(Debug, Clone)]
pub(crate) struct Measured {
  pub(crate) text: String,
  pub(crate) scales: Scales,
  shaped: OnceCell<Shaped>,
  extents: OnceCell<Extents>,
}

impl Measured {
  /// The measure of `text` at `scales`.
  pub(crate) const fn new(text: String, scales: Scales) -> Self {
    Self {
      text,
      scales,
      shaped: OnceCell::new(),
      extents: OnceCell::new(),
    }
  }

  /// Forgets the shape and the extents measured, after the terminal changed how it
  /// measures text.
  pub(crate) fn forget_extents(&mut self) {
    self.shaped.take();
    self.extents.take();
  }

  /// The cells and rows the text takes wrapped at `limit` cells: at its widest where
  /// that fits.
  fn size_at(&self, limit: usize) -> (usize, usize) {
    let extents = self.extents();

    if limit >= extents.widest.0 {
      extents.widest
    }
    else if limit == extents.longest_word {
      extents.narrowest
    }
    else {
      self.wrapped(limit)
    }
  }

  /// The text's extents at its narrowest and widest.
  fn extents(&self) -> Extents {
    *self.extents.get_or_init(|| {
      let longest_word = self.shaped().longest_word(&self.text, &self.scales);

      Extents {
        longest_word,
        narrowest: self.wrapped(longest_word),
        widest: self.wrapped(usize::MAX),
      }
    })
  }

  /// The text split into clusters and the places a line may end.
  fn shaped(&self) -> &Shaped {
    self.shaped.get_or_init(|| Shaped::of(&self.text))
  }

  /// The cells and rows the text takes wrapped at `limit` cells.
  fn wrapped(&self, limit: usize) -> (usize, usize) {
    self.shaped().extent(&self.text, limit, &self.scales)
  }
}

impl PartialEq for Measured {
  fn eq(&self, other: &Self) -> bool {
    self.text == other.text && self.scales == other.scales
  }
}

impl Eq for Measured {}

/// A text's extents: the widest stretch between two breaks, and its cells and rows
/// wrapped there and unwrapped.
#[derive(Debug, Clone, Copy)]
struct Extents {
  longest_word: usize,
  narrowest: (usize, usize),
  widest: (usize, usize),
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

/// The inline boxes of the paragraph `handle`, each with the cell its placeholder
/// flowed into, relative to the paragraph's origin, against the last layout.
pub(crate) fn inline_box_cells(records: &Records, handle: Handle) -> Vec<(Handle, (i64, i64))> {
  let runs = paragraph_runs(records, handle);

  if !runs.iter().any(|run| is_inline_box(records, *run)) {
    return Vec::new();
  }

  let (width, _) = records.node_size(handle);
  let scales = paragraph_scales(records, handle);
  let lines = wrap_sized(
    &paragraph_text(records, handle),
    usize::try_from(width).unwrap_or(1),
    &scales,
  );
  let rows = sized_rows(&lines);
  let mut offset = 0;
  let mut cells = Vec::new();

  for run in runs {
    let Some(record) = records.record(run)
    else {
      continue;
    };
    let start = offset;

    offset += text::count(&record.text);

    if !is_inline_box(records, run) {
      continue;
    }

    let Some((index, line)) = lines
      .iter()
      .enumerate()
      .rev()
      .find(|(_, line)| line.start <= start)
    else {
      continue;
    };
    // The box sits on the line's bottom rows, as a smaller cluster does; its line is at
    // least as tall as it.
    let column = line
      .advances(&scales)
      .take(start - line.start)
      .sum::<usize>();
    let row = rows[index] + line.rows.saturating_sub(scales.rows(start));

    cells.push((
      run,
      (
        i64::try_from(column).unwrap_or(0),
        i64::try_from(row).unwrap_or(0),
      ),
    ));
  }

  cells
}

/// The offset of the cluster of the paragraph `handle`'s text drawn at its local cell
/// (`column`, `row`), against the last layout: exact hits only, a smaller cluster or an
/// inline box held by the rows it draws on alone.
pub(crate) fn paragraph_cluster_at(
  records: &Records,
  handle: Handle,
  column: usize,
  row: usize,
) -> Option<usize> {
  let (width, _) = records.node_size(handle);
  let scales = paragraph_scales(records, handle);
  let lines = wrap_sized(
    &paragraph_text(records, handle),
    usize::try_from(width).unwrap_or(1),
    &scales,
  );
  let rows = sized_rows(&lines);
  let index = line_holding(&lines, row)?;
  let line = &lines[index];
  let offset = line.start + line.cluster_at(column, &scales)?;
  let top = rows[index] + line.rows.saturating_sub(scales.rows(offset));

  (row >= top).then_some(offset)
}

/// The leaf run of the paragraph `handle` that holds the cluster at `offset` of its
/// text.
pub(crate) fn paragraph_run_at(records: &Records, handle: Handle, offset: usize) -> Option<Handle> {
  let mut cursor = 0;

  paragraph_runs(records, handle).into_iter().find(|run| {
    let count = records
      .record(*run)
      .map_or(0, |record| text::count(&record.text));
    let hit = (cursor..cursor + count).contains(&offset);

    cursor += count;

    hit
  })
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
      measure_run,
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

    if records.tree.set_context(handle, restyled.text) {
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
  let moved = std::mem::replace(&mut records.interaction_seen, interaction) != interaction;
  let queued = std::mem::take(&mut records.restyle_queue);
  let moving = std::mem::take(&mut records.moving);
  let every = std::mem::take(&mut records.restyle_all) || records.structure_dirty;
  let (mut handles, displayed) = if every {
    (records.handles(), Some(records.displayed()))
  }
  else {
    let mut handles = queued;

    handles.extend(moving);

    if moved {
      handles.extend(
        records
          .interactive
          .iter()
          .copied()
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
  let generation = records.generation;
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
      records.interactive.remove(&handle);
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

    match reads_interaction {
      Some(true) => {
        records.interactive.insert(handle);
      }
      Some(false) => {
        records.interactive.remove(&handle);
      }
      None => {}
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
        props,
        cache,
        motion,
        ..
      } => {
        let target = resolved_run.map(|target| (target, resolved_layout));

        present_run(props, cache, motion, target, &run_keyframes, &frame)
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
      records.moving.push(handle);
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
    .map(|inner| &inner.props)
    .map(|props| {
      props
        .dynamics
        .animations
        .iter()
        .map(|animation| animation.map_keyframes(|frame| env.resolve_box_keyframe(frame)))
        .collect::<Vec<_>>()
    })
    .unwrap_or_default();
  let run_keyframes = match &record.kind {
    Kind::Run { props, .. } => props
      .dynamics
      .animations
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
  props: &skene_foundation::RunProps<crate::Terminal>,
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
      &props.dynamics,
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
    &inner.props.dynamics,
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
        Some(Measured::new(
          record.text.clone(),
          Scales::uniform(run_scale(records, handle)),
        ))
      };

      place.apply(&mut style);

      if inline && has_children {
        style.flex_wrap = taffy::style::FlexWrap::Wrap;
      }

      Restyled { style, text }
    }
    Kind::Anchor | Kind::Scope => Restyled {
      style: Style::default(),
      text: None,
    },
    Kind::Box(inner) => {
      let resolved = inner.style().cloned().unwrap_or_default();
      let paragraph = is_paragraph(records, handle);
      let mut style = resolved.taffy_style(inner.props.layout, place);

      if fills_root {
        style.flex_grow = 1.;
      }

      let text = paragraph.then(|| {
        Measured::new(
          paragraph_text(records, handle),
          paragraph_scales(records, handle),
        )
      });

      if paragraph {
        style.display = Display::Block;
      }

      Restyled { style, text }
    }
    Kind::Control { inner, field, .. } => {
      let resolved = inner.style().cloned().unwrap_or_default();
      let mut style = resolved.taffy_style(inner.props.layout, place);

      if let Some(field) = field {
        let chrome = chrome(&resolved);

        if resolved.width.is_none() {
          // One cell past the value's longest line keeps the caret inside the box at
          // its end.
          let widest = |text: &str| text.split('\n').map(text::cells).max().unwrap_or(0);
          let content = (widest(&field.value) + 1)
            .max(field.placeholder.as_deref().map_or(0, widest))
            .max(8);

          style.size.width = length(cell(
            u32::try_from(content).unwrap_or(u32::MAX) + chrome.start + chrome.end,
          ));
        }

        if resolved.height.is_none() {
          style.size.height = length(cell(u32::from(field.lines) + chrome.top + chrome.bottom));
        }
      }

      if fills_root {
        style.flex_grow = 1.;
      }

      Restyled { style, text: None }
    }
    Kind::Overlay { inner, open, .. } => {
      let resolved = inner.style().cloned().unwrap_or_default();
      let mut style = resolved.taffy_style(inner.props.layout, place.detached());

      style.position = taffy::style::Position::Absolute;

      if !*open {
        style.display = Display::None;
      }

      Restyled { style, text: None }
    }
  }
}

/// Hands every laid-out node its flattened children, where the tree changed since the
/// last layout.
pub(crate) fn rebuild_children(records: &mut Records) {
  if !std::mem::take(&mut records.structure_dirty) {
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
    measure_run,
  );
  records.layout_generation += 1;
  records.covers_stale = true;
}

/// Measures a text leaf: its text wrapped at the width constraint, in cells, each
/// cluster at its scale and each line as tall as its tallest.
fn measure_run(
  known: taffy::geometry::Size<Option<f32>>,
  available: taffy::geometry::Size<AvailableSpace>,
  measured: &Measured,
) -> Size<f32> {
  #[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a width is clamped non-negative and counts cells far below usize::MAX"
  )]
  let (cells, rows) = match (known.width, available.width) {
    (Some(width), _) | (None, AvailableSpace::Definite(width)) => {
      measured.size_at(width.max(0.) as usize)
    }
    (None, AvailableSpace::MinContent) => measured.extents().narrowest,
    (None, AvailableSpace::MaxContent) => measured.extents().widest,
  };

  #[expect(
    clippy::cast_precision_loss,
    reason = "a line's length in cells stays far below the integers f32 holds exactly"
  )]
  Size::new(
    known.width.unwrap_or(cells as f32),
    known.height.unwrap_or(rows as f32),
  )
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

/// The cells the resolved style's border and padding take on each edge.
fn chrome(style: &PlainBox) -> Inset<u32> {
  let edges = border_edges(style);
  let padding = style.padding.unwrap_or_default();

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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod properties {
  use proptest::prelude::*;

  use super::Measured;
  use crate::{
    paint::{Shaped, wrap_sized},
    text::{self, CellScale, Scales},
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

  proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Any text at any width and scale measures as the lines it wraps into, whether
    /// from its shape or from the extents a measure keeps.
    #[test]
    fn a_measure_matches_its_wrapped_lines(
      text in arbitrary_text(),
      width in 0..40_usize,
      scale in 1..4_u8,
    ) {
      text::reset();

      let scales = Scales::uniform(CellScale::Whole(scale));
      let lines = wrap_sized(&text, width, &scales);
      let wrapped = (
        lines
          .iter()
          .map(|line| line.advances(&scales).sum::<usize>())
          .max()
          .unwrap_or(0),
        lines.iter().map(|line| line.rows).sum::<usize>(),
      );
      let measured = Measured::new(text.clone(), scales.clone());

      prop_assert_eq!(Shaped::of(&text).extent(&text, width, &scales), wrapped);
      prop_assert_eq!(measured.size_at(width), wrapped);
    }
  }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod wrap_properties {
  use proptest::prelude::*;

  use crate::{
    paint::wrap_sized,
    text::{self, Scales},
  };

  /// Text mostly of the clusters wrapping turns on.
  fn arbitrary_text() -> impl Strategy<Value = String> {
    const PIECES: &[&str] = &[
      "a", "bc", "word", " ", "  ", "\t", "\n", "-", "日", "本語", "e\u{301}", "(x)",
    ];

    proptest::collection::vec(proptest::sample::select(PIECES), 0..24)
      .prop_map(|pieces| pieces.concat())
  }

  proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Any text wraps at any width into lines that fit, but for a lone cluster too wide
    /// for one, and that leave out nothing but the blanks a line ends in and the
    /// newlines between paragraphs.
    #[test]
    fn a_wrap_fits_and_keeps_the_text(text in arbitrary_text(), width in 1..30_usize) {
      text::reset();

      let scales = Scales::default();
      let lines = wrap_sized(&text, width, &scales);
      let clusters: Vec<&str> = text::clusters(&text).map(|(_, cluster)| cluster).collect();
      let mut expected_start = 0;

      prop_assert!(!lines.is_empty());

      for line in &lines {
        let cells: usize = line.advances(&scales).sum();
        let count = text::count(&line.text);

        prop_assert!(
          cells <= width || count == 1,
          "{:?} spans {} cells at width {}",
          line.text,
          cells,
          width
        );

        prop_assert!(line.start >= expected_start);

        for skipped in &clusters[expected_start..line.start] {
          prop_assert!(
            matches!(*skipped, " " | "\t" | "\n" | "\r\n"),
            "{skipped:?} was left out"
          );
        }

        prop_assert_eq!(
          clusters[line.start..line.start + count].concat(),
          line.text.clone()
        );

        expected_start = line.start + count;
      }

      for skipped in &clusters[expected_start..] {
        prop_assert!(matches!(*skipped, " " | "\t" | "\n"), "{skipped:?} was left out");
      }
    }
  }
}
