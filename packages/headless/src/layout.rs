//! The layout of the scene: each record's part in it, a box styled from its resolved
//! style and a run measured by the platform's approximate metrics, laid out through the
//! shared layout at the viewport.

use skene_foundation::{Layout, NodeId, Platform, Scene, Size, StaticBoxStyle, StaticRunStyle};
use skene_layout::{
  Borders,
  BoxLayout,
  Frames,
  Part,
  Place,
  Tree,
  taffy::{prelude::AvailableSpace, style::Style},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
  Headless,
  records::{DEFAULT_VIEWPORT, Kind},
  resolve::{Env, Records},
  with_records,
};

/// The line height a run takes where its style sets none, as a multiple of its font
/// size.
const LINE_HEIGHT: f32 = 1.5;

/// A run's text as the layout measures it: each grapheme cluster half the font size
/// wide, each line the font size times the line height tall, wrapped between words.
pub(crate) struct Measured {
  text: String,
  size: f32,
  line_height: f32,
}

impl Measured {
  /// The size the text takes within `known` dimensions and the space `available`.
  fn measure(
    &self,
    known: skene_layout::taffy::geometry::Size<Option<f32>>,
    available: skene_layout::taffy::geometry::Size<AvailableSpace>,
  ) -> Size<f32> {
    let advance = self.size / 2.;
    let width_of = |text: &str| {
      #[expect(
        clippy::cast_precision_loss,
        reason = "a cluster count stays far below the integers f32 holds exactly"
      )]
      let clusters = text.graphemes(true).count() as f32;

      clusters * advance
    };
    let limit = known.width.or(match available.width {
      AvailableSpace::Definite(width) => Some(width),
      AvailableSpace::MinContent => Some(0.),
      AvailableSpace::MaxContent => None,
    });
    let mut widest = 0_f32;
    let mut lines = 0_u32;

    if !self.text.is_empty() {
      for paragraph in self.text.split('\n') {
        let mut line = 0_f32;

        lines += 1;

        for word in paragraph.split_inclusive(' ') {
          let ink = width_of(word.trim_end());

          if line > 0. && limit.is_some_and(|limit| line + ink > limit) {
            lines += 1;
            line = 0.;
          }

          widest = widest.max(line + ink);
          line += width_of(word);
        }
      }
    }

    #[expect(
      clippy::cast_precision_loss,
      reason = "a line count stays far below the integers f32 holds exactly"
    )]
    let height = lines as f32 * self.size * self.line_height;

    Size::new(
      known.width.unwrap_or(widest),
      known.height.unwrap_or(height),
    )
  }
}

/// Lays out every tree of the scene at the viewport.
pub(crate) fn lay_out() -> Frames {
  let viewport = with_records(|records| records.viewport).unwrap_or(DEFAULT_VIEWPORT);
  let roots: Vec<NodeId> = Scene::<Headless>::read(|scene| scene.roots().collect());
  let mut frames = Frames::default();

  for root in roots {
    frames.extend(Tree::<Headless, Measured>::lay_out(
      root,
      viewport,
      false,
      part,
      |known, available, text| text.measure(known, available),
    ));
  }

  frames
}

/// What `node` is to the layout: a box styled from its resolved style, a run measured
/// from its text, an open overlay out of the flow, an anchor or a scope transparent,
/// and a hidden box or a closed overlay left out.
fn part(node: NodeId) -> Part<Measured> {
  let Some(env) = Env::of(&Records, node)
  else {
    return Part::Absent;
  };
  let kind = with_records(|records| {
    let record = records.record(node)?;

    Some(match &record.kind {
      Kind::Box(props) => Shape::Box(props.layout, Box::new(props.style.clone()), false),
      Kind::Control(props) => Shape::Box(
        props.box_props.layout,
        Box::new(props.box_props.style.clone()),
        false,
      ),
      Kind::Overlay(props) if record.open => Shape::Box(
        props.box_props.layout,
        Box::new(props.box_props.style.clone()),
        true,
      ),
      Kind::Overlay(_) => Shape::Absent,
      Kind::Run(props) => Shape::Run(Box::new(props.style.clone()), record.text.clone()),
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
    Some(Shape::Box(layout, style, overlay)) => {
      let resolved = env.with_resolver(|resolver| resolver.box_style(&style));

      if resolved.hidden == Some(true) {
        return Part::Absent;
      }

      if overlay {
        let mut style = resolved.taffy_style(layout, place.detached());

        style.position = skene_layout::taffy::style::Position::Absolute;

        Part::Overlay(style)
      }
      else {
        Part::Box(resolved.taffy_style(layout, place))
      }
    }
    Some(Shape::Run(style, text)) => {
      let (resolved, layout) = env.with_resolver(|resolver| {
        let annotation = Scene::<Headless>::read(|scene| scene.record(node)?.annotation().cloned());

        (
          resolver.run_style(&style),
          annotation.map(|annotation| annotation.resolve(resolver)),
        )
      });
      let mut taffy = Style::default();

      match layout {
        Some(layout) => layout.fold_into(&mut taffy, place),
        None => place.apply(&mut taffy),
      }

      Part::Leaf(
        taffy,
        Measured {
          text,
          size: resolved
            .ext
            .font_size
            .map_or(Headless::FONT_SIZE, |size| size.0),
          line_height: resolved.ext.line_height.unwrap_or(LINE_HEIGHT),
        },
      )
    }
  }
}

/// What a record holds that its part is made from, read out of the records.
enum Shape {
  Box(Layout, Box<StaticBoxStyle<Headless>>, bool),
  Run(Box<StaticRunStyle<Headless>>, String),
  Transparent,
  Absent,
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
