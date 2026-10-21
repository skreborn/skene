//! The reader's settings in a test: a stand-in for xdg-desktop-portal's Settings interface on
//! the test's private session bus, served from a thread of its own. The app reads and follows
//! it through its real path (the portal's `ReadOne` and `SettingChanged`), as it reads the
//! desktop's portal outside a test.
//!
//! It serves `org.freedesktop.appearance`'s `color-scheme` (0 no preference, 1 dark),
//! `contrast` (0 none, 1 more) and `reduced-motion` (0 none, 1 reduce), and
//! `org.gnome.desktop.interface`'s `cursor-blink` (off: a test reads a caret that stays),
//! `font-antialiasing`, `font-hinting`, `text-scaling-factor` and `font-name`.

use std::{
  collections::HashMap,
  sync::{Arc, Mutex, PoisonError},
};

use zbus::{
  blocking::Connection,
  zvariant::{OwnedValue, Value},
};

use self::served::Settings;

const APPEARANCE: &str = "org.freedesktop.appearance";
const INTERFACE: &str = "org.gnome.desktop.interface";
const PATH: &str = "/org/freedesktop/portal/desktop";

/// A setting the stand-in serves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Setting {
  /// `color-scheme`: 0 no preference, 1 dark, 2 light.
  ColorScheme(u32),
  /// `contrast`: 0 none, 1 more.
  Contrast(u32),
  /// `reduced-motion`: 0 none, 1 reduce.
  ReducedMotion(u32),
  /// `cursor-blink`.
  CursorBlink(bool),
  /// `text-scaling-factor`, in thousandths.
  TextScalingFactor(u32),
  /// `font-name`: a Pango font description, empty for none stated.
  FontName(&'static str),
  /// `font-antialiasing`: `none`, `grayscale` or `rgba`, empty for none stated.
  FontAntialiasing(&'static str),
  /// `font-hinting`: `none`, `slight`, `medium` or `full`, empty for none stated.
  FontHinting(&'static str),
}

impl Setting {
  const fn key(self) -> (&'static str, &'static str) {
    match self {
      Self::ColorScheme(_) => (APPEARANCE, "color-scheme"),
      Self::Contrast(_) => (APPEARANCE, "contrast"),
      Self::ReducedMotion(_) => (APPEARANCE, "reduced-motion"),
      Self::CursorBlink(_) => (INTERFACE, "cursor-blink"),
      Self::TextScalingFactor(_) => (INTERFACE, "text-scaling-factor"),
      Self::FontName(_) => (INTERFACE, "font-name"),
      Self::FontAntialiasing(_) => (INTERFACE, "font-antialiasing"),
      Self::FontHinting(_) => (INTERFACE, "font-hinting"),
    }
  }

  fn value(self) -> OwnedValue {
    match self {
      Self::ColorScheme(value) | Self::Contrast(value) | Self::ReducedMotion(value) => {
        OwnedValue::from(value)
      }
      Self::CursorBlink(blinks) => OwnedValue::from(blinks),
      Self::TextScalingFactor(thousandths) => OwnedValue::from(f64::from(thousandths) / 1000.),
      Self::FontName(text) | Self::FontAntialiasing(text) | Self::FontHinting(text) => {
        Value::from(text)
          .try_into()
          .unwrap_or_else(|_| OwnedValue::from(0_u32))
      }
    }
  }

  /// The setting as the app hears it from the portal: its namespace, key and value.
  #[must_use]
  pub fn heard(self) -> (&'static str, &'static str, crate::environment::SettingValue) {
    use crate::environment::SettingValue;

    let (namespace, key) = self.key();
    let value = match self {
      Self::ColorScheme(value) | Self::Contrast(value) | Self::ReducedMotion(value) => {
        SettingValue::U32(value)
      }
      Self::CursorBlink(blinks) => SettingValue::Bool(blinks),
      Self::TextScalingFactor(thousandths) => SettingValue::F64(f64::from(thousandths) / 1000.),
      Self::FontName(text) | Self::FontAntialiasing(text) | Self::FontHinting(text) => {
        SettingValue::Text(text.to_owned())
      }
    };

    (namespace, key, value)
  }

  /// What a test starts from: no preference, no more contrast, motion, a caret that stays,
  /// text unscaled in Skene's own family, rendered as fontconfig's own rules say.
  pub const DEFAULTS: [Self; 8] = [
    Self::ColorScheme(0),
    Self::Contrast(0),
    Self::ReducedMotion(0),
    Self::CursorBlink(false),
    Self::TextScalingFactor(1000),
    Self::FontName(""),
    Self::FontAntialiasing(""),
    Self::FontHinting(""),
  ];
}

type Values = Arc<Mutex<HashMap<(&'static str, &'static str), Setting>>>;

/// The served interface, in a module of its own: zbus's macro adds a signal trait beside it.
mod served {
  use std::sync::PoisonError;

  use zbus::{
    interface,
    object_server::SignalEmitter,
    zvariant::{OwnedValue, Value},
  };

  use super::Values;

  /// The settings portal's interface, answering from the stand-in's values.
  pub(super) struct Settings {
    /// What each setting holds now.
    pub(super) values: Values,
  }

  #[interface(name = "org.freedesktop.portal.Settings")]
  impl Settings {
    /// The setting `key` of `namespace`.
    fn read_one(&self, namespace: &str, key: &str) -> zbus::fdo::Result<OwnedValue> {
      let values = self.values.lock().unwrap_or_else(PoisonError::into_inner);

      values
        .iter()
        .find(|((space, name), _)| *space == namespace && *name == key)
        .map(|(_, setting)| setting.value())
        .ok_or_else(|| zbus::fdo::Error::UnknownProperty(format!("{namespace} {key}")))
    }

    /// The setting `key` of `namespace`, wrapped in a variant (the interface's first version).
    fn read(&self, namespace: &str, key: &str) -> zbus::fdo::Result<OwnedValue> {
      let inner = self.read_one(namespace, key)?;

      Value::Value(Box::new(Value::from(inner)))
        .try_into()
        .map_err(|error: zbus::zvariant::Error| zbus::fdo::Error::Failed(error.to_string()))
    }

    /// A setting changed.
    #[zbus(signal)]
    pub(super) async fn setting_changed(
      emitter: &SignalEmitter<'_>,
      namespace: &str,
      key: &str,
      value: Value<'_>,
    ) -> zbus::Result<()>;

    /// The interface's version.
    #[zbus(property)]
    const fn version(&self) -> u32 {
      2
    }
  }
}

/// The stand-in, served while it lives.
pub struct Portal {
  connection: Connection,
  values: Values,
}

impl Portal {
  /// Serves the stand-in as `org.freedesktop.portal.Desktop` on the session bus, holding
  /// [`Setting::DEFAULTS`].
  ///
  /// # Errors
  ///
  /// Where there is no session bus, or another portal holds the name.
  pub fn serve() -> zbus::Result<Self> {
    let values: Values = Arc::new(Mutex::new(
      Setting::DEFAULTS
        .into_iter()
        .map(|setting| (setting.key(), setting))
        .collect(),
    ));
    let connection = zbus::blocking::connection::Builder::session()?
      .name("org.freedesktop.portal.Desktop")?
      .serve_at(
        PATH,
        Settings {
          values: Arc::clone(&values),
        },
      )?
      .build()?;

    Ok(Self { connection, values })
  }

  /// Changes `setting`, telling the app as the portal does where it changed; whether it
  /// changed.
  ///
  /// # Errors
  ///
  /// Where the signal cannot be sent.
  pub fn set(&self, setting: Setting) -> zbus::Result<bool> {
    let previous = self
      .values
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .insert(setting.key(), setting);

    if previous == Some(setting) {
      return Ok(false);
    }

    let interface = self
      .connection
      .object_server()
      .interface::<_, Settings>(PATH)?;
    let (namespace, key) = setting.key();

    zbus::block_on(Settings::setting_changed(
      interface.signal_emitter(),
      namespace,
      key,
      Value::from(setting.value()),
    ))?;

    Ok(true)
  }
}
