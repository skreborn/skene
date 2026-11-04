//! The environment the drawn runtime keeps of its window: what the platform reports, the
//! app's choice of scheme, whether the window paints pictures, and the signals following
//! them, moved together by one writer.

use std::fmt;

use skene_foundation::{
  ColorScheme,
  ContainerName,
  Environment,
  MediaSignals,
  MediaWriters,
  Scalar,
  Size,
  reactive::{Owner, ReadSignal, Set, Signal, WriteSignal},
};

use super::{Drawn, Runtime};

/// What the runtime keeps of one window: the environment its platform reports, whether
/// it paints pictures, and the signals following them once first asked for or moved.
#[must_use]
pub struct Window<P: Drawn> {
  reported: Environment<P::Scalar>,
  painting: bool,
  media: Option<Media<P::Scalar>>,
}

impl<P: Drawn> fmt::Debug for Window<P> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Window")
      .field("reported", &self.reported)
      .field("painting", &self.painting)
      .finish_non_exhaustive()
  }
}

impl<P: Drawn> Window<P> {
  /// A window whose platform reports `reported` and whose pictures paint where
  /// `painting` holds, its signals yet to be asked for.
  pub(super) const fn new(reported: Environment<P::Scalar>, painting: bool) -> Self {
    Self {
      reported,
      painting,
      media: None,
    }
  }

  /// The environment the platform reports, its scheme the platform's own.
  #[inline(always)]
  pub const fn reported(&self) -> &Environment<P::Scalar> {
    &self.reported
  }

  /// Whether the window paints pictures, as
  /// [`Painting::available`](skene_foundation::Painting::available) reports.
  #[inline(always)]
  #[must_use]
  pub const fn painting(&self) -> bool {
    self.painting
  }

  /// The size last published for the named container `name`
  /// ([`Runtime::publish_container_sizes`]); `None` where none was.
  #[must_use]
  pub fn container_size(&self, name: ContainerName) -> Option<Size<P::Scalar>> {
    self
      .media
      .as_ref()?
      .signals
      .container_sizes
      .get_untracked(name)
  }
}

/// The environment's signals with their write halves, under a root owner of their own.
struct Media<N: 'static> {
  owner: Owner,
  signals: MediaSignals<N>,
  writers: MediaWriters<N>,
  painting: ReadSignal<bool>,
  set_painting: WriteSignal<bool>,
}

/// What one report changes: the environment the platform reports, its scheme the
/// platform's own, whether the window paints pictures, and the app's choice of scheme,
/// `None` following the platform's.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Facts<N> {
  /// The environment the platform reports.
  pub reported: Environment<N>,
  /// Whether the window paints pictures.
  pub painting: bool,
  /// The app's choice of scheme, `None` following the platform's.
  pub choice: Option<ColorScheme>,
}

/// The signal writes a report leaves for once the runtime is released: the environment
/// in force and whether the window paints, before and after, with the write halves where
/// the signals exist.
#[must_use = "a report's signals move only once it is written"]
pub struct Pending<N: 'static> {
  writers: Option<(MediaWriters<N>, WriteSignal<bool>)>,
  before: (Environment<N>, bool),
  after: (Environment<N>, bool),
}

impl<N: 'static> fmt::Debug for Pending<N> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.debug_struct("Pending").finish_non_exhaustive()
  }
}

impl<N: Scalar> Pending<N> {
  /// Whether the report moved the environment in force or whether the window paints.
  #[must_use]
  pub fn moved(&self) -> bool {
    self.before != self.after
  }

  /// Writes every signal whose value the report moved, running what reads it.
  pub fn write(self) {
    let Some((writers, set_painting)) = self.writers
    else {
      return;
    };
    let ((before, painted), (after, painting)) = (self.before, self.after);

    if before.viewport != after.viewport {
      writers.viewport.set(after.viewport);
    }

    if before.scheme != after.scheme {
      writers.scheme.set(after.scheme);
    }

    if before.reduced_motion != after.reduced_motion {
      writers.reduced_motion.set(after.reduced_motion);
    }

    if before.reduced_transparency != after.reduced_transparency {
      writers.reduced_transparency.set(after.reduced_transparency);
    }

    if before.high_contrast != after.high_contrast {
      writers.high_contrast.set(after.high_contrast);
    }

    if before.pointer != after.pointer {
      writers.pointer.set(after.pointer);
    }

    if before.hover != after.hover {
      writers.hover.set(after.hover);
    }

    if before.safe_area != after.safe_area {
      writers.safe_area.set(after.safe_area);
    }

    if painted != painting {
      set_painting.set(painting);
    }
  }
}

impl<P: Drawn> Runtime<P> {
  /// What the runtime keeps of its window.
  #[inline(always)]
  pub const fn window(&self) -> &Window<P> {
    &self.window
  }

  /// The scheme the app has chosen, `None` where it follows the platform's.
  #[inline(always)]
  #[must_use]
  pub const fn chosen_scheme(&self) -> Option<ColorScheme> {
    self.choice
  }

  /// The color scheme in force: the app's choice where it made one, else the one the
  /// platform reports.
  pub const fn scheme(&self) -> ColorScheme {
    match self.choice {
      Some(scheme) => scheme,
      None => self.window.reported.scheme,
    }
  }

  /// The environment in force: the one the platform reports under the scheme in force.
  pub const fn environment(&self) -> Environment<P::Scalar> {
    Environment {
      scheme: self.scheme(),
      ..self.window.reported
    }
  }

  /// Marks every resolved style stale and the frame in need of layout and repaint.
  #[inline(always)]
  pub const fn invalidate_styles(&mut self) {
    self.restyle.invalidate();
    self.layout_dirty = true;
    self.paint_dirty = true;
  }

  /// The environment's signals, created holding the environment in force on first use,
  /// under a root owner of their own.
  pub fn media(&mut self) -> MediaSignals<P::Scalar> {
    self.media_state().signals
  }

  /// Whether the window paints pictures, as a signal, created on first use beside the
  /// environment's.
  pub fn painting_available(&mut self) -> ReadSignal<bool> {
    self.media_state().painting
  }

  /// Takes the owner of the environment's signals, for a teardown to dispose; `None`
  /// where they were never created.
  pub fn take_media_owner(&mut self) -> Option<Owner> {
    self.window.media.take().map(|media| media.owner)
  }

  /// The environment's signals with their write halves, created on first use.
  fn media_state(&mut self) -> &Media<P::Scalar> {
    let environment = self.environment();
    let painting = self.window.painting;

    self.window.media.get_or_insert_with(|| {
      let owner = Owner::new();

      owner.enter(|| {
        let (signals, writers) = MediaSignals::new(environment);
        let (painting, set_painting) = Signal::new(painting).split();

        Media {
          owner,
          signals,
          writers,
          painting,
          set_painting,
        }
      })
    })
  }

  /// Applies `change` to what the runtime keeps of its window and the app's choice of
  /// scheme, the one writer of either.
  ///
  /// Where the environment in force moved, every resolved style goes stale and the frame
  /// lays out and repaints; where only whether the window paints moved, the frame
  /// repaints. Where anything moved, the signals are created if they were not yet, and
  /// those that moved are written once the runtime is released, by the [`Pending`]
  /// returned, so what reads them runs with nothing borrowed; a platform that redefines
  /// tokens from the new scheme does so before writing it.
  ///
  /// # Panics
  ///
  /// Panics if no runtime is installed for this thread, or if the platform's state is
  /// borrowed already.
  pub fn report(change: impl FnOnce(&mut Facts<P::Scalar>)) -> Pending<P::Scalar> {
    Self::with(|runtime| {
      let mut facts = Facts {
        reported: runtime.window.reported,
        painting: runtime.window.painting,
        choice: runtime.choice,
      };
      let before = (runtime.environment(), runtime.window.painting);

      change(&mut facts);

      runtime.window.reported = facts.reported;
      runtime.window.painting = facts.painting;
      runtime.choice = facts.choice;

      let after = (runtime.environment(), runtime.window.painting);

      if before.0 != after.0 {
        runtime.invalidate_styles();
        runtime.invalidate_frames();
      }

      if before.1 != after.1 {
        runtime.paint_dirty = true;
      }

      let writers = (before != after).then(|| {
        let media = runtime.media_state();

        (media.writers, media.set_painting)
      });

      Pending {
        writers,
        before,
        after,
      }
    })
  }

  /// Publishes the sizes the platform `measured` of its named query containers, the
  /// first of each name that differs from the size its signal holds; `true` where one
  /// did, in which case every resolved style goes stale and, once the runtime is
  /// released, the container sizes signal moves.
  ///
  /// # Panics
  ///
  /// Panics if no runtime is installed for this thread, or if the platform's state is
  /// borrowed already.
  #[must_use]
  pub fn publish_container_sizes(measured: Vec<(ContainerName, Size<P::Scalar>)>) -> bool {
    let publish = Self::with(|runtime| {
      if measured.is_empty() {
        return None;
      }

      let sizes = runtime.media().container_sizes;
      let mut changed: Vec<(ContainerName, Size<P::Scalar>)> = Vec::new();

      for (name, size) in measured {
        if sizes.get_untracked(name) != Some(size) && !changed.iter().any(|(held, _)| *held == name)
        {
          changed.push((name, size));
        }
      }

      if changed.is_empty() {
        return None;
      }

      runtime.invalidate_styles();

      Some((sizes, changed))
    });
    let Some((sizes, changed)) = publish
    else {
      return false;
    };

    for (name, size) in changed {
      sizes.set(name, size);
    }

    true
  }
}
