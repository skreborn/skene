//! The environment as signals: the viewport from `resize`, every media feature from
//! one `MediaQueryList` per query text, and the scheme in force composed from the app's
//! choice and the platform's preference.

use std::cell::RefCell;

use rustc_hash::FxHashMap;
use skene_core::{ColorScheme, ContainerSizes, Media, MediaSignals, Size};
use skene_reactive::{Effect, Get, Memo, ReadSignal, Set, Signal, signal};
use web_sys::MediaQueryList;

use super::events::Listener;
use crate::condition::{Conjunction, SCHEME_ATTRIBUTE};

/// The query text of the platform's dark preference.
const DARK: &str = "(prefers-color-scheme: dark)";

/// The query text of the reduced-motion preference.
const REDUCED_MOTION: &str = "(prefers-reduced-motion: reduce)";

/// One live media query: its list, its signal and the listener keeping them equal.
struct Query {
  read: ReadSignal<bool>,
  _listener: Listener,
}

/// The environment's signals with their sources.
struct MediaState {
  signals: MediaSignals<f32>,
  choice: Signal<Option<ColorScheme>>,
  queries: FxHashMap<String, Query>,
  _resize: Listener,
}

thread_local! {
  static MEDIA: RefCell<Option<MediaState>> = const { RefCell::new(None) };
}

/// The window's inner size, in logical pixels.
fn window_size() -> Size<f32> {
  let window = web_sys::window().expect("a window");

  #[expect(clippy::cast_possible_truncation)]
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
  let (read, write) = signal(list.as_ref().is_some_and(MediaQueryList::matches));
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
  let (viewport, set_viewport) = signal(window_size());
  let (scheme, set_scheme) = signal(ColorScheme::Light);
  let (reduced_motion, set_reduced_motion) = signal(false);
  let choice = Signal::new(None);
  let resize = Listener::new(
    web_sys::window().expect("a window").into(),
    "resize",
    false,
    move |_| set_viewport.set(window_size()),
  );
  let mut state = MediaState {
    signals: MediaSignals {
      viewport,
      scheme,
      reduced_motion,
      container_sizes: ContainerSizes::new(),
    },
    choice,
    queries: FxHashMap::default(),
    _resize: resize,
  };
  let dark = query(&mut state, DARK);
  let reduced = query(&mut state, REDUCED_MOTION);

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

  state
}

/// The environment's signals.
pub(super) fn signals() -> MediaSignals<f32> {
  with_media(|state| state.signals)
}

/// Forgets the environment state, so the next use creates it afresh.
#[cfg(feature = "test")]
pub(super) fn reset() {
  MEDIA.with(|media| *media.borrow_mut() = None);
}

/// Whether the media query `text` matches, as a signal following the platform.
///
/// One `MediaQueryList` serves every reader of the same text.
pub fn media_query(text: &str) -> ReadSignal<bool> {
  with_media(|state| query(state, text))
}

/// Whether `media` holds, as a memo built from the same query text the stylesheet
/// renders, so the signal and the sheet cannot disagree. A scheme leaf follows the
/// scheme in force, the app's choice included; a container leaf never holds here.
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
/// Every scheme-conditional rule the driver compiles is gated on the root attribute, so
/// one write re-inks the whole document without touching a node. `color-scheme` rides
/// along so the user agent's own widgets, scrollbars and form controls follow the
/// choice too.
pub fn set_scheme(scheme: Option<ColorScheme>) {
  if let Some(root) = web_sys::window()
    .and_then(|window| window.document())
    .and_then(|document| document.document_element())
  {
    match scheme.map(scheme_name) {
      Some(name) => {
        let _ = root.set_attribute(SCHEME_ATTRIBUTE, name);
        let _ = root.set_attribute("style", &format!("color-scheme:{name}"));
        let _ = storage().map(|storage| storage.set_item(SCHEME_KEY, name));
      }
      None => {
        let _ = root.remove_attribute(SCHEME_ATTRIBUTE);
        let _ = root.remove_attribute("style");
        let _ = storage().map(|storage| storage.remove_item(SCHEME_KEY));
      }
    }
  }

  with_media(|state| state.choice).set(scheme);
}

/// The color scheme the reader chose on a previous visit.
///
/// Read it where an app initializes the setting it binds to `derive_scheme`, so the
/// choice the head script already applied is the one the app goes on to hold.
#[must_use]
pub fn stored_scheme() -> Option<ColorScheme> {
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
