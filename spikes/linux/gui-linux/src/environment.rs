//! The reader's preferences, reported into the foundation's environment:
//!
//! - the XDG settings portal's `org.freedesktop.appearance` `color-scheme` (1 dark),
//!   `contrast` (1 more), `accent-color` (three fractions) and `reduced-motion` (1 reduce),
//!   read at start and followed through `SettingChanged` on a thread of their own, which posts
//!   each change to the loop; `org.gnome.desktop.interface` `enable-animations` where the
//!   portal has no `reduced-motion`, its `cursor-blink`, and the fonts' settings:
//!   `font-antialiasing`, `font-hinting` (beneath fontconfig's own rules, as XSETTINGS'
//!   `Xft/*`), `text-scaling-factor` (every text's size) and `font-name` (the family of
//!   [`FontFamily::System`]);
//! - where no portal answers, the X server's XSETTINGS (`Net/ThemeName` naming a dark or a
//!   high-contrast theme, `Gtk/EnableAnimations`, `Net/CursorBlink`), which the X11 backend
//!   reads; its `Xft/Antialias`, `Xft/Hinting` and `Xft/HintStyle` win over the portal's
//!   font settings where both state one;
//! - the app's own choice of scheme ([`set_scheme`]), which wins over the reader's;
//! - what a primary press on a scroll bar's track does ([`primary_warps_slider`]): XSETTINGS'
//!   `Gtk/PrimaryButtonWarpsSlider`, else the GTK 4 settings files'.

use std::{cell::RefCell, time::Duration};

use skene_foundation::{
  Color, FontFamily, Scheme,
  environment::{self, Fact},
};
use skene_parley::linux::DesktopSettings;

use crate::{runtime::Message, shell, trace};

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SETTINGS: &str = "org.freedesktop.portal.Settings";
const APPEARANCE: &str = "org.freedesktop.appearance";
const INTERFACE: &str = "org.gnome.desktop.interface";

/// A setting's value, as the portal sends it.
#[derive(Clone, PartialEq, Debug)]
pub enum SettingValue {
  /// An unsigned integer.
  U32(u32),
  /// A flag.
  Bool(bool),
  /// A color: red, green, blue, 0 to 1.
  Color(f64, f64, f64),
  /// A number.
  F64(f64),
  /// A string.
  Text(String),
  /// Something else.
  Other,
}

/// The reader's preferences and the app's choice.
#[derive(Clone, Debug, Default)]
struct Preferences {
  dark: bool,
  high_contrast: bool,
  reduced_motion: Option<bool>,
  animations: Option<bool>,
  accent: Option<Color>,
  cursor_blink: Option<bool>,
  choice: Option<Scheme>,
  portal: bool,
  fonts: DesktopSettings,
  xft: DesktopSettings,
  text_scale: Option<f64>,
  family: Option<std::rc::Rc<str>>,
}

thread_local! {
  static PREFERENCES: RefCell<Preferences> = RefCell::new(Preferences::default());
  static HEARD: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
  static WARPS: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Takes XSETTINGS' `Gtk/PrimaryButtonWarpsSlider`, `None` where the X server's settings state
/// none.
pub fn set_warps_slider(warps: Option<bool>) {
  WARPS.with(|stated| stated.set(warps));
}

/// Whether a primary press on a scroll bar's track jumps its thumb to the point, as GTK's
/// `gtk-primary-button-warps-slider` says; a middle press, or one with `Shift`, does the other.
/// XSETTINGS' `Gtk/PrimaryButtonWarpsSlider` where the X server states it, else what the GTK 4
/// settings files state (`gtk-4.0/settings.ini` in the user's configuration directory, then in
/// each of `$XDG_CONFIG_DIRS` and `/etc`), else yes, GTK's default; the files are read at each
/// call, so a change applies at the next press.
pub fn primary_warps_slider() -> bool {
  WARPS
    .with(std::cell::Cell::get)
    .or_else(warps_slider_file)
    .unwrap_or(true)
}

/// What the first GTK 4 settings file that states `gtk-primary-button-warps-slider` says.
fn warps_slider_file() -> Option<bool> {
  let home = std::env::var_os("XDG_CONFIG_HOME")
    .map(std::path::PathBuf::from)
    .or_else(|| std::env::var_os("HOME").map(|home| std::path::Path::new(&home).join(".config")));
  let system = std::env::var("XDG_CONFIG_DIRS").unwrap_or_else(|_| "/etc/xdg".to_owned());
  let directories = home
    .into_iter()
    .chain(
      system
        .split(':')
        .filter(|directory| !directory.is_empty())
        .map(std::path::PathBuf::from),
    )
    .chain(std::iter::once(std::path::PathBuf::from("/etc")));

  for directory in directories {
    let Ok(text) = std::fs::read_to_string(directory.join("gtk-4.0/settings.ini")) else {
      continue;
    };

    if let Some(warps) = settings_flag(&text, "gtk-primary-button-warps-slider") {
      return Some(warps);
    }
  }

  None
}

/// The flag `key` a GTK settings file states in its `[Settings]` group, where it states one.
fn settings_flag(text: &str, key: &str) -> Option<bool> {
  let mut in_settings = false;

  for line in text.lines().map(str::trim) {
    if line.starts_with('[') {
      in_settings = line == "[Settings]";

      continue;
    }

    let Some((name, value)) = line.split_once('=').filter(|_| in_settings) else {
      continue;
    };

    if name.trim() == key {
      return match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
      };
    }
  }

  None
}

/// How many setting changes the portal's thread has delivered to this thread's loop.
pub fn settings_heard() -> u64 {
  HEARD.with(std::cell::Cell::get)
}

fn value_of(value: &zbus::zvariant::Value<'_>) -> SettingValue {
  use zbus::zvariant::Value;

  match value {
    Value::U32(value) => SettingValue::U32(*value),
    Value::I32(value) => SettingValue::U32(u32::try_from(*value).unwrap_or(0)),
    Value::Bool(value) => SettingValue::Bool(*value),
    Value::F64(value) => SettingValue::F64(*value),
    Value::Str(value) => SettingValue::Text(value.as_str().to_owned()),
    Value::Value(inner) => value_of(inner),
    Value::Structure(structure) => {
      let fields = structure.fields();
      let number = |index: usize| match fields.get(index) {
        Some(Value::F64(value)) => Some(*value),
        _ => None,
      };

      match (number(0), number(1), number(2)) {
        (Some(red), Some(green), Some(blue)) => SettingValue::Color(red, green, blue),
        _ => SettingValue::Other,
      }
    }
    _ => SettingValue::Other,
  }
}

fn read(
  connection: &zbus::blocking::Connection,
  namespace: &str,
  key: &str,
) -> Option<SettingValue> {
  let call = |method: &str| {
    connection
      .call_method(
        Some(PORTAL),
        PORTAL_PATH,
        Some(SETTINGS),
        method,
        &(namespace, key),
      )
      .ok()
  };
  let reply = call("ReadOne").or_else(|| call("Read"))?;
  let body = reply.body();
  let value: zbus::zvariant::OwnedValue = body.deserialize().ok()?;

  Some(value_of(&value))
}

fn apply_setting(preferences: &mut Preferences, namespace: &str, key: &str, value: &SettingValue) {
  match (namespace, key, value) {
    (APPEARANCE, "color-scheme", SettingValue::U32(scheme)) => preferences.dark = *scheme == 1,
    (APPEARANCE, "contrast", SettingValue::U32(contrast)) => {
      preferences.high_contrast = *contrast == 1;
    }
    (APPEARANCE, "reduced-motion", SettingValue::U32(reduced)) => {
      preferences.reduced_motion = Some(*reduced == 1);
    }
    (APPEARANCE, "accent-color", SettingValue::Color(red, green, blue)) => {
      let in_range = |value: f64| (0. ..=1.).contains(&value);

      preferences.accent = (in_range(*red) && in_range(*green) && in_range(*blue)).then(|| {
        let channel = |value: f64| (value * 255.).round() as u8;

        Color {
          red: channel(*red),
          green: channel(*green),
          blue: channel(*blue),
          alpha: 255,
        }
      });
    }
    (INTERFACE, "enable-animations", SettingValue::Bool(enabled)) => {
      preferences.animations = Some(*enabled);
    }
    (INTERFACE, "cursor-blink", SettingValue::Bool(blinks)) => {
      preferences.cursor_blink = Some(*blinks);
    }
    (INTERFACE, "font-antialiasing", SettingValue::Text(antialiasing)) => {
      preferences.fonts.antialias = match antialiasing.as_str() {
        "none" => Some(false),
        "grayscale" | "rgba" => Some(true),
        _ => None,
      };
    }
    (INTERFACE, "font-hinting", SettingValue::Text(hinting)) => {
      let style = match hinting.as_str() {
        "none" => Some(0),
        "slight" => Some(1),
        "medium" => Some(2),
        "full" => Some(3),
        _ => None,
      };

      preferences.fonts.hinting = style.map(|style| style > 0);
      preferences.fonts.hint_style = style;
    }
    (INTERFACE, "text-scaling-factor", SettingValue::F64(factor)) => {
      preferences.text_scale = (*factor > 0. && factor.is_finite()).then_some(*factor);
    }
    (INTERFACE, "font-name", SettingValue::Text(name)) => {
      preferences.family = family_of(name).map(std::rc::Rc::from);
    }
    _ => {}
  }
}

/// The family a Pango font description names (`Cantarell 11`, `Noto Sans Bold 10`): the
/// description without its size and its style words.
fn family_of(description: &str) -> Option<&str> {
  const STYLES: [&str; 12] = [
    "Thin",
    "Light",
    "Regular",
    "Medium",
    "Semi-Bold",
    "Bold",
    "Heavy",
    "Black",
    "Italic",
    "Oblique",
    "Condensed",
    "Expanded",
  ];

  let mut family = description.trim();

  if let Some((rest, size)) = family.rsplit_once(' ')
    && size.trim_end_matches("px").parse::<f64>().is_ok()
  {
    family = rest.trim_end();
  }

  while let Some((rest, word)) = family.rsplit_once(' ')
    && STYLES.contains(&word)
  {
    family = rest.trim_end();
  }

  (!family.is_empty()).then_some(family)
}

/// How much every text's size is scaled: the desktop's `text-scaling-factor`, 1 where none is
/// stated.
pub fn text_scale() -> f32 {
  PREFERENCES.with(|preferences| preferences.borrow().text_scale.unwrap_or(1.) as f32)
}

/// The family [`FontFamily::System`] names: the desktop's interface font where it states one.
pub fn system_family() -> FontFamily {
  PREFERENCES.with(|preferences| {
    preferences
      .borrow()
      .family
      .clone()
      .map_or(FontFamily::System, FontFamily::Named)
  })
}

/// What the X server's XSETTINGS say of the fonts' rendering: they win over the portal's.
pub fn xft(desktop: DesktopSettings) {
  PREFERENCES.with(|preferences| preferences.borrow_mut().xft = desktop);
  apply();
}

/// Reads the reader's preferences from the settings portal, and follows their changes on a
/// thread that posts each to the loop; `false` where no portal answers.
pub fn follow_portal() -> bool {
  if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
    return false;
  }

  let Ok(connection) = zbus::blocking::connection::Builder::session()
    .and_then(|builder| builder.method_timeout(Duration::from_millis(500)).build())
  else {
    return false;
  };
  let keys = [
    (APPEARANCE, "color-scheme"),
    (APPEARANCE, "contrast"),
    (APPEARANCE, "accent-color"),
    (APPEARANCE, "reduced-motion"),
    (INTERFACE, "enable-animations"),
    (INTERFACE, "cursor-blink"),
    (INTERFACE, "font-antialiasing"),
    (INTERFACE, "font-hinting"),
    (INTERFACE, "text-scaling-factor"),
    (INTERFACE, "font-name"),
  ];
  let mut answered = false;

  PREFERENCES.with(|preferences| {
    let mut preferences = preferences.borrow_mut();

    for (namespace, key) in keys {
      if let Some(value) = read(&connection, namespace, key) {
        trace::log(|| format!("[portal] {namespace} {key} = {value:?}"));
        answered |= namespace == APPEARANCE;
        apply_setting(&mut preferences, namespace, key, &value);
      }
    }

    preferences.portal = answered;
  });

  if !answered {
    return false;
  }

  let inbox = crate::runtime::inbox();

  std::thread::Builder::new()
    .name("skene-portal".into())
    .spawn(move || {
      let Ok(proxy) = zbus::blocking::Proxy::new(&connection, PORTAL, PORTAL_PATH, SETTINGS) else {
        return;
      };
      let Ok(signals) = proxy.receive_signal("SettingChanged") else {
        return;
      };

      for message in signals {
        let body = message.body();
        let Ok((namespace, key, value)) =
          body.deserialize::<(String, String, zbus::zvariant::OwnedValue)>()
        else {
          continue;
        };

        inbox.send(Message::Setting(namespace, key, value_of(&value)));
      }
    })
    .is_ok()
}

/// Applies a setting the portal's thread posted.
pub fn setting_changed(namespace: &str, key: &str, value: &SettingValue) {
  HEARD.with(|heard| heard.set(heard.get() + 1));
  trace::log(|| format!("[portal] changed {namespace} {key} = {value:?}"));
  PREFERENCES
    .with(|preferences| apply_setting(&mut preferences.borrow_mut(), namespace, key, value));
  apply();
}

/// What the X server's XSETTINGS say, where no portal answered: a dark or high-contrast
/// theme, whether animations are on and whether a caret blinks.
pub fn xsettings(theme: Option<&str>, animations: Option<bool>, cursor_blink: Option<bool>) {
  PREFERENCES.with(|preferences| {
    let mut preferences = preferences.borrow_mut();

    if preferences.portal {
      return;
    }

    if let Some(theme) = theme {
      let lower = theme.to_ascii_lowercase();

      preferences.dark = lower.contains("dark");
      preferences.high_contrast = lower.contains("highcontrast") || lower.ends_with("-hc");
    }

    preferences.animations = animations.or(preferences.animations);
    preferences.cursor_blink = cursor_blink.or(preferences.cursor_blink);
  });
  apply();
}

/// Writes the preferences in force into the environment.
pub fn apply() {
  let preferences = PREFERENCES.with(|preferences| preferences.borrow().clone());
  let dark = preferences
    .choice
    .map_or(preferences.dark, |choice| choice == Scheme::Dark);
  let reduced = preferences
    .reduced_motion
    .or(preferences.animations.map(|enabled| !enabled))
    .unwrap_or(false);

  environment::report(Fact::Dark(dark));
  environment::report(Fact::HighContrast(preferences.high_contrast));
  environment::report(Fact::ReducedMotion(reduced));
  environment::report(Fact::Accent(preferences.accent));

  if let Some(mut fallback) = crate::text::engine().fallback() {
    fallback.fontconfig().set_desktop(DesktopSettings {
      antialias: preferences.xft.antialias.or(preferences.fonts.antialias),
      hinting: preferences.xft.hinting.or(preferences.fonts.hinting),
      hint_style: preferences.xft.hint_style.or(preferences.fonts.hint_style),
    });
  }

  shell::invalidate_prepare();
  shell::invalidate_all();
}

/// Whether a text entry's caret blinks: the reader's setting, `true` where none is known.
pub fn cursor_blinks() -> bool {
  PREFERENCES.with(|preferences| preferences.borrow().cursor_blink.unwrap_or(true))
}

/// Chooses the scheme for the whole app, or follows the reader's again.
pub fn set_scheme(choice: Option<Scheme>) {
  PREFERENCES.with(|preferences| preferences.borrow_mut().choice = choice);
  apply();
}
