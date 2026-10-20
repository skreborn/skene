//! # skene-conformance-linux
//!
//! The conformance suite on Linux, and skene-gui-linux's own driver tests: everything that
//! drives the Linux realization through the facade, kept in a crate above both, so the
//! realization depends on neither the facade nor the suite. Every test runs through the display
//! runner's private session on the leg `SKENE_TEST_SESSION` names, as a reader reads the window
//! and a user acts on it.
//!
//! - `tests/it`: the suite's scenarios, a `#[skene_gui::test]` each (`suite`), and the
//!   realization's own driver tests, a module per area (the window chrome, drag and drop, touch,
//!   scale, scrolling, what reaches the screen, ...).
//! - `tests/gpu`: the GPU path, built where the realization builds it (skene-gui-linux's `gpu`
//!   feature): every page of the suite painted on the GPU and on the CPU, compared.
//!
//! The library holds what both share: a page drawn again in part, and the check that a window
//! drawn again in part shows what it showed drawn whole.

use skene_gui::{prelude::*, testing::Screenshot};

/// The page beneath everything: a mid gray, which a translucent paint over it shows the
/// rounding of.
pub const PAGE: Color = Color::rgb(0x808080);

/// A translucent white veil over the page, along the plate's row.
pub const VEIL: Color = Color::rgba(0xffff_ff80);

/// The plate unlit.
pub const PLATE: Color = Color::rgb(0x2f6fdf);

/// The plate lit.
pub const PLATE_LIT: Color = Color::rgb(0x1f57b8);

/// What the plate says.
pub const WORDS: &str = "Grumpy wizards make toxic brew";

/// The plate's padding: top, trailing, bottom, leading.
pub const PLATE_PADDING: (f32, f32, f32, f32) = (7., 11., 9., 5.);

/// A mid-gray page holding, 13 pixels in, a translucent veil along a row: a 31-pixel gap, a
/// blue plate holding white text, its left edge 53 pixels in (off any grid of 2 or 4 pixels),
/// and a switch that lights the plate.
pub fn plate() -> impl View {
  let lit = Signal::new(false);
  let (top, trailing, bottom, leading) = PLATE_PADDING;

  Container::new().set_background(PAGE).set_child(
    Column::new().set_padding(13).add_child(
      Container::new().set_background(VEIL).set_child(
        Row::new()
          .set_spacing(9)
          .set_item_alignment(AxisAlignment::Center)
          .add_child(Container::new().set_width(31).set_height(8))
          .add_child(
            Container::new()
              .set_padding(Inset::new(top, trailing, bottom, leading))
              .derive_background(move || if lit.get() { PLATE_LIT } else { PLATE })
              .set_child(Text::new(WORDS).set_color(Color::WHITE).set_font_size(15)),
          )
          .add_child(Toggle::new("Lit").bind_on(lit)),
      ),
    ),
  )
}

/// Asserts that `again`, the window drawn again in part, shows pixel for pixel what `whole`,
/// the window drawn whole, showed; at a fractional scale, within one level in a few of its
/// edges' pixels, which vello_cpu's coverage, computed in `f32` at the part's own coordinates,
/// may round one level apart from the whole frame's at a fractional device position.
pub fn assert_drawn_alike(whole: &Screenshot, again: &Screenshot) {
  let (before, _) = whole.rgba.as_chunks::<4>();
  let (after, _) = again.rgba.as_chunks::<4>();
  let mut differing = 0;
  let mut most = 0;
  let mut first = None;

  for (index, (was, is)) in before.iter().zip(after).enumerate() {
    for (was, is) in was.iter().zip(is) {
      most = most.max(was.abs_diff(*is));
    }

    if was != is {
      differing += 1;
      first = first.or(Some((
        index % whole.width as usize,
        index / whole.width as usize,
      )));
    }
  }

  let fractional = whole.scale.fract() != 0.;
  let allowed = if fractional { (8, 1) } else { (0, 0) };

  assert_eq!((whole.width, whole.height), (again.width, again.height));
  assert!(
    differing <= allowed.0 && most <= allowed.1,
    "the window drawn again in part shows {differing} pixels unlike the window drawn whole, a \
     channel at most {most} apart, the first at {first:?} (device pixels; at scale {} at most \
     {} pixels {} apart)",
    whole.scale,
    allowed.0,
    allowed.1
  );
}
