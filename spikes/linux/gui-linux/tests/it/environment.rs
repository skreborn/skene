//! The desktop's font settings as the settings portal and XSETTINGS state them, beneath
//! fontconfig's own rules: the portal's where XSETTINGS state none.

use skene_gui_linux::environment::{self, SettingValue};
use skene_parley::linux::DesktopSettings;

const INTERFACE: &str = "org.gnome.desktop.interface";

fn desktop() -> DesktopSettings {
  skene_gui_linux::text::engine()
    .fallback()
    .expect("fontconfig loads")
    .fontconfig()
    .desktop()
}

#[test]
fn the_portals_font_settings_reach_fontconfig_and_xsettings_win() {
  let text = |value: &str| SettingValue::Text(value.to_owned());

  environment::setting_changed(INTERFACE, "font-antialiasing", &text("none"));
  environment::setting_changed(INTERFACE, "font-hinting", &text("full"));
  assert_eq!(
    desktop(),
    DesktopSettings {
      antialias: Some(false),
      hinting: Some(true),
      hint_style: Some(3),
    }
  );

  environment::xft(DesktopSettings {
    antialias: Some(true),
    ..DesktopSettings::default()
  });
  assert_eq!(
    desktop(),
    DesktopSettings {
      antialias: Some(true),
      hinting: Some(true),
      hint_style: Some(3),
    },
    "XSETTINGS win where they state a value"
  );

  environment::setting_changed(INTERFACE, "font-hinting", &text("none"));
  assert_eq!(desktop().hint_style, Some(0));
  assert_eq!(desktop().hinting, Some(false));
}
