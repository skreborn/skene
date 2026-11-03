//! Presented styles to the layout tree: each node the drawn runtime presents mapped onto
//! its taffy style and the runtime's leaf (a text node's flow, an entry's native extent),
//! the flattening of anchors and scopes into their content, and the layout computation.
//!
//! Layout is taffy with `1.0 = 1 cell`. A node's taffy style is written only when the
//! style presented for it maps to one other than the one applied, so a keystroke relays
//! out the changed subtree and a hover change repaints without a relayout.

use skene_foundation::{Inset, Layout, Point, Rect, RunPlacement, Size};
use skene_layout::{
  BoxLayout,
  Leaf,
  Place,
  runtime::{Drawn, Flow, Measured},
  taffy::{
    self,
    prelude::{AvailableSpace, length},
    style::{Display, Style},
  },
};

use crate::{
  CellEngine,
  Terminal,
  records::{EntryState, Field, Handle, Kind, PlainBox, Record, Records},
};

/// How far into its cell a point stands for the cell: just past its left edge, so a caret
/// placed at a cell goes to the cell's left edge, where a cluster's edge lies there, and
/// otherwise to the nearer edge of the cluster whose block holds it: before a cluster from
/// the cell its block starts at, after a wide cluster from its second cell.
const INTO_CELL: f32 = 1. / 16.;

/// What a node's presented style maps to in the layout tree.
struct Restyled {
  style: Style,
  leaf: Option<Measured<CellEngine>>,
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

/// The flow the text node `handle` lays out as, when it holds one.
pub(crate) fn flow_of(records: &Records, handle: Handle) -> Option<&Flow<CellEngine>> {
  records.tree.context(handle)?.flow()
}

/// The inline boxes of the paragraph `handle`, in its flow's order, each a child of the
/// paragraph in the tree, which lays it out where the flow places it.
pub(crate) fn inline_boxes(records: &Records, handle: Handle) -> Vec<Handle> {
  flow_of(records, handle).map_or_else(Vec::new, |flow| {
    flow
      .boxes()
      .filter(|node| records.runtime.is_inline_box(*node))
      .collect()
  })
}

/// Presents the styles that may have moved through the runtime, and writes the taffy
/// styles and leaves of every node it presented that changed; `true` when any changed.
pub(crate) fn restyle(records: &mut Records) -> bool {
  let mut changed = false;

  for handle in records.runtime.present() {
    let restyled = taffy_style_of(records, handle);

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

/// The taffy style `handle` takes from its resolved style and its place in the tree.
fn taffy_style_of(records: &Records, handle: Handle) -> Restyled {
  let record = records.runtime.record(handle).expect("a live record");
  let layout_parent = records.runtime.layout_parent(handle);
  let in_stack = layout_parent
    .and_then(|parent| records.runtime.record(parent))
    .and_then(Record::layout)
    == Some(Layout::Stack);
  let place = Place {
    in_stack,
    borders: Terminal::BORDERS,
  };
  // The root's child fills the viewport, unless it sizes its own height.
  let fills_root = layout_parent == Some(records.runtime.root())
    && !record.is_overlay()
    && record
      .kind
      .box_style()
      .is_none_or(|style| style.height.is_none());

  match &record.kind {
    Kind::Run { facts, cache, .. } => {
      let mut style = Style::default();
      let inline = facts.placement != RunPlacement::Line;
      let has_children = !records.runtime.layout_children(handle).is_empty();

      // A wrapper's properties on a bare run: its flexibility, alignment, pinning and
      // grid placement, folded like a box's.
      if let Some(cache) = cache {
        cache.layout.fold_into(&mut style, place);
      }

      let text = if facts.placement == RunPlacement::Embedded {
        // An inline box lays out at its content's size, inside its paragraph.
        style.display = Display::Flex;

        None
      }
      else if record.text.is_empty() && has_children {
        style.display = Display::Flex;

        None
      }
      else {
        let mut flow = records.runtime.flow();

        flow.add_run(
          handle,
          &[],
          &record.text,
          Terminal::text_style(record.kind.run_style()),
        );

        Some(flow)
      };

      place.apply(&mut style);

      if inline && has_children {
        style.flex_wrap = taffy::style::FlexWrap::Wrap;
      }

      Restyled {
        style,
        leaf: text.map(|flow| Measured::Flow(Box::new(flow))),
      }
    }
    Kind::Box(inner) => {
      let resolved = inner.style().cloned().unwrap_or_default();
      let paragraph = records.runtime.is_paragraph(handle);
      let mut style = resolved.taffy_style(inner.facts.layout, place);

      if fills_root {
        style.flex_grow = 1.;
      }

      let leaf = paragraph.then(|| Measured::Flow(Box::new(paragraph_flow(records, handle))));

      if paragraph {
        style.display = Display::Block;
      }

      Restyled { style, leaf }
    }
    Kind::Control { inner, .. } => {
      let resolved = inner.style().cloned().unwrap_or_default();
      let mut style = resolved.taffy_style(inner.facts.layout, place);
      let leaf = records.field(handle).map(|field| {
        entry_leaf(
          &mut style,
          &resolved,
          field,
          records.entry(handle).as_deref(),
        )
      });

      if fills_root {
        style.flex_grow = 1.;
      }

      Restyled { style, leaf }
    }
    Kind::Overlay { inner, open, .. } => {
      let resolved = inner.style().cloned().unwrap_or_default();
      let mut style = resolved.taffy_style(inner.facts.layout, place.detached());

      style.position = taffy::style::Position::Absolute;

      if !*open {
        style.display = Display::None;
      }

      Restyled { style, leaf: None }
    }
  }
}

/// The flow the paragraph `handle` lays out as: each leaf run's text in its style, and
/// each inline box, which the tree sizes, each inside the group runs around it.
fn paragraph_flow(records: &Records, handle: Handle) -> Flow<CellEngine> {
  let mut flow = records.runtime.flow();

  records
    .runtime
    .visit_runs(handle, &mut Vec::new(), &mut |run, groups| {
      let Some(record) = records.runtime.record(run)
      else {
        return;
      };

      if records.runtime.is_inline_box(run) {
        flow.add_box(run, groups);
      }
      else {
        flow.add_run(
          run,
          groups,
          &record.text,
          Terminal::text_style(record.kind.run_style()),
        );
      }
    });

  flow
}

/// Hands every laid-out node its flattened children, where the tree changed since the
/// last layout and the restyle after it: a paragraph the inline boxes its flow names, in
/// order, and a group run inside one none.
pub(crate) fn rebuild_children(records: &mut Records) {
  if !records.runtime.restyle.take_structure() {
    return;
  }

  let handles = records.runtime.handles();

  for handle in handles {
    if !records.tree.contains(handle) {
      continue;
    }

    let mut children = if records.runtime.is_paragraph(handle) {
      flow_of(records, handle).map_or_else(Vec::new, |flow| flow.boxes().collect())
    }
    else {
      let children = records.runtime.layout_children(handle);
      let grouped = !children.is_empty()
        && records.runtime.is_inline_run(handle)
        && !records.runtime.is_inline_box(handle)
        && records.runtime.enclosing_paragraph(handle).is_some();

      if grouped { Vec::new() } else { children }
    };

    if handle == records.runtime.root() {
      children.extend(
        records
          .runtime
          .overlays
          .iter()
          .copied()
          .filter(|overlay| records.runtime.is_reachable(*overlay)),
      );
    }

    records.tree.set_children(handle, children);
  }

  records.runtime.layout_dirty = true;
}

/// Computes the layout at `width` x `height`, the root sized to the terminal.
pub(crate) fn compute(records: &mut Records, width: u16, height: u16) {
  let root = records.runtime.root();
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
  records.runtime.geometry_stale = true;
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
fn entry_leaf(
  style: &mut Style,
  resolved: &PlainBox,
  field: &Field,
  entry: Option<&EntryState>,
) -> Measured<CellEngine> {
  let chrome = chrome(resolved);
  let columns = (field.widest(entry) + 1)
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

  Measured::Entry(native)
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
mod tests {
  use skene_foundation::{Container, RichText, Span, View};
  use skene_layout::Leaf as _;

  use super::{content, flow_of};
  use crate::{Terminal, frame, install, mount, uninstall, with_records};

  /// Each leaf run and inline box of a paragraph flows inside the group runs around it,
  /// outermost first, and its flow places each group where the run or box it holds lies.
  #[test]
  fn a_paragraphs_runs_flow_inside_their_groups() {
    install();

    let paragraph = mount(|| {
      RichText::<Terminal>::new()
        .add_spans([
          Span::new("a"),
          Span::group([Span::group([Span::new("b")]), Span::view(Container::new())]),
        ])
        .build()
    });

    frame::settle(20, 1);

    with_records(|records| {
      let [a, outer] = records.runtime.layout_children(paragraph)[..]
      else {
        panic!("the paragraph holds a run and a group");
      };
      let [inner, inline] = records.runtime.layout_children(outer)[..]
      else {
        panic!("the outer group holds a group and an inline box");
      };
      let [b] = records.runtime.layout_children(inner)[..]
      else {
        panic!("the inner group holds a run");
      };
      let mut visited = Vec::new();

      records
        .runtime
        .visit_runs(paragraph, &mut Vec::new(), &mut |run, groups| {
          visited.push((run, groups.to_vec()));
        });

      assert_eq!(
        visited,
        [(a, vec![]), (b, vec![outer, inner]), (inline, vec![outer])]
      );

      let placed: Vec<_> = flow_of(records, paragraph)
        .expect("a paragraph measures by its flow")
        .flow()
        .place(content(20))
        .into_iter()
        .map(|(node, _)| node)
        .collect();

      assert_eq!(placed, [a, outer, inner, b, outer, inline]);
    });

    uninstall();
  }
}
