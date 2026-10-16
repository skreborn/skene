//! XSETTINGS, the X desktop's settings channel: the `Xft/*` rendering settings a desktop
//! states beneath fontconfig.

use crate::linux::fontconfig::DesktopSettings;

/// A value of one setting.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Setting {
  /// An integer.
  Integer(i32),
  /// A string.
  Text(String),
  /// A color: red, green, blue, alpha.
  Color([u16; 4]),
}

/// The settings of an `_XSETTINGS_SETTINGS` property, by name, in the order the manager
/// wrote them; `None` where the data is not well formed.
#[must_use]
pub fn parse(data: &[u8]) -> Option<Vec<(String, Setting)>> {
  let big = match data.first()? {
    0 => false,
    1 => true,
    _ => return None,
  };
  let u16_at = |at: usize| -> Option<u16> {
    let bytes = data.get(at..at + 2)?.try_into().ok()?;

    Some(if big {
      u16::from_be_bytes(bytes)
    } else {
      u16::from_le_bytes(bytes)
    })
  };
  let u32_at = |at: usize| -> Option<u32> {
    let bytes = data.get(at..at + 4)?.try_into().ok()?;

    Some(if big {
      u32::from_be_bytes(bytes)
    } else {
      u32::from_le_bytes(bytes)
    })
  };
  let count = u32_at(8)?;
  let mut at = 12;
  let mut settings = Vec::new();

  for _ in 0..count {
    let kind = *data.get(at)?;
    let name_len = usize::from(u16_at(at + 2)?);
    let name = String::from_utf8_lossy(data.get(at + 4..at + 4 + name_len)?).into_owned();

    at += 4 + name_len.next_multiple_of(4) + 4;

    let value = match kind {
      0 => {
        let value = u32_at(at)?.cast_signed();

        at += 4;
        Setting::Integer(value)
      }
      1 => {
        let len = usize::try_from(u32_at(at)?).ok()?;
        let text = String::from_utf8_lossy(data.get(at + 4..at + 4 + len)?).into_owned();

        at += 4 + len.next_multiple_of(4);
        Setting::Text(text)
      }
      2 => {
        let color = [
          u16_at(at)?,
          u16_at(at + 2)?,
          u16_at(at + 4)?,
          u16_at(at + 6)?,
        ];

        at += 8;
        Setting::Color(color)
      }
      _ => return None,
    };

    settings.push((name, value));
  }

  Some(settings)
}

/// The rendering settings `settings` state: `Xft/Antialias`, `Xft/Hinting` and
/// `Xft/HintStyle`, a value of -1 or an unknown style stating nothing.
#[must_use]
pub fn desktop_settings(settings: &[(String, Setting)]) -> DesktopSettings {
  let mut desktop = DesktopSettings::default();

  for (name, value) in settings {
    match (name.as_str(), value) {
      ("Xft/Antialias", Setting::Integer(value)) if *value >= 0 => {
        desktop.antialias = Some(*value != 0);
      }
      ("Xft/Hinting", Setting::Integer(value)) if *value >= 0 => {
        desktop.hinting = Some(*value != 0);
      }
      ("Xft/HintStyle", Setting::Text(style)) => {
        desktop.hint_style = match style.as_str() {
          "hintnone" => Some(0),
          "hintslight" => Some(1),
          "hintmedium" => Some(2),
          "hintfull" => Some(3),
          _ => None,
        };
      }
      _ => {}
    }
  }

  desktop
}

#[cfg(test)]
mod tests {
  use super::{Setting, desktop_settings, parse};

  /// An `_XSETTINGS_SETTINGS` property as a manager writes it, little-endian.
  fn property(settings: &[(&str, Setting)]) -> Vec<u8> {
    let mut data = vec![0, 0, 0, 0];

    data.extend(7_u32.to_le_bytes());
    data.extend(u32::try_from(settings.len()).unwrap().to_le_bytes());

    for (name, value) in settings {
      let kind = match value {
        Setting::Integer(_) => 0,
        Setting::Text(_) => 1,
        Setting::Color(_) => 2,
      };

      data.extend([kind, 0]);
      data.extend(u16::try_from(name.len()).unwrap().to_le_bytes());
      data.extend(name.as_bytes());
      data.resize(data.len() + name.len().next_multiple_of(4) - name.len(), 0);
      data.extend(0_u32.to_le_bytes());

      match value {
        Setting::Integer(value) => data.extend(value.to_le_bytes()),
        Setting::Text(text) => {
          data.extend(u32::try_from(text.len()).unwrap().to_le_bytes());
          data.extend(text.as_bytes());
          data.resize(data.len() + text.len().next_multiple_of(4) - text.len(), 0);
        }
        Setting::Color(color) => color
          .iter()
          .for_each(|channel| data.extend(channel.to_le_bytes())),
      }
    }

    data
  }

  #[test]
  fn the_xft_settings_are_read() {
    let data = property(&[
      ("Net/ThemeName", Setting::Text("Adwaita".into())),
      ("Xft/Antialias", Setting::Integer(0)),
      ("Xft/Hinting", Setting::Integer(1)),
      ("Xft/HintStyle", Setting::Text("hintfull".into())),
      ("Gtk/CursorThemeSize", Setting::Integer(24)),
      ("Net/Accent", Setting::Color([1, 2, 3, 4])),
    ]);
    let settings = parse(&data).expect("well formed");

    assert_eq!(settings.len(), 6);
    assert_eq!(settings[5].1, Setting::Color([1, 2, 3, 4]));

    let desktop = desktop_settings(&settings);

    assert_eq!(desktop.antialias, Some(false));
    assert_eq!(desktop.hinting, Some(true));
    assert_eq!(desktop.hint_style, Some(3));
  }

  #[test]
  fn a_default_states_nothing() {
    let data = property(&[("Xft/Antialias", Setting::Integer(-1))]);

    assert_eq!(desktop_settings(&parse(&data).unwrap()).antialias, None);
    assert!(parse(&[9]).is_none());
  }
}
