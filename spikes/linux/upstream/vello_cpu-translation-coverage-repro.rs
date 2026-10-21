//! A shape drawn at a whole number of pixels lower, in a context that much taller, should cover
//! the same pixels alike. With vello_cpu 0.3.0's u8 pipeline, a circle at fractional
//! coordinates does not: a few edge pixels come out one level apart.
use vello_cpu::{
  Pixmap, RenderContext, Resources,
  kurbo::{Affine, Circle, Shape as _},
  peniko::color::{AlphaColor, Srgb},
};

fn draw(shift: f64, height: u16) -> Pixmap {
  let mut ctx = RenderContext::new(96, height);
  let mut pixmap = Pixmap::new(96, height);
  let mut resources = Resources::new();

  ctx.set_transform(Affine::translate((0., shift)));
  ctx.set_paint(AlphaColor::<Srgb>::from_rgba8(255, 255, 255, 255));
  ctx.fill_path(&Circle::new((47.25, 30.75), 16.5).to_path(0.1));
  ctx.flush();
  ctx.render_with(&mut pixmap, &mut resources, vello_cpu::RasterizerSettings::default());

  pixmap
}

fn main() {
  let near = draw(0., 64);
  let mut total = 0;

  for shift in [4u16, 8, 100, 400, 1000, 4000] {
    let far = draw(f64::from(shift), 64 + shift);
    let mut differing = Vec::new();

    for y in 0..64usize {
      for x in 0..96usize {
        let a = near.data()[y * 96 + x];
        let b = far.data()[(y + usize::from(shift)) * 96 + x];

        if a != b {
          differing.push((x, y, a.a, b.a));
        }
      }
    }

    total += differing.len();
    println!(
      "shifted down {shift} px: {} pixels differ {:?}",
      differing.len(),
      &differing[..differing.len().min(4)]
    );
  }

  println!("{total} differing in all");
}
