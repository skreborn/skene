//! Drawing a laid paragraph through vello: its glyphs as outlines, each in its paint's color,
//! at the origin it is given, and its decorations in theirs.

use skene_foundation::{
  Color,
  TextDecoration,
  TextDecorationStyle,
  text::{
    AttributedText,
    Laid as _,
    PaintStyle,
    Paragraph as _,
    ParagraphStyle,
    TextEngine as _,
    TextRun,
    TextStyle,
  },
};
use skene_parley::ParleyLaid;
use vello_cpu::{Pixmap, RenderContext, Resources};

use crate::{SIZE, inter};

/// The pixmap's side.
const SIDE: u16 = 96;

/// `laid` drawn with its top left at `origin` on a transparent pixmap: each pixel's red,
/// green, blue and alpha, row by row.
fn draw(laid: &ParleyLaid, origin: (f64, f64)) -> Vec<[u8; 4]> {
  let mut context = RenderContext::new(SIDE, SIDE);
  let mut pixmap = Pixmap::new(SIDE, SIDE);

  laid.paint_outlines(&mut context, origin);
  context.flush();
  context.render(&mut pixmap, &mut Resources::new());

  pixmap
    .data()
    .iter()
    .map(|pixel| [pixel.r, pixel.g, pixel.b, pixel.a])
    .collect()
}

/// The rows and columns (`top`, `bottom`, `left`, `right`, inclusive) of the pixels `inked`
/// holds, where any does.
fn extent(pixels: &[[u8; 4]], inked: impl Fn([u8; 4]) -> bool) -> Option<[usize; 4]> {
  let side = usize::from(SIDE);

  pixels
    .iter()
    .enumerate()
    .filter(|(_, pixel)| inked(**pixel))
    .map(|(at, _)| (at / side, at % side))
    .fold(None, |found, (row, column)| {
      let [top, bottom, left, right] = found.unwrap_or([row, row, column, column]);

      Some([
        top.min(row),
        bottom.max(row),
        left.min(column),
        right.max(column),
      ])
    })
}

/// Pixel row `at` as a coordinate.
#[expect(clippy::cast_precision_loss, reason = "small pixel rows")]
const fn row(at: usize) -> f32 {
  at as f32
}

/// Whether a pixel is mostly red.
const fn red(pixel: [u8; 4]) -> bool {
  pixel[3] > 128 && pixel[0] > 128 && pixel[2] < 64
}

/// Whether a pixel is mostly blue.
const fn blue(pixel: [u8; 4]) -> bool {
  pixel[3] > 128 && pixel[2] > 128 && pixel[0] < 64
}

#[test]
fn a_layout_draws_its_glyphs_in_their_paint_where_it_is_placed() {
  let text = "Hi Hi";
  let red_paint = PaintStyle::new(Color::srgb(1., 0., 0.));
  let blue_paint = PaintStyle::new(Color::srgb(0., 0., 1.));
  let attributed = AttributedText::new(
    text,
    TextStyle::new(SIZE).with_paint(red_paint),
    vec![TextRun {
      range: 3..5,
      style: TextStyle::new(SIZE).with_paint(blue_paint),
    }],
  );
  let laid = inter()
    .paragraph(&attributed, &ParagraphStyle::default())
    .layout(None);
  let pixels = draw(&laid, (8., 8.));
  let reds = extent(&pixels, red).expect("the first word is drawn in red");
  let blues = extent(&pixels, blue).expect("the second word is drawn in blue");
  let baseline = 8. + laid.first_baseline();

  assert!(
    reds[3] < blues[2],
    "the first word left of the second: {reds:?} {blues:?}"
  );
  assert!(
    reds[2] >= 8,
    "nothing is drawn left of the origin: {reds:?}"
  );
  for bounds in [reds, blues] {
    assert!(
      bounds[0] >= 8 && row(bounds[1]) < baseline + 1.,
      "the capitals stand on the baseline at {baseline}: {bounds:?}"
    );
    assert!(
      row(bounds[1]) > baseline - 2.,
      "and reach down to it: {bounds:?}"
    );
  }

  let moved = draw(&laid, (20., 30.));
  let moved_reds = extent(&moved, red).expect("drawn again");

  assert_eq!(
    [moved_reds[0], moved_reds[2]],
    [reds[0] + 22, reds[2] + 12],
    "the origin moves the drawing"
  );
}

#[test]
fn a_decoration_is_drawn_in_its_color_where_its_line_lies() {
  let paint = PaintStyle {
    decoration_color: Some(Color::srgb(0., 0., 1.)),
    ..PaintStyle::new(Color::srgb(1., 0., 0.))
      .with_decoration(TextDecoration::Underline, TextDecorationStyle::Solid)
  };
  let laid = inter()
    .paragraph(
      &AttributedText::plain("xxxx", TextStyle::new(SIZE).with_paint(paint)),
      &ParagraphStyle::default(),
    )
    .layout(None);
  let pixels = draw(&laid, (0., 0.));
  let glyphs = extent(&pixels, red).expect("the glyphs are drawn in red");
  let line = extent(&pixels, blue).expect("the underline is drawn in blue");

  assert!(
    line[0] > glyphs[1],
    "the underline lies beneath the glyphs: {line:?} {glyphs:?}"
  );
  assert!(line[1] - line[0] <= 2, "a thin line: {line:?}");
  assert!(
    line[2] <= glyphs[2] && line[3] >= glyphs[3],
    "under the whole run: {line:?} {glyphs:?}"
  );
}
