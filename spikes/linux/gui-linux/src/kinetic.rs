//! Kinetic scrolling: a touchpad's scroll going on once its fingers lift, slowing down, as
//! GTK's kinetic scrolling does it: the velocity of the last 150 ms of the fingers' motion, an
//! exponential deceleration (GTK's friction of 4 per second), and a fling flicked again the same
//! way adding to the one still moving. A wheel never flings: its detents send no end of motion.

use std::{collections::VecDeque, time::Instant};

/// The motion a velocity is taken over, in milliseconds before the fingers lift.
const CAPTURE: u32 = 150;

/// How fast a fling slows down: its velocity falls by e every quarter of a second.
const FRICTION: f32 = 4.;

/// A fling stops once what is left of its travel is below this many pixels.
const REST: f32 = 0.5;

/// A fling flicked again the same way at least this share of the one still moving adds to it.
const ACCUMULATION_FLOOR: f32 = 0.33;

/// Up to this many times the moving fling's velocity.
const ACCUMULATION_MAX: f32 = 6.;

/// The fingers' recent motion on a touchpad.
#[derive(Default)]
pub struct Tracker {
  /// Each frame of motion: when (the compositor's milliseconds) and by how much.
  history: VecDeque<(u32, f32, f32)>,
}

impl Tracker {
  /// The fingers moved by `dx`, `dy` pixels at `time`.
  pub fn push(&mut self, time: u32, dx: f32, dy: f32) {
    while self
      .history
      .front()
      .is_some_and(|(then, ..)| time.wrapping_sub(*then) > CAPTURE)
    {
      self.history.pop_front();
    }

    self.history.push_back((time, dx, dy));
  }

  /// The fingers lifted at `time`: their velocity over the motion captured, in pixels a second,
  /// and the motion forgotten.
  pub fn finish(&mut self, time: u32) -> (f32, f32) {
    self.push(time, 0., 0.);

    let first = self.history.front().map_or(time, |(then, ..)| *then);
    let (dx, dy) = self
      .history
      .iter()
      .fold((0., 0.), |(x, y), (_, dx, dy)| (x + dx, y + dy));
    let span = time.wrapping_sub(first);

    self.history.clear();

    if span == 0 {
      return (0., 0.);
    }

    let seconds = span as f32 / 1000.;

    (dx / seconds, dy / seconds)
  }

  /// Forgets the motion.
  pub fn reset(&mut self) {
    self.history.clear();
  }
}

/// A scroll going on by itself, slowing down.
#[derive(Clone, Copy, Debug)]
pub struct Fling {
  /// Its velocity when it started, in pixels a second.
  velocity: (f32, f32),
  /// When it started.
  started: Instant,
  /// How far it has moved so far.
  travelled: (f32, f32),
}

impl Fling {
  /// A fling at `velocity` pixels a second from `now`, the one `moving` still moved adding to
  /// it where it goes the same way.
  pub fn new(velocity: (f32, f32), now: Instant, moving: Option<&Self>) -> Self {
    let accumulate = |velocity: f32, last: f32| {
      if last == 0. || velocity.signum() != last.signum() {
        return velocity;
      }

      let floor = last * ACCUMULATION_FLOOR;

      if velocity.abs() < floor.abs() {
        return velocity;
      }

      let share = (velocity - floor) / (last - floor);

      velocity + last * share.min(ACCUMULATION_MAX)
    };
    let velocity = match moving {
      Some(moving) => {
        let last = moving.velocity_at(now);

        (
          accumulate(velocity.0, last.0),
          accumulate(velocity.1, last.1),
        )
      }
      None => velocity,
    };

    Self {
      velocity,
      started: now,
      travelled: (0., 0.),
    }
  }

  /// Whether it moves at all: what is left of its travel is a pixel or more.
  pub fn moves(&self) -> bool {
    self.velocity.0.abs() / FRICTION >= REST || self.velocity.1.abs() / FRICTION >= REST
  }

  /// Its velocity at `now`, in pixels a second.
  pub fn velocity_at(&self, now: Instant) -> (f32, f32) {
    let decay = (-FRICTION * now.saturating_duration_since(self.started).as_secs_f32()).exp();

    (self.velocity.0 * decay, self.velocity.1 * decay)
  }

  /// How far to scroll at `now` since the last step, and whether it goes on after.
  pub fn step(&mut self, now: Instant) -> ((f32, f32), bool) {
    let elapsed = now.saturating_duration_since(self.started).as_secs_f32();
    let reach = (1. - (-FRICTION * elapsed).exp()) / FRICTION;
    let at = (self.velocity.0 * reach, self.velocity.1 * reach);
    let delta = (at.0 - self.travelled.0, at.1 - self.travelled.1);
    let (vx, vy) = self.velocity_at(now);

    self.travelled = at;

    (
      delta,
      vx.abs() / FRICTION >= REST || vy.abs() / FRICTION >= REST,
    )
  }
}
