//! The environment conditions are answered against: one per thread, its facts signals the
//! realization writes ([`report`]) and an app may read.

use std::cell::OnceCell;

use crate::{Color, Media, Owner, Signal};

/// A color scheme.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Scheme {
  /// Light.
  Light,
  /// Dark.
  Dark,
}

/// What the realization reports about where the app is shown.
///
/// Each fact is a signal: the realization writes it when it changes, and every value resolved
/// against it follows. Before a realization writes anything, the window is 1024 by 768, the
/// scheme light, the contrast and motion unchanged, the scale 1, the direction left to
/// right and no accent.
#[derive(Clone, Copy, Debug)]
pub struct Environment {
  /// The window's width, in pixels.
  pub width: Signal<f32>,
  /// The window's height, in pixels.
  pub height: Signal<f32>,
  /// Whether the dark scheme is in force: the reader's preference, or the app's choice where
  /// it made one.
  pub dark: Signal<bool>,
  /// Whether the reader asked for more contrast.
  pub high_contrast: Signal<bool>,
  /// Whether the reader asked for reduced motion.
  pub reduced_motion: Signal<bool>,
  /// Device pixels per pixel, fractional where the platform is.
  pub scale: Signal<f64>,
  /// Whether the layout direction is right to left.
  pub right_to_left: Signal<bool>,
  /// The reader's accent color, where the platform reports one.
  pub accent: Signal<Option<Color>>,
}

thread_local! {
  static ENVIRONMENT: OnceCell<Environment> = const { OnceCell::new() };
}

impl Environment {
  /// This thread's environment.
  #[must_use]
  pub fn current() -> Self {
    ENVIRONMENT.with(|environment| {
      *environment.get_or_init(|| {
        Owner::root().run(|| Self {
          width: Signal::new(1024.),
          height: Signal::new(768.),
          dark: Signal::new(false),
          high_contrast: Signal::new(false),
          reduced_motion: Signal::new(false),
          scale: Signal::new(1.),
          right_to_left: Signal::new(false),
          accent: Signal::new(None),
        })
      })
    })
  }

  /// Whether `media` holds now, subscribing the running effect to what it reads.
  #[must_use]
  pub fn holds(&self, media: &Media) -> bool {
    match media {
      Media::MinWidth(width) => self.width.get() >= *width,
      Media::MaxWidth(width) => self.width.get() <= *width,
      Media::Dark => self.dark.get(),
      Media::HighContrast => self.high_contrast.get(),
      Media::ReducedMotion => self.reduced_motion.get(),
    }
  }

  /// Puts every fact back to its value before any report: what a test harness does between
  /// tests.
  pub fn reset(&self) {
    report(Fact::Width(1024.));
    report(Fact::Height(768.));
    report(Fact::Dark(false));
    report(Fact::HighContrast(false));
    report(Fact::ReducedMotion(false));
    report(Fact::Scale(1.));
    report(Fact::RightToLeft(false));
    report(Fact::Accent(None));
  }
}

/// A fact about the environment, as a realization reports it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Fact {
  /// The window's width, in pixels.
  Width(f32),
  /// The window's height, in pixels.
  Height(f32),
  /// Whether the dark scheme is in force.
  Dark(bool),
  /// Whether the reader asked for more contrast.
  HighContrast(bool),
  /// Whether the reader asked for reduced motion.
  ReducedMotion(bool),
  /// Device pixels per pixel.
  Scale(f64),
  /// Whether the layout direction is right to left.
  RightToLeft(bool),
  /// The reader's accent color, where the platform reports one.
  Accent(Option<Color>),
}

/// Writes a fact a realization observed, where it changed: every value resolved against it
/// follows, and an unchanged fact reruns nothing.
pub fn report(fact: Fact) {
  let environment = Environment::current();

  match fact {
    Fact::Width(width) => set(environment.width, width),
    Fact::Height(height) => set(environment.height, height),
    Fact::Dark(dark) => set(environment.dark, dark),
    Fact::HighContrast(high) => set(environment.high_contrast, high),
    Fact::ReducedMotion(reduced) => set(environment.reduced_motion, reduced),
    Fact::Scale(scale) => set(environment.scale, scale),
    Fact::RightToLeft(rtl) => set(environment.right_to_left, rtl),
    Fact::Accent(accent) => set(environment.accent, accent),
  }
}

fn set<T: Clone + PartialEq + 'static>(signal: Signal<T>, value: T) {
  if signal.with_untracked(|current| *current != value) {
    signal.set(value);
  }
}
