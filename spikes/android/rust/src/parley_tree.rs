//! The same taffy tree with Parley measuring its leaves in process: option B's and C's layout,
//! timed against the JNI path into `StaticLayout` on the same tree.
//!
//! Each text is shaped once into a Parley layout kept per leaf; intrinsics come from
//! `calculate_content_widths` and a width breaks the shaped text again, as Skene's engine
//! would.

use std::{borrow::Cow, time::Instant};

use jni::{
  objects::{JClass, JFloatArray, JLongArray, JObjectArray, JString},
  sys::{jdouble, jfloat, jint, jlong},
  JNIEnv,
};
use parley::{
  FontContext,
  FontFamily,
  FontFamilyName,
  GenericFamily,
  Layout,
  LayoutContext,
  LineHeight,
  OverflowWrap,
  StyleProperty,
};
use text_parity::fonts::{FontSource, Fonts};

use crate::{Backend, Intrinsics, Laid, Tree};

/// Parley over a font collection, one shaped layout per text.
pub struct ParleyBackend {
  fonts: FontContext,
  layouts: LayoutContext<()>,
  texts: Vec<String>,
  sizes: Vec<f32>,
  shaped: Vec<Option<Layout<()>>>,
  /// How long making the font collection took.
  pub fonts_ms: f64,
}

/// `text` at `size` in the system UI family, emoji-presentation clusters asking the emoji family
/// first, as the harness's engine does.
fn shape(
  layouts: &mut LayoutContext<()>,
  fonts: &mut FontContext,
  text: &str,
  size: f32,
) -> Layout<()> {
  let system = [
    FontFamilyName::Generic(GenericFamily::SystemUi),
    FontFamilyName::Generic(GenericFamily::SansSerif),
  ];
  let emoji = [
    FontFamilyName::Generic(GenericFamily::Emoji),
    FontFamilyName::Generic(GenericFamily::SystemUi),
    FontFamilyName::Generic(GenericFamily::SansSerif),
  ];
  let mut builder = layouts.ranged_builder(fonts, text, 1., false);

  builder.push_default(StyleProperty::FontFamily(FontFamily::List(Cow::Borrowed(
    &system,
  ))));
  builder.push_default(StyleProperty::FontSize(size));
  builder.push_default(StyleProperty::LineHeight(LineHeight::MetricsRelative(1.)));
  builder.push_default(StyleProperty::OverflowWrap(OverflowWrap::BreakWord));

  for range in text_parity::emoji::emoji_ranges(text) {
    builder.push(
      StyleProperty::FontFamily(FontFamily::List(Cow::Borrowed(&emoji))),
      range,
    );
  }

  builder.build(text)
}

fn baselines(layout: &Layout<()>) -> (f32, f32) {
  let first = layout
    .lines()
    .next()
    .map_or(0., |line| line.metrics().baseline);
  let last = layout
    .lines()
    .last()
    .map_or(0., |line| line.metrics().baseline);

  (first, last)
}

impl ParleyBackend {
  /// Parley over the fonts `source` names, for `texts` at `sizes` (pixels).
  ///
  /// # Errors
  ///
  /// Where the font source fails.
  pub fn new(source: &FontSource, texts: Vec<String>, sizes: Vec<f32>) -> Result<Self, String> {
    let fonts = Fonts::new(source)?;
    let count = texts.len();

    Ok(Self {
      fonts_ms: fonts.startup_ms,
      fonts: fonts.context,
      layouts: LayoutContext::new(),
      texts,
      sizes,
      shaped: (0..count).map(|_| None).collect(),
    })
  }

  fn shaped(&mut self, text: usize) -> &mut Layout<()> {
    let Self {
      fonts,
      layouts,
      texts,
      sizes,
      shaped,
      ..
    } = self;

    shaped[text].get_or_insert_with(|| shape(layouts, fonts, &texts[text], sizes[text]))
  }

  /// Drops every shaped layout.
  pub fn clear(&mut self) {
    self.shaped.iter_mut().for_each(|layout| *layout = None);
  }

  /// Times `frames` frames of text `text` animating its size from `from` to `to` at `width`:
  /// each frame shapes and breaks it afresh. Writes each frame's nanoseconds into `out`.
  pub fn animate(
    &mut self,
    text: usize,
    (from, to): (f32, f32),
    width: f32,
    out: &mut [i64],
  ) -> f32 {
    let frames = out.len();
    let mut height = 0.;

    for (frame, slot) in out.iter_mut().enumerate() {
      let size = from + (to - from) * frame as f32 / (frames.max(2) - 1) as f32;
      let started = Instant::now();
      let mut layout = shape(&mut self.layouts, &mut self.fonts, &self.texts[text], size);

      layout.break_all_lines(Some(width));
      height = layout.height();
      *slot = started.elapsed().as_nanos() as i64;
    }

    height
  }
}

impl ParleyBackend {
  /// The intrinsics of text `text`, with the height and baselines at max-content where
  /// `with_height`.
  pub fn measure_intrinsics(&mut self, text: usize, with_height: bool) -> Intrinsics {
    let layout = self.shaped(text);
    let widths = layout.calculate_content_widths();
    let max = widths.max.ceil();
    let (h_max, base_max) = if with_height {
      layout.break_all_lines(Some(max));

      (layout.height(), baselines(layout))
    }
    else {
      (f32::NAN, (f32::NAN, f32::NAN))
    };

    Intrinsics {
      min: widths.min.ceil().min(max),
      max,
      h_max,
      base_max,
    }
  }

  /// Text `text` broken at `width`.
  pub fn measure_layout(&mut self, text: usize, width: i32) -> Laid {
    let layout = self.shaped(text);

    layout.break_all_lines(Some(width as f32));

    let used = layout.width();
    let ceiled = used.ceil();

    Laid {
      width,
      used: if ceiled <= width as f32 { ceiled } else { used },
      height: layout.height(),
      baselines: baselines(layout),
    }
  }
}

impl Backend for ParleyBackend {
  fn intrinsics(
    &mut self,
    _env: &mut JNIEnv,
    text: i32,
    _with_min: bool,
    with_height: bool,
  ) -> Intrinsics {
    self.measure_intrinsics(text as usize, with_height)
  }

  fn layout(&mut self, _env: &mut JNIEnv, text: i32, width: i32) -> Laid {
    self.measure_layout(text as usize, width)
  }
}

fn tree<'a>(handle: jlong) -> &'a mut Tree<ParleyBackend> {
  // SAFETY: the handle came from `create` and is used by one thread at a time.
  unsafe { &mut *(handle as *mut Tree<ParleyBackend>) }
}

/// Creates a tree of `cards` cards over Parley with the fonts the JSON `fonts` names, leaf `i`
/// showing `texts[i]` at `sizes[i]` pixels; 0 where the fonts fail (the reason is logged).
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_ParleyTree_create<'local>(
  mut env: JNIEnv<'local>,
  _class: JClass<'local>,
  texts: JObjectArray<'local>,
  sizes: JFloatArray<'local>,
  fonts: JString<'local>,
  cards: jint,
) -> jlong {
  let count = env.get_array_length(&texts).unwrap_or(0) as usize;
  let mut strings = Vec::with_capacity(count);

  for at in 0..count {
    let element = env.get_object_array_element(&texts, at as i32).unwrap();
    let text: String = env
      .get_string(&JString::from(element))
      .map(Into::into)
      .unwrap_or_default();

    strings.push(text);
  }

  let mut pixels = vec![0f32; count];

  env.get_float_array_region(&sizes, 0, &mut pixels).unwrap();

  let fonts: String = env.get_string(&fonts).map(Into::into).unwrap_or_default();
  let source = match serde_json::from_str::<FontSource>(&fonts) {
    Ok(source) => source,
    Err(error) => {
      eprintln!("fonts: {error}");

      return 0;
    }
  };

  match ParleyBackend::new(&source, strings, pixels) {
    Ok(backend) => Box::into_raw(Box::new(Tree::new(backend, cards as usize))) as jlong,
    Err(error) => {
      eprintln!("fonts: {error}");

      0
    }
  }
}

/// How long the tree's font collection took to make, in milliseconds.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_ParleyTree_fontsMs<'local>(
  _env: JNIEnv<'local>,
  _class: JClass<'local>,
  handle: jlong,
) -> jdouble {
  tree(handle).engine_mut().fonts_ms
}

/// Flag: drop every shaped layout before the pass, so each text is shaped again.
pub const FLAG_RESHAPE: jint = 8;

/// Runs a pass at `width` under `flags` ([`FLAG_RESHAPE`] as well), writing the pass's
/// numbers into `stats` (8 longs).
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_ParleyTree_layout<'local>(
  mut env: JNIEnv<'local>,
  _class: JClass<'local>,
  handle: jlong,
  width: jfloat,
  flags: jint,
  stats: JLongArray<'local>,
) {
  let tree = tree(handle);

  if flags & FLAG_RESHAPE != 0 {
    tree.engine_mut().clear();
  }

  let numbers = tree.layout(&mut env, width, flags);

  env.set_long_array_region(&stats, 0, &numbers).unwrap();
}

/// Writes the leaves' absolute rectangles and the root's size into `out`.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_ParleyTree_rects<'local>(
  env: JNIEnv<'local>,
  _class: JClass<'local>,
  handle: jlong,
  out: JFloatArray<'local>,
) {
  let tree = tree(handle);
  let mut numbers = vec![0.0f32; tree.leaf_count() * 4 + 2];

  tree.rects(&mut numbers);
  env.set_float_array_region(&out, 0, &numbers).unwrap();
}

/// Animates leaf `text`'s size from `from` to `to` over `out.len()` frames at `width`, each
/// frame's nanoseconds into `out`; answers the last frame's height.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_ParleyTree_animate<'local>(
  env: JNIEnv<'local>,
  _class: JClass<'local>,
  handle: jlong,
  text: jint,
  from: jfloat,
  to: jfloat,
  width: jfloat,
  out: JLongArray<'local>,
) -> jfloat {
  let frames = env.get_array_length(&out).unwrap_or(0) as usize;
  let mut nanos = vec![0i64; frames];
  let height = tree(handle)
    .engine_mut()
    .animate(text as usize, (from, to), width, &mut nanos);

  env.set_long_array_region(&out, 0, &nanos).unwrap();

  height
}

/// Frees the tree.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_ParleyTree_destroy<'local>(
  _env: JNIEnv<'local>,
  _class: JClass<'local>,
  handle: jlong,
) {
  // SAFETY: the handle came from `create` and is not used again.
  drop(unsafe { Box::from_raw(handle as *mut Tree<ParleyBackend>) });
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn measures() {
    let mut backend = ParleyBackend::new(
      &FontSource::system(),
      vec![
        "The quick brown fox jumps over the lazy dog".to_owned(),
        "".to_owned(),
      ],
      vec![40., 34.],
    )
    .expect("fonts");
    let intrinsics = backend.measure_intrinsics(0, true);

    assert!(
      intrinsics.min > 0. && intrinsics.min < intrinsics.max,
      "{}..{}",
      intrinsics.min,
      intrinsics.max
    );
    assert!(intrinsics.h_max > 30.);

    let narrow = backend.measure_layout(0, 200);

    assert!(narrow.height > intrinsics.h_max * 2., "wraps at 200");
    assert!(narrow.used <= 200.);

    let empty = backend.measure_layout(1, 100);

    assert!(empty.height > 0., "an empty text lays out one line");

    let (frames, height) = {
      let mut out = [0i64; 5];
      let height = backend.animate(0, (34., 68.), 500., &mut out);

      (out, height)
    };

    assert!(frames.iter().all(|nanos| *nanos > 0) && height > 0.);
  }
}
