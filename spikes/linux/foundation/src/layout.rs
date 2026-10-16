//! Layout: the resolved layout record, the host protocol a toolkit implements, and the
//! engine (taffy) that runs one small tree per container over it.

use taffy::{
  AvailableSpace, Cache, CacheTree, Layout, LayoutInput, LayoutOutput, LayoutPartialTree,
  NodeId as TaffyId, RoundTree, RunMode, Size, SizingMode, Style,
  geometry::{Line, Rect as TaffyRect},
  prelude::{TaffyAuto, TaffyGridLine, TaffyGridSpan, fr, length, percent},
  style::{self as ts, Display},
  tree::RequestedAxis,
};

use std::rc::Rc;

use crate::{
  seam::{
    node::LayoutKind,
    record::{Prop, Resolved},
    style::{CompiledStyle, KeptLeaf},
  },
  text::{MeasureText, Space},
  value::{
    Alignment, Arrangement, AxisAlignment, Dimension, Edges, Flexibility, Flow, GridLine,
    GridPlacement, GridTrack, Inset, Spacing, Tracks,
  },
};

/// A node's layout properties, resolved: a container's own and its placement in its
/// parent, what [`crate::seam::Realization::apply_layout`] receives.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ResolvedLayout {
  /// The width.
  pub width: Option<Dimension>,
  /// The height.
  pub height: Option<Dimension>,
  /// The least width.
  pub min_width: Option<Dimension>,
  /// The least height.
  pub min_height: Option<Dimension>,
  /// The greatest width.
  pub max_width: Option<Dimension>,
  /// The greatest height.
  pub max_height: Option<Dimension>,
  /// Space inside the edges.
  pub padding: Option<Inset>,
  /// Space between neighbors.
  pub spacing: Option<Spacing>,
  /// Distribution along a row's or a column's axis.
  pub arrangement: Option<Arrangement>,
  /// Alignment across a row's or a column's axis.
  pub item_alignment: Option<AxisAlignment>,
  /// Alignment of a container's, a stack's or a grid's children in their cells.
  pub alignment: Option<Alignment>,
  /// Distribution of a wrapping row's lines.
  pub line_arrangement: Option<Arrangement>,
  /// Whether a row wraps.
  pub wrap: bool,
  /// A grid's columns.
  pub columns: Option<Tracks>,
  /// A grid's rows.
  pub rows: Option<Tracks>,
  /// A grid's implicit columns.
  pub implicit_columns: Option<Tracks>,
  /// A grid's implicit rows.
  pub implicit_rows: Option<Tracks>,
  /// A grid's placement order.
  pub flow: Option<Flow>,
  /// Grow and shrink weights in a row or a column.
  pub flexibility: Option<Flexibility>,
  /// Alignment in its own cell or across its row or column.
  pub self_alignment: Option<Alignment>,
  /// Pinned edges in a stack.
  pub pin: Option<Edges>,
  /// A cell in a grid.
  pub cell: Option<GridPlacement>,
}

impl ResolvedLayout {
  /// The layout properties of a style compiled with nothing kept.
  ///
  /// # Panics
  ///
  /// In a debug build, where a layout value kept a condition or a token.
  #[must_use]
  pub fn from_compiled(style: &CompiledStyle) -> Self {
    let mut layout = Self::default();

    for (prop, value) in &style.props {
      debug_assert!(value.variants.is_empty(), "layout is resolved whole");

      let KeptLeaf::Value(resolved) = &value.base else {
        debug_assert!(false, "layout is resolved whole");

        continue;
      };

      layout.put(*prop, resolved);
    }

    layout
  }

  fn put(&mut self, prop: Prop, value: &Resolved) {
    use Resolved as R;

    match (prop, value) {
      (Prop::Width, R::Dimension(v)) => self.width = Some(*v),
      (Prop::Height, R::Dimension(v)) => self.height = Some(*v),
      (Prop::MinWidth, R::Dimension(v)) => self.min_width = Some(*v),
      (Prop::MinHeight, R::Dimension(v)) => self.min_height = Some(*v),
      (Prop::MaxWidth, R::Dimension(v)) => self.max_width = Some(*v),
      (Prop::MaxHeight, R::Dimension(v)) => self.max_height = Some(*v),
      (Prop::Padding, R::Inset(v)) => self.padding = Some(*v),
      (Prop::Spacing, R::Spacing(v)) => self.spacing = Some(*v),
      (Prop::Arrangement, R::Arrangement(v)) => self.arrangement = Some(*v),
      (Prop::ItemAlignment, R::AxisAlignment(v)) => self.item_alignment = Some(*v),
      (Prop::Alignment, R::Alignment(v)) => self.alignment = Some(*v),
      (Prop::LineArrangement, R::Arrangement(v)) => self.line_arrangement = Some(*v),
      (Prop::Wrap, R::Flag(v)) => self.wrap = *v,
      (Prop::Columns, R::Tracks(v)) => self.columns = Some(v.clone()),
      (Prop::Rows, R::Tracks(v)) => self.rows = Some(v.clone()),
      (Prop::ImplicitColumns, R::Tracks(v)) => self.implicit_columns = Some(v.clone()),
      (Prop::ImplicitRows, R::Tracks(v)) => self.implicit_rows = Some(v.clone()),
      (Prop::Flow, R::Flow(v)) => self.flow = Some(*v),
      (Prop::Flexibility, R::Flexibility(v)) => self.flexibility = Some(*v),
      (Prop::SelfAlignment, R::Alignment(v)) => self.self_alignment = Some(*v),
      (Prop::Pin, R::Edges(v)) => self.pin = Some(*v),
      (Prop::Cell, R::GridPlacement(v)) => self.cell = Some(*v),
      _ => {}
    }
  }
}

/// An axis.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Axis {
  /// Left to right.
  Horizontal,
  /// Top to bottom.
  Vertical,
}

/// What a toolkit says of a widget along one axis.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Measurement {
  /// The least it can be drawn at.
  pub minimum: f32,
  /// The size it would like.
  pub natural: f32,
  /// Its first baseline from its top at its natural size, measured vertically.
  pub baseline: Option<f32>,
}

/// A rectangle in pixels.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rect {
  /// The left edge.
  pub x: f32,
  /// The top edge.
  pub y: f32,
  /// The width.
  pub width: f32,
  /// The height.
  pub height: f32,
}

/// The host protocol a toolkit implements once, for the engine to lay a container's
/// children out: measure a child through the toolkit's own protocol, and allocate it.
///
/// The toolkit measures its own widgets; the engine measures Skene containers; a Skene
/// container inside a native one is measured by the toolkit as any widget.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a toolkit's layout host",
  label = "the engine lays out through a host here",
  note = "a realization implements `LayoutHost` once: measure a child through the toolkit and allocate it"
)]
pub trait LayoutHost: 'static {
  /// A child, as the toolkit holds it.
  type Child: Clone;

  /// Measures `child` along `axis`, given the other axis's size where known (a height for
  /// a width).
  fn measure(child: &Self::Child, axis: Axis, for_size: Option<f32>) -> Measurement;

  /// Whether `child`'s height does not depend on its width.
  fn constant_height(child: &Self::Child) -> bool;

  /// Whether `child` is shown; a hidden child takes no room.
  fn is_visible(child: &Self::Child) -> bool;

  /// Whether `child` is a Skene container, which keeps CSS's automatic minimum rather than
  /// the toolkit's floor.
  fn is_container(child: &Self::Child) -> bool;

  /// The text `child` lays out, where it is a text leaf the engine measures through the
  /// realization's text engine rather than through the toolkit: CSS's sizing then holds,
  /// with no toolkit minimum as a floor.
  fn text(child: &Self::Child) -> Option<Rc<dyn MeasureText>> {
    let _ = child;

    None
  }

  /// Places `child` at `rect` (whole pixels, relative to the container) and hands a Skene
  /// child the fractional rectangle the engine computed (in the window), so nested
  /// containers round as one tree would.
  fn allocate(child: &Self::Child, rect: Rect, fractional: Rect, baseline: Option<f32>);
}

struct TreeNode<C> {
  style: Style,
  base: Style,
  children: Vec<usize>,
  child: Option<C>,
  cache: Cache,
  unrounded: Layout,
  layout: Layout,
}

/// One container's layout: a small taffy tree of the container and one leaf per child,
/// each child measured through the host protocol (engine variant T4).
pub struct ContainerLayout<H: LayoutHost> {
  kind: LayoutKind,
  own: ResolvedLayout,
  rtl: bool,
  nodes: Vec<TreeNode<H::Child>>,
  placements: Vec<ResolvedLayout>,
}

impl<H: LayoutHost> ContainerLayout<H> {
  /// An empty container of `kind` with its own layout `own`.
  #[must_use]
  pub fn new(kind: LayoutKind, own: ResolvedLayout) -> Self {
    let mut this = Self {
      kind,
      own,
      rtl: false,
      nodes: Vec::new(),
      placements: Vec::new(),
    };

    this.rebuild(Vec::new());

    this
  }

  /// Sets the container's own layout.
  pub fn set_own(&mut self, own: ResolvedLayout) {
    if own != self.own {
      self.own = own;
      self.rebuild(self.children());
    }
  }

  /// Sets the direction, right to left where `rtl`.
  pub fn set_rtl(&mut self, rtl: bool) {
    if rtl != self.rtl {
      self.rtl = rtl;
      self.rebuild(self.children());
    }
  }

  /// Sets the children, each with its placement and outer sizing.
  pub fn set_children(&mut self, children: Vec<(H::Child, ResolvedLayout)>) {
    self.rebuild(children);
  }

  fn children(&self) -> Vec<(H::Child, ResolvedLayout)> {
    self.nodes[1..]
      .iter()
      .zip(&self.placements)
      .filter_map(|(node, placement)| Some((node.child.clone()?, placement.clone())))
      .collect()
  }

  fn rebuild(&mut self, children: Vec<(H::Child, ResolvedLayout)>) {
    let root = taffy_style(self.kind, &self.own, None, Part::Inner, self.rtl);

    self.nodes = vec![TreeNode::new(root, None)];
    self.placements = Vec::new();

    for (child, placement) in children {
      let style = taffy_style(
        self.kind,
        &placement,
        Some(self.kind),
        Part::Outer,
        self.rtl,
      );
      let index = self.nodes.len();

      self.nodes.push(TreeNode::new(style, Some(child)));
      self.nodes[0].children.push(index);
      self.placements.push(placement);
    }
  }

  /// Writes each toolkit child's minimum into its style where its layout sets none, and
  /// takes a hidden child out: a widget cannot be drawn below its toolkit minimum, where
  /// CSS would let a stretch go below.
  fn refresh(&mut self) {
    for node in &mut self.nodes {
      node.cache.clear();

      let Some(child) = &node.child else {
        continue;
      };

      node.style = node.base.clone();

      if !H::is_visible(child) {
        node.style.display = Display::None;

        continue;
      }

      if H::is_container(child) || H::text(child).is_some() {
        continue;
      }

      let auto = ts::LengthPercentageAuto::AUTO;

      if node.base.min_size.width == auto {
        node.style.min_size.width = length(H::measure(child, Axis::Horizontal, None).minimum);
      }

      if node.base.min_size.height == auto && H::constant_height(child) {
        node.style.min_size.height = length(H::measure(child, Axis::Vertical, None).minimum);
      }
    }
  }

  /// The container's measurement along `axis`, given the other axis's size where known: a
  /// toolkit's measure hook calls it.
  pub fn measure(&mut self, axis: Axis, for_size: Option<f32>) -> Measurement {
    self.refresh();

    let space =
      |value: Option<f32>| value.map_or(AvailableSpace::MaxContent, AvailableSpace::Definite);

    match axis {
      Axis::Horizontal => {
        let known = Size {
          width: None,
          height: for_size,
        };
        let minimum = self
          .size(known, AvailableSpace::MinContent, space(for_size), false)
          .size
          .width;
        let natural = self
          .size(known, AvailableSpace::MaxContent, space(for_size), false)
          .size
          .width;

        Measurement {
          minimum: minimum.ceil(),
          natural: natural.ceil().max(minimum.ceil()),
          baseline: None,
        }
      }
      Axis::Vertical => {
        let known = Size {
          width: for_size,
          height: None,
        };
        let minimum = self
          .size(known, space(for_size), AvailableSpace::MinContent, false)
          .size
          .height
          .ceil();
        let output = self.size(known, space(for_size), AvailableSpace::MaxContent, true);
        let natural = output.size.height.ceil();

        Measurement {
          minimum: minimum.min(natural),
          natural,
          baseline: output.baselines.first,
        }
      }
    }
  }

  fn size(
    &mut self,
    known: Size<Option<f32>>,
    width: AvailableSpace,
    height: AvailableSpace,
    perform: bool,
  ) -> LayoutOutput {
    self.compute_child_layout(
      TaffyId::from(0usize),
      LayoutInput {
        run_mode: if perform {
          RunMode::PerformLayout
        } else {
          RunMode::ComputeSize
        },
        sizing_mode: SizingMode::InherentSize,
        axis: RequestedAxis::Both,
        known_dimensions: known,
        known_dimensions_are_definite: Size {
          width: true,
          height: true,
        },
        parent_size: Size::NONE,
        available_space: Size { width, height },
        vertical_margins_are_collapsible: Line::FALSE,
      },
    )
  }

  /// Lays the children out in `size` and allocates each through the host: a toolkit's
  /// allocate hook calls it, with the fractional rectangle a Skene parent handed this
  /// container where it has one, inset by whatever the toolkit draws inside the container's
  /// edges (a border), so it is the rectangle `size` is.
  pub fn allocate(&mut self, size: (f32, f32), fractional: Option<Rect>) {
    self.refresh();

    let origin = fractional.unwrap_or(Rect {
      x: 0.,
      y: 0.,
      width: size.0,
      height: size.1,
    });
    let size = Size {
      width: origin.width,
      height: origin.height,
    };
    let output = self.compute_child_layout(
      TaffyId::from(0usize),
      LayoutInput {
        run_mode: RunMode::PerformLayout,
        sizing_mode: SizingMode::InherentSize,
        axis: RequestedAxis::Both,
        known_dimensions: size.map(Some),
        known_dimensions_are_definite: Size {
          width: true,
          height: true,
        },
        parent_size: size.map(Some),
        available_space: size.map(AvailableSpace::Definite),
        vertical_margins_are_collapsible: Line::FALSE,
      },
    );
    let mut root = Layout::with_order(0);

    root.size = output.size;
    root.location = taffy::Point {
      x: origin.x,
      y: origin.y,
    };

    self.nodes[0].unrounded = root;
    taffy::round_layout(self, TaffyId::from(0usize));

    for node in &self.nodes[1..] {
      let Some(child) = &node.child else {
        continue;
      };

      if node.style.display == Display::None {
        continue;
      }

      let (rounded, unrounded) = (node.layout, node.unrounded);

      H::allocate(
        child,
        Rect {
          x: rounded.location.x,
          y: rounded.location.y,
          width: rounded.size.width,
          height: rounded.size.height,
        },
        Rect {
          x: origin.x + unrounded.location.x,
          y: origin.y + unrounded.location.y,
          width: unrounded.size.width,
          height: unrounded.size.height,
        },
        None,
      );
    }
  }
}

impl<H: LayoutHost> ContainerLayout<H> {
  /// A grid's first baseline, once its children are laid out: the first baseline of the first
  /// child in order in its first row, as CSS's grid takes it, where that child has one.
  fn first_row_baseline(&self) -> Option<f32> {
    let placed: Vec<&TreeNode<H::Child>> = self.nodes[0]
      .children
      .iter()
      .map(|index| &self.nodes[*index])
      .filter(|node| {
        node.style.display != Display::None && node.style.position != ts::Position::Absolute
      })
      .collect();
    let first_row_end = placed
      .iter()
      .map(|node| node.unrounded.location.y + node.unrounded.size.height)
      .fold(f32::INFINITY, f32::min);
    let first = placed
      .into_iter()
      .find(|node| node.unrounded.location.y < first_row_end || node.unrounded.size.height == 0.)?;
    let child = first.child.as_ref()?;
    let baseline = match H::text(child) {
      Some(text) => {
        text
          .measure(
            (Some(first.unrounded.size.width), None),
            (Space::MaxContent, Space::MaxContent),
          )
          .first_baseline
      }
      None => {
        H::measure(
          child,
          Axis::Vertical,
          Some(first.unrounded.size.width.round()),
        )
        .baseline
      }
    };

    baseline.map(|baseline| first.unrounded.location.y + baseline)
  }
}

impl<C> TreeNode<C> {
  fn new(style: Style, child: Option<C>) -> Self {
    Self {
      base: style.clone(),
      style,
      children: Vec::new(),
      child,
      cache: Cache::new(),
      unrounded: Layout::with_order(0),
      layout: Layout::with_order(0),
    }
  }
}

/// Which part of a node's layout a tree holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
  /// The container at its own tree's root: its mode, padding and pixel sizes.
  Inner,
  /// A child in its parent's tree: its outer sizing and its placement.
  Outer,
}

fn dimension(value: Dimension) -> ts::Dimension {
  match value {
    Dimension::Auto => ts::Dimension::AUTO,
    Dimension::Length(value) => length(value),
    Dimension::Fraction(value) => percent(value),
  }
}

fn bound(value: Dimension) -> ts::LengthPercentageAuto {
  match value {
    Dimension::Auto => ts::LengthPercentageAuto::AUTO,
    Dimension::Length(value) => length(value),
    Dimension::Fraction(value) => percent(value),
  }
}

fn track(value: GridTrack) -> ts::TrackSizingFunction {
  match value {
    GridTrack::Length(value) => length(value),
    GridTrack::Fraction(value) => percent(value),
    GridTrack::Flex(value) => fr(value),
    GridTrack::Auto => ts::TrackSizingFunction::AUTO,
  }
}

const fn arrangement(value: Arrangement) -> ts::AlignContent {
  match value {
    Arrangement::Start => ts::AlignContent::START,
    Arrangement::Center => ts::AlignContent::CENTER,
    Arrangement::End => ts::AlignContent::END,
    Arrangement::SpaceBetween => ts::AlignContent::SPACE_BETWEEN,
    Arrangement::SpaceAround => ts::AlignContent::SPACE_AROUND,
    Arrangement::SpaceEvenly => ts::AlignContent::SPACE_EVENLY,
  }
}

const fn item(value: AxisAlignment) -> ts::AlignItems {
  match value {
    AxisAlignment::Start => ts::AlignItems::START,
    AxisAlignment::Center => ts::AlignItems::CENTER,
    AxisAlignment::End => ts::AlignItems::END,
    AxisAlignment::Stretch => ts::AlignItems::STRETCH,
    AxisAlignment::Baseline => ts::AlignItems::BASELINE,
  }
}

fn line(value: GridLine) -> Line<ts::GridPlacement> {
  Line {
    start: value
      .start
      .map_or(ts::GridPlacement::Auto, ts::GridPlacement::from_line_index),
    end: ts::GridPlacement::from_span(value.span),
  }
}

fn tracks(value: &Tracks) -> Vec<ts::GridTemplateComponent<String>> {
  value
    .0
    .iter()
    .map(|value| ts::GridTemplateComponent::Single(track(*value)))
    .collect()
}

/// The taffy style of a node of `kind` (its own, or its parent's where it is a child) with
/// `layout`, the logical edges flipped under right to left: a port of old `skene-layout`'s
/// fold, which the layout spike held to taffy's whole-tree reference within 1px.
fn taffy_style(
  kind: LayoutKind,
  layout: &ResolvedLayout,
  parent: Option<LayoutKind>,
  part: Part,
  rtl: bool,
) -> Style {
  let mut out = Style {
    direction: if rtl {
      ts::Direction::Rtl
    } else {
      ts::Direction::Ltr
    },
    ..Style::default()
  };
  let horizontal = |leading: f32, trailing: f32| {
    if rtl {
      (trailing, leading)
    } else {
      (leading, trailing)
    }
  };

  if part == Part::Inner {
    out.display = match kind {
      LayoutKind::Row | LayoutKind::Column => Display::Flex,
      _ => Display::Grid,
    };

    match kind {
      LayoutKind::Row => out.flex_direction = ts::FlexDirection::Row,
      LayoutKind::Column => out.flex_direction = ts::FlexDirection::Column,
      LayoutKind::Box | LayoutKind::Scroll { .. } | LayoutKind::Stack => {
        out.grid_template_columns = vec![ts::GridTemplateComponent::Single(fr(1.))];
        out.grid_template_rows = vec![ts::GridTemplateComponent::Single(fr(1.))];
      }
      LayoutKind::Grid => {}
    }

    if let Some(inset) = layout.padding {
      let (left, right) = horizontal(inset.leading, inset.trailing);

      out.padding = TaffyRect {
        left: length(left),
        right: length(right),
        top: length(inset.top),
        bottom: length(inset.bottom),
      };
    }

    if let Some(spacing) = layout.spacing {
      out.gap = Size {
        width: length(spacing.horizontal),
        height: length(spacing.vertical),
      };
    }

    if layout.wrap {
      out.flex_wrap = ts::FlexWrap::Wrap;
    }

    out.justify_content = layout.arrangement.map(arrangement);
    out.align_content = layout.line_arrangement.map(arrangement);
    out.align_items = layout.item_alignment.map(item);

    if let Some(alignment) = layout.alignment {
      out.justify_items = Some(item(alignment.horizontal));
      out.align_items = Some(item(alignment.vertical));
    }

    if let Some(value) = &layout.columns {
      out.grid_template_columns = tracks(value);
    }

    if let Some(value) = &layout.rows {
      out.grid_template_rows = tracks(value);
    }

    if let Some(value) = &layout.implicit_columns {
      out.grid_auto_columns = value.0.iter().map(|value| track(*value)).collect();
    }

    if let Some(value) = &layout.implicit_rows {
      out.grid_auto_rows = value.0.iter().map(|value| track(*value)).collect();
    }

    if let Some(flow) = layout.flow {
      out.grid_auto_flow = match flow {
        Flow::Rows => ts::GridAutoFlow::Row,
        Flow::Columns => ts::GridAutoFlow::Column,
      };
    }
  }

  let keep = |value: Option<Dimension>| match (part, value) {
    (Part::Inner, Some(Dimension::Fraction(_))) => None,
    (_, value) => value,
  };

  out.size = Size {
    width: keep(layout.width).map_or(ts::Dimension::AUTO, dimension),
    height: keep(layout.height).map_or(ts::Dimension::AUTO, dimension),
  };
  out.min_size = Size {
    width: keep(layout.min_width).map_or(ts::LengthPercentageAuto::AUTO, bound),
    height: keep(layout.min_height).map_or(ts::LengthPercentageAuto::AUTO, bound),
  };
  out.max_size = Size {
    width: keep(layout.max_width).map_or(ts::LengthPercentageAuto::AUTO, bound),
    height: keep(layout.max_height).map_or(ts::LengthPercentageAuto::AUTO, bound),
  };

  if part == Part::Outer {
    if let Some(edges) = layout.pin {
      let (left, right) = if rtl {
        (edges.trailing, edges.leading)
      } else {
        (edges.leading, edges.trailing)
      };
      let edge = |value: Option<f32>| value.map_or(ts::LengthPercentageAuto::AUTO, length);

      out.position = ts::Position::Absolute;
      out.inset = TaffyRect {
        left: edge(left),
        right: edge(right),
        top: edge(edges.top),
        bottom: edge(edges.bottom),
      };
    }

    if let Some(flexibility) = layout.flexibility {
      out.flex_grow = flexibility.grow;
      out.flex_shrink = flexibility.shrink;
      out.flex_basis = dimension(flexibility.initial);
    }

    if let Some(alignment) = layout.self_alignment {
      match parent {
        Some(LayoutKind::Row) => out.align_self = Some(item(alignment.vertical)),
        Some(LayoutKind::Column) => out.align_self = Some(item(alignment.horizontal)),
        _ => {
          out.justify_self = Some(item(alignment.horizontal));
          out.align_self = Some(item(alignment.vertical));
        }
      }
    }

    if let Some(cell) = layout.cell {
      out.grid_row = line(cell.row);
      out.grid_column = line(cell.column);
    }

    if matches!(parent, Some(LayoutKind::Stack)) {
      let first = Line {
        start: ts::GridPlacement::from_line_index(1),
        end: ts::GridPlacement::Auto,
      };

      out.grid_row = first.clone();
      out.grid_column = first;
    }
  }

  out
}

/// Iterates a tree node's children.
pub struct ChildIter<'a>(std::slice::Iter<'a, usize>);

impl Iterator for ChildIter<'_> {
  type Item = TaffyId;

  fn next(&mut self) -> Option<TaffyId> {
    self.0.next().map(|index| TaffyId::from(*index))
  }
}

impl<H: LayoutHost> taffy::TraversePartialTree for ContainerLayout<H> {
  type ChildIter<'a> = ChildIter<'a>;

  fn child_ids(&self, node: TaffyId) -> ChildIter<'_> {
    ChildIter(self.nodes[usize::from(node)].children.iter())
  }

  fn child_count(&self, node: TaffyId) -> usize {
    self.nodes[usize::from(node)].children.len()
  }

  fn get_child_id(&self, node: TaffyId, index: usize) -> TaffyId {
    TaffyId::from(self.nodes[usize::from(node)].children[index])
  }
}

impl<H: LayoutHost> taffy::TraverseTree for ContainerLayout<H> {}

impl<H: LayoutHost> LayoutPartialTree for ContainerLayout<H> {
  type CustomIdent = String;
  type CoreContainerStyle<'a> = &'a Style;

  fn get_core_container_style(&self, node: TaffyId) -> &Style {
    &self.nodes[usize::from(node)].style
  }

  fn set_unrounded_layout(&mut self, node: TaffyId, layout: &Layout) {
    self.nodes[usize::from(node)].unrounded = *layout;
  }

  fn resolve_calc_value(&self, _: *const (), _: f32) -> f32 {
    0.
  }

  fn compute_child_layout(&mut self, node: TaffyId, inputs: LayoutInput) -> LayoutOutput {
    taffy::compute_cached_layout(self, node, inputs, |tree, node, inputs| {
      let index = usize::from(node);

      if tree.nodes[index].style.display == Display::None {
        return taffy::compute_hidden_layout(tree, node);
      }

      if let Some(child) = tree.nodes[index].child.clone() {
        if let Some(text) = H::text(&child) {
          let mut baseline = None;
          let mut output = taffy::compute_leaf_layout(
            inputs,
            &tree.nodes[index].style,
            |_, _| 0.,
            |known, available| {
              let measured = text.measure(
                (known.width, known.height),
                (space(available.width), space(available.height)),
              );

              baseline = measured.first_baseline;

              Size {
                width: measured.width,
                height: measured.height,
              }
            },
          );

          output.baselines.first = baseline;

          return output;
        }

        let mut baseline = None;
        let mut output = taffy::compute_leaf_layout(
          inputs,
          &tree.nodes[index].style,
          |_, _| 0.,
          |known, available| {
            let (size, first) = measure_leaf::<H>(&child, known, available);

            baseline = first;
            size
          },
        );

        output.baselines.first = baseline;

        return output;
      }

      match tree.nodes[index].style.display {
        Display::Grid => {
          let mut output = taffy::compute_grid_layout(tree, node, inputs);

          if inputs.run_mode == RunMode::PerformLayout
            && let Some(baseline) = tree.first_row_baseline()
          {
            output.baselines.first = Some(baseline);
          }

          output
        }
        _ => taffy::compute_flexbox_layout(tree, node, inputs),
      }
    })
  }
}

/// Taffy's available space as a text leaf's.
const fn space(available: AvailableSpace) -> Space {
  match available {
    AvailableSpace::MinContent => Space::MinContent,
    AvailableSpace::MaxContent => Space::MaxContent,
    AvailableSpace::Definite(width) => Space::Definite(width),
  }
}

/// A leaf measured as taffy asks, mapped onto the toolkit's protocol: min-content is the
/// minimum, max-content the natural size, fit-content the natural size clamped to the space
/// but never below the minimum, a height for a width the vertical measure for that width.
fn measure_leaf<H: LayoutHost>(
  child: &H::Child,
  known: Size<Option<f32>>,
  available: Size<AvailableSpace>,
) -> (Size<f32>, Option<f32>) {
  let width = match known.width {
    Some(width) => width,
    None => {
      let measured = H::measure(child, Axis::Horizontal, known.height.map(f32::round));

      match available.width {
        AvailableSpace::MinContent => measured.minimum,
        AvailableSpace::MaxContent => measured.natural,
        AvailableSpace::Definite(space) => measured.natural.min(space.max(measured.minimum)),
      }
    }
  };
  let vertical = H::measure(child, Axis::Vertical, Some(width.round()));
  let height = match (known.height, available.height) {
    (Some(height), _) => height,
    (None, AvailableSpace::MinContent) => vertical.minimum,
    (None, _) => vertical.natural,
  };

  (Size { width, height }, vertical.baseline)
}

impl<H: LayoutHost> CacheTree for ContainerLayout<H> {
  fn cache_get(&mut self, node: TaffyId, inputs: &LayoutInput) -> Option<LayoutOutput> {
    self.nodes[usize::from(node)].cache.get(inputs)
  }

  fn cache_store(&mut self, node: TaffyId, inputs: &LayoutInput, output: LayoutOutput) {
    self.nodes[usize::from(node)].cache.store(inputs, output);
  }

  fn cache_clear(&mut self, node: TaffyId) {
    self.nodes[usize::from(node)].cache.clear();
  }
}

impl<H: LayoutHost> taffy::LayoutFlexboxContainer for ContainerLayout<H> {
  type FlexboxContainerStyle<'a> = &'a Style;
  type FlexboxItemStyle<'a> = &'a Style;

  fn get_flexbox_container_style(&self, node: TaffyId) -> &Style {
    &self.nodes[usize::from(node)].style
  }

  fn get_flexbox_child_style(&self, child: TaffyId) -> &Style {
    &self.nodes[usize::from(child)].style
  }
}

impl<H: LayoutHost> taffy::LayoutGridContainer for ContainerLayout<H> {
  type GridContainerStyle<'a> = &'a Style;
  type GridItemStyle<'a> = &'a Style;

  fn get_grid_container_style(&self, node: TaffyId) -> &Style {
    &self.nodes[usize::from(node)].style
  }

  fn get_grid_child_style(&self, child: TaffyId) -> &Style {
    &self.nodes[usize::from(child)].style
  }
}

impl<H: LayoutHost> RoundTree for ContainerLayout<H> {
  fn get_unrounded_layout(&self, node: TaffyId) -> Layout {
    self.nodes[usize::from(node)].unrounded
  }

  fn set_final_layout(&mut self, node: TaffyId, layout: &Layout) {
    self.nodes[usize::from(node)].layout = *layout;
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use super::*;
  use crate::value::Flexibility;

  /// A leaf of fixed sizes, recording where it was placed.
  #[derive(Clone)]
  struct Leaf {
    minimum: f32,
    natural: f32,
    height: f32,
    baseline: Option<f32>,
    placed: Rc<Cell<Option<Rect>>>,
  }

  impl Leaf {
    fn new(minimum: f32, natural: f32, height: f32) -> Self {
      Self {
        minimum,
        natural,
        height,
        baseline: None,
        placed: Rc::new(Cell::new(None)),
      }
    }

    const fn with_baseline(mut self, baseline: f32) -> Self {
      self.baseline = Some(baseline);

      self
    }

    fn at(&self) -> (f32, f32, f32, f32) {
      let rect = self.placed.get().expect("the leaf was placed");

      (rect.x, rect.y, rect.width, rect.height)
    }
  }

  /// Leaves of fixed sizes as the engine's host.
  struct Fixed;

  impl LayoutHost for Fixed {
    type Child = Leaf;

    fn measure(child: &Leaf, axis: Axis, _: Option<f32>) -> Measurement {
      match axis {
        Axis::Horizontal => Measurement {
          minimum: child.minimum,
          natural: child.natural,
          baseline: None,
        },
        Axis::Vertical => Measurement {
          minimum: child.height,
          natural: child.height,
          baseline: child.baseline,
        },
      }
    }

    fn constant_height(_: &Leaf) -> bool {
      true
    }

    fn is_visible(_: &Leaf) -> bool {
      true
    }

    fn is_container(_: &Leaf) -> bool {
      false
    }

    fn allocate(child: &Leaf, rect: Rect, _: Rect, _: Option<f32>) {
      child.placed.set(Some(rect));
    }
  }

  fn row(own: ResolvedLayout, children: &[(Leaf, ResolvedLayout)]) -> ContainerLayout<Fixed> {
    let mut layout = ContainerLayout::<Fixed>::new(LayoutKind::Row, own);

    layout.set_children(children.to_vec());
    layout
  }

  fn padded(spacing: f32) -> ResolvedLayout {
    ResolvedLayout {
      padding: Some(Inset::all(8)),
      spacing: Some(Spacing::from(spacing)),
      item_alignment: Some(AxisAlignment::Start),
      ..ResolvedLayout::default()
    }
  }

  #[test]
  fn a_row_measures_and_places_its_children_with_padding_and_spacing() {
    let (a, b) = (Leaf::new(50., 50., 20.), Leaf::new(30., 30., 40.));
    let mut layout = row(
      padded(10.),
      &[
        (a.clone(), ResolvedLayout::default()),
        (b.clone(), ResolvedLayout::default()),
      ],
    );

    assert_eq!(layout.measure(Axis::Horizontal, None).natural, 106.);
    assert_eq!(layout.measure(Axis::Vertical, Some(106.)).natural, 56.);
    layout.allocate((200., 100.), None);
    assert_eq!(a.at(), (8., 8., 50., 20.));
    assert_eq!(b.at(), (68., 8., 30., 40.));
  }

  #[test]
  fn a_box_and_a_grid_take_their_first_childs_baseline() {
    let text = Leaf::new(40., 40., 40.).with_baseline(31.);
    let mut boxed = ContainerLayout::<Fixed>::new(
      LayoutKind::Box,
      ResolvedLayout {
        padding: Some(Inset::new(10., 0., 0., 0.)),
        ..ResolvedLayout::default()
      },
    );

    boxed.set_children(vec![(text.clone(), ResolvedLayout::default())]);
    assert_eq!(boxed.measure(Axis::Vertical, Some(40.)).baseline, Some(41.));

    let mut grid = ContainerLayout::<Fixed>::new(LayoutKind::Grid, ResolvedLayout::default());

    grid.set_children(vec![
      (text, ResolvedLayout::default()),
      (Leaf::new(20., 20., 50.), ResolvedLayout::default()),
    ]);
    assert_eq!(grid.measure(Axis::Vertical, Some(60.)).baseline, Some(31.));
  }

  #[test]
  fn a_flexible_child_takes_the_space_left() {
    let (a, b) = (Leaf::new(50., 50., 20.), Leaf::new(30., 30., 20.));
    let grow = ResolvedLayout {
      flexibility: Some(Flexibility::grow(1.)),
      ..ResolvedLayout::default()
    };
    let mut layout = row(
      padded(10.),
      &[(a.clone(), ResolvedLayout::default()), (b.clone(), grow)],
    );

    layout.allocate((200., 100.), None);
    assert_eq!(b.at(), (68., 8., 124., 20.));
  }

  #[test]
  fn the_toolkit_minimum_is_a_floor_where_the_view_sets_none() {
    let (a, b) = (Leaf::new(40., 60., 20.), Leaf::new(40., 60., 20.));
    let mut layout = row(
      ResolvedLayout::default(),
      &[
        (a.clone(), ResolvedLayout::default()),
        (b.clone(), ResolvedLayout::default()),
      ],
    );

    layout.allocate((60., 20.), None);
    assert_eq!(a.at().2, 40.);
    assert_eq!(b.at().2, 40.);
  }

  #[test]
  fn right_to_left_mirrors_a_row() {
    let (a, b) = (Leaf::new(50., 50., 20.), Leaf::new(30., 30., 20.));
    let mut layout = row(
      padded(10.),
      &[
        (a.clone(), ResolvedLayout::default()),
        (b.clone(), ResolvedLayout::default()),
      ],
    );

    layout.set_rtl(true);
    layout.allocate((200., 100.), None);
    assert_eq!(a.at().0, 200. - 8. - 50.);
    assert_eq!(b.at().0, 200. - 8. - 50. - 10. - 30.);
  }

  #[test]
  fn a_grid_places_children_in_flexible_tracks_and_a_stack_pins() {
    let leaves: Vec<Leaf> = (0..4).map(|_| Leaf::new(10., 10., 10.)).collect();
    let mut grid = ContainerLayout::<Fixed>::new(
      LayoutKind::Grid,
      ResolvedLayout {
        columns: Some(Tracks::from([GridTrack::Flex(1.), GridTrack::Flex(3.)])),
        spacing: Some(Spacing::from(4.)),
        ..ResolvedLayout::default()
      },
    );

    grid.set_children(
      leaves
        .iter()
        .map(|leaf| (leaf.clone(), ResolvedLayout::default()))
        .collect(),
    );
    grid.allocate((204., 100.), None);
    assert_eq!(leaves[1].at().0, 54.);
    assert_eq!(leaves[1].at().2, 150.);
    // Auto rows stretch over the grid's height, as CSS's do: (100 - 4) / 2 + 4.
    assert_eq!(leaves[2].at().1, 52.);

    let (back, badge) = (Leaf::new(10., 10., 10.), Leaf::new(10., 10., 10.));
    let mut stack = ContainerLayout::<Fixed>::new(LayoutKind::Stack, ResolvedLayout::default());

    stack.set_children(vec![
      (back.clone(), ResolvedLayout::default()),
      (
        badge.clone(),
        ResolvedLayout {
          pin: Some(Edges {
            bottom: Some(4.),
            trailing: Some(4.),
            ..Edges::default()
          }),
          ..ResolvedLayout::default()
        },
      ),
    ]);
    stack.allocate((100., 50.), None);
    assert_eq!(back.at(), (0., 0., 100., 50.));
    assert_eq!(badge.at(), (86., 36., 10., 10.));
  }

  #[test]
  fn a_column_may_shrink_to_its_children_minimums() {
    let (header, scroll) = (Leaf::new(10., 10., 40.), Leaf::new(10., 10., 0.));
    let tall = Leaf {
      height: 600.,
      ..scroll.clone()
    };
    let mut column =
      ContainerLayout::<Shrinking>::new(LayoutKind::Column, ResolvedLayout::default());

    column.set_children(vec![
      (header.clone(), ResolvedLayout::default()),
      (
        tall.clone(),
        ResolvedLayout {
          flexibility: Some(Flexibility::grow(1.)),
          ..ResolvedLayout::default()
        },
      ),
    ]);

    let measured = column.measure(Axis::Vertical, Some(100.));

    assert_eq!((measured.minimum, measured.natural), (40. + 46., 640.));
    column.allocate((100., 300.), None);
    assert_eq!(tall.at(), (0., 40., 100., 260.));
  }

  /// Leaves whose least height is 46 below their natural one, as a toolkit's scroller's.
  struct Shrinking;

  impl LayoutHost for Shrinking {
    type Child = Leaf;

    fn measure(child: &Leaf, axis: Axis, for_size: Option<f32>) -> Measurement {
      match axis {
        Axis::Horizontal => Fixed::measure(child, axis, for_size),
        Axis::Vertical => Measurement {
          minimum: child.height.min(46.),
          natural: child.height,
          baseline: None,
        },
      }
    }

    fn constant_height(_: &Leaf) -> bool {
      false
    }

    fn is_visible(_: &Leaf) -> bool {
      true
    }

    fn is_container(_: &Leaf) -> bool {
      false
    }

    fn allocate(child: &Leaf, rect: Rect, _: Rect, _: Option<f32>) {
      child.placed.set(Some(rect));
    }
  }

  #[test]
  fn a_nested_container_receives_the_fractional_rectangle() {
    let thirds: Vec<Leaf> = (0..3).map(|_| Leaf::new(0., 0., 10.)).collect();
    let grow = ResolvedLayout {
      flexibility: Some(Flexibility::grow(1.)),
      ..ResolvedLayout::default()
    };
    let mut layout = row(
      ResolvedLayout::default(),
      &thirds
        .iter()
        .map(|leaf| (leaf.clone(), grow.clone()))
        .collect::<Vec<_>>(),
    );

    layout.allocate((100., 10.), None);

    let widths: Vec<f32> = thirds.iter().map(|leaf| leaf.at().2).collect();

    assert_eq!(widths.iter().sum::<f32>(), 100.);
    assert!(widths.iter().all(|width| (33. ..=34.).contains(width)));
  }
}
