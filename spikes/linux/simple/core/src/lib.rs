//! # skene-simple-core
//!
//! What the Simple component library's presentation crates share, and nothing that names a
//! platform or a medium: its colors, light and dark in each definition, under their token
//! names ([`SimpleColors`]), and the calendar date its date picker shows ([`SimpleDate`]).
//! `skene-simple-gui` adds the pixel platforms' lengths to the colors to make its palette, and
//! draws the components.

use std::fmt;

use skene_foundation::{Color, Definitions, Media, Token, TokenRef, TokenSet};

/// The accent: a control's on state, a primary button's fill.
pub const ACCENT: TokenRef<Color> = TokenRef::new("simple-accent");
/// What sits on the accent: a primary button's label.
pub const ON_ACCENT: TokenRef<Color> = TokenRef::new("simple-on-accent");
/// A card's fill.
pub const CARD: TokenRef<Color> = TokenRef::new("simple-card");
/// The text's color.
pub const INK: TokenRef<Color> = TokenRef::new("simple-ink");
/// Secondary text.
pub const MUTED: TokenRef<Color> = TokenRef::new("simple-muted");
/// A card's outline.
pub const LINE: TokenRef<Color> = TokenRef::new("simple-line");

/// Simple's colors, each a token whose definition holds its dark variant.
#[derive(Clone, PartialEq, Debug)]
pub struct SimpleColors {
  /// [`ACCENT`].
  pub accent: Token<Color>,
  /// [`ON_ACCENT`].
  pub on_accent: Token<Color>,
  /// [`CARD`].
  pub card: Token<Color>,
  /// [`INK`].
  pub ink: Token<Color>,
  /// [`MUTED`].
  pub muted: Token<Color>,
  /// [`LINE`].
  pub line: Token<Color>,
}

impl Default for SimpleColors {
  fn default() -> Self {
    Self {
      accent: Token::new(ACCENT, Color::rgb(0x0a84ff)).when(Media::Dark, Color::rgb(0x409cff)),
      on_accent: Token::new(ON_ACCENT, Color::WHITE),
      card: Token::new(CARD, Color::WHITE).when(Media::Dark, Color::rgb(0x1c1c1e)),
      ink: Token::new(INK, Color::rgb(0x1d1d1f)).when(Media::Dark, Color::rgb(0xf5f5f7)),
      muted: Token::new(MUTED, Color::rgb(0x6e6e73)).when(Media::Dark, Color::rgb(0x98989d)),
      line: Token::new(LINE, Color::rgb(0xd2d2d7)).when(Media::Dark, Color::rgb(0x3a3a3c)),
    }
  }
}

impl TokenSet for SimpleColors {
  fn define(&self, definitions: &mut Definitions) {
    definitions
      .add(&self.accent)
      .add(&self.on_accent)
      .add(&self.card)
      .add(&self.ink)
      .add(&self.muted)
      .add(&self.line);
  }
}

/// A day of the proleptic Gregorian calendar, written `YYYY-MM-DD`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SimpleDate {
  /// The year.
  pub year: i32,
  /// The month, 1 to 12.
  pub month: u8,
  /// The day of the month, from 1.
  pub day: u8,
}

impl SimpleDate {
  /// The date, where `month` and `day` name a day of `year`.
  #[must_use]
  pub const fn new(year: i32, month: u8, day: u8) -> Option<Self> {
    if month == 0 || month > 12 || day == 0 || day > Self::days_in(year, month) {
      return None;
    }

    Some(Self { year, month, day })
  }

  /// How many days `month` of `year` has.
  #[must_use]
  pub const fn days_in(year: i32, month: u8) -> u8 {
    match month {
      2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
      2 => 28,
      4 | 6 | 9 | 11 => 30,
      _ => 31,
    }
  }

  /// The date `text` names as `YYYY-MM-DD`.
  #[must_use]
  pub fn parse(text: &str) -> Option<Self> {
    let mut parts = text.trim().splitn(3, '-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;

    Self::new(year, month, day)
  }

  /// The day of the week, 0 for Monday to 6 for Sunday.
  #[must_use]
  pub const fn weekday(self) -> u8 {
    const OFFSETS: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let year = if self.month < 3 {
      self.year - 1
    } else {
      self.year
    };
    let sunday_first = (year + year / 4 - year / 100
      + year / 400
      + OFFSETS[(self.month - 1) as usize]
      + self.day as i32)
      .rem_euclid(7);

    ((sunday_first + 6) % 7) as u8
  }

  /// Whether the date is a Saturday or a Sunday.
  #[must_use]
  pub const fn is_weekend(self) -> bool {
    self.weekday() >= 5
  }
}

impl fmt::Display for SimpleDate {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      formatter,
      "{:04}-{:02}-{:02}",
      self.year, self.month, self.day
    )
  }
}

#[cfg(test)]
mod tests {
  use super::SimpleDate;

  #[test]
  fn a_date_reads_and_writes_as_iso() {
    let date = SimpleDate::parse("2026-10-14").expect("a date");

    assert_eq!(
      date,
      SimpleDate {
        year: 2026,
        month: 10,
        day: 14
      }
    );
    assert_eq!(date.to_string(), "2026-10-14");
    assert_eq!(SimpleDate::parse("2026-02-29"), None);
    assert_eq!(
      SimpleDate::parse("2024-02-29").map(|date| date.day),
      Some(29)
    );
    assert_eq!(SimpleDate::parse("2026-13-01"), None);
  }

  #[test]
  fn weekdays_and_weekends() {
    let day = |text| SimpleDate::parse(text).expect("a date");

    assert_eq!(day("2026-10-14").weekday(), 2);
    assert!(day("2026-10-17").is_weekend());
    assert!(day("2026-10-18").is_weekend());
    assert!(!day("2026-10-19").is_weekend());
    assert_eq!(day("2000-01-01").weekday(), 5);
  }
}
