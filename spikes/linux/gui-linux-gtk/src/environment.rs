//! The environment GTK 4.14 reports, written into the foundation's
//! [`Environment`](skene_foundation::Environment) through [`environment::report`].
//!
//! - **dark, high contrast**: the XDG settings portal's `org.freedesktop.appearance`
//!   `color-scheme` and `contrast`, read over D-Bus and followed through `SettingChanged`;
//!   `GtkSettings` (`gtk-application-prefer-dark-theme`, a high-contrast theme name:
//!   `HighContrast`, `Default-hc`, `Default-hc-dark`) where no portal answers, followed through
//!   `notify`.
//! - **reduced motion**: `GtkSettings:gtk-enable-animations`, negated, followed through
//!   `notify`.
//! - **width, height**: the window's laid-out size, followed as its surface is laid out (a
//!   maximized, tiled or fullscreen window included); before it is laid out, its
//!   `default-width` and `default-height`.
//! - **scale**: `GdkSurface:scale`, followed through `notify`.
//! - **right to left**: GTK's default text direction, and a window's own as it changes.
//!
//! The scheme in force is the app's choice where it made one ([`set_scheme`]), the reader's
//! otherwise; it reaches GTK's own theme through `gtk-application-prefer-dark-theme`, so a
//! switch restyles every toolkit control, and Skene's named colors are rewritten from it.

use std::cell::{Cell, RefCell};

use gtk::{gio, glib, prelude::*};
use skene_foundation::{
  Scheme,
  environment::{self, Fact},
};

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SETTINGS: &str = "org.freedesktop.portal.Settings";
const APPEARANCE: &str = "org.freedesktop.appearance";

/// Where the reader's preferences come from, and the app's choice.
struct Preferences {
  /// Whether the reader prefers dark.
  dark: bool,
  /// Whether the reader asked for more contrast.
  high_contrast: bool,
  /// Whether the settings portal answered, so `GtkSettings` is Skene's to write.
  portal: bool,
  /// The app's choice, where it made one.
  choice: Option<Scheme>,
  /// The session bus's subscription to the portal's changes.
  subscription: Option<gio::SignalSubscription>,
}

thread_local! {
  static PREFERENCES: RefCell<Preferences> = const {
    RefCell::new(Preferences {
      dark: false,
      high_contrast: false,
      portal: false,
      choice: None,
      subscription: None,
    })
  };
  /// What Skene wrote to `gtk-application-prefer-dark-theme` and has not heard back yet: the
  /// next notification of it is the echo, not the reader's, and is consumed.
  static WRITTEN: Cell<Option<bool>> = const { Cell::new(None) };
  static ATTACHED: Cell<bool> = const { Cell::new(false) };
}

/// Reads one appearance setting from the portal: `ReadOne` (version 2), else `Read`, whose
/// value is wrapped once more.
fn read(bus: &gio::DBusConnection, key: &str) -> Option<u32> {
  let call = |method: &str| {
    bus
      .call_sync(
        Some(PORTAL),
        PORTAL_PATH,
        SETTINGS,
        method,
        Some(&(APPEARANCE, key).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        500,
        None::<&gio::Cancellable>,
      )
      .ok()
  };

  if let Some(reply) = call("ReadOne") {
    return reply.child_value(0).as_variant()?.get::<u32>();
  }

  let reply = call("Read")?;
  let value = reply.child_value(0).as_variant()?;

  value
    .get::<u32>()
    .or_else(|| value.as_variant()?.get::<u32>())
}

/// Reads the reader's scheme and contrast from the settings portal and follows their changes;
/// `false` where no portal answers.
fn follow_portal() -> bool {
  if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
    return false;
  }

  let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) else {
    return false;
  };
  let Some(scheme) = read(&bus, "color-scheme") else {
    return false;
  };
  let contrast = read(&bus, "contrast").unwrap_or(0);
  let subscription = bus.subscribe_to_signal(
    Some(PORTAL),
    Some(SETTINGS),
    Some("SettingChanged"),
    Some(PORTAL_PATH),
    None,
    gio::DBusSignalFlags::NONE,
    |signal| {
      let Some((namespace, key, value)) =
        signal.parameters.get::<(String, String, glib::Variant)>()
      else {
        return;
      };

      if namespace != APPEARANCE {
        return;
      }

      let value = value.get::<u32>().unwrap_or(0);

      PREFERENCES.with(|preferences| {
        let mut preferences = preferences.borrow_mut();

        match key.as_str() {
          "color-scheme" => preferences.dark = value == 1,
          "contrast" => preferences.high_contrast = value == 1,
          _ => {}
        }
      });
      apply();
    },
  );

  PREFERENCES.with(|preferences| {
    let mut preferences = preferences.borrow_mut();

    preferences.dark = scheme == 1;
    preferences.high_contrast = contrast == 1;
    preferences.portal = true;
    preferences.subscription = Some(subscription);
  });

  true
}

/// Writes the scheme in force into GTK and the environment.
fn apply() {
  let (dark, high_contrast) = PREFERENCES.with(|preferences| {
    let preferences = preferences.borrow();

    (
      preferences
        .choice
        .map_or(preferences.dark, |choice| choice == Scheme::Dark),
      preferences.high_contrast,
    )
  });

  if let Some(settings) = gtk::Settings::default()
    && settings.is_gtk_application_prefer_dark_theme() != dark
  {
    WRITTEN.with(|written| written.set(Some(dark)));
    settings.set_gtk_application_prefer_dark_theme(dark);
  }

  environment::report(Fact::Dark(dark));
  environment::report(Fact::HighContrast(high_contrast));
}

/// Chooses the scheme for the whole app, or follows the reader's again (`None`).
pub fn set_scheme(choice: Option<Scheme>) {
  PREFERENCES.with(|preferences| preferences.borrow_mut().choice = choice);
  apply();
}

/// Forgets the app's scheme choice and reports the reader's preferences, the animations
/// setting and the text direction again: what a test starts from.
pub fn reset() {
  PREFERENCES.with(|preferences| preferences.borrow_mut().choice = None);
  apply();

  if let Some(settings) = gtk::Settings::default() {
    environment::report(Fact::ReducedMotion(!settings.is_gtk_enable_animations()));
  }

  environment::report(Fact::RightToLeft(
    gtk::Widget::default_direction() == gtk::TextDirection::Rtl,
  ));
}

/// Whether the GTK theme `name` is a high-contrast one.
fn high_contrast(name: &str) -> bool {
  name.contains("HighContrast") || name.ends_with("-hc") || name.contains("-hc-")
}

/// Reads the reader's preferences and the display's facts now and follows their changes, once
/// per thread.
pub fn attach_settings() {
  if ATTACHED.with(|attached| attached.replace(true)) {
    return;
  }

  let settings = gtk::Settings::default().expect("GTK is initialized");

  environment::report(Fact::ReducedMotion(!settings.is_gtk_enable_animations()));

  settings.connect_gtk_enable_animations_notify(|settings| {
    environment::report(Fact::ReducedMotion(!settings.is_gtk_enable_animations()));
  });

  environment::report(Fact::RightToLeft(
    gtk::Widget::default_direction() == gtk::TextDirection::Rtl,
  ));

  if !follow_portal() {
    let theme = settings.gtk_theme_name().unwrap_or_default();

    PREFERENCES.with(|preferences| {
      let mut preferences = preferences.borrow_mut();

      preferences.dark = settings.is_gtk_application_prefer_dark_theme();
      preferences.high_contrast = high_contrast(&theme);
    });
    settings.connect_gtk_application_prefer_dark_theme_notify(|settings| {
      let dark = settings.is_gtk_application_prefer_dark_theme();

      if WRITTEN.with(|written| written.take()) == Some(dark) {
        return;
      }

      PREFERENCES.with(|preferences| preferences.borrow_mut().dark = dark);
      apply();
    });
    settings.connect_gtk_theme_name_notify(|settings| {
      let theme = settings.gtk_theme_name().unwrap_or_default();

      PREFERENCES
        .with(|preferences| preferences.borrow_mut().high_contrast = high_contrast(&theme));
      apply();
    });
  }

  apply();
}

/// Reports a window's size, where it has one.
fn report_size(width: i32, height: i32) {
  if width > 0 {
    environment::report(Fact::Width(width as f32));
  }

  if height > 0 {
    environment::report(Fact::Height(height as f32));
  }
}

/// Whether `window` is sized by the window manager rather than by its default size:
/// maximized, fullscreen or tiled.
fn placed_by_manager(window: &gtk::Window) -> bool {
  let tiled = window
    .surface()
    .and_downcast::<gtk::gdk::Toplevel>()
    .is_some_and(|toplevel| toplevel.state().contains(gtk::gdk::ToplevelState::TILED));

  window.is_maximized() || window.is_fullscreen() || tiled
}

/// Reports `window`'s size, scale and direction now and as they change.
pub fn attach_window(window: &gtk::Window) {
  let requested = |window: &gtk::Window| {
    if !placed_by_manager(window) {
      let (width, height) = window.default_size();

      report_size(width, height);
    }
  };

  let direction = |window: &gtk::Window| {
    environment::report(Fact::RightToLeft(
      window.direction() == gtk::TextDirection::Rtl,
    ));
  };

  requested(window);
  direction(window);
  window.connect_default_width_notify(requested);
  window.connect_default_height_notify(requested);
  window.connect_direction_changed(move |window, _| direction(window));
  window.connect_realize(|window| {
    let Some(surface) = window.surface() else {
      return;
    };
    let laid_out = window.downgrade();

    environment::report(Fact::Scale(surface.scale()));
    surface.connect_scale_notify(|surface| environment::report(Fact::Scale(surface.scale())));
    surface.connect_layout(move |_, _, _| {
      let window = laid_out.clone();

      glib::idle_add_local_once(move || {
        if let Some(window) = window.upgrade() {
          report_size(window.width(), window.height());
        }
      });
    });
  });
}
