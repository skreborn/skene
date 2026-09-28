//! Resolved styles to the layout tree: the per-node restyle pass, the flattening of
//! anchors and scopes into their content, and the layout computation.
//!
//! Layout is taffy with `1.0 = 1 cell`. A node's taffy style is written only when the
//! style resolved for it differs from the one applied, so a keystroke relays out the
//! changed subtree and a hover change repaints without a relayout.

use skene_core::{Layout, Overflow, RunDisplay};
use taffy::{
  prelude::{
    AvailableSpace,
    TaffyAuto,
    TaffyGridLine,
    TaffyGridSpan,
    fit_content,
    fr,
    length,
    minmax,
    percent,
  },
  style::{Display, Style},
};

use crate::{
  motion::{Presented, Previous},
  paint::{longest_word, paragraph_scales, run_scale, sized_rows, wrap_sized},
  resolve::Env,
  store::{BoxCache, Handle, Kind, PlainBox, PlainRun, Record, RunCache, Store},
  text::{self, Scales},
};

/// The character standing in for one cell of an inline box in its paragraph's text.
pub(crate) const INLINE_BOX: char = '\u{fffc}';

/// What one restyle of a node decided.
struct Restyled {
  style: Style,
  text: Option<Measured>,
}

/// What the layout measures a text leaf by: its text, and the scale of each stretch
/// of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Measured {
  pub(crate) text: String,
  pub(crate) scales: Scales,
}

/// Whether `handle` lays its inline runs out as one paragraph: a flow box whose every
/// laid-out child is an inline text run or an inline box.
pub(crate) fn is_paragraph(store: &Store, handle: Handle) -> bool {
  if store.record(handle).and_then(Record::layout) != Some(Layout::Flow) {
    return false;
  }

  let children = store.layout_children(handle);

  !children.is_empty()
    && children.iter().all(|child| {
      matches!(
        store.record(*child).map(|record| &record.kind),
        Some(Kind::Run { props, .. }) if props.display != RunDisplay::Block
      )
    })
}

/// Whether `handle` is an inline box: a run laid out as a box among its paragraph's
/// text, its text the placeholder its measured width fills.
pub(crate) fn is_inline_box(store: &Store, handle: Handle) -> bool {
  matches!(
    store.record(handle).map(|record| &record.kind),
    Some(Kind::Run { props, .. }) if props.display == RunDisplay::InlineBox
  )
}

/// The inline boxes of the paragraph `handle`, each with the cell its placeholder
/// flowed into, relative to the paragraph's origin, against the last layout.
pub(crate) fn inline_box_cells(store: &Store, handle: Handle) -> Vec<(Handle, (i64, i64))> {
  let runs = paragraph_runs(store, handle);

  if !runs.iter().any(|run| is_inline_box(store, *run)) {
    return Vec::new();
  }

  let (width, _) = store.node_size(handle);
  let scales = paragraph_scales(store, handle);
  let lines = wrap_sized(
    &paragraph_text(store, handle),
    usize::try_from(width).unwrap_or(1),
    &scales,
  );
  let rows = sized_rows(&lines);
  let mut offset = 0;
  let mut cells = Vec::new();

  for run in runs {
    let Some(record) = store.record(run)
    else {
      continue;
    };
    let start = offset;

    offset += text::count(&record.text);

    if !is_inline_box(store, run) {
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
    // The box sits on the line's bottom row, as a smaller cluster does.
    let column = text::clusters(&line.text)
      .take(start - line.start)
      .enumerate()
      .map(|(index, (_, cluster))| scales.cells(line.start + index, cluster))
      .sum::<usize>();

    cells.push((
      run,
      (
        i64::try_from(column).unwrap_or(0),
        i64::try_from(rows[index] + line.rows - 1).unwrap_or(0),
      ),
    ));
  }

  cells
}

/// Lays every inline box out on its own and writes the placeholder its width fills
/// into its text; `true` when a placeholder changed, in which case its paragraph
/// measures again.
pub(crate) fn measure_inline_boxes(store: &mut Store) -> bool {
  let mut changed = false;

  for handle in store.handles() {
    if !is_inline_box(store, handle) {
      continue;
    }

    let Some(taffy) = store.record(handle).and_then(|record| record.taffy)
    else {
      continue;
    };

    store
      .taffy
      .compute_layout_with_measure(
        taffy,
        taffy::geometry::Size {
          width: AvailableSpace::MaxContent,
          height: AvailableSpace::MaxContent,
        },
        |known, available, _, measured, _| measure_run(known, available, measured.as_deref()),
      )
      .expect("compute an inline box's layout");

    let (width, _) = store.node_size(handle);
    let placeholder: String =
      std::iter::repeat_n(INLINE_BOX, usize::try_from(width).unwrap_or(1).max(1)).collect();
    let Some(record) = store.record_mut(handle)
    else {
      continue;
    };

    if record.text != placeholder {
      record.text = placeholder;
      changed = true;
    }
  }

  if changed {
    store.layout_dirty = true;
  }

  changed
}

/// The text of the paragraph `handle`: every inline run beneath it, in order.
pub(crate) fn paragraph_text(store: &Store, handle: Handle) -> String {
  let mut text = String::new();

  for run in paragraph_runs(store, handle) {
    if let Some(record) = store.record(run) {
      text.push_str(&record.text);
    }
  }

  text
}

/// The leaf runs of the paragraph `handle`, in order: a group run contributes its
/// runs, never its own (empty) text.
pub(crate) fn paragraph_runs(store: &Store, handle: Handle) -> Vec<Handle> {
  let mut runs = Vec::new();

  collect_runs(store, handle, &mut runs);

  runs
}

fn collect_runs(store: &Store, handle: Handle, runs: &mut Vec<Handle>) {
  for child in store.layout_children(handle) {
    let is_run = matches!(
      store.record(child).map(|record| &record.kind),
      Some(Kind::Run { .. })
    );

    if !is_run {
      continue;
    }

    if is_inline_box(store, child) || store.layout_children(child).is_empty() {
      runs.push(child);
    }
    else {
      collect_runs(store, child, runs);
    }
  }
}

/// Resolves every stale node's style and writes the taffy styles that changed; `true`
/// when any changed.
pub(crate) fn restyle(store: &mut Store) -> bool {
  let mut handles = store.handles();
  let mut changed = false;

  // A paragraph measures by its runs' scales, so the runs resolve first.
  handles.sort_by_key(|handle| {
    !matches!(
      store.record(*handle).map(|record| &record.kind),
      Some(Kind::Run { .. })
    )
  });

  store.motion_active = false;
  store.restyle_pending = false;

  let displayed = store.displayed();

  for handle in handles {
    let Some(restyled) = restyle_one(store, handle, displayed.contains(&handle))
    else {
      continue;
    };
    let Some(record) = store.record_mut(handle)
    else {
      continue;
    };
    let Some(taffy) = record.taffy
    else {
      continue;
    };

    if record.measured_text != restyled.text {
      record.measured_text.clone_from(&restyled.text);
      store
        .taffy
        .set_node_context(taffy, restyled.text)
        .expect("carry a run's text");
      changed = true;
    }

    let record = store.record_mut(handle).expect("checked live");

    if record.applied.as_ref() != Some(&restyled.style) {
      record.applied = Some(restyled.style.clone());
      store
        .taffy
        .set_style(taffy, restyled.style)
        .expect("style a node");
      store.set_style_calls += 1;
      changed = true;
    }
  }

  changed
}

/// Resolves `handle`'s style if stale or in motion, and returns the taffy style it maps
/// to; `None` for a node outside the layout. A node out of the display runs no motion
/// and forgets its presentation, so it shows afresh once `displayed` again.
fn restyle_one(store: &mut Store, handle: Handle, displayed: bool) -> Option<Restyled> {
  let generation = store.generation;
  let now = store.now();
  let reduced = store.reduced_motion;
  let record = store.record_mut(handle)?;

  if !record.kind.is_laid_out() {
    return None;
  }

  if !displayed {
    if !record.hidden {
      record.hidden = true;
      record.in_motion = false;
      record.kind.forget_presentation();
    }

    return Some(taffy_style_of(store, handle));
  }

  record.hidden = false;

  let record = store.record(handle)?;

  let interaction = store.interaction(handle);
  let fresh = match &record.kind {
    Kind::Run { cache, .. } => cache
      .as_ref()
      .is_some_and(|cache| cache.generation == generation && cache.interaction == interaction),
    kind => kind
      .box_record()
      .and_then(|inner| inner.cache.as_ref())
      .is_some_and(|cache| cache.generation == generation && cache.interaction == interaction),
  };
  let in_motion = record.in_motion;

  if !fresh || in_motion {
    let Targets {
      resolved_box,
      resolved_run,
      resolved_layout,
      box_keyframes,
      run_keyframes,
    } = targets(store, handle, fresh)?;
    let record = store.record_mut(handle)?;
    let frame = Frame {
      generation,
      interaction,
      now,
      reduced,
    };
    // A node whose presentation came out different from the last one repaints.
    let (repaint, active) = match &mut record.kind {
      Kind::Run {
        props,
        cache,
        motion,
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
    store.motion_active |= active;

    if repaint {
      store.damage(handle);
    }
  }

  Some(taffy_style_of(store, handle))
}

/// What a restyle presents: the resolved box or run target, the wrapper layout of a
/// run, and the keyframes of each.
struct Targets {
  resolved_box: Option<(PlainBox, Option<skene_core::Color>)>,
  resolved_run: Option<PlainRun>,
  resolved_layout: PlainBox,
  box_keyframes: Vec<skene_core::Animation<PlainBox>>,
  run_keyframes: Vec<skene_core::Animation<PlainRun>>,
}

/// The targets of `handle`: a fresh node in motion samples its clock over the targets
/// and keyframes it already resolved; only a stale one resolves again.
fn targets(store: &mut Store, handle: Handle, fresh: bool) -> Option<Targets> {
  let record = store.record(handle)?;
  let (resolved_box, resolved_run, resolved_layout, box_keyframes, run_keyframes) = if fresh {
    match &record.kind {
      Kind::Run { cache, .. } => {
        let cache = cache.as_ref()?;

        (
          None,
          Some(cache.target.clone()),
          cache.layout.clone(),
          Vec::new(),
          cache.keyframes.clone(),
        )
      }
      kind => {
        let cache = kind.box_record()?.cache.as_ref()?;

        (
          Some((cache.target.clone(), cache.target_ink)),
          None,
          PlainBox::default(),
          cache.keyframes.clone(),
          Vec::new(),
        )
      }
    }
  }
  else {
    let env = Env::of(store, handle);
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

    store.style_resolutions += 1;

    (
      resolved_box,
      resolved_run,
      resolved_layout,
      box_keyframes,
      run_keyframes,
    )
  };

  Some(Targets {
    resolved_box,
    resolved_run,
    resolved_layout,
    box_keyframes,
    run_keyframes,
  })
}

/// What one restyle resolves under: the generation and interaction it is cached as,
/// and the clock and preference its motion samples with.
struct Frame {
  generation: u64,
  interaction: crate::store::Interaction,
  now: f64,
  reduced: bool,
}

/// Presents a run's resolution over its motion and caches it; `(repaint, active)`.
fn present_run(
  props: &skene_core::RunProps<crate::Tui>,
  cache: &mut Option<RunCache>,
  motion: &mut crate::motion::RunMotion,
  resolved: Option<(PlainRun, PlainBox)>,
  keyframes: &[skene_core::Animation<PlainRun>],
  frame: &Frame,
) -> (bool, bool) {
  let mut active = false;
  let previous = cache.as_ref().map(|cache| Previous {
    presented: &cache.style,
    target: &cache.target,
  });
  let presented = resolved.map(|(target, layout)| {
    let Presented {
      style,
      active: running,
    } = motion.present(
      previous.as_ref(),
      target.clone(),
      &props.dynamics,
      keyframes,
      frame.now,
      frame.reduced,
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
  inner: &mut crate::store::BoxRecord,
  resolved: Option<(PlainBox, Option<skene_core::Color>)>,
  keyframes: &[skene_core::Animation<PlainBox>],
  frame: &Frame,
) -> (bool, bool) {
  let Some((target, target_ink)) = resolved
  else {
    return (false, false);
  };
  let previous = inner.cache.as_ref().map(|cache| Previous {
    presented: &cache.style,
    target: &cache.target,
  });
  let Presented {
    style,
    active: running,
  } = inner.motion.present(
    previous.as_ref(),
    target.clone(),
    &inner.props.dynamics,
    keyframes,
    frame.now,
    frame.reduced,
  );
  let (border_ink, inking) = inner.motion.present_ink(
    inner
      .cache
      .as_ref()
      .map(|cache| (cache.border_ink, cache.target_ink)),
    target_ink,
    &inner.props.dynamics,
    frame.now,
    frame.reduced,
  );
  let repaint = inner
    .cache
    .as_ref()
    .is_none_or(|cache| cache.style != style || cache.border_ink != border_ink);

  inner.cache = Some(BoxCache {
    generation: frame.generation,
    interaction: frame.interaction,
    style,
    target,
    border_ink,
    target_ink,
    keyframes: keyframes.to_vec(),
  });

  (repaint, running || inking)
}

/// The taffy style `handle` takes from its resolved style and its place in the tree.
fn taffy_style_of(store: &Store, handle: Handle) -> Restyled {
  let record = store.record(handle).expect("a live record");
  let layout_parent = store.layout_parent(handle);
  let in_stack = layout_parent
    .and_then(|parent| store.record(parent))
    .and_then(Record::layout)
    == Some(Layout::Stack);
  // The root's child fills the viewport, unless it sizes its own height.
  let fills_root = layout_parent == Some(store.root)
    && !record.is_overlay()
    && record
      .kind
      .box_style()
      .is_none_or(|style| style.height.is_none());

  match &record.kind {
    Kind::Run { props, cache, .. } => {
      let mut style = Style::default();
      let inline = props.display != RunDisplay::Block;
      let has_children = !store.layout_children(handle).is_empty();

      // A wrapper's properties on a bare run: its flexibility, alignment, pinning and
      // grid placement, folded like a box's.
      if let Some(cache) = cache {
        apply_box_sizing(&mut style, &cache.layout, in_stack);
      }

      let text = if props.display == RunDisplay::InlineBox {
        // An inline box lays out on its own, at its content's size.
        style.display = Display::Flex;

        None
      }
      else if record.text.is_empty() && has_children {
        style.display = Display::Flex;

        None
      }
      else {
        Some(Measured {
          text: record.text.clone(),
          scales: Scales::uniform(run_scale(store, handle)),
        })
      };

      if in_stack {
        stack_cell(&mut style);
      }

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
      let paragraph = is_paragraph(store, handle);
      let mut style = box_taffy_style(inner.props.layout, &resolved, in_stack);

      if fills_root {
        style.flex_grow = 1.;
      }

      let text = paragraph.then(|| Measured {
        text: paragraph_text(store, handle),
        scales: paragraph_scales(store, handle),
      });

      if paragraph {
        style.display = Display::Block;
      }

      Restyled { style, text }
    }
    Kind::Control { inner, field, .. } => {
      let resolved = inner.style().cloned().unwrap_or_default();
      let mut style = box_taffy_style(inner.props.layout, &resolved, in_stack);

      if let Some(field) = field {
        let border = u32::from(has_border(&resolved));
        let padding = resolved.padding.unwrap_or_default();

        if resolved.width.is_none() {
          // One cell past the value's longest line keeps the caret inside the box at
          // its end.
          let widest = |text: &str| text.split('\n').map(text::cells).max().unwrap_or(0);
          let content = (widest(&field.value) + 1)
            .max(field.placeholder.as_deref().map_or(0, widest))
            .max(8);

          style.size.width = length(cell(
            u32::try_from(content).unwrap_or(u32::MAX) + 2 * border + padding.left + padding.right,
          ));
        }

        if resolved.height.is_none() {
          style.size.height = length(cell(
            u32::from(field.lines) + 2 * border + padding.top + padding.bottom,
          ));
        }
      }

      if fills_root {
        style.flex_grow = 1.;
      }

      Restyled { style, text: None }
    }
    Kind::Overlay { inner, open, .. } => {
      let resolved = inner.style().cloned().unwrap_or_default();
      let mut style = box_taffy_style(inner.props.layout, &resolved, false);

      style.position = taffy::style::Position::Absolute;

      if !*open {
        style.display = Display::None;
      }

      Restyled { style, text: None }
    }
  }
}

/// Places a node in a stack's one cell.
fn stack_cell(style: &mut Style) {
  style.grid_row = taffy::geometry::Line {
    start: taffy::style::GridPlacement::from_line_index(1),
    end: taffy::style::GridPlacement::Auto,
  };
  style.grid_column = taffy::geometry::Line {
    start: taffy::style::GridPlacement::from_line_index(1),
    end: taffy::style::GridPlacement::Auto,
  };
}

/// Hands every laid-out node its flattened children, where the tree changed since the
/// last layout.
pub(crate) fn rebuild_children(store: &mut Store) {
  if !std::mem::take(&mut store.structure_dirty) {
    return;
  }

  let handles = store.handles();

  for handle in handles {
    let Some(record) = store.record(handle)
    else {
      continue;
    };
    let Some(taffy) = record.taffy
    else {
      continue;
    };

    if !record.kind.is_laid_out() {
      continue;
    }

    let mut children: Vec<taffy::NodeId> = if is_paragraph(store, handle) {
      Vec::new()
    }
    else {
      store
        .layout_children(handle)
        .into_iter()
        .filter_map(|child| store.record(child).and_then(|record| record.taffy))
        .collect()
    };

    if handle == store.root {
      let overlays: Vec<taffy::NodeId> = store
        .overlays
        .clone()
        .into_iter()
        .filter(|overlay| store.is_reachable(*overlay))
        .filter_map(|overlay| store.record(overlay).and_then(|record| record.taffy))
        .collect();

      children.extend(overlays);
    }

    let record = store.record_mut(handle).expect("checked live");

    if record.taffy_children != children {
      record.taffy_children.clone_from(&children);
      store
        .taffy
        .set_children(taffy, &children)
        .expect("set a node's children");
    }
  }

  store.layout_dirty = true;
}

/// Computes the layout at `width` x `height`, the root sized to the terminal.
pub(crate) fn compute(store: &mut Store, width: u16, height: u16) {
  let root_taffy = store
    .record(store.root)
    .and_then(|record| record.taffy)
    .expect("the root");

  store
    .taffy
    .set_style(
      root_taffy,
      Style {
        display: Display::Flex,
        flex_direction: taffy::style::FlexDirection::Column,
        size: taffy::geometry::Size {
          width: length(f32::from(width)),
          height: length(f32::from(height)),
        },
        ..Style::default()
      },
    )
    .expect("style the root");
  store
    .taffy
    .compute_layout_with_measure(
      root_taffy,
      taffy::geometry::Size {
        width: AvailableSpace::Definite(f32::from(width)),
        height: AvailableSpace::Definite(f32::from(height)),
      },
      |known, available, _, measured, _| measure_run(known, available, measured.as_deref()),
    )
    .expect("compute the layout");
  store.layout_generation += 1;
}

/// Measures a text leaf: its text wrapped at the width constraint, in cells, each
/// cluster at its scale and each line as tall as its tallest.
///
/// A leaf carrying no text measures to its known dimensions or zero.
fn measure_run(
  known: taffy::geometry::Size<Option<f32>>,
  available: taffy::geometry::Size<AvailableSpace>,
  measured: Option<&Measured>,
) -> taffy::geometry::Size<f32> {
  let Some(Measured { text, scales }) = measured
  else {
    return taffy::geometry::Size {
      width: known.width.unwrap_or(0.),
      height: known.height.unwrap_or(0.),
    };
  };

  #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  let limit = match (known.width, available.width) {
    (Some(width), _) | (None, AvailableSpace::Definite(width)) => width.max(0.) as usize,
    (None, AvailableSpace::MinContent) => longest_word(text, scales),
    (None, AvailableSpace::MaxContent) => usize::MAX,
  };
  let lines = wrap_sized(text, limit, scales);

  #[expect(clippy::cast_precision_loss)]
  taffy::geometry::Size {
    width: known.width.unwrap_or_else(|| {
      lines
        .iter()
        .map(|line| line.cells(scales))
        .max()
        .unwrap_or(0) as f32
    }),
    height: known
      .height
      .unwrap_or(lines.iter().map(|line| line.rows).sum::<usize>() as f32),
  }
}

/// The taffy style for a box: its layout mode plus the resolved style's sizing.
pub(crate) fn box_taffy_style(layout: Layout, style: &PlainBox, in_stack: bool) -> Style {
  let mut out = Style {
    display: match layout {
      Layout::Flow => Display::Block,
      Layout::Flex { .. } => Display::Flex,
      Layout::Grid | Layout::Stack | Layout::Single => Display::Grid,
    },
    ..Style::default()
  };

  if layout == Layout::Single {
    let single = vec![taffy::style::GridTemplateComponent::Single(fr(1.))];

    out.grid_template_columns = single.clone();
    out.grid_template_rows = single;
  }

  if let Layout::Flex { direction } = layout {
    out.flex_direction = match direction {
      skene_core::FlexDirection::Row => taffy::style::FlexDirection::Row,
      skene_core::FlexDirection::Column => taffy::style::FlexDirection::Column,
      skene_core::FlexDirection::RowReverse => taffy::style::FlexDirection::RowReverse,
      skene_core::FlexDirection::ColumnReverse => taffy::style::FlexDirection::ColumnReverse,
    };
  }

  apply_box_sizing(&mut out, style, in_stack);

  out
}

/// A [`Dimension`](skene_core::Dimension) in cells as a taffy dimension.
fn dimension(value: skene_core::Dimension<u32>) -> taffy::style::Dimension {
  match value {
    skene_core::Dimension::Length(cells) => length(cell(cells)),
    skene_core::Dimension::Fraction(value) => percent(value),
    skene_core::Dimension::Auto => taffy::style::Dimension::AUTO,
  }
}

/// A [`Dimension`](skene_core::Dimension) in cells as a taffy auto-capable length.
fn dimension_lpa(value: skene_core::Dimension<u32>) -> taffy::style::LengthPercentageAuto {
  match value {
    skene_core::Dimension::Length(cells) => length(cell(cells)),
    skene_core::Dimension::Fraction(value) => percent(value),
    skene_core::Dimension::Auto => taffy::style::LengthPercentageAuto::AUTO,
  }
}

/// A dimension inset as a taffy rect.
fn dimension_rect(
  inset: skene_core::Inset<skene_core::Dimension<u32>>,
) -> taffy::geometry::Rect<taffy::style::LengthPercentageAuto> {
  taffy::geometry::Rect {
    left: dimension_lpa(inset.left),
    right: dimension_lpa(inset.right),
    top: dimension_lpa(inset.top),
    bottom: dimension_lpa(inset.bottom),
  }
}

/// One [`TrackLimit`](skene_core::TrackLimit) as a taffy min-track sizing function.
fn min_track(limit: skene_core::TrackLimit<u32>) -> taffy::style::MinTrackSizingFunction {
  match limit {
    skene_core::TrackLimit::Length(cells) => length(cell(cells)),
    skene_core::TrackLimit::Fraction(value) => percent(value),
    skene_core::TrackLimit::Flex(_) | skene_core::TrackLimit::Auto => {
      taffy::style::MinTrackSizingFunction::AUTO
    }
    skene_core::TrackLimit::MinContent => taffy::prelude::min_content(),
    skene_core::TrackLimit::MaxContent => taffy::prelude::max_content(),
  }
}

/// One [`TrackLimit`](skene_core::TrackLimit) as a taffy max-track sizing function.
fn max_track(limit: skene_core::TrackLimit<u32>) -> taffy::style::MaxTrackSizingFunction {
  match limit {
    skene_core::TrackLimit::Length(cells) => length(cell(cells)),
    skene_core::TrackLimit::Fraction(value) => percent(value),
    skene_core::TrackLimit::Flex(value) => fr(value),
    skene_core::TrackLimit::Auto => taffy::style::MaxTrackSizingFunction::AUTO,
    skene_core::TrackLimit::MinContent => taffy::prelude::min_content(),
    skene_core::TrackLimit::MaxContent => taffy::prelude::max_content(),
  }
}

/// One [`GridTrack`](skene_core::GridTrack) as a taffy track sizing function.
fn track(value: skene_core::GridTrack<u32>) -> taffy::style::TrackSizingFunction {
  match value {
    skene_core::GridTrack::Length(cells) => length(cell(cells)),
    skene_core::GridTrack::Fraction(value) => percent(value),
    skene_core::GridTrack::Flex(value) => fr(value),
    skene_core::GridTrack::Auto => taffy::style::TrackSizingFunction::AUTO,
    skene_core::GridTrack::MinContent => taffy::prelude::min_content(),
    skene_core::GridTrack::MaxContent => taffy::prelude::max_content(),
    skene_core::GridTrack::FitContent(cells) => fit_content(length(cell(cells))),
    skene_core::GridTrack::Minmax(min, max) => minmax(min_track(min), max_track(max)),
  }
}

/// A track list as a taffy template.
fn template(
  tracks: &[skene_core::GridTrack<u32>],
) -> Vec<taffy::style::GridTemplateComponent<String>> {
  tracks
    .iter()
    .map(|entry| taffy::style::GridTemplateComponent::Single(track(*entry)))
    .collect()
}

/// One [`GridPlacement`](skene_core::GridPlacement) as taffy's.
fn placement(value: skene_core::GridPlacement) -> taffy::style::GridPlacement<String> {
  match value {
    skene_core::GridPlacement::Auto => taffy::style::GridPlacement::Auto,
    skene_core::GridPlacement::Line(index) => taffy::style::GridPlacement::from_line_index(index),
    skene_core::GridPlacement::Span(count) => taffy::style::GridPlacement::from_span(count),
  }
}

/// An [`Arrangement`](skene_core::Arrangement) as taffy's content alignment.
fn arrangement(value: skene_core::Arrangement) -> taffy::style::AlignContent {
  match value {
    skene_core::Arrangement::Start => taffy::style::AlignContent::Start,
    skene_core::Arrangement::End => taffy::style::AlignContent::End,
    skene_core::Arrangement::Center => taffy::style::AlignContent::Center,
    skene_core::Arrangement::Stretch => taffy::style::AlignContent::Stretch,
    skene_core::Arrangement::SpaceBetween => taffy::style::AlignContent::SpaceBetween,
    skene_core::Arrangement::SpaceAround => taffy::style::AlignContent::SpaceAround,
    skene_core::Arrangement::SpaceEvenly => taffy::style::AlignContent::SpaceEvenly,
  }
}

/// An [`ItemAlignment`](skene_core::ItemAlignment) as taffy's item alignment.
fn item(value: skene_core::ItemAlignment) -> taffy::style::AlignItems {
  match value {
    skene_core::ItemAlignment::Start => taffy::style::AlignItems::Start,
    skene_core::ItemAlignment::End => taffy::style::AlignItems::End,
    skene_core::ItemAlignment::Center => taffy::style::AlignItems::Center,
    skene_core::ItemAlignment::Baseline => taffy::style::AlignItems::Baseline,
    skene_core::ItemAlignment::Stretch => taffy::style::AlignItems::Stretch,
  }
}

/// Folds the resolved style's layout properties into `out`.
// One fold per property; length is the property count, not complexity.
#[expect(clippy::too_many_lines)]
fn apply_box_sizing(out: &mut Style, style: &PlainBox, in_stack: bool) {
  out.size = taffy::geometry::Size {
    width: style.width.map_or(taffy::style::Dimension::AUTO, dimension),
    height: style
      .height
      .map_or(taffy::style::Dimension::AUTO, dimension),
  };

  out.min_size = taffy::geometry::Size {
    width: style
      .min_width
      .map_or(taffy::style::Dimension::AUTO, dimension),
    height: style
      .min_height
      .map_or(taffy::style::Dimension::AUTO, dimension),
  };

  out.max_size = taffy::geometry::Size {
    width: style
      .max_width
      .map_or(taffy::style::Dimension::AUTO, dimension),
    height: style
      .max_height
      .map_or(taffy::style::Dimension::AUTO, dimension),
  };

  out.aspect_ratio = style.aspect_ratio;

  if let Some(margin) = style.margin {
    out.margin = dimension_rect(margin);
  }

  if let Some(padding) = style.padding {
    out.padding = taffy::geometry::Rect {
      left: length(cell(padding.left)),
      right: length(cell(padding.right)),
      top: length(cell(padding.top)),
      bottom: length(cell(padding.bottom)),
    };
  }

  // A pinned box leaves the flow; an offset one shifts within it. Pinned wins when
  // both are set.
  if let Some(pinned) = style.pinned {
    out.position = taffy::style::Position::Absolute;
    out.inset = dimension_rect(pinned);
  }
  else if let Some(offset) = style.offset {
    out.position = taffy::style::Position::Relative;
    out.inset = dimension_rect(offset);
  }

  if has_border(style) {
    out.border = taffy::geometry::Rect {
      left: length(1.),
      right: length(1.),
      top: length(1.),
      bottom: length(1.),
    };
  }

  if let Some(spacing) = style.spacing {
    out.gap = taffy::geometry::Size {
      width: length(cell(spacing.column)),
      height: length(cell(spacing.row)),
    };
  }

  if let Some(overflow) = style.overflow {
    let mapped = match overflow {
      Overflow::Visible => taffy::style::Overflow::Visible,
      Overflow::Hidden => taffy::style::Overflow::Hidden,
      Overflow::Scroll | Overflow::Auto => taffy::style::Overflow::Scroll,
    };

    out.overflow = taffy::geometry::Point {
      x: mapped,
      y: mapped,
    };
  }

  if let Some(wrap) = style.wrap {
    out.flex_wrap = match wrap {
      skene_core::Wrap::None => taffy::style::FlexWrap::NoWrap,
      skene_core::Wrap::Forward => taffy::style::FlexWrap::Wrap,
      skene_core::Wrap::Reverse => taffy::style::FlexWrap::WrapReverse,
    };
  }

  if let Some(flex) = style.flex {
    out.flex_basis = dimension(flex.initial);
    out.flex_grow = flex.grow_factor;
    out.flex_shrink = flex.shrink_factor;
  }

  out.justify_content = style.arrangement.map(arrangement);
  out.align_items = style.item_alignment.map(item);
  out.align_content = style.line_arrangement.map(arrangement);
  out.justify_items = style.item_horizontal_alignment.map(item);
  out.align_self = style.self_alignment.map(item);
  out.justify_self = style.self_horizontal_alignment.map(item);

  if let Some(tracks) = &style.grid_columns {
    out.grid_template_columns = template(tracks);
  }

  if let Some(tracks) = &style.grid_rows {
    out.grid_template_rows = template(tracks);
  }

  if let Some(tracks) = &style.grid_implicit_columns {
    out.grid_auto_columns = tracks.iter().map(|entry| track(*entry)).collect();
  }

  if let Some(tracks) = &style.grid_implicit_rows {
    out.grid_auto_rows = tracks.iter().map(|entry| track(*entry)).collect();
  }

  if let Some(flow) = style.grid_flow {
    out.grid_auto_flow = match flow {
      skene_core::GridFlow::Rows => taffy::style::GridAutoFlow::Row,
      skene_core::GridFlow::Columns => taffy::style::GridAutoFlow::Column,
      skene_core::GridFlow::DenseRows => taffy::style::GridAutoFlow::RowDense,
      skene_core::GridFlow::DenseColumns => taffy::style::GridAutoFlow::ColumnDense,
    };
  }

  if let Some(row) = style.grid_row_placement {
    out.grid_row = taffy::geometry::Line {
      start: placement(row.start),
      end: placement(row.end),
    };
  }

  if let Some(column) = style.grid_column_placement {
    out.grid_column = taffy::geometry::Line {
      start: placement(column.start),
      end: placement(column.end),
    };
  }

  if in_stack {
    stack_cell(out);
  }
}

/// Whether the resolved style draws a border: one of a width above zero, which the
/// terminal draws as one cell.
#[inline(always)]
pub(crate) fn has_border(style: &PlainBox) -> bool {
  style.border.as_ref().is_some_and(|border| border.width > 0)
}

/// A cell length as layout units.
#[inline(always)]
pub(crate) fn cell(value: u32) -> f32 {
  #[expect(clippy::cast_precision_loss)]
  {
    value as f32
  }
}
