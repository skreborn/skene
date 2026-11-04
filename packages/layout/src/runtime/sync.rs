//! The layout tree written from the scene: each presented node's style mapped onto its
//! taffy style and its leaf (a paragraph's flow, a text entry's native extent), every
//! laid-out node's children rebuilt over the scene walk, anchors and scopes flattened into
//! their content, and the layout computed at the window's size.

use skene_foundation::{Inset, Layout, NodeId, RunPlacement, Size};
use taffy::{
  prelude::{AvailableSpace, length},
  style::{Dimension, Display, FlexDirection, FlexWrap, LengthPercentageAuto, Position, Style},
};

use super::{Drawn, Flow, Kind, Measured, PlainBox, Record, Runtime};
use crate::{BoxLayout, Leaf, Place};

/// What a node's presented style maps to in the layout tree.
struct Restyled<P: Drawn> {
  style: Style,
  leaf: Option<Measured<P::Engine>>,
}

impl<P: Drawn> Runtime<P> {
  /// Presents every node whose style may have moved and writes the taffy style and the
  /// leaf of each into the layout tree, a text entry's leaf measuring at the native
  /// content extent [`Drawn::entry_extent`] reports; `true` where any changed.
  ///
  /// Every node is revisited once the definitions, the environment or the display
  /// moved; otherwise the nodes queued since the last pass, those in motion, and those
  /// whose last resolution read an interaction that has moved since, each with the
  /// paragraph it flows in. A revisited node resolves again where its resolution is
  /// stale, and presents over its motion where it resolved again or is in motion, all
  /// at one reading of the frame clock; one whose presentation changed is damaged, and
  /// one whose reach or visibility changed moves the covers ([`Runtime::covers_moved`])
  /// and the index of window rectangles ([`Runtime::frames`]). A node
  /// out of the display runs no motion and forgets its presentation, so it shows afresh
  /// once displayed again.
  ///
  /// A node's taffy style is written only where its presented style maps to one other
  /// than the one applied, so a change confined to paint lays nothing out again.
  pub fn write_styles(&mut self) -> bool {
    let mut changed = false;

    for node in self.present() {
      let restyled = self.restyled(node);

      if self.tree.set_context(node, restyled.leaf) {
        changed = true;
      }

      if self.tree.set_style(node, restyled.style) {
        self.style_writes += 1;
        changed = true;
      }
    }

    changed
  }

  /// Hands every laid-out node its flattened children, where the scene's structure moved
  /// since the last time: a paragraph the inline boxes its flow names, in order, a group
  /// run inside one none, and the root its overlays the scene reaches; the layout then
  /// due again ([`Runtime::layout_dirty`]).
  pub fn write_children(&mut self) {
    if !self.restyle.take_structure() {
      return;
    }

    for node in self.handles() {
      if !self.tree.contains(node) {
        continue;
      }

      let mut children = if self.is_paragraph(node) {
        self
          .flow_of(node)
          .map_or_else(Vec::new, |flow| flow.boxes().collect())
      }
      else {
        let children = self.layout_children(node);
        let grouped = !children.is_empty()
          && self.is_inline_run(node)
          && !self.is_inline_box(node)
          && self.enclosing_paragraph(node).is_some();

        if grouped { Vec::new() } else { children }
      };

      if node == self.root {
        children.extend(
          self
            .overlays
            .iter()
            .copied()
            .filter(|overlay| self.is_reachable(*overlay)),
        );
      }

      self.tree.set_children(node, children);
    }

    self.layout_dirty = true;
  }

  /// Computes the layout with the root sized to the window's `size`, a vertical flex
  /// box; a node's rectangle on the window may have moved.
  pub fn compute(&mut self, size: Size<f32>) {
    let root = self.root;

    self.tree.set_style(
      root,
      Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        size: taffy::geometry::Size {
          width: length(size.width),
          height: length(size.height),
        },
        ..Style::default()
      },
    );
    self.tree.compute(
      root,
      taffy::geometry::Size {
        width: AvailableSpace::Definite(size.width),
        height: AvailableSpace::Definite(size.height),
      },
    );
    self.layout_generation += 1;
    self.move_geometry();
  }

  /// The flow the text node `node` lays out as, where it holds one.
  #[must_use]
  pub fn flow_of(&self, node: NodeId) -> Option<&Flow<P::Engine>> {
    self.tree.context(node)?.flow()
  }

  /// The inline boxes of the paragraph `node`, in its flow's order, each a child of the
  /// paragraph in the layout tree, which lays it out where the flow places it.
  #[must_use]
  pub fn inline_boxes(&self, node: NodeId) -> Vec<NodeId> {
    self.flow_of(node).map_or_else(Vec::new, |flow| {
      flow
        .boxes()
        .filter(|node| self.is_inline_box(*node))
        .collect()
    })
  }

  /// The taffy style and the leaf `node` takes from its presented style and its place in
  /// the scene.
  fn restyled(&self, node: NodeId) -> Restyled<P> {
    let record = self.record(node).expect("a presented node has a record");
    let layout_parent = self.layout_parent(node);
    let in_stack = layout_parent
      .and_then(|parent| self.record(parent))
      .and_then(Record::layout)
      == Some(Layout::Stack);
    let place = Place {
      in_stack,
      borders: P::BORDERS,
    };
    // The root's child fills the window, unless it sizes its own height.
    let fills_root = layout_parent == Some(self.root)
      && !record.is_overlay()
      && record
        .kind
        .box_style()
        .is_none_or(|style| style.height.is_none());

    match &record.kind {
      Kind::Run { facts, cache, .. } => {
        let mut style = Style::default();
        let inline = facts.placement != RunPlacement::Line;
        let has_children = !self.layout_children(node).is_empty();

        // A wrapper's properties on a bare run: its flexibility, alignment, pinning and
        // grid placement, folded like a box's.
        if let Some(cache) = cache {
          cache.layout.fold_into(&mut style, place);
        }

        let text = if facts.placement == RunPlacement::Embedded
          || (record.text.is_empty() && has_children)
        {
          // An inline box lays out at its content's size, inside its paragraph; a run
          // holding runs and no text of its own lays them out.
          style.display = Display::Flex;

          None
        }
        else {
          let mut flow = self.flow();

          flow.add_run(
            node,
            &[],
            &record.text,
            P::text_style(record.kind.run_style()),
          );

          Some(flow)
        };

        place.apply(&mut style);

        if inline && has_children {
          style.flex_wrap = FlexWrap::Wrap;
        }

        Restyled {
          style,
          leaf: text.map(|flow| Measured::Flow(Box::new(flow))),
        }
      }
      Kind::Box(inner) => {
        let resolved = inner.style().cloned().unwrap_or_default();
        let paragraph = self.is_paragraph(node);
        let mut style = resolved.taffy_style(inner.facts.layout, place);

        if fills_root {
          style.flex_grow = 1.;
        }

        let leaf = paragraph.then(|| Measured::Flow(Box::new(self.paragraph_flow(node))));

        if paragraph {
          style.display = Display::Block;
        }

        Restyled { style, leaf }
      }
      Kind::Control { inner, .. } => {
        let resolved = inner.style().cloned().unwrap_or_default();
        let mut style = resolved.taffy_style(inner.facts.layout, place);
        let leaf = P::entry_extent(node, &record.platform).map(|native| {
          let chrome = Self::chrome(&resolved);

          native_size(
            &mut style.size.width,
            &mut style.min_size.width,
            style.max_size.width,
            native.width + chrome.start + chrome.end,
          );
          native_size(
            &mut style.size.height,
            &mut style.min_size.height,
            style.max_size.height,
            native.height + chrome.top + chrome.bottom,
          );

          Measured::Entry(native)
        });

        if fills_root {
          style.flex_grow = 1.;
        }

        Restyled { style, leaf }
      }
      Kind::Overlay { inner, open, .. } => {
        let resolved = inner.style().cloned().unwrap_or_default();
        let mut style = resolved.taffy_style(inner.facts.layout, place.detached());

        style.position = Position::Absolute;

        if !*open {
          style.display = Display::None;
        }

        Restyled { style, leaf: None }
      }
    }
  }

  /// The flow the paragraph `node` lays out as: each leaf run's text in its style, and
  /// each inline box, which the tree sizes, each inside the group runs around it.
  fn paragraph_flow(&self, node: NodeId) -> Flow<P::Engine> {
    let mut flow = self.flow();

    self.visit_runs(node, &mut Vec::new(), &mut |run, groups| {
      let Some(record) = self.record(run)
      else {
        return;
      };

      if self.is_inline_box(run) {
        flow.add_box(run, groups);
      }
      else {
        flow.add_run(
          run,
          groups,
          &record.text,
          P::text_style(record.kind.run_style()),
        );
      }
    });

    flow
  }

  /// The room a box styled `style` takes around its content on each edge: its border as
  /// the platform's layout takes it, and its padding, none below zero.
  fn chrome(style: &PlainBox<P>) -> Inset<f32> {
    let border = P::BORDERS.room(style);
    let padding = style.padding.unwrap_or_default();

    Inset {
      top: border.top + crate::extent(padding.top),
      end: border.end + crate::extent(padding.end),
      bottom: border.bottom + crate::extent(padding.bottom),
      start: border.start + crate::extent(padding.start),
    }
  }
}

/// Lays an entry out along one axis at its native extent `native`, its chrome included,
/// where its `size` is unset, as an `<input>`'s intrinsic size is: as its least, held to
/// its `max` and raised to its `min`, where each is a length or unset, and otherwise as
/// its size, which its `min` and `max` then bound.
fn native_size(
  size: &mut Dimension,
  min: &mut LengthPercentageAuto,
  max: LengthPercentageAuto,
  native: f32,
) {
  if !size.is_auto() {
    return;
  }

  let least = Dimension::from(*min).into_option();
  let most = Dimension::from(max).into_option();

  if (min.is_auto() || least.is_some()) && (max.is_auto() || most.is_some()) {
    let held = most.map_or(native, |most| native.min(most));

    *min = length(least.map_or(held, |least| held.max(least)));
  }
  else {
    *size = length(native);
  }
}

#[cfg(test)]
mod tests {
  use taffy::{
    prelude::{auto, length, percent},
    style::{Dimension, LengthPercentageAuto},
  };

  use super::native_size;

  /// The size and the least an entry `native` units long lays out at along an axis
  /// styled `size`, `min` and `max`.
  fn laid(
    size: Dimension,
    min: LengthPercentageAuto,
    max: LengthPercentageAuto,
    native: f32,
  ) -> (Dimension, LengthPercentageAuto) {
    let (mut size, mut min) = (size, min);

    native_size(&mut size, &mut min, max, native);

    (size, min)
  }

  /// An unsized entry lays out at its native extent as its least, so a layout
  /// stretching it may grow it and none shrinks it below: held to its most and raised to
  /// its least, its size left unset.
  #[test]
  fn an_unsized_entry_lays_out_at_least_at_its_native_extent() {
    assert_eq!(laid(auto(), auto(), auto(), 12.), (auto(), length(12.)));
    assert_eq!(laid(auto(), auto(), length(8.), 12.), (auto(), length(8.)));
    assert_eq!(
      laid(auto(), length(20.), auto(), 12.),
      (auto(), length(20.))
    );
    assert_eq!(
      laid(auto(), length(20.), length(8.), 12.),
      (auto(), length(20.)),
      "the least wins over the most, as CSS's min-width wins over its max-width"
    );
  }

  /// A sized entry keeps its size, its least and its most, its native extent unread.
  #[test]
  fn a_sized_entry_keeps_its_size() {
    for size in [length(5.), percent(0.5)] {
      assert_eq!(laid(size, length(3.), length(9.), 12.), (size, length(3.)));
    }
  }

  /// An unsized entry whose least or most is a percentage lays out at its native extent
  /// as its size, which the percentage then bounds, its least kept.
  #[test]
  fn a_percentage_bound_lays_an_unsized_entry_out_at_its_native_size() {
    assert_eq!(
      laid(auto(), percent(0.5), auto(), 12.),
      (length(12.), percent(0.5))
    );
    assert_eq!(
      laid(auto(), auto(), percent(0.5), 12.),
      (length(12.), auto())
    );
  }
}
