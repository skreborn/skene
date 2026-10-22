//! The layout of the scene: each record's part in it, a box styled from its resolved
//! style, a paragraph of runs laid out through the engine in the pinned face and a text
//! entry at its native element's size, laid out through the shared layout at the
//! viewport, on a tree kept across frames.

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
  TextDecoration,
  text::{LineHeight, PaintStyle, ParagraphStyle, TextStyle},
  typography,
  web::WebStyle,
};
use skene_layout::{
  Borders,
  BoxLayout,
  Frames,
  Leaf,
  Part,
  Place,
  Revisit,
  TextFlow,
  Tree,
  taffy::{
    self,
    prelude::{AvailableSpace, length},
    style::{Dimension, Display, FlexDirection, Style},
  },
};
use skene_parley::ParleyEngine;

use crate::{
  Headless,
  SlotValue,
  font::{self, Pinned},
  records::{DEFAULT_VIEWPORT, Kind},
  resolve::{self, Env, Records},
  with_records,
};

/// The line height a run takes where neither its style nor the typography in force
/// sets one, as a multiple of its font size.
pub(crate) const LINE_HEIGHT: f32 = 1.5;

/// A paragraph of runs, laid out through the engine in the pinned face.
pub(crate) type Text = TextFlow<ParleyEngine>;

/// What the layout measures a leaf by.
#[derive(Debug, PartialEq)]
pub(crate) enum Measured {
  /// The runs of a paragraph.
  Text(Text),
  /// A text entry's native element, its content box this size along each axis its
  /// style leaves unsized, as an `<input>`'s or a `<textarea>`'s intrinsic size is.
  Entry(Size<f32>),
}

impl Leaf for Measured {
  fn measure(
    &self,
    known: taffy::geometry::Size<Option<f32>>,
    available: taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32> {
    match self {
      Self::Text(text) => text.measure(known, available),
      Self::Entry(size) => Size::new(
        known.width.unwrap_or(size.width),
        known.height.unwrap_or(size.height),
      ),
    }
  }

  fn place(
    &self,
    content: skene_foundation::Rect<f32>,
  ) -> Vec<(NodeId, skene_foundation::Rect<f32>)> {
    match self {
      Self::Text(text) => text.place(content),
      Self::Entry(_) => Vec::new(),
    }
  }

  fn boxes(&self) -> Vec<NodeId> {
    match self {
      Self::Text(text) => text.boxes(),
      Self::Entry(_) => Vec::new(),
    }
  }

  fn size_box(&mut self, node: NodeId, size: Size<f32>) {
    if let Self::Text(text) = self {
      text.size_box(node, size);
    }
  }
}

thread_local! {
  /// The tree the scene lays out on, kept across frames.
  static TREE: RefCell<Tree<Measured>> = RefCell::new(Tree::new(false));
}

/// Lays out every tree of the scene at the viewport, each root along a row as the web's
/// reset lays out the body's children: only the nodes the restyle revisits written
/// afresh where the structure held, else the whole scene.
pub(crate) fn lay_out() -> Frames {
  let viewport = with_records(|records| records.viewport).unwrap_or(DEFAULT_VIEWPORT);
  let roots: Vec<NodeId> = Scene::<Headless>::read(|scene| scene.roots().collect());
  let interaction = Scene::<Headless>::read(Scene::interaction_generation);
  let revisited = with_records(|records| match records.restyle.take(interaction) {
    Revisit::Every => None,
    Revisit::Nodes(mut nodes, moved) => {
      if moved {
        nodes.extend(records.restyle.interactive());
      }

      Some(nodes)
    }
  });

  TREE.with_borrow_mut(|tree| {
    revisited
      .and_then(|nodes| {
        tree.lay_out_revisited::<Headless>(&roots, viewport, FlexDirection::Row, &nodes, part)
      })
      .unwrap_or_else(|| {
        with_records(|records| records.restyle.take_structure());

        tree.lay_out::<Headless>(&roots, viewport, FlexDirection::Row, part)
      })
  })
}

/// Forgets every layout, as a fresh platform holds none.
pub(crate) fn reset() {
  TREE.with_borrow_mut(|tree| *tree = Tree::new(false));
}

/// What `node` is to the layout: a box styled from its resolved style, a box whose
/// children are all inline runs a paragraph of them, a run a paragraph of its own text,
/// an inline box a box around its view, an open overlay out of the flow, an anchor or a
/// scope transparent, a hidden box laid out but taking no hit, and a closed overlay left
/// out.
///
/// The restyle notes whether its resolution read its interaction, and revisits it at the
/// next layout where it is laid out and declares motion.
fn part(node: NodeId) -> Part<Measured> {
  let Some(env) = Env::of(&Records, node)
  else {
    return Part::Absent;
  };
  let its = part_under(node, &env);
  let moving = !matches!(its, Part::Transparent | Part::Absent) && moves(node);

  with_records(|records| {
    records.style_resolutions += 1;
    records
      .restyle
      .read_interaction(node, env.read_interaction());

    if moving {
      records.restyle.keep_moving(node);
    }
  });

  its
}

/// What `node` is to the layout, its properties resolved under `env`.
fn part_under(node: NodeId, env: &Env) -> Part<Measured> {
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
      let resolved = presented(node, env, &style);
      let its = box_part(node, env, &resolved, layout, scroll, overlay, entry, place);

      if resolved.hidden == Some(true) {
        Part::Hidden(Box::new(its))
      }
      else {
        its
      }
    }
    Some(Shape::Embedded) => {
      let mut taffy = run_style(env, node, place);

      taffy.display = Display::Flex;

      Part::Box(taffy, Point::new(0., 0.))
    }
    Some(Shape::Run(text)) => {
      let mut style = run_style(env, node, place);

      if first(node) {
        fill_viewport(&mut style, &BoxStyle::default());
      }

      Part::Leaf(style, Measured::Text(lone(node, text)))
    }
  }
}

/// What the box `node` resolved to `resolved` under `env` is to the layout, `layout` its
/// layout mode, `scroll` its offsets and `place` its place: an open overlay out of the
/// flow, a text entry at its native element's size, a paragraph of its runs or a box.
#[expect(
  clippy::too_many_arguments,
  reason = "the facts a box's part is made of, read once by the caller"
)]
fn box_part(
  node: NodeId,
  env: &Env,
  resolved: &BoxStyle<f32, Plain, WebStyle>,
  layout: Layout,
  scroll: Point<f32>,
  overlay: bool,
  entry: Option<u16>,
  place: Place,
) -> Part<Measured> {
  if overlay {
    let mut style = resolved.taffy_style(layout, place.detached());

    style.position = skene_layout::taffy::style::Position::Absolute;

    return Part::Overlay(style, scroll);
  }

  let mut style = resolved.taffy_style(layout, place);

  if first(node) {
    fill_viewport(&mut style, resolved);
  }

  if let Some(lines) = entry {
    return Part::Leaf(style, Measured::Entry(entry_size(env, lines)));
  }

  match (layout == Layout::Document)
    .then(|| paragraph(node))
    .flatten()
  {
    Some(paragraph) => Part::Leaf(style, Measured::Text(paragraph)),
    None => Part::Box(style, scroll),
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
pub(crate) fn lone(node: NodeId, text: String) -> Text {
  let children = placed_children(node);
  let mut runs = Vec::new();

  if children.is_empty()
    || !children
      .into_iter()
      .all(|child| inlines(child, &[node], &mut runs))
  {
    runs = vec![Inline::Run(node, Vec::new(), text)];
  }

  flow(text_style(node), runs).trimmed(Pinned::get().cap_height())
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
pub(crate) fn paragraph(node: NodeId) -> Option<Text> {
  let children = placed_children(node);
  let mut runs = Vec::new();
  let inline = !children.is_empty()
    && children
      .into_iter()
      .all(|child| inlines(child, &[], &mut runs));

  inline.then(|| {
    let (size, line_height) = Env::of(&Records, node)
      .as_ref()
      .map_or((Headless::FONT_SIZE, LINE_HEIGHT), typography);

    flow(style_of(size, line_height, 0.), runs).trimmed(Pinned::get().cap_height())
  })
}

/// What flows in a paragraph: a run's node, the groups around it and its text, or an inline
/// box's node and the groups around it.
enum Inline {
  Run(NodeId, Vec<NodeId>, String),
  Box(NodeId, Vec<NodeId>),
}

/// The paragraph of `runs`, in order, through the engine in the pinned face, its empty
/// lines in `base`.
fn flow(base: TextStyle, runs: Vec<Inline>) -> Text {
  let mut flow = TextFlow::new(font::engine(), base, ParagraphStyle::default());

  for run in runs {
    match run {
      Inline::Run(node, groups, text) => flow.add_run(node, &groups, &text, text_style(node)),
      Inline::Box(node, groups) => flow.add_box(node, &groups, Size::new(0., 0.)),
    }
  }

  flow
}

/// Adds the runs `node` holds to a paragraph's `runs`, inside `groups`: a text run or
/// an inline box itself, a group's runs inside it, and an anchor's or a scope's in its
/// place; `false` where it holds anything else.
fn inlines(node: NodeId, groups: &[NodeId], runs: &mut Vec<Inline>) -> bool {
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
      runs.push(Inline::Run(node, groups.to_vec(), text));

      true
    }
    Some(Flow::Box) => {
      runs.push(Inline::Box(node, groups.to_vec()));

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

/// The style the run `node` lays out and paints its text in, as its style and the
/// typography in force set it: its ink the typography's where it sets none, and a link's
/// underlined where its decoration says nothing; the restyle notes whether its resolution
/// read its interaction.
pub(crate) fn text_style(node: NodeId) -> TextStyle {
  let props = with_records(|records| match &records.record(node)?.kind {
    Kind::Run(props) => Some((props.style.clone(), props.link.is_some())),
    _ => None,
  });
  let resolved = Env::of(&Records, node)
    .zip(props)
    .map(|(env, (style, link))| {
      let resolved = env.with_resolver(|resolver| {
        let mut resolved = resolver.run_style(&style);
        let ink = if link {
          typography::LINK_INK
        }
        else {
          typography::INK
        };

        resolved.ext.font_size = resolved
          .ext
          .font_size
          .or_else(|| resolver.keyframe_run_value(&StaticLeaf::from(typography::SIZE)));
        resolved.ext.line_height = resolved
          .ext
          .line_height
          .or_else(|| resolver.keyframe_run_value(&StaticLeaf::from(typography::LINE_HEIGHT)))
          .or(Some(LINE_HEIGHT));
        resolved.ext.letter_spacing = resolved
          .ext
          .letter_spacing
          .or_else(|| resolver.keyframe_run_value(&StaticLeaf::from(typography::LETTER_SPACING)));
        resolved.color = resolved
          .color
          .or_else(|| resolver.keyframe_run_value(&StaticLeaf::from(ink)));

        if link && resolved.decoration.is_none() {
          resolved.decoration = Some(TextDecoration::Underline);
        }

        resolved
      });

      with_records(|records| {
        records
          .restyle
          .read_interaction(node, env.read_interaction());
      });

      TextStyle::resolved(&resolved, Headless::FONT_SIZE)
    });

  resolved.unwrap_or_else(|| style_of(Headless::FONT_SIZE, LINE_HEIGHT, 0.))
}

/// The style of text in the pinned face at the font size `size`, its line height
/// `line_height` times that and `spacing` past each cluster, in black.
pub(crate) fn style_of(size: f32, line_height: f32, spacing: f32) -> TextStyle {
  TextStyle::new(size)
    .with_letter_spacing(spacing)
    .with_line_height(LineHeight::Factor(line_height))
    .with_paint(PaintStyle::default())
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

/// The content box of a text entry's native element `lines` tall, as Chromium sizes
/// one in the typography in force under `env`: as wide as [`Pinned::entry_width`] and
/// as tall as its lines.
fn entry_size(env: &Env, lines: u16) -> Size<f32> {
  let (size, line_height) = typography(env);

  Size::new(
    Pinned::get().entry_width(size, lines),
    f32::from(lines.max(1)) * size * line_height,
  )
}

/// Whether `node` is one of the app's first boxes, as the web's reset finds them: no
/// box, control, overlay or run above it but the body, anchors and scopes aside.
fn first(node: NodeId) -> bool {
  let body = with_records(|records| records.body);
  let mut current = Scene::<Headless>::read(|scene| scene.placed_parent(node));

  while let Some(parent) = current {
    let transparent = with_records(|records| {
      records
        .record(parent)
        .is_some_and(|record| matches!(record.kind, Kind::Anchor | Kind::Scope))
    });

    if Some(parent) == body {
      return true;
    }

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
