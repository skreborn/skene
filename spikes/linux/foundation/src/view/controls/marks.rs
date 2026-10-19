//! The marks the built-in controls paint, public so a library's control draws the same: each
//! a `Painted` closure in fractions of its box's layout size, drawn in layout units through
//! [`Canvas::to_device`], inked with [`Paint::Ink`] and drawn under the enclosing control's
//! states.

use crate::{
  State,
  seam::picture::kurbo::{BezPath, Circle, Rect, Stroke},
  view::painted::{Canvas, Paint},
};

/// A path through `points`, each a fraction of `canvas`'s layout width and height.
fn polyline(canvas: &Canvas<'_>, points: &[(f64, f64)]) -> BezPath {
  let size = canvas.layout_size();
  let mut path = BezPath::new();

  for (index, (x, y)) in points.iter().enumerate() {
    let point = (x * size.width, y * size.height);

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
  let size = canvas.layout_size();

  canvas.set_transform(canvas.to_device());

  if states.contains(State::Indeterminate) {
    let dash = Rect::new(
      0.167 * size.width,
      0.433 * size.height,
      0.833 * size.width,
      0.567 * size.height,
    );

    canvas.fill(&dash, Paint::Ink);
  } else if states.contains(State::Checked) {
    let tick = polyline(canvas, &[(0.2, 0.533), (0.4, 0.733), (0.8, 0.267)]);

    canvas.stroke(&tick, &Stroke::new(2.), Paint::Ink);
  }
}

/// A radio's mark: a dot, 0.48 of the canvas across at its center, while chosen.
pub fn dot(canvas: &mut Canvas<'_>) {
  if canvas.states().contains(State::Checked) {
    let size = canvas.layout_size();

    canvas.set_transform(canvas.to_device());
    canvas.fill(
      &Circle::new(
        (size.width / 2., size.height / 2.),
        size.width.min(size.height) * 0.24,
      ),
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

  canvas.set_transform(canvas.to_device());
  canvas.stroke(&path, &Stroke::new(1.5), Paint::Ink);
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

    canvas.set_transform(canvas.to_device());
    canvas.stroke(&tick, &Stroke::new(2.), Paint::Ink);
  }
}
