//! The environment as signals: the viewport from `resize`, every media feature from
//! one `MediaQueryList` per query text, and the scheme in force composed from the app's
//! choice and the platform's preference.

use std::cell::{Cell, RefCell};

use rustc_hash::FxHashMap;
use skene_core::{
  ColorScheme,
  Environment,
  Inset,
  Media,
  MediaSignals,
  MediaWriters,
  PointerPrecision,
  Size,
};
use skene_reactive::{Effect, Get, Memo, ReadSignal, Set, Signal, WriteSignal};
use wasm_bindgen::JsCast as _;
use web_sys::MediaQueryList;

use super::events::Listener;
use crate::condition::{Conjunction, SCHEME_ATTRIBUTE};

/// The query text of the platform's dark preference.
const DARK: &str = "(prefers-color-scheme: dark)";

/// The query text of the reduced-motion preference.
const REDUCED_MOTION: &str = "(prefers-reduced-motion: reduce)";

/// The query text of the reduced-transparency preference.
const REDUCED_TRANSPARENCY: &str = "(prefers-reduced-transparency: reduce)";

/// The query text of the preference for more contrast.
const HIGH_CONTRAST: &str = "(prefers-contrast: more)";

/// The query text of a fine primary pointer.
const FINE_POINTER: &str = "(pointer: fine)";

/// The query text of a coarse primary pointer.
const COARSE_POINTER: &str = "(pointer: coarse)";

/// The query text of a primary pointer that hovers.
const HOVER: &str = "(hover: hover)";

/// One live media query: its list, its signal and the listener keeping them equal.
struct Query {
  read: ReadSignal<bool>,
  _listener: Listener,
}

/// The environment's signals with their sources.
struct MediaState {
  /// The environment's signals.
  signals: MediaSignals<f32>,
  /// Whether the bundle runs the tree: false while a hydration adopts the server's
  /// markup, true once it has finished and on a mount that hydrates nothing.
  live: (ReadSignal<bool>, WriteSignal<bool>),
  /// The color scheme the app has chosen; `None` follows the platform's.
  choice: Signal<Option<ColorScheme>>,
  /// The live media queries, by their text.
  queries: FxHashMap<String, Query>,
  /// The window's resize listener, writing the viewport and the safe area.
  _resize: Listener,
}

/// The insets the display keeps clear of the viewport's edges, as a hidden probe padded
/// by them measures them.
fn safe_area() -> Inset<f32> {
  let Some(document) = web_sys::window().and_then(|window| window.document())
  else {
    return Inset::ZERO;
  };
  let Some(probe) = document.get_element_by_id(SAFE_AREA_PROBE).or_else(|| {
    let probe = document.create_element("div").ok()?;

    probe.set_id(SAFE_AREA_PROBE);
    let _ = probe.set_attribute(
      "style",
      "position:fixed;visibility:hidden;pointer-events:none;\
         padding:env(safe-area-inset-top,0px) env(safe-area-inset-right,0px) \
         env(safe-area-inset-bottom,0px) env(safe-area-inset-left,0px)",
    );
    let _ = probe.set_attribute("aria-hidden", "true");
    let _ = document.document_element()?.append_child(&probe);

    Some(probe)
  })
  else {
    return Inset::ZERO;
  };
  let Some(style) = web_sys::window()
    .and_then(|window| window.get_computed_style(&probe).ok())
    .flatten()
  else {
    return Inset::ZERO;
  };
  let edge = |property: &str| {
    style
      .get_property_value(property)
      .ok()
      .and_then(|value| value.strip_suffix("px")?.parse::<f32>().ok())
      .unwrap_or(0.)
  };

  Inset {
    top: edge("padding-top"),
    right: edge("padding-right"),
    bottom: edge("padding-bottom"),
    left: edge("padding-left"),
  }
}

/// The id of the probe measuring the safe area.
const SAFE_AREA_PROBE: &str = "skene-safe-area";

thread_local! {
  static MEDIA: RefCell<Option<MediaState>> = const { RefCell::new(None) };
  /// Whether the bundle runs the tree, for the state to start from when created.
  static LIVE: Cell<bool> = const { Cell::new(true) };
}

/// The window's inner size, in logical pixels.
fn window_size() -> Size<f32> {
  let window = web_sys::window().expect("a window");

  #[expect(
    clippy::cast_possible_truncation,
    reason = "a viewport length in CSS pixels is precise enough in f32"
  )]
  let length = |value: Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>| {
    value.ok().and_then(|value| value.as_f64()).unwrap_or(0.) as f32
  };

  Size {
    width: length(window.inner_width()),
    height: length(window.inner_height()),
  }
}

/// The signal answering the media query `text`, created on first use.
fn query(state: &mut MediaState, text: &str) -> ReadSignal<bool> {
  if let Some(query) = state.queries.get(text) {
    return query.read;
  }

  let list: Option<MediaQueryList> = web_sys::window()
    .and_then(|window| window.match_media(text).ok())
    .flatten();
  let (read, write) = Signal::transient(list.as_ref().is_some_and(MediaQueryList::matches)).split();
  let listener = match list {
    Some(list) => {
      let source = list.clone();

      Listener::new(list.into(), "change", false, move |_| {
        write.set(source.matches());
      })
    }
    None => Listener::none(),
  };

  state.queries.insert(
    text.to_owned(),
    Query {
      read,
      _listener: listener,
    },
  );

  read
}

/// Runs `operation` on the environment state, creating it under the root owner on
/// first use.
fn with_media<R>(operation: impl FnOnce(&mut MediaState) -> R) -> R {
  MEDIA.with(|media| {
    let mut media = media.borrow_mut();

    if media.is_none() {
      *media = Some(skene_core::root_owner().enter(create));
    }

    operation(
      media
        .as_mut()
        .expect("the environment state was just created"),
    )
  })
}

/// The environment state: its signals, and the effects keeping the composed ones current.
fn create() -> MediaState {
  let (signals, writers) = MediaSignals::new(Environment {
    viewport: window_size(),
    scheme: ColorScheme::Light,
    reduced_motion: false,
    reduced_transparency: false,
    high_contrast: false,
    pointer: PointerPrecision::Fine,
    hover: true,
    safe_area: safe_area(),
  });
  let MediaWriters {
    viewport: set_viewport,
    scheme: set_scheme,
    reduced_motion: set_reduced_motion,
    reduced_transparency: set_reduced_transparency,
    high_contrast: set_high_contrast,
    pointer: set_pointer,
    hover: set_hover,
    safe_area: set_safe_area,
  } = writers;
  let choice = Signal::transient(None);
  let resize = Listener::new(
    web_sys::window().expect("a window").into(),
    "resize",
    false,
    move |_| {
      set_viewport.set(window_size());
      set_safe_area.set(self::safe_area());
    },
  );
  let mut state = MediaState {
    signals,
    live: Signal::transient(LIVE.with(Cell::get)).split(),
    choice,
    queries: FxHashMap::default(),
    _resize: resize,
  };
  let dark = query(&mut state, DARK);
  let reduced = query(&mut state, REDUCED_MOTION);
  let transparency = query(&mut state, REDUCED_TRANSPARENCY);
  let contrast = query(&mut state, HIGH_CONTRAST);
  let fine = query(&mut state, FINE_POINTER);
  let coarse = query(&mut state, COARSE_POINTER);
  let hovers = query(&mut state, HOVER);

  Effect::new(move |_: Option<()>| {
    let platform = if dark.get() {
      ColorScheme::Dark
    }
    else {
      ColorScheme::Light
    };

    set_scheme.set(choice.get().unwrap_or(platform));
  });
  Effect::new(move |_: Option<()>| set_reduced_motion.set(reduced.get()));
  Effect::new(move |_: Option<()>| set_reduced_transparency.set(transparency.get()));
  Effect::new(move |_: Option<()>| set_high_contrast.set(contrast.get()));
  Effect::new(move |_: Option<()>| {
    set_pointer.set(if fine.get() {
      PointerPrecision::Fine
    }
    else if coarse.get() {
      PointerPrecision::Coarse
    }
    else {
      PointerPrecision::Absent
    });
  });
  Effect::new(move |_: Option<()>| set_hover.set(hovers.get()));

  state
}

/// The environment's signals.
pub(super) fn signals() -> MediaSignals<f32> {
  with_media(|state| state.signals)
}

/// Whether the bundle runs the tree, as a signal: what hydration and the pixels wait on.
pub(super) fn live() -> ReadSignal<bool> {
  with_media(|state| state.live.0)
}

/// Records whether the bundle runs the tree: false while a hydration adopts, true
/// once it has finished or when nothing is adopted.
pub(super) fn set_live(live: bool) {
  LIVE.with(|current| current.set(live));
  MEDIA.with(|media| {
    if let Some(state) = &*media.borrow() {
      state.live.1.set(live);
    }
  });
}

/// Forgets the environment state, so the next use creates it afresh.
pub(super) fn reset() {
  MEDIA.with(|media| *media.borrow_mut() = None);
  LIVE.with(|live| live.set(true));
}

/// Whether the media query `text` matches, as a signal following the platform.
///
/// One `MediaQueryList` serves every reader of the same text.
pub fn media_query(text: &str) -> ReadSignal<bool> {
  with_media(|state| query(state, text))
}

/// Whether `media` holds, as a memo built from the same query text the stylesheet
/// renders.
///
/// The signal and the sheet cannot disagree. A scheme leaf follows the scheme in force,
/// the app's choice included; a container leaf never holds here.
pub fn watch_media(media: &Media<f32>) -> Memo<bool> {
  let scheme = signals().scheme;
  let terms: Vec<(Option<ReadSignal<bool>>, Option<ColorScheme>, bool)> =
    Conjunction::of_media(media)
      .iter()
      .map(|conjunction| {
        let (text, scheme, containers) = conjunction.query();

        (text.map(|text| media_query(&text)), scheme, containers)
      })
      .collect();

  Memo::new(move |_| {
    terms.iter().any(|(query, named, containers)| {
      !containers
        && query.is_none_or(|query| query.get())
        && named.is_none_or(|named| scheme.get() == named)
    })
  })
}

/// Writes the app's chosen color scheme onto the document root, stores it for the
/// next visit and makes it the scheme in force; `None` returns to following the
/// platform.
///
/// Every scheme-conditional rule the platform compiles is gated on the root attribute, so
/// one write re-inks the whole document without touching a node. `color-scheme` rides
/// along so the user agent's own widgets, scrollbars and form controls follow the
/// choice too.
pub fn set_scheme(scheme: Option<ColorScheme>) {
  if let Some(root) = web_sys::window()
    .and_then(|window| window.document())
    .and_then(|document| document.document_element())
  {
    let style = root
      .dyn_ref::<web_sys::HtmlElement>()
      .map(web_sys::HtmlElement::style);

    if let Some(name) = scheme.map(scheme_name) {
      let _ = root.set_attribute(SCHEME_ATTRIBUTE, name);
      let _ = style.map(|style| style.set_property("color-scheme", name));
      let _ = storage().map(|storage| storage.set_item(SCHEME_KEY, name));
    }
    else {
      let _ = root.remove_attribute(SCHEME_ATTRIBUTE);
      let _ = style.map(|style| style.remove_property("color-scheme"));
      let _ = storage().map(|storage| storage.remove_item(SCHEME_KEY));
    }
  }

  with_media(|state| state.choice).set(scheme);
}

/// The color scheme the app chose this run, else the one a previous visit persisted.
pub(crate) fn chosen_scheme() -> Option<ColorScheme> {
  with_media(|state| state.choice)
    .get_untracked()
    .or_else(persisted_scheme)
}

/// The color scheme a previous visit persisted, as the head script applies it before the
/// first paint.
#[must_use]
pub fn persisted_scheme() -> Option<ColorScheme> {
  match storage()?.get_item(SCHEME_KEY).ok().flatten()?.as_str() {
    "light" => Some(ColorScheme::Light),
    "dark" => Some(ColorScheme::Dark),
    _ => None,
  }
}

/// That scheme's name, as the attribute and the store spell it.
const fn scheme_name(scheme: ColorScheme) -> &'static str {
  match scheme {
    ColorScheme::Light => "light",
    ColorScheme::Dark => "dark",
  }
}

/// The key a chosen scheme is stored under, which the server's head script reads.
const SCHEME_KEY: &str = "skene-scheme";

/// The reader's local storage, when the browser grants it; a page in a partitioned or
/// private context may not.
fn storage() -> Option<web_sys::Storage> {
  web_sys::window()?.local_storage().ok().flatten()
}
