//! Shaping as data: what shaping a paragraph takes, owned, so it shapes on any thread, and off
//! the web the threads that shape a batch of paragraphs beside the engine's own.

use std::ops::Range;
#[cfg(not(target_arch = "wasm32"))]
use std::{
  collections::VecDeque,
  sync::{Arc, Condvar, Mutex, mpsc},
  thread,
};

use parley::{
  Alignment,
  AlignmentOptions,
  BaseDirection,
  FontContext,
  InlineBox,
  Layout,
  LayoutContext,
  StyleProperty,
};
use skene_foundation::text::Intrinsic;

use crate::engine::ParleyParagraph;

/// The fewest paragraphs a batch shapes on threads beside the engine's own, off the web:
/// below it, a thread's hand-off costs more than its share saves.
pub const PARALLEL: usize = 64;

/// How many paragraphs a share of a batch holds: few enough that the threads finish close
/// together and start soon after the batch does, enough that a hand-off costs little beside
/// its shaping.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const SHARE: usize = 24;

/// What shaping a paragraph takes: its text, the properties Parley is given, the default
/// ones and those of each range, and its inline boxes.
#[derive(Clone)]
pub(crate) struct Job {
  /// The text.
  pub(crate) text: String,
  /// The properties every byte takes unless a range says otherwise.
  pub(crate) defaults: Vec<StyleProperty<'static, ()>>,
  /// The properties of each range, in order: a later one wins where two overlap.
  pub(crate) ranged: Vec<(StyleProperty<'static, ()>, Range<usize>)>,
  /// The inline boxes, in the text's order, each its place in that order as its id.
  pub(crate) boxes: Vec<InlineBox>,
  /// The base direction.
  pub(crate) direction: BaseDirection,
  /// How its lines align, where its unbounded layout is made with it: one cutting no line,
  /// which breaks at hard breaks alone.
  pub(crate) unbounded: Option<Alignment>,
}

/// A paragraph shaped, with its intrinsic widths.
pub(crate) struct Shaped {
  /// The shaped layout: its lines broken and aligned unbounded where its job asked.
  pub(crate) layout: Layout<()>,
  /// Its narrowest and widest widths.
  pub(crate) widths: Intrinsic,
  /// Whether its lines are broken and aligned unbounded.
  pub(crate) unbounded: bool,
}

impl Job {
  /// Shapes the paragraph through `fonts` and `layouts`.
  pub(crate) fn shape(&self, fonts: &mut FontContext, layouts: &mut LayoutContext<()>) -> Shaped {
    let mut builder = layouts.ranged_builder(fonts, &self.text, 1., true);

    for property in &self.defaults {
      builder.push_default(property.clone());
    }

    builder.set_base_direction(self.direction);

    for (property, range) in &self.ranged {
      builder.push(property.clone(), range.clone());
    }

    for inline in &self.boxes {
      builder.push_inline_box(inline.clone());
    }

    let mut layout = builder.build(&self.text);
    let widths = layout.calculate_content_widths();

    if let Some(alignment) = self.unbounded {
      ParleyParagraph::break_into(&mut layout, f32::MAX, &[], None);
      layout.align(alignment, AlignmentOptions::default());
    }

    Shaped {
      layout,
      widths: Intrinsic {
        min: widths.min,
        max: widths.max,
      },
      unbounded: self.unbounded.is_some(),
    }
  }
}

/// A share of a batch: its place in the batch and its jobs.
#[cfg(not(target_arch = "wasm32"))]
type Share = (usize, Vec<Job>);

/// The shares waiting to be shaped, and the signal a thread waits on for the next.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct Queue {
  shares: Mutex<VecDeque<Share>>,
  ready: Condvar,
}

#[cfg(not(target_arch = "wasm32"))]
impl Queue {
  /// Queues `share` and wakes a thread for it.
  fn push(&self, share: Share) {
    if let Ok(mut shares) = self.shares.lock() {
      shares.push_back(share);
      self.ready.notify_one();
    }
  }

  /// The next share, where one waits.
  fn take(&self) -> Option<Share> {
    self.shares.lock().ok()?.pop_front()
  }

  /// The next share, waiting for one: `None` where the queue is poisoned.
  fn wait(&self) -> Option<Share> {
    let mut shares = self.shares.lock().ok()?;

    loop {
      if let Some(share) = shares.pop_front() {
        return Some(share);
      }

      shares = self.ready.wait(shares).ok()?;
    }
  }
}

/// Threads shaping beside an engine's own, each over contexts of its own on the engine's
/// fonts (a font loaded on one is loaded on all), taking a batch's shares from one queue
/// the engine's thread drains too.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct Shapers {
  queue: Arc<Queue>,
  done: mpsc::Receiver<(usize, Vec<Shaped>)>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Shapers {
  /// Up to `count` threads over copies of `fonts`, as many as start, each keeping
  /// `shape_cache` shaped words and shaping `warm` first so the fonts and the shaping it
  /// reaches are ready before a batch: `None` where none does.
  pub(crate) fn start(
    fonts: &FontContext,
    count: usize,
    warm: &Job,
    shape_cache: usize,
  ) -> Option<Self> {
    let queue = Arc::new(Queue::default());
    let (results, done) = mpsc::channel();
    let started = (0..count)
      .map_while(|_| {
        let queue = Arc::clone(&queue);
        let results = results.clone();
        let mut fonts = fonts.clone();
        let warm = warm.clone();

        thread::Builder::new()
          .name("skene-shaper".to_owned())
          .spawn(move || {
            let mut layouts = LayoutContext::new();

            layouts.set_shape_cache_capacity(shape_cache);

            let _ = warm.shape(&mut fonts, &mut layouts);

            while let Some((index, jobs)) = queue.wait() {
              let shaped = jobs
                .iter()
                .map(|job| job.shape(&mut fonts, &mut layouts))
                .collect();

              if results.send((index, shaped)).is_err() {
                break;
              }
            }
          })
          .ok()
      })
      .count();

    (started > 0).then_some(Self { queue, done })
  }

  /// Shapes `jobs` in order, in shares of `share` jobs queued as they come, the threads and
  /// `local` on the calling thread taking them from the queue.
  ///
  /// # Panics
  ///
  /// Where a thread stopped before answering: its shaping panicked.
  pub(crate) fn shape(
    &self,
    jobs: impl IntoIterator<Item = Job>,
    share: usize,
    mut local: impl FnMut(&Job) -> Shaped,
  ) -> Vec<Shaped> {
    let mut jobs = jobs.into_iter().peekable();
    let mut queued = 0;

    while jobs.peek().is_some() {
      self
        .queue
        .push((queued, jobs.by_ref().take(share.max(1)).collect()));
      queued += 1;
    }

    let mut answered: Vec<(usize, Vec<Shaped>)> = Vec::with_capacity(queued);

    while let Some((index, jobs)) = self.queue.take() {
      answered.push((index, jobs.iter().map(&mut local).collect()));
    }

    while answered.len() < queued {
      answered.push(
        self
          .done
          .recv()
          .expect("a shaper thread stopped before answering"),
      );
    }

    answered.sort_unstable_by_key(|(index, _)| *index);

    answered
      .into_iter()
      .flat_map(|(_, shaped)| shaped)
      .collect()
  }
}
