//! # acme-rating
//!
//! A row of stars a reader rates by: a [`Control`] over a behavior of the crate's own
//! ([`Rating`]), drawn by a `Painted` part, reported to readers as a slider and answering the
//! shipped slider's keys. It is written against skene-gui's public API alone, as any library's
//! drawn control is, and names no platform.
//!
//! ```
//! use acme_rating::star_rating;
//! use skene_gui::prelude::*;
//!
//! let stars = Signal::new(3_u8);
//!
//! Column::new().add_child(star_rating("Rating", stars));
//! ```

use skene_gui::{
  AccessFacts, AccessRequest, Axis, KeyEvent, LabelPlacement, Part, PointerEvent, PointerPhase,
  Prevent, Range, RangeValue, Role,
  behavior::{Behavior, ControlHandle, Controlled, SliderKeys},
  painted::shapes::BezPath,
  prelude::*,
};

/// How many stars.
pub const STARS: u8 = 5;

/// A rating's behavior: 0 to [`STARS`] whole stars, the slider's keys ([`SliderKeys`]), a press
/// choosing the star under it.
///
/// It is controlled: it shows what its value reads and only asks its handler, so a rating its
/// handler refuses never shows.
pub struct Rating {
  value: Controlled<u8>,
}

impl Rating {
  /// A rating showing what `value` holds and writing each choice into it.
  pub fn bind(value: Signal<u8>) -> Self {
    Self {
      value: Controlled::bind(value),
    }
  }

  /// The rating as it reads now, tracking it.
  fn now(&self) -> u8 {
    self.value.read().unwrap_or(0)
  }

  /// The shipped slider's key table over the rating as it shows now.
  fn keys(&self) -> SliderKeys {
    SliderKeys::new(Range {
      value: f64::from(skene_gui::untrack(|| self.now())),
      minimum: 0.,
      maximum: f64::from(STARS),
      step: 1.,
    })
  }

  /// Asks for `stars`, held within the stars there are.
  fn ask(&self, stars: f64) {
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "clamped to the stars there are"
    )]
    let stars = stars.round().clamp(0., f64::from(STARS)) as u8;

    self.value.ask(stars);
  }
}

impl Behavior for Rating {
  fn role(&self) -> Role {
    Role::Slider
  }

  fn label_placement(&self) -> LabelPlacement {
    LabelPlacement::Beside
  }

  fn on_key(&self, event: &KeyEvent, _: &ControlHandle) -> Prevent {
    let response = self.keys().key(event);

    if let Some(value) = response.act {
      self.ask(value);
    }

    response.verdict()
  }

  fn on_pointer(&self, event: &PointerEvent, control: &ControlHandle) -> Prevent {
    let (PointerPhase::Down, Some(stars)) = (event.phase, control.frame(Part::Track)) else {
      return Prevent::None;
    };
    let star = ((event.window_x - stars.x) / stars.width.max(1.) * f32::from(STARS)).ceil();

    self.ask(f64::from(star.max(1.)));

    Prevent::All
  }

  fn on_access(&self, request: AccessRequest, _: &ControlHandle) -> bool {
    let now = f64::from(skene_gui::untrack(|| self.now()));

    match request {
      AccessRequest::Increment => self.ask(now + 1.),
      AccessRequest::Decrement => self.ask(now - 1.),
      AccessRequest::SetValue(value) => self.ask(value),
      _ => return false,
    }

    true
  }

  fn access(&self) -> AccessFacts {
    let now = self.now();

    AccessFacts {
      value: Some(RangeValue {
        now: f64::from(now),
        minimum: 0.,
        maximum: f64::from(STARS),
        step: Some(1.),
        text: Some(format!("{now} of {STARS} stars").into()),
      }),
      orientation: Some(Axis::Horizontal),
      actions: vec![AccessRequest::Increment, AccessRequest::Decrement],
      ..AccessFacts::default()
    }
  }
}

/// A star rating labelled `label`, bound to `value`: the stars up to it filled in the system's
/// accent, the rest outlined, a ring of 2 while focus shows. It is a [`Control`], so every
/// member of one applies: `set_disabled` dims it and takes it out of the Tab order with no code
/// of its own.
pub fn star_rating(label: &str, value: Signal<u8>) -> Control<Rating> {
  Control::new(Rating::bind(value))
    .set_color(SystemColors::ACCENT)
    .set_corner_radius(4)
    .set_ring(Ring::NONE.when(State::FocusVisible, Ring::new(2)))
    .set_ring_color(SystemColors::ACCENT)
    .set_label(label)
    .add_part(
      Part::Track,
      Painted::new(move |canvas| {
        let size = f64::from(canvas.height());

        for star in 0..STARS {
          let path = star_path(size * f64::from(star), size);

          if star < value.get() {
            canvas.fill(&path, Paint::Ink);
          } else {
            canvas.stroke(&path, 1.5, Paint::Ink);
          }
        }
      })
      .set_width(24 * i32::from(STARS))
      .set_height(24),
    )
}

/// A five-pointed star in a `size` square whose leading edge is at `left`, inset so a stroke
/// stays inside it.
fn star_path(left: f64, size: f64) -> BezPath {
  let (center_x, center_y) = (left + size / 2., size / 2.);
  let outer = size / 2. - 2.;
  let inner = outer * 0.4;
  let mut path = BezPath::new();

  for point in 0..10 {
    let radius = if point % 2 == 0 { outer } else { inner };
    let angle = std::f64::consts::PI * (f64::from(point) / 5. - 0.5);
    let at = (
      center_x + radius * angle.cos(),
      center_y + radius * angle.sin(),
    );

    if point == 0 {
      path.move_to(at);
    } else {
      path.line_to(at);
    }
  }

  path.close_path();

  path
}
