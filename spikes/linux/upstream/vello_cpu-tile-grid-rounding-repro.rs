//! vello_cpu 0.3.0: a pixel wholly inside a translucent shape comes out one level apart
//! depending on where the drawing's 4x4 tile grid falls relative to the shape's edges: drawing
//! the same scene into a context whose origin sits two pixels further right (as a renderer
//! drawing only a damaged part of a frame does) changes pixels the shape covers entirely.
use vello_cpu::{
  Pixmap, RenderContext, Resources,
  kurbo::{Affine, BezPath, Rect, Shape as _},
  peniko::Color,
};

/// The scene: an opaque gray background and a translucent rectangle from `start` to 32 across,
/// as a path; drawn into a 32x4 context whose origin is `origin` pixels into the scene.
/// Returns the scene's pixel at x = `at`, y = 1.
fn pixel(start: f64, origin: f64, at: usize, paint: Color) -> [u8; 4] {
  let mut ctx = RenderContext::new(32, 4);
  ctx.set_transform(Affine::translate((-origin, 0.)));
  ctx.set_paint(Color::from_rgba8(128, 128, 128, 255));
  ctx.fill_rect(&Rect::new(0., 0., 64., 4.));
  ctx.set_paint(paint);
  let path: BezPath = Rect::new(start, 0., 32., 4.).to_path(0.1);
  ctx.fill_path(&path);
  ctx.flush();
  let mut pixmap = Pixmap::new(32, 4);
  let mut resources = Resources::new();
  ctx.render(&mut pixmap, &mut resources);
  let p = pixmap.data()[32 + at - origin as usize];
  [p.r, p.g, p.b, p.a]
}

fn main() {
  let mut cases = 0;
  let mut apart = 0;
  for alpha in [37u8, 64, 100, 128, 150, 200] {
    for (r, g, b) in [(255u8, 0u8, 0u8), (30, 144, 255), (255, 255, 255)] {
      let paint = Color::from_rgba8(r, g, b, alpha);
      for start in [2.0, 2.5, 5.0, 5.5, 6.25] {
        // Pixel 9 is wholly inside the rectangle in every case.
        let reference = pixel(start, 0., 9, paint);
        for origin in [1., 2., 3.] {
          cases += 1;
          let shifted = pixel(start, origin, 9, paint);
          if shifted != reference {
            apart += 1;
            if apart <= 12 {
              println!(
                "paint {r},{g},{b},{alpha}; rectangle from x={start}: pixel 9 drawn from origin 0 {reference:?}, from origin {origin} {shifted:?}"
              );
            }
          }
        }
      }
    }
  }
  println!("{apart} of {cases} cases draw a wholly covered pixel differently");
}
