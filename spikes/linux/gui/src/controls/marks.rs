//! The marks the built-in controls paint, public so a library's control draws the same: each
//! a [`Painted`](crate::Painted) closure in fractions of its canvas, inked with
//! [`Paint::Ink`] and drawn under the enclosing control's states.

use skene_foundation::{
  State,
  seam::picture::kurbo::{BezPath, Circle, Rect},
};

use crate::{Canvas, Paint};

/// A path through `points`, each a fraction of `canvas`'s width and height.
fn polyline(canvas: &Canvas<'_>, points: &[(f64, f64)]) -> BezPath {
  let (width, height) = (f64::from(canvas.width()), f64::from(canvas.height()));
  let mut path = BezPath::new();

  for (index, (x, y)) in points.iter().enumerate() {
    let point = (x * width, y * height);

    if index == 0 {
      path.move_to(point);
    } else {
      path.line_to(point);
    }
  }

  path
}

/// A checkbox's mark: a tick stroked 2 pixels wide while checked, a dash while mixed,
/// nothing otherwise.
pub fn check(canvas: &mut Canvas<'_>) {
  let states = canvas.states();
  let (width, height) = (f64::from(canvas.width()), f64::from(canvas.height()));

  if states.contains(State::Indeterminate) {
    let dash = Rect::new(0.167 * width, 0.433 * height, 0.833 * width, 0.567 * height);

    canvas.fill(&dash, Paint::Ink);
  } else if states.contains(State::Checked) {
    let tick = polyline(canvas, &[(0.2, 0.533), (0.4, 0.733), (0.8, 0.267)]);

    canvas.stroke(&tick, 2., Paint::Ink);
  }
}

/// A radio's mark: a dot, 0.48 of the canvas across at its center, while chosen.
pub fn dot(canvas: &mut Canvas<'_>) {
  if canvas.states().contains(State::Checked) {
    let (width, height) = (f64::from(canvas.width()), f64::from(canvas.height()));

    canvas.fill(
      &Circle::new((width / 2., height / 2.), width.min(height) * 0.24),
      Paint::Ink,
    );
  }
}

/// A picker's chevron, pointing down, stroked 1.5 pixels wide.
pub fn chevron(canvas: &mut Canvas<'_>) {
  let path = polyline(
    canvas,
    &[(1. / 12., 0.1875), (0.5, 0.8125), (11. / 12., 0.1875)],
  );

  canvas.stroke(&path, 1.5, Paint::Ink);
}

/// A list option's mark: a tick stroked 2 pixels wide while chosen, nothing otherwise.
pub fn check_mark(canvas: &mut Canvas<'_>) {
  if canvas.states().contains(State::Checked) {
    let tick = polyline(
      canvas,
      &[
        (1.5 / 14., 0.5),
        (5.5 / 14., 10. / 12.),
        (12.5 / 14., 1. / 12.),
      ],
    );

    canvas.stroke(&tick, 2., Paint::Ink);
  }
}
