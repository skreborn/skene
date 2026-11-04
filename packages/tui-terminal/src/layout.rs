//! The terminal's cell geometry over the drawn runtime's layout, which is taffy with
//! `1.0 = 1 cell`: a text node's content box and the point standing for one of its cells,
//! counts of cells as the layout counts them, and the edges a box's border takes in
//! cells.
//!
//! The runtime writes the layout tree from the scene and computes it; the terminal hands
//! it only a text entry's native extent ([`Field::extent`](crate::records::Field::extent)).

use skene_foundation::{Inset, Point, Rect, Size};

use crate::records::PlainBox;

/// How far into its cell a point stands for the cell: just past its left edge, so a caret
/// placed at a cell goes to the cell's left edge, where a cluster's edge lies there, and
/// otherwise to the nearer edge of the cluster whose block holds it: before a cluster from
/// the cell its block starts at, after a wide cluster from its second cell.
const INTO_CELL: f32 = 1. / 16.;

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

/// Whether the resolved style draws its border on any edge.
#[inline(always)]
pub(crate) fn has_border(style: &PlainBox) -> bool {
  border_edges(style) != Inset::default()
}

#[cfg(test)]
mod tests {
  use skene_foundation::{Container, RichText, Span, View};
  use skene_layout::Leaf as _;

  use super::content;
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

      let placed: Vec<_> = records
        .runtime
        .flow_of(paragraph)
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
