//! The layout of the scene: each record's part in it, a box styled from its resolved
//! style and a paragraph of runs measured in the pinned face, laid out through the
//! shared layout at the viewport, on a tree kept across frames.

use std::cell::RefCell;

use skene_foundation::{
  BoxField,
  BoxFieldVisitor,
  BoxStyle,
  IntoBoxValue,
  Layout,
  Length,
  NativeElement,
  NodeId,
  Plain,
  Platform,
  Point,
  RunPlacement,
  Scene,
  Size,
  StaticBoxStyle,
  StaticLeaf,
  typography,
  web::WebStyle,
};
use skene_layout::{
  Borders,
  BoxLayout,
  Content,
  Frames,
  Inline,
  Metrics,
  Paragraph,
  Part,
  Place,
  Tree,
  taffy::{
    prelude::length,
    style::{Dimension, Display, Style},
  },
};

use crate::{
  Headless,
  SlotValue,
  font::Pinned,
  records::{DEFAULT_VIEWPORT, Kind},
  resolve::{self, Env, Records},
  with_records,
};

/// The line height a run takes where neither its style nor the typography in force
/// sets one, as a multiple of its font size.
pub(crate) const LINE_HEIGHT: f32 = 1.5;

/// A run's metrics as the layout measures its text: each grapheme cluster as far as the
/// pinned face advances it at the font size, and the letter spacing past it, each line
/// the font size times the line height tall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RunMetrics {
  /// The font size.
  pub size: f32,
  /// The line height, as a multiple of the font size.
  pub line_height: f32,
  /// The letter spacing past each cluster.
  pub spacing: f32,
}

impl Metrics for RunMetrics {
  fn advance(&self, cluster: &str) -> f32 {
    Pinned::get().advance(cluster, self.size) + self.spacing
  }

  fn line_height(&self) -> f32 {
    self.size * self.line_height
  }
}

/// What the layout measures a leaf by: the runs of its paragraph.
pub(crate) type Measured = Paragraph<RunMetrics>;

thread_local! {
  /// The tree the scene lays out on, kept across frames.
  static TREE: RefCell<Tree<Measured>> = RefCell::new(Tree::new(false));
}

/// Lays out every tree of the scene at the viewport.
pub(crate) fn lay_out() -> Frames {
  let viewport = with_records(|records| records.viewport).unwrap_or(DEFAULT_VIEWPORT);
  let roots: Vec<NodeId> = Scene::<Headless>::read(|scene| scene.roots().collect());

  TREE.with_borrow_mut(|tree| tree.lay_out::<Headless>(&roots, viewport, part))
}

/// Forgets every layout, as a fresh platform holds none.
pub(crate) fn reset() {
  TREE.with_borrow_mut(|tree| *tree = Tree::new(false));
}

/// What `node` is to the layout: a box styled from its resolved style, a box whose
/// children are all inline runs a paragraph of them, a run a paragraph of its own text,
/// an inline box a box around its view, an open overlay out of the flow, an anchor or a
/// scope transparent, and a hidden box or a closed overlay left out.
fn part(node: NodeId) -> Part<Measured> {
  let Some(env) = Env::of(&Records, node)
  else {
    return Part::Absent;
  };
  let kind = with_records(|records| {
    let record = records.record(node)?;

    Some(match &record.kind {
      Kind::Box(props) => Shape::Box(
        props.layout,
        Box::new(props.style.clone()),
        record.scroll,
        false,
        None,
      ),
      Kind::Control(props) => Shape::Box(
        props.box_props.layout,
        Box::new(props.box_props.style.clone()),
        record.scroll,
        false,
        matches!(
          props.native,
          Some(NativeElement::TextField | NativeElement::TextEditor)
        )
        .then_some(props.lines),
      ),
      Kind::Overlay(props) if record.open => Shape::Box(
        props.box_props.layout,
        Box::new(props.box_props.style.clone()),
        record.scroll,
        true,
        None,
      ),
      Kind::Overlay(_) => Shape::Absent,
      Kind::Run(props) if props.placement == RunPlacement::Embedded => Shape::Embedded,
      Kind::Run(_) => Shape::Run(record.text.clone()),
      Kind::Anchor | Kind::Scope => Shape::Transparent,
    })
  });
  let place = Place {
    in_stack: in_stack(node),
    borders: Borders::Width,
  };

  match kind {
    None | Some(Shape::Absent) => Part::Absent,
    Some(Shape::Transparent) => Part::Transparent,
    Some(Shape::Box(layout, style, scroll, overlay, entry)) => {
      let resolved = presented(node, &env, &style);

      if resolved.hidden == Some(true) {
        return Part::Absent;
      }

      if overlay {
        let mut style = resolved.taffy_style(layout, place.detached());

        style.position = skene_layout::taffy::style::Position::Absolute;

        return Part::Overlay(style, scroll);
      }

      let mut style = resolved.taffy_style(layout, place);

      if first(node) {
        fill_viewport(&mut style, &resolved);
      }

      if let Some(lines) = entry {
        size_entry(&mut style, &resolved, &env, lines);
      }

      match (layout == Layout::Document)
        .then(|| paragraph(node))
        .flatten()
      {
        Some(paragraph) => Part::Leaf(style, paragraph),
        None => Part::Box(style, scroll),
      }
    }
    Some(Shape::Embedded) => {
      let mut taffy = run_style(&env, node, place);

      taffy.display = Display::Flex;

      Part::Box(taffy, Point::new(0., 0.))
    }
    Some(Shape::Run(text)) => {
      let mut style = run_style(&env, node, place);

      if first(node) {
        fill_viewport(&mut style, &BoxStyle::default());
      }

      Part::Leaf(style, lone(node, text))
    }
  }
}

/// The box style `node` resolves to under `env` from its static `style`, its motion laid
/// over it.
pub(crate) fn presented(
  node: NodeId,
  env: &Env,
  style: &StaticBoxStyle<Headless>,
) -> BoxStyle<f32, Plain, WebStyle> {
  let mut resolved = env.with_resolver(|resolver| resolver.box_style(style));

  if moves(node) {
    BoxStyle::<f32, Plain, WebStyle>::for_each_field(&mut Presented {
      node,
      style: &mut resolved,
    });
  }

  resolved
}

/// The paragraph the run `node`, outside any paragraph, lays out as: its text, or a
/// group's runs.
pub(crate) fn lone(node: NodeId, text: String) -> Measured {
  let children = placed_children(node);
  let mut runs = Vec::new();

  if children.is_empty()
    || !children
      .into_iter()
      .all(|child| inlines(child, &[node], &mut runs))
  {
    runs = vec![Inline {
      node,
      groups: Vec::new(),
      content: Content::Text(text),
      metrics: metrics(node),
    }];
  }

  Paragraph::new(runs)
}

/// The taffy style the run `node` takes from its place and the wrapper's properties it
/// carries.
fn run_style(env: &Env, node: NodeId, place: Place) -> Style {
  let layout = env.with_resolver(|resolver| {
    Scene::<Headless>::read(|scene| scene.record(node)?.annotation().cloned())
      .map(|annotation| annotation.resolve(resolver))
  });
  let mut taffy = Style::default();

  match layout {
    Some(layout) => layout.fold_into(&mut taffy, place),
    None => place.apply(&mut taffy),
  }

  taffy
}

/// The paragraph the box `node` lays its children out as, where every child it places
/// is an inline run, an inline box, a group of them, or an anchor or a scope holding
/// them.
pub(crate) fn paragraph(node: NodeId) -> Option<Measured> {
  let children = placed_children(node);
  let mut runs = Vec::new();
  let inline = !children.is_empty()
    && children
      .into_iter()
      .all(|child| inlines(child, &[], &mut runs));

  inline.then(|| Paragraph::new(runs))
}

/// Adds the runs `node` holds to a paragraph's `runs`, inside `groups`: a text run or
/// an inline box itself, a group's runs inside it, and an anchor's or a scope's in its
/// place; `false` where it holds anything else.
fn inlines(node: NodeId, groups: &[NodeId], runs: &mut Vec<Inline<RunMetrics>>) -> bool {
  let flow = with_records(|records| {
    let record = records.record(node)?;

    Some(match &record.kind {
      Kind::Run(props) if props.placement == RunPlacement::Inline => Flow::Run(record.text.clone()),
      Kind::Run(props) if props.placement == RunPlacement::Embedded => Flow::Box,
      Kind::Anchor | Kind::Scope => Flow::Transparent,
      _ => Flow::Other,
    })
  });
  let children = placed_children(node);

  match flow {
    Some(Flow::Transparent) => children
      .into_iter()
      .all(|child| inlines(child, groups, runs)),
    Some(Flow::Run(text)) if children.is_empty() => {
      runs.push(Inline {
        node,
        groups: groups.to_vec(),
        content: Content::Text(text),
        metrics: metrics(node),
      });

      true
    }
    Some(Flow::Box) => {
      runs.push(Inline {
        node,
        groups: groups.to_vec(),
        content: Content::Box(Size::new(0., 0.)),
        metrics: metrics(node),
      });

      true
    }
    Some(Flow::Run(_)) => {
      let mut inside = groups.to_vec();

      inside.push(node);
      children
        .into_iter()
        .all(|child| inlines(child, &inside, runs))
    }
    None | Some(Flow::Other) => false,
  }
}

/// What a node is to the paragraph it flows in.
enum Flow {
  Run(String),
  Box,
  Transparent,
  Other,
}

/// The metrics the run `node` measures its text by, as its style and the typography in
/// force set them.
pub(crate) fn metrics(node: NodeId) -> RunMetrics {
  let style = with_records(|records| match &records.record(node)?.kind {
    Kind::Run(props) => Some(props.style.clone()),
    _ => None,
  });
  let (size, line_height, spacing) = Env::of(&Records, node)
    .zip(style)
    .map(|(env, style)| {
      env.with_resolver(|resolver| {
        let resolved = resolver.run_style(&style);

        (
          resolved
            .ext
            .font_size
            .or_else(|| resolver.keyframe_run_value(&StaticLeaf::from(typography::SIZE))),
          resolved
            .ext
            .line_height
            .or_else(|| resolver.keyframe_run_value(&StaticLeaf::from(typography::LINE_HEIGHT))),
          resolved
            .ext
            .letter_spacing
            .or_else(|| resolver.keyframe_run_value(&StaticLeaf::from(typography::LETTER_SPACING))),
        )
      })
    })
    .unwrap_or_default();

  RunMetrics {
    size: size.map_or(Headless::FONT_SIZE, |size| size.0),
    line_height: line_height.unwrap_or(LINE_HEIGHT),
    spacing: spacing.map_or(0., |spacing| spacing.0),
  }
}

/// The children `node` places, in order.
pub(crate) fn placed_children(node: NodeId) -> Vec<NodeId> {
  Scene::<Headless>::read(|scene| scene.placed_children(node).collect())
}

/// Whether `node` declares a transition or an animation of its box.
fn moves(node: NodeId) -> bool {
  with_records(|records| {
    records
      .record(node)
      .and_then(|record| record.kind.box_props())
      .is_some_and(|props| {
        !props.dynamics.transitions.is_empty() || !props.dynamics.animations.is_empty()
      })
  })
}

/// Writes the value each property of `node` presents, its motion laid over what it
/// resolved to, into `style`.
struct Presented<'a> {
  node: NodeId,
  style: &'a mut BoxStyle<f32, Plain, WebStyle>,
}

impl BoxFieldVisitor<f32, WebStyle> for Presented<'_> {
  fn field<F: BoxField<f32, WebStyle>>(&mut self) {
    *F::get_mut(self.style) = match resolve::resolve(self.node, F::KEY) {
      Some(SlotValue::Box(value)) => F::Value::from_box_value(value),
      _ => None,
    };
  }
}

/// What a record holds that its part is made from, read out of the records.
enum Shape {
  Box(
    Layout,
    Box<StaticBoxStyle<Headless>>,
    Point<f32>,
    bool,
    Option<u16>,
  ),
  Embedded,
  Run(String),
  Transparent,
  Absent,
}

/// The font size and the line height, as a multiple of it, of the typography in force
/// under `env`.
pub(crate) fn typography(env: &Env) -> (f32, f32) {
  env.with_resolver(|resolver| {
    (
      resolver
        .keyframe_run_value(&StaticLeaf::from(typography::SIZE))
        .map_or(Headless::FONT_SIZE, |size: Length| size.0),
      resolver
        .keyframe_run_value(&StaticLeaf::from(typography::LINE_HEIGHT))
        .unwrap_or(LINE_HEIGHT),
    )
  })
}

/// Sizes `style` as the native element of a text entry `lines` tall lays out, as
/// Chromium sizes one, in the typography in force under `env`: its content box as wide
/// as [`Pinned::entry_width`] and as tall as its lines, inside the padding and border
/// `resolved`, its own style, gives it, wherever that style sets no size of its own.
fn size_entry(style: &mut Style, resolved: &BoxStyle<f32, Plain, WebStyle>, env: &Env, lines: u16) {
  let (size, line_height) = typography(env);
  let padding = resolved.padding.unwrap_or_default();
  let border = resolved.border_width.unwrap_or_default();

  if resolved.width.is_none() {
    style.size.width = length(
      Pinned::get().entry_width(size, lines)
        + padding.start
        + padding.end
        + border.start
        + border.end,
    );
  }

  if resolved.height.is_none() {
    style.size.height = length(
      f32::from(lines.max(1)) * size * line_height
        + padding.top
        + padding.bottom
        + border.top
        + border.bottom,
    );
  }
}

/// Whether `node` is the app's first box, as the web's reset finds it: no box, control,
/// overlay or run above it, anchors and scopes aside.
fn first(node: NodeId) -> bool {
  let mut current = Scene::<Headless>::read(|scene| scene.placed_parent(node));

  while let Some(parent) = current {
    let transparent = with_records(|records| {
      records
        .record(parent)
        .is_some_and(|record| matches!(record.kind, Kind::Anchor | Kind::Scope))
    });

    if !transparent {
      return false;
    }

    current = Scene::<Headless>::read(|scene| scene.placed_parent(parent));
  }

  true
}

/// Makes `style` fill the viewport as the web's reset makes the app's first box: growing
/// and shrinking with no floor, and scrolling what overflows it, where `resolved`, its
/// own style, sets none of these.
fn fill_viewport(style: &mut Style, resolved: &BoxStyle<f32, Plain, WebStyle>) {
  if resolved.flex.is_none() {
    style.flex_grow = 1.;
    style.flex_shrink = 1.;
    style.flex_basis = Dimension::auto();
  }

  if resolved.min_width.is_none() {
    style.min_size.width = length(0.);
  }

  if resolved.min_height.is_none() {
    style.min_size.height = length(0.);
  }

  if resolved.overflow.is_none() {
    style.overflow = skene_layout::taffy::geometry::Point {
      x: skene_layout::taffy::style::Overflow::Scroll,
      y: skene_layout::taffy::style::Overflow::Scroll,
    };
  }
}

/// Whether `node`'s nearest laid-out ancestor is a stack, which places it in its one
/// cell.
fn in_stack(node: NodeId) -> bool {
  let mut current = Scene::<Headless>::read(|scene| scene.placed_parent(node));

  while let Some(parent) = current {
    let layout = with_records(|records| match &records.record(parent)?.kind {
      Kind::Anchor | Kind::Scope => None,
      kind => Some(kind.box_props().map(|props| props.layout)),
    });

    match layout {
      Some(layout) => return layout == Some(Layout::Stack),
      None => current = Scene::<Headless>::read(|scene| scene.placed_parent(parent)),
    }
  }

  false
}
