//! Where an open overlay sits in its window.

use skene_foundation::{NodeId, Placement, Point, Rect, Scalar, Scene, Size};

use super::{Drawn, Kind, Runtime};

impl<P: Drawn> Runtime<P> {
  /// The window origin of the overlay `node` at its laid-out size: pinned at its point,
  /// moved back into the window; opening below or above its anchor at the window
  /// rectangle `anchor_rect` gives for the anchor's placement node, on the other side
  /// where the preferred one runs out of room and the other fits; else at one of the
  /// window's seven placements. Where the layout rounds ([`Drawn::ROUNDED`]), a pinned
  /// point is floored to the unit it falls in and a centered origin truncated toward
  /// zero. Zero for a node that is no overlay.
  ///
  /// `anchor_rect` is called only for an overlay opening against an anchor.
  pub fn overlay_origin(
    &self,
    node: NodeId,
    anchor_rect: impl FnOnce(NodeId) -> Rect<f32>,
  ) -> Point<f32> {
    let Some(Kind::Overlay {
      placement,
      position,
      ..
    }) = self.record(node).map(|record| &record.kind)
    else {
      return Point::ZERO;
    };
    let (placement, position) = (*placement, *position);
    let viewport = self.window().reported().viewport;
    let window = Size::new(viewport.width.to_f32(), viewport.height.to_f32());
    let size = self.tree().layout(node).map_or(Size::ZERO, |layout| {
      Size::new(
        layout.rect.size.width.max(0.),
        layout.rect.size.height.max(0.),
      )
    });
    let anchor = if reads_anchor(placement, position) {
      Scene::<P>::read(|scene| {
        scene
          .overlay_anchor(node)
          .and_then(|anchor| scene.placement(anchor))
      })
      .map(anchor_rect)
    }
    else {
      None
    };

    place(window, size, placement, position, anchor, P::ROUNDED)
  }
}

/// Whether an overlay at `placement`, pinned to `position` if it is, is placed against
/// its anchor's rectangle.
const fn reads_anchor(placement: Placement, position: Option<Point<f32>>) -> bool {
  position.is_none() && matches!(placement, Placement::Top | Placement::Bottom)
}

/// The origin of an overlay of `size` in a window of `window`, at `placement`, pinned to
/// `position` if it is, against the window rectangle `anchor` of the node it opened
/// against, if any, as [`Runtime::overlay_origin`] places it; where the layout rounds
/// (`rounded`), a pinned point is floored to the unit it falls in and a centered origin
/// truncated toward zero.
const fn place(
  window: Size<f32>,
  size: Size<f32>,
  placement: Placement,
  position: Option<Point<f32>>,
  anchor: Option<Rect<f32>>,
  rounded: bool,
) -> Point<f32> {
  let room = Size::new(window.width - size.width, window.height - size.height);

  if let Some(point) = position {
    let (x, y) = if rounded {
      (point.x.floor(), point.y.floor())
    }
    else {
      (point.x, point.y)
    };

    return Point::new(x.min(room.width).max(0.), y.min(room.height).max(0.));
  }

  if let Some(anchor) = anchor
    && reads_anchor(placement, position)
  {
    let x = anchor.origin.x.min(room.width).max(0.);
    let below = anchor.origin.y + anchor.size.height;
    let above = anchor.origin.y - size.height;
    let y = if matches!(placement, Placement::Top) {
      if above < 0. && below + size.height <= window.height {
        below
      }
      else {
        above.max(0.)
      }
    }
    else if below + size.height > window.height && above >= 0. {
      above
    }
    else {
      below.min(room.height).max(0.)
    };

    return Point::new(x, y);
  }

  let center = if rounded {
    Point::new((room.width / 2.).trunc(), (room.height / 2.).trunc())
  }
  else {
    Point::new(room.width / 2., room.height / 2.)
  };

  match placement {
    Placement::Center => center,
    Placement::Top => Point::new(center.x, 0.),
    Placement::TopStart => Point::new(0., 0.),
    Placement::TopEnd => Point::new(room.width, 0.),
    Placement::Bottom => Point::new(center.x, room.height),
    Placement::BottomStart => Point::new(0., room.height),
    Placement::BottomEnd => Point::new(room.width, room.height),
  }
}

#[cfg(test)]
mod tests {
  use skene_foundation::{Placement, Point, Rect, Size};

  use super::place;

  /// A window twenty units wide and ten tall.
  const WINDOW: Size<f32> = Size::new(20., 10.);

  /// Places an unpinned, unanchored overlay of `size` at `placement`, rounded or not.
  const fn in_window(size: Size<f32>, placement: Placement, rounded: bool) -> Point<f32> {
    place(WINDOW, size, placement, None, None, rounded)
  }

  /// Pins an overlay four units wide and three tall to `point`, rounded or not.
  const fn pinned(point: Point<f32>, rounded: bool) -> Point<f32> {
    place(
      WINDOW,
      Size::new(4., 3.),
      Placement::Center,
      Some(point),
      None,
      rounded,
    )
  }

  /// Opens an overlay four units wide and `height` tall at `placement` against an anchor
  /// two units tall at (`3`, `anchor_y`), rounded or not.
  const fn anchored(height: f32, placement: Placement, anchor_y: f32, rounded: bool) -> Point<f32> {
    let anchor = Rect::new(Point::new(3., anchor_y), Size::new(6., 2.));

    place(
      WINDOW,
      Size::new(4., height),
      placement,
      None,
      Some(anchor),
      rounded,
    )
  }

  #[test]
  fn the_window_placements_split_an_even_remainder() {
    let size = Size::new(4., 2.);

    assert_eq!(in_window(size, Placement::Center, true), Point::new(8., 4.));
    assert_eq!(in_window(size, Placement::Top, true), Point::new(8., 0.));
    assert_eq!(
      in_window(size, Placement::TopStart, true),
      Point::new(0., 0.)
    );
    assert_eq!(
      in_window(size, Placement::TopEnd, true),
      Point::new(16., 0.)
    );
    assert_eq!(in_window(size, Placement::Bottom, true), Point::new(8., 8.));
    assert_eq!(
      in_window(size, Placement::BottomStart, true),
      Point::new(0., 8.)
    );
    assert_eq!(
      in_window(size, Placement::BottomEnd, true),
      Point::new(16., 8.)
    );
  }

  #[test]
  fn the_window_placements_truncate_an_odd_remainder() {
    let size = Size::new(5., 3.);

    assert_eq!(in_window(size, Placement::Center, true), Point::new(7., 3.));
    assert_eq!(in_window(size, Placement::Top, true), Point::new(7., 0.));
    assert_eq!(
      in_window(size, Placement::TopStart, true),
      Point::new(0., 0.)
    );
    assert_eq!(
      in_window(size, Placement::TopEnd, true),
      Point::new(15., 0.)
    );
    assert_eq!(in_window(size, Placement::Bottom, true), Point::new(7., 7.));
    assert_eq!(
      in_window(size, Placement::BottomStart, true),
      Point::new(0., 7.)
    );
    assert_eq!(
      in_window(size, Placement::BottomEnd, true),
      Point::new(15., 7.)
    );
  }

  #[test]
  fn an_oversized_overlay_truncates_toward_zero_and_overhangs() {
    let size = Size::new(23., 13.);

    assert_eq!(
      in_window(size, Placement::Center, true),
      Point::new(-1., -1.)
    );
    assert_eq!(
      in_window(size, Placement::TopEnd, true),
      Point::new(-3., 0.)
    );
    assert_eq!(
      in_window(size, Placement::BottomEnd, true),
      Point::new(-3., -3.)
    );
  }

  #[test]
  fn a_pinned_point_sits_at_its_unit() {
    assert_eq!(pinned(Point::new(2.7, 1.5), true), Point::new(2., 1.));
  }

  #[test]
  fn a_pinned_point_past_the_right_edge_moves_back_in() {
    assert_eq!(pinned(Point::new(18.5, 1.5), true), Point::new(16., 1.));
  }

  #[test]
  fn a_pinned_point_past_the_bottom_edge_moves_back_in() {
    assert_eq!(pinned(Point::new(2.5, 9.5), true), Point::new(2., 7.));
  }

  #[test]
  fn bottom_opens_below_its_anchor_with_room() {
    assert_eq!(
      anchored(3., Placement::Bottom, 2., true),
      Point::new(3., 4.)
    );
  }

  #[test]
  fn bottom_flips_above_its_anchor_without_room_below() {
    assert_eq!(
      anchored(3., Placement::Bottom, 6., true),
      Point::new(3., 3.)
    );
  }

  #[test]
  fn bottom_without_room_on_either_side_moves_back_in_below() {
    assert_eq!(
      anchored(8., Placement::Bottom, 4., true),
      Point::new(3., 2.)
    );
  }

  #[test]
  fn top_opens_above_its_anchor_with_room() {
    assert_eq!(anchored(3., Placement::Top, 5., true), Point::new(3., 2.));
  }

  #[test]
  fn top_flips_below_its_anchor_without_room_above() {
    assert_eq!(anchored(3., Placement::Top, 1., true), Point::new(3., 3.));
  }

  #[test]
  fn top_without_room_on_either_side_moves_back_in_above() {
    assert_eq!(anchored(8., Placement::Top, 4., true), Point::new(3., 0.));
  }

  #[test]
  fn an_anchored_overlay_moves_back_in_across() {
    let anchor = Rect::new(Point::new(18., 2.), Size::new(2., 2.));

    assert_eq!(
      place(
        WINDOW,
        Size::new(4., 3.),
        Placement::Bottom,
        None,
        Some(anchor),
        true
      ),
      Point::new(16., 4.)
    );
  }

  #[test]
  fn unrounded_window_placements_split_the_remainder_exactly() {
    let size = Size::new(5., 3.);

    assert_eq!(
      in_window(size, Placement::Center, false),
      Point::new(7.5, 3.5)
    );
    assert_eq!(in_window(size, Placement::Top, false), Point::new(7.5, 0.));
    assert_eq!(
      in_window(size, Placement::TopStart, false),
      Point::new(0., 0.)
    );
    assert_eq!(
      in_window(size, Placement::TopEnd, false),
      Point::new(15., 0.)
    );
    assert_eq!(
      in_window(size, Placement::Bottom, false),
      Point::new(7.5, 7.)
    );
    assert_eq!(
      in_window(size, Placement::BottomStart, false),
      Point::new(0., 7.)
    );
    assert_eq!(
      in_window(size, Placement::BottomEnd, false),
      Point::new(15., 7.)
    );
    assert_eq!(
      in_window(Size::new(23., 13.), Placement::Center, false),
      Point::new(-1.5, -1.5)
    );
  }

  #[test]
  fn an_unrounded_pinned_point_keeps_its_fraction() {
    assert_eq!(pinned(Point::new(2.7, 1.5), false), Point::new(2.7, 1.5));
    assert_eq!(pinned(Point::new(18.5, 1.5), false), Point::new(16., 1.5));
    assert_eq!(pinned(Point::new(2.5, 9.5), false), Point::new(2.5, 7.));
  }

  #[test]
  fn an_unrounded_anchor_places_at_its_fraction() {
    assert_eq!(
      anchored(3., Placement::Bottom, 2.5, false),
      Point::new(3., 4.5)
    );
    assert_eq!(
      anchored(3., Placement::Bottom, 5.5, false),
      Point::new(3., 2.5)
    );
    assert_eq!(
      anchored(8., Placement::Bottom, 4.5, false),
      Point::new(3., 2.)
    );
    assert_eq!(
      anchored(3., Placement::Top, 4.5, false),
      Point::new(3., 1.5)
    );
    assert_eq!(
      anchored(3., Placement::Top, 0.5, false),
      Point::new(3., 2.5)
    );
    assert_eq!(anchored(8., Placement::Top, 4.5, false), Point::new(3., 0.));
  }
}
