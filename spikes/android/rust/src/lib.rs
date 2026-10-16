//! Android text-measure spike: a taffy tree of text leaves whose measure functions cross JNI into
//! a Kotlin `StaticLayout`/`BoringLayout` engine that answers with numbers only.
//!
//! The leaf follows the memo's corrected taffy leaf: min-content is the engine's intrinsic
//! minimum, max-content its intrinsic maximum, a definite width lays out at that width and
//! reports the used width (never clamped up to min-content), and a known width lays out at it.
//!
//! Caching is the foundation's policy from the memo: intrinsics once per content epoch, and a
//! small per-leaf cache of laid results by width (three entries). The Kotlin side keeps its own
//! cache of layouts keyed by `(text id, width)`, which serves whenever the Rust side misses.

use std::time::Instant;

use jni::{
  objects::{GlobalRef, JByteBuffer, JClass, JFloatArray, JLongArray, JMethodID, JObject},
  signature::{Primitive, ReturnType},
  sys::{jfloat, jint, jlong, jvalue},
  JNIEnv,
};
use taffy::{compute_leaf_layout, prelude::*, Baselines, LayoutInput, LayoutOutput, RequestedAxis};

pub mod advances;
mod ndk;
pub mod parity;
pub mod parley_tree;

/// Leaves per card; the Kotlin corpus and the Compose tree use the same shape.
pub const LEAVES_PER_CARD: usize = 10;
/// Gap between cards in the root column, in pixels.
pub const ROOT_GAP: f32 = 12.0;
/// A card's padding, in pixels.
pub const CARD_PAD: f32 = 16.0;
/// Gap between a card's rows, in pixels.
pub const CARD_GAP: f32 = 8.0;
/// Gap between the items of a row, in pixels.
pub const ROW_GAP: f32 = 8.0;
/// Width of the fixed-width label, in pixels.
pub const FIXED: f32 = 240.0;
/// Horizontal gap between wrapping tags, in pixels.
pub const TAG_GAP_H: f32 = 12.0;
/// Vertical gap between wrapped tag lines, in pixels.
pub const TAG_GAP_V: f32 = 8.0;

/// Flag: mark every leaf dirty in taffy before the pass.
pub const FLAG_DIRTY: jint = 1;
/// Flag: drop every leaf's Rust-side laid cache before the pass.
pub const FLAG_CLEAR_LAID: jint = 2;
/// Flag: drop every leaf's Rust-side intrinsics before the pass.
pub const FLAG_CLEAR_INTRINSICS: jint = 4;

#[derive(Clone, Copy)]
pub struct Intrinsics {
  // The min-content width, NaN until a min-content query asks for it.
  pub min: f32,
  pub max: f32,
  // The height and baselines at the max-content width, NaN until a query needs a height there.
  pub h_max: f32,
  pub base_max: (f32, f32),
}

#[derive(Clone, Copy)]
pub struct Laid {
  pub width: i32,
  pub used: f32,
  pub height: f32,
  pub baselines: (f32, f32),
}

/// What measures a leaf's text: the Kotlin engine through JNI, or Parley in process.
pub trait Backend {
  /// The intrinsics of text `text`: the min-content width where `with_min`, the height and
  /// baselines at max-content where `with_height` (NaN otherwise).
  fn intrinsics(
    &mut self,
    env: &mut JNIEnv,
    text: i32,
    with_min: bool,
    with_height: bool,
  ) -> Intrinsics;

  /// Text `text` laid out at `width` whole pixels.
  fn layout(&mut self, env: &mut JNIEnv, text: i32, width: i32) -> Laid;
}

struct Leaf {
  text: i32,
  intrinsics: Option<Intrinsics>,
  laid: [Option<Laid>; 3],
  next: usize,
  crossings: u32,
}

/// The engine handle and the method ids, valid on any attached thread.
struct Engine {
  object: GlobalRef,
  intrinsics: JMethodID,
  layout: JMethodID,
  buffer: *const f32,
}

#[derive(Default)]
struct PassStats {
  crossings: u64,
  intrinsics_calls: u64,
  layout_calls: u64,
  measure_calls: u64,
}

pub struct Tree<B> {
  taffy: TaffyTree<usize>,
  root: NodeId,
  leaves: Vec<Leaf>,
  leaf_nodes: Vec<NodeId>,
  engine: B,
  width: f32,
}

impl Engine {
  fn read(&self, n: usize) -> [f32; 8] {
    let mut out = [0.0; 8];

    // SAFETY: the Kotlin side allocated at least 8 floats in the direct buffer, kept alive by
    // `object`, and wrote them before the call returned on this thread.
    unsafe {
      std::ptr::copy_nonoverlapping(self.buffer, out.as_mut_ptr(), n);
    }

    out
  }
}

impl Backend for Engine {
  fn intrinsics(
    &mut self,
    env: &mut JNIEnv,
    text: i32,
    with_min: bool,
    with_height: bool,
  ) -> Intrinsics {
    // SAFETY: the method id belongs to the engine's class with signature (IZZ)V.
    unsafe {
      env
        .call_method_unchecked(
          &self.object,
          self.intrinsics,
          ReturnType::Primitive(Primitive::Void),
          &[
            jvalue { i: text },
            jvalue { z: with_min as u8 },
            jvalue {
              z: with_height as u8,
            },
          ],
        )
        .expect("intrinsics");
    }

    let b = self.read(5);

    Intrinsics {
      min: b[0],
      max: b[1],
      h_max: b[2],
      base_max: (b[3], b[4]),
    }
  }

  fn layout(&mut self, env: &mut JNIEnv, text: i32, width: i32) -> Laid {
    // SAFETY: the method id belongs to the engine's class with signature (II)V.
    unsafe {
      env
        .call_method_unchecked(
          &self.object,
          self.layout,
          ReturnType::Primitive(Primitive::Void),
          &[jvalue { i: text }, jvalue { i: width }],
        )
        .expect("layout");
    }

    let b = self.read(4);

    Laid {
      width,
      used: b[0],
      height: b[1],
      baselines: (b[2], b[3]),
    }
  }
}

impl Leaf {
  /// The leaf's intrinsics, crossing JNI when they are missing or lack a min-content width or a
  /// max-content height that is wanted.
  fn intrinsics<B: Backend>(
    &mut self,
    env: &mut JNIEnv,
    engine: &mut B,
    stats: &mut PassStats,
    with_min: bool,
    with_height: bool,
  ) -> Intrinsics {
    if let Some(intrinsics) = self.intrinsics {
      if (!with_min || !intrinsics.min.is_nan()) && (!with_height || !intrinsics.h_max.is_nan()) {
        return intrinsics;
      }
    }

    stats.crossings += 1;
    stats.intrinsics_calls += 1;
    self.crossings += 1;

    let intrinsics = engine.intrinsics(env, self.text, with_min, with_height);
    self.intrinsics = Some(intrinsics);
    intrinsics
  }

  /// Measures the leaf under taffy's query, crossing JNI only on a cache miss.
  ///
  /// Intrinsics are fetched only for a min-content or max-content query, the min-content width
  /// only for a min-content one, and a height only when the query asks for one; a definite or
  /// known width lays out directly.
  fn measure<B: Backend>(
    &mut self,
    env: &mut JNIEnv,
    engine: &mut B,
    stats: &mut PassStats,
    axis: RequestedAxis,
    known: Size<Option<f32>>,
    available: Size<AvailableSpace>,
  ) -> (Size<f32>, (f32, f32)) {
    stats.measure_calls += 1;

    let height = !matches!(axis, RequestedAxis::Horizontal);

    let (width, fixed) = match (known.width, available.width) {
      (Some(width), _) => (width, true),
      (None, AvailableSpace::Definite(width)) => (width, false),
      (None, AvailableSpace::MinContent) => {
        let min = self.intrinsics(env, engine, stats, true, false).min;

        if !height {
          return (
            Size {
              width: min,
              height: 0.0,
            },
            (0.0, 0.0),
          );
        }

        (min, false)
      }
      (None, AvailableSpace::MaxContent) => {
        let intrinsics = self.intrinsics(env, engine, stats, false, height);

        return (
          Size {
            width: intrinsics.max,
            height: if height { intrinsics.h_max } else { 0.0 },
          },
          intrinsics.base_max,
        );
      }
    };

    let whole = (width + 0.001).floor() as i32;

    // A width at or past max-content lays out as max-content does: fetch its height there once.
    if let Some(intrinsics) = self.intrinsics {
      if height && whole as f32 >= intrinsics.max && intrinsics.h_max.is_nan() {
        self.intrinsics(env, engine, stats, false, true);
      }
    }

    let (used, height, baselines) = match self.intrinsics {
      Some(intrinsics)
        if whole as f32 >= intrinsics.max && (!height || !intrinsics.h_max.is_nan()) =>
      {
        let h_max = if intrinsics.h_max.is_nan() {
          0.0
        }
        else {
          intrinsics.h_max
        };
        (intrinsics.max, h_max, intrinsics.base_max)
      }
      _ => {
        let laid = match self.laid.iter().flatten().find(|laid| laid.width == whole) {
          Some(laid) => *laid,
          None => {
            stats.crossings += 1;
            stats.layout_calls += 1;
            self.crossings += 1;

            let laid = engine.layout(env, self.text, whole);
            self.laid[self.next] = Some(laid);
            self.next = (self.next + 1) % self.laid.len();
            laid
          }
        };

        (laid.used, laid.height, laid.baselines)
      }
    };

    let width = if fixed { width } else { used };

    (Size { width, height }, baselines)
  }
}

impl<B: Backend> Tree<B> {
  /// A tree of `cards` cards whose leaves measure through `engine`.
  pub fn new(engine: B, cards: usize) -> Self {
    let mut taffy: TaffyTree<usize> = TaffyTree::new();
    taffy.disable_rounding();
    let mut leaves = Vec::with_capacity(cards * LEAVES_PER_CARD);
    let mut leaf_nodes = Vec::with_capacity(cards * LEAVES_PER_CARD);

    let mut leaf = |taffy: &mut TaffyTree<usize>, style: Style| {
      let index = leaves.len();

      leaves.push(Leaf {
        text: index as i32,
        intrinsics: None,
        laid: [None; 3],
        next: 0,
        crossings: 0,
      });

      let node = taffy.new_leaf_with_context(style, index).unwrap();
      leaf_nodes.push(node);
      node
    };

    let weighted = Style {
      flex_grow: 1.0,
      flex_shrink: 1.0,
      flex_basis: Dimension::length(0.0),
      min_size: Size {
        width: LengthPercentageAuto::length(0.0),
        height: LengthPercentageAuto::auto(),
      },
      ..Style::default()
    };

    let row = Style {
      flex_direction: FlexDirection::Row,
      gap: Size {
        width: LengthPercentage::length(ROW_GAP),
        height: LengthPercentage::length(0.0),
      },
      align_items: Some(AlignItems::FLEX_START),
      ..Style::default()
    };

    let mut cards_nodes = Vec::with_capacity(cards);

    for _ in 0..cards {
      let paragraph = leaf(&mut taffy, Style::default());
      let a = leaf(&mut taffy, weighted.clone());
      let b = leaf(&mut taffy, weighted.clone());
      let c = leaf(&mut taffy, weighted.clone());
      let three = taffy.new_with_children(row.clone(), &[a, b, c]).unwrap();

      let fixed = leaf(
        &mut taffy,
        Style {
          size: Size {
            width: Dimension::length(FIXED),
            height: Dimension::auto(),
          },
          flex_shrink: 0.0,
          ..Style::default()
        },
      );

      let rest = leaf(&mut taffy, weighted.clone());
      let pair = taffy
        .new_with_children(row.clone(), &[fixed, rest])
        .unwrap();

      let tags: Vec<NodeId> = (0..4).map(|_| leaf(&mut taffy, Style::default())).collect();

      let wrap = taffy
        .new_with_children(
          Style {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            gap: Size {
              width: LengthPercentage::length(TAG_GAP_H),
              height: LengthPercentage::length(TAG_GAP_V),
            },
            align_items: Some(AlignItems::FLEX_START),
            ..Style::default()
          },
          &tags,
        )
        .unwrap();

      let card = taffy
        .new_with_children(
          Style {
            flex_direction: FlexDirection::Column,
            padding: Rect {
              left: LengthPercentage::length(CARD_PAD),
              right: LengthPercentage::length(CARD_PAD),
              top: LengthPercentage::length(CARD_PAD),
              bottom: LengthPercentage::length(CARD_PAD),
            },
            gap: Size {
              width: LengthPercentage::length(0.0),
              height: LengthPercentage::length(CARD_GAP),
            },
            ..Style::default()
          },
          &[paragraph, three, pair, wrap],
        )
        .unwrap();

      cards_nodes.push(card);
    }

    let root = taffy
      .new_with_children(Self::root_style(0.0), &cards_nodes)
      .unwrap();

    Self {
      taffy,
      root,
      leaves,
      leaf_nodes,
      engine,
      width: 0.0,
    }
  }

  fn root_style(width: f32) -> Style {
    Style {
      flex_direction: FlexDirection::Column,
      size: Size {
        width: Dimension::length(width),
        height: Dimension::auto(),
      },
      gap: Size {
        width: LengthPercentage::length(0.0),
        height: LengthPercentage::length(ROOT_GAP),
      },
      ..Style::default()
    }
  }

  /// Runs one layout pass and returns `[nanos, crossings, intrinsics calls, layout calls, the
  /// most crossings of any leaf, leaves crossing more than once, measure callbacks, leaves
  /// crossing at all]`.
  pub fn layout(&mut self, env: &mut JNIEnv, width: f32, flags: jint) -> [i64; 8] {
    let start = Instant::now();

    if width != self.width {
      self
        .taffy
        .set_style(self.root, Self::root_style(width))
        .unwrap();
      self.width = width;
    }

    for (leaf, node) in self.leaves.iter_mut().zip(&self.leaf_nodes) {
      leaf.crossings = 0;

      if flags & FLAG_CLEAR_LAID != 0 {
        leaf.laid = [None; 3];
      }

      if flags & FLAG_CLEAR_INTRINSICS != 0 {
        leaf.intrinsics = None;
      }

      if flags & FLAG_DIRTY != 0 {
        self.taffy.mark_dirty(*node).unwrap();
      }
    }

    let mut stats = PassStats::default();
    let Self {
      taffy,
      leaves,
      engine,
      root,
      ..
    } = self;

    taffy
      .compute_layout_with_measure(
        *root,
        Size {
          width: AvailableSpace::Definite(width),
          height: AvailableSpace::MaxContent,
        },
        |inputs: LayoutInput, _node, context: Option<&mut usize>, style: &Style| {
          let Some(&mut index) = context
          else {
            return LayoutOutput::HIDDEN;
          };

          let leaf = &mut leaves[index];
          let axis = inputs.axis;
          let mut baselines = (0.0, 0.0);

          let mut out = compute_leaf_layout(
            inputs,
            style,
            |_, _| 0.0,
            |known, available| {
              let (size, b) = leaf.measure(env, &mut *engine, &mut stats, axis, known, available);
              baselines = b;
              size
            },
          );

          out.baselines = Baselines {
            first: Some(baselines.0),
            last: Some(baselines.1),
          };

          out
        },
      )
      .unwrap();

    let nanos = start.elapsed().as_nanos() as i64;
    let most = self
      .leaves
      .iter()
      .map(|leaf| leaf.crossings)
      .max()
      .unwrap_or(0);
    let over = self.leaves.iter().filter(|leaf| leaf.crossings > 1).count();
    let crossed = self.leaves.iter().filter(|leaf| leaf.crossings > 0).count();

    [
      nanos,
      stats.crossings as i64,
      stats.intrinsics_calls as i64,
      stats.layout_calls as i64,
      most as i64,
      over as i64,
      stats.measure_calls as i64,
      crossed as i64,
    ]
  }

  /// The number of leaves.
  pub fn leaf_count(&self) -> usize {
    self.leaf_nodes.len()
  }

  /// The backend.
  pub fn engine_mut(&mut self) -> &mut B {
    &mut self.engine
  }

  /// Writes each leaf's absolute `x, y, width, height`, then the root's width and height.
  pub fn rects(&self, out: &mut [f32]) {
    for (index, node) in self.leaf_nodes.iter().enumerate() {
      let mut x = 0.0;
      let mut y = 0.0;
      let mut cursor = Some(*node);

      while let Some(current) = cursor {
        let layout = self.taffy.layout(current).unwrap();
        x += layout.location.x;
        y += layout.location.y;
        cursor = self.taffy.parent(current);
      }

      let size = self.taffy.layout(*node).unwrap().size;
      out[index * 4] = x;
      out[index * 4 + 1] = y;
      out[index * 4 + 2] = size.width;
      out[index * 4 + 3] = size.height;
    }

    let root = self.taffy.layout(self.root).unwrap().size;
    let n = self.leaf_nodes.len() * 4;
    out[n] = root.width;
    out[n + 1] = root.height;
  }
}

/// Creates a tree of `cards` cards over `engine`, whose direct `buffer` receives its answers.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_NativeTree_create<'local>(
  mut env: JNIEnv<'local>,
  _class: JClass<'local>,
  engine: JObject<'local>,
  buffer: JByteBuffer<'local>,
  cards: jint,
) -> jlong {
  let class = env.get_object_class(&engine).unwrap();
  let intrinsics = env.get_method_id(&class, "intrinsics", "(IZZ)V").unwrap();
  let layout = env.get_method_id(&class, "layout", "(II)V").unwrap();
  let address = env.get_direct_buffer_address(&buffer).unwrap();
  let object = env.new_global_ref(&engine).unwrap();

  let engine = Engine {
    object,
    intrinsics,
    layout,
    buffer: address as *const f32,
  };

  Box::into_raw(Box::new(Tree::new(engine, cards as usize))) as jlong
}

/// Runs a pass at `width` under `flags`, writing the pass's numbers into `stats` (8 longs).
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_NativeTree_layout<'local>(
  mut env: JNIEnv<'local>,
  _class: JClass<'local>,
  handle: jlong,
  width: jfloat,
  flags: jint,
  stats: JLongArray<'local>,
) {
  // SAFETY: the handle came from `create` and is used by one thread at a time.
  let tree = unsafe { &mut *(handle as *mut Tree<Engine>) };
  let numbers = tree.layout(&mut env, width, flags);
  env.set_long_array_region(&stats, 0, &numbers).unwrap();
}

/// Writes the leaves' absolute rectangles and the root's size into `out`.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_NativeTree_rects<'local>(
  env: JNIEnv<'local>,
  _class: JClass<'local>,
  handle: jlong,
  out: JFloatArray<'local>,
) {
  // SAFETY: as in `layout`.
  let tree = unsafe { &*(handle as *const Tree<Engine>) };
  let mut numbers = vec![0.0f32; tree.leaf_nodes.len() * 4 + 2];
  tree.rects(&mut numbers);
  env.set_float_array_region(&out, 0, &numbers).unwrap();
}

/// Frees the tree.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_NativeTree_destroy<'local>(
  _env: JNIEnv<'local>,
  _class: JClass<'local>,
  handle: jlong,
) {
  // SAFETY: the handle came from `create` and is not used again.
  drop(unsafe { Box::from_raw(handle as *mut Tree<Engine>) });
}

/// Times `count` JNI upcalls into `engine` and returns the nanoseconds they took: `kind` 0 calls
/// `noop()`, 1 calls `layout(0, 500)`, a cache hit once warm.
#[no_mangle]
pub extern "system" fn Java_dev_skene_spike_text_NativeTree_upcalls<'local>(
  mut env: JNIEnv<'local>,
  _class: JClass<'local>,
  engine: JObject<'local>,
  kind: jint,
  count: jint,
) -> jlong {
  let class = env.get_object_class(&engine).unwrap();
  let noop = env.get_method_id(&class, "noop", "()V").unwrap();
  let layout = env.get_method_id(&class, "layout", "(II)V").unwrap();
  let start = Instant::now();

  for _ in 0..count {
    // SAFETY: both method ids belong to the engine's class with the signatures named above.
    unsafe {
      if kind == 0 {
        env
          .call_method_unchecked(&engine, noop, ReturnType::Primitive(Primitive::Void), &[])
          .unwrap();
      }
      else {
        env
          .call_method_unchecked(
            &engine,
            layout,
            ReturnType::Primitive(Primitive::Void),
            &[jvalue { i: 0 }, jvalue { i: 500 }],
          )
          .unwrap();
      }
    }
  }

  start.elapsed().as_nanos() as jlong
}
