//! The launch configuration: what the app takes from the terminal.

use skene_common::{ContextMenu, MenuItem, typography};
use skene_core::{Border, Color, Inset, IntoView, Media, State, When};

use crate::{Tui, terminal, with_store};

/// The clipboard operations offered: `Copy` and `Cut` write the clipboard through
/// OSC 52, `Paste` reads it through OSC 52. A terminal without OSC 52 discards the
/// write and answers no read; the terminal's own paste key inserts regardless.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Clipboard {
  /// Whether `Copy` and `Cut` (`Ctrl-C`, `Ctrl-X`, the menus) write the selection. Off,
  /// no text outside an entry is selectable and the copy menu never opens.
  pub copy: bool,
  /// Whether `Paste` (`Ctrl-V`, the edit menu) reads the clipboard.
  pub paste: bool,
}

impl Clipboard {
  /// Copy, cut and paste.
  pub const ALL: Self = Self {
    copy: true,
    paste: true,
  };
  /// Copy and cut only; the terminal's own paste key still inserts.
  pub const COPY: Self = Self {
    copy: true,
    paste: false,
  };
  /// No clipboard operation; the terminal's own paste key still inserts.
  pub const NONE: Self = Self {
    copy: false,
    paste: false,
  };
  /// Paste only.
  pub const PASTE: Self = Self {
    copy: false,
    paste: true,
  };
}

impl Default for Clipboard {
  #[inline(always)]
  fn default() -> Self {
    Self::ALL
  }
}

/// The terminal platform's launch configuration: the clipboard operations offered,
/// whether the mouse is taken, and the key handing it to the terminal at run time.
///
/// `Launch::new().set_clipboard(Clipboard::COPY).run(app)` runs the app in the terminal;
/// [`launch`] is the default configuration's. The terminal's own selection—a drag with
/// `Shift` held on most terminals, `Option` on iTerm2—never reaches the app and stays
/// available under every configuration; with the mouse off, a plain drag selects
/// natively too.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Launch {
  clipboard: Clipboard,
  mouse: bool,
  mouse_key: Option<&'static str>,
}

/// The key handing the mouse to the terminal by default.
pub const MOUSE_KEY: &str = "Alt+m";

impl Default for Launch {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Launch {
  /// The default configuration: every clipboard operation, the mouse taken, and
  /// [`MOUSE_KEY`] handing it over.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      clipboard: Clipboard::ALL,
      mouse: true,
      mouse_key: Some(MOUSE_KEY),
    }
  }

  /// Sets the clipboard operations offered.
  #[inline(always)]
  pub const fn set_clipboard(mut self, clipboard: Clipboard) -> Self {
    self.clipboard = clipboard;

    self
  }

  /// Sets whether the app takes the mouse: off, the terminal reports no mouse event,
  /// so hover, clicks, drags, in-app selection and wheel scrolling are gone, and a
  /// plain drag is the terminal's own selection.
  #[inline(always)]
  pub const fn set_mouse(mut self, mouse: bool) -> Self {
    self.mouse = mouse;

    self
  }

  /// Sets the key handing the mouse to the terminal for its own selection and taking
  /// it back, in the form [`trigger_key`](crate::trigger_key) reads (`"Alt+m"`,
  /// `"F9"`); any other key takes the mouse back as well, and then acts as usual.
  /// `None` leaves the mouse where the launch put it.
  #[inline(always)]
  pub const fn set_mouse_key(mut self, key: Option<&'static str>) -> Self {
    self.mouse_key = key;

    self
  }

  /// Installs the terminal platform under this configuration, for a headless test; see
  /// [`install`].
  ///
  /// # Panics
  ///
  /// Panics if the platform is already installed for this thread.
  pub fn install(self) {
    crate::platform::install();

    with_store(|store| {
      store.clipboard = self.clipboard;
      store.mouse = self.mouse;
      store.mouse_key = self.mouse_key.map(terminal::key_event);
    });
    serve_standard_menu();
  }

  /// Mounts and runs a skene application in the terminal under this configuration;
  /// see [`launch`].
  ///
  /// # Panics
  ///
  /// Panics if the platform is already installed, or when the terminal cannot be set up.
  pub fn run<V, F>(self, app: F)
  where
    F: FnOnce() -> V + 'static,
    V: IntoView<Tui>,
  {
    terminal::run(self, app);
  }
}

/// Serves the terminal's own context menu from the root: the standard clipboard
/// items, drawn in the terminal's default colors, which any menu closer to the pressed
/// node takes precedence over.
fn serve_standard_menu() {
  let Some(root) = crate::platform::root_node()
  else {
    return;
  };
  let gray = Color::srgb(0.5, 0.5, 0.5);

  skene_core::root_owner().enter(|| {
    let _ = ContextMenu::<Tui>::new()
      .set_items([MenuItem::cut(), MenuItem::copy(), MenuItem::paste()])
      .set_color(Color::srgb(1., 1., 1.).when(Media::Dark, Color::srgb(0., 0., 0.)))
      .set_border(Border::new(1, gray))
      .set_item_padding(Inset::symmetric(1, 0))
      .set_item_color(
        Color::TRANSPARENT
          .when(State::Hovered, gray)
          .when(State::Focused, gray),
      )
      .set_item_text_color(typography::INK.when(State::Disabled, gray))
      .serve(root);
  });
}

/// Mounts and runs a skene application in the terminal under the default
/// configuration.
///
/// Installs the terminal platform, builds `app`, and runs the event loop until `Ctrl-Q`.
/// The first frame paints for a dark background at 16 colors; the terminal's replies
/// arrive as events and upgrade the scheme, the depth and the key encoding as they
/// land. A panic anywhere in the process restores the terminal before its message
/// prints.
///
/// # Panics
///
/// Panics if the platform is already installed, or when the terminal cannot be set up.
#[inline(always)]
pub fn launch<V, F>(app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Tui>,
{
  Launch::new().run(app);
}

/// Installs the terminal platform for this thread under the default configuration, for
/// a headless test: the reactive runtime, the host executor, the scene and the store.
///
/// # Panics
///
/// Panics if the platform is already installed for this thread.
#[inline(always)]
pub fn install() {
  Launch::new().install();
}
