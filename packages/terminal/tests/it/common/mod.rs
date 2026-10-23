//! What the render tests share: the platform install, and the probes reading
//! a frame back.

pub(crate) use skene_terminal::testing::{locate, with_launch, with_platform};
use skene_terminal::{Color, NodeId, testing};

/// A cell's foreground and background.
pub(crate) type CellColors = (Option<(u8, u8, u8)>, Option<(u8, u8, u8)>);

/// The foreground and background of the cell (`x`, `y`) in a `width` x `height` frame.
pub(crate) fn colors_at(width: u16, height: u16, x: usize, y: usize) -> CellColors {
  testing::render_to_colors(width, height)[y * usize::from(width) + x]
}

pub(crate) fn red() -> Color {
  Color::srgb(1., 0., 0.)
}

pub(crate) fn green() -> Color {
  Color::srgb(0., 1., 0.)
}

pub(crate) fn blue() -> Color {
  Color::srgb(0., 0., 1.)
}

pub(crate) fn white() -> Color {
  Color::srgb(1., 1., 1.)
}

pub(crate) fn black() -> Color {
  Color::srgb(0., 0., 0.)
}

/// The RGB triple of `color`, as the frame reports it.
pub(crate) fn rgb(color: Color) -> (u8, u8, u8) {
  let [red, green, blue, _] = color.to_rgba();
  let channel = |value: f32| {
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "the channel is clamped to 0..=1 before it is scaled to 0..=255"
    )]
    let quantized = (value.clamp(0., 1.) * 255.).round() as u8;

    quantized
  };

  (channel(red), channel(green), channel(blue))
}

/// A frame's rows without the scrollbar an overflowing editor paints down its last
/// column.
pub(crate) fn rows(frame: &str) -> String {
  frame.lines().fold(String::new(), |mut rows, line| {
    rows.push_str(line.trim_end_matches(|glyph: char| glyph == ' ' || "▁▂▃▄▅▆▇█".contains(glyph)));
    rows.push('\n');

    rows
  })
}

/// What a reader finds of `node`: its node in the accessibility tree the turn ends with.
///
/// # Panics
///
/// Panics where the tree holds no node for `node`.
pub(crate) fn read(node: NodeId) -> accesskit::Node {
  let tree = testing::access_tree();
  let id = node.access_id();

  tree
    .nodes
    .into_iter()
    .find_map(|(held, access)| (held == id).then_some(access))
    .unwrap_or_else(|| panic!("the tree holds no node for {node:?}"))
}

/// Mounts what `views` builds, one beneath another, paints a frame 40 cells by 12
/// so what a reader finds is on screen, and returns the nodes they built, in order.
pub(crate) fn mount_nodes<const N: usize>(views: impl FnOnce() -> [NodeId; N]) -> [NodeId; N] {
  use skene_terminal::prelude::*;

  let mut built = None;
  let _ = skene_terminal::mount(|| {
    let nodes = views();

    built = Some(nodes);

    nodes
      .into_iter()
      .fold(Column::new(), |column, node| {
        column.add_child(AnyView::node(node))
      })
      .build()
  });

  let _ = testing::render_to_text(40, 12);

  built.expect("the views built")
}
