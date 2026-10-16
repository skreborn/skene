//! # skene-simple-gui
//!
//! The Simple component library on the pixel platforms, written against skene-gui's public API
//! alone, as a third party's library is.
//!
//! - [`SimplePalette`]: Simple's colors (from `skene-simple-core`) and its lengths in pixels.
//!   Beneath its theme the platform's accent ([`SystemColors`]) is Simple's, so a plain
//!   built-in control takes Simple's accent too.
//! - [`SimpleCard`], [`SimpleToggle`], [`SimpleCheckbox`] and [`SimpleButton`] are presets:
//!   each returns the built-in view styled by the palette (a part restyled from the
//!   built-in's own default, `Checkbox::default_face()`), so every member of the built-in
//!   applies and the library forwards no setter.
//! - [`SimpleDatePicker`] is a view of the library's own, a month's calendar composed of the
//!   shared views (Skene draws every control on Linux, so there is no toolkit calendar to
//!   adopt), controlled as the built-in controls are.
//!
//! ```
//! use skene_gui::prelude::*;
//! use skene_simple_gui::prelude::*;
//!
//! fn settings() -> impl View {
//!   let sync = Signal::new(true);
//!   let due = Signal::new("2026-10-14".to_owned());
//!
//!   Theme::new(SimplePalette::default()).set_child(
//!     SimpleCard::new().set_child(
//!       Column::new()
//!         .set_spacing(SimplePalette::GAP)
//!         .add_child(SimpleToggle::new("Sync").bind_on(sync))
//!         .add_child(SimpleDatePicker::new("Due").bind_date(due))
//!         .add_child(SimpleButton::new("Save")),
//!     ),
//!   )
//! }
//! ```

use std::rc::Rc;

use skene_gui::{
  AxisAlignment, Border, Button, Checkbox, Color, Column, Container, Cx, Definitions, Derived,
  Flexible, FontWeight, Grid, GridTrack, Node, Row, Signal, State, SystemColors, Text, Toggle,
  Token, TokenRef, TokenSet, Value, View, untrack,
};
pub use skene_simple_core::{SimpleColors, SimpleDate};

/// Simple's palette on the pixel platforms: its colors, a corner radius and a gap.
#[derive(Clone, PartialEq, Debug)]
pub struct SimplePalette {
  /// The colors, light and dark.
  pub colors: SimpleColors,
  /// [`SimplePalette::RADIUS`]: 12 pixels.
  pub radius: Token<f32>,
  /// [`SimplePalette::GAP`]: 16 pixels.
  pub gap: Token<f32>,
}

impl SimplePalette {
  /// The accent: a control's on state, a primary button's fill.
  pub const ACCENT: TokenRef<Color> = skene_simple_core::ACCENT;
  /// The accent under the pointer.
  pub const ACCENT_HOVER: TokenRef<Color> = skene_simple_core::ACCENT_HOVER;
  /// The accent while pressed.
  pub const ACCENT_PRESSED: TokenRef<Color> = skene_simple_core::ACCENT_PRESSED;
  /// What sits on the accent.
  pub const ON_ACCENT: TokenRef<Color> = skene_simple_core::ON_ACCENT;
  /// A card's fill.
  pub const CARD: TokenRef<Color> = skene_simple_core::CARD;
  /// The text's color.
  pub const INK: TokenRef<Color> = skene_simple_core::INK;
  /// Secondary text.
  pub const MUTED: TokenRef<Color> = skene_simple_core::MUTED;
  /// A card's outline.
  pub const LINE: TokenRef<Color> = skene_simple_core::LINE;
  /// A card's corner radius, in pixels.
  pub const RADIUS: TokenRef<f32> = TokenRef::new("simple-radius");
  /// The space inside a card and between its rows, in pixels.
  pub const GAP: TokenRef<f32> = TokenRef::new("simple-gap");
}

impl Default for SimplePalette {
  fn default() -> Self {
    Self {
      colors: SimpleColors::default(),
      radius: Token::new(Self::RADIUS, 12),
      gap: Token::new(Self::GAP, 16),
    }
  }
}

impl TokenSet for SimplePalette {
  fn define(&self, definitions: &mut Definitions) {
    self.colors.define(definitions);
    definitions.add(&self.radius).add(&self.gap);

    // The platform's accent beneath a Simple theme is Simple's.
    for (system, simple) in [
      (SystemColors::ACCENT, Self::ACCENT),
      (SystemColors::ACCENT_HOVER, Self::ACCENT_HOVER),
      (SystemColors::ACCENT_PRESSED, Self::ACCENT_PRESSED),
      (SystemColors::ON_ACCENT, Self::ON_ACCENT),
    ] {
      definitions.add(&Token::new(system, simple));
    }
  }
}

/// Simple's card: a box in the palette's card color, rounded, outlined and padded by its gap.
pub struct SimpleCard;

impl SimpleCard {
  /// A card. It is a [`Container`], so every member of one applies, its child last.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new() -> Container {
    Container::new()
      .set_padding(SimplePalette::GAP)
      .set_background(SimplePalette::CARD)
      .set_corner_radius(SimplePalette::RADIUS)
      .set_border(Border::new(1))
      .set_border_color(SimplePalette::LINE)
      .set_color(SimplePalette::INK)
  }
}

/// Simple's toggle: the built-in switch, its track in Simple's colors, its caption a gap from
/// it.
pub struct SimpleToggle;

impl SimpleToggle {
  /// A toggle labelled `label`. It is a [`Toggle`], so every member of one applies.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new(label: impl Into<String>) -> Toggle {
    Toggle::new(label)
      .set_spacing(SimplePalette::GAP)
      .set_track(
        Toggle::default_track()
          .set_background(
            Value::from(SimplePalette::LINE).when(State::Checked, SimplePalette::ACCENT),
          )
          .set_ring_color(SimplePalette::ACCENT),
      )
  }
}

/// Simple's checkbox: the built-in checkbox, its face in Simple's colors.
pub struct SimpleCheckbox;

impl SimpleCheckbox {
  /// A checkbox labelled `label`. It is a [`Checkbox`], so every member of one applies.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new(label: impl Into<String>) -> Checkbox {
    Checkbox::new(label).set_face(
      Checkbox::default_face()
        .set_background(
          Value::from(SimplePalette::CARD)
            .when(State::Checked, SimplePalette::ACCENT)
            .when(State::Indeterminate, SimplePalette::ACCENT),
        )
        .set_border_color(
          Value::from(SimplePalette::LINE).when(State::Checked, SimplePalette::ACCENT),
        )
        .set_color(SimplePalette::ON_ACCENT)
        .set_ring_color(SimplePalette::ACCENT),
    )
  }
}

/// Simple's primary button: the screen's primary action, filled with Simple's accent and
/// rounded by its radius.
pub struct SimpleButton;

impl SimpleButton {
  /// A button labelled `label`. It is a [`Button`], so every member of one applies.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new(label: impl Into<String>) -> Button {
    Self::style(Button::new(label))
  }

  /// A button labelled what `label` returns, kept current. It is a [`Button`].
  pub fn derive<S: Into<String>>(label: impl Fn() -> S + 'static) -> Button {
    Self::style(Button::derive(label))
  }

  /// Simple's fill for a button: its accent and the accent's hover and pressed variants.
  pub fn fill() -> Value<Color> {
    Value::from(SimplePalette::ACCENT)
      .when(State::Hovered, SimplePalette::ACCENT_HOVER)
      .when(State::Pressed, SimplePalette::ACCENT_PRESSED)
  }

  /// `button` in Simple's style.
  fn style(button: Button) -> Button {
    button
      .set_prominent(true)
      .set_background(Self::fill())
      .set_color(SimplePalette::ON_ACCENT)
      .set_corner_radius(SimplePalette::RADIUS)
      .set_ring_color(SimplePalette::ACCENT)
  }
}

/// A day, picked on a month's calendar: the month's name between its previous and next
/// month's buttons, and a button per day, the date shown drawn prominent.
///
/// It is written with skene-gui's shared views alone, so it is the same on every platform
/// that draws Skene's controls. It is controlled: it shows what
/// [`SimpleDatePicker::derive_date`] returns, reports the day the reader picks to
/// [`SimpleDatePicker::handle_change`], and shows the truth, so a day its handler refuses is
/// never shown picked. Dates are `YYYY-MM-DD` ([`SimpleDate`]).
#[must_use]
pub struct SimpleDatePicker {
  name: Rc<str>,
  date: Rc<dyn Fn() -> String>,
  change: Option<Rc<dyn Fn(String)>>,
}

impl SimpleDatePicker {
  /// A picker a reader knows as `name`, showing 2026-01-01 until a date is given.
  pub fn new(name: impl Into<Rc<str>>) -> Self {
    Self {
      name: name.into(),
      date: Rc::new(|| "2026-01-01".to_owned()),
      change: None,
    }
  }

  /// The date shown, `YYYY-MM-DD`.
  pub fn set_date(self, date: impl Into<String>) -> Self {
    let date = date.into();

    self.derive_date(move || date.clone())
  }

  /// The date shown, `YYYY-MM-DD`, kept current.
  pub fn derive_date(mut self, date: impl Fn() -> String + 'static) -> Self {
    self.date = Rc::new(date);

    self
  }

  /// Runs `change` with the day the reader picked, `YYYY-MM-DD`.
  pub fn handle_change(mut self, change: impl Fn(String) + 'static) -> Self {
    self.change = Some(Rc::new(change));

    self
  }

  /// Shows `date` and writes each pick into it.
  pub fn bind_date(self, date: Signal<String>) -> Self {
    self
      .derive_date(move || date.get())
      .handle_change(move |picked| date.set(picked))
  }
}

/// The months' names.
const MONTHS: [&str; 12] = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

/// The weekdays' short names, Monday first.
const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

impl SimpleDatePicker {
  /// The month `(year, month)` moved by `months`.
  const fn moved((year, month): (i32, u8), months: i32) -> (i32, u8) {
    let index = year * 12 + month as i32 - 1 + months;

    (index.div_euclid(12), (index.rem_euclid(12) + 1) as u8)
  }

  /// The day grid of `(year, month)`: a blank per weekday before the first, then a button per
  /// day, `date` the one drawn prominent.
  fn days(
    (year, month): (i32, u8),
    date: &Rc<dyn Fn() -> String>,
    change: Option<&Rc<dyn Fn(String)>>,
  ) -> Grid {
    let first = SimpleDate::new(year, month, 1).map_or(0, SimpleDate::weekday);
    let mut grid = Grid::new()
      .set_spacing(4)
      .set_columns([GridTrack::Flex(1.); 7]);

    for weekday in WEEKDAYS {
      grid = grid.add_child(Text::new(weekday).set_color(SimplePalette::MUTED));
    }

    for _ in 0..first {
      grid = grid.add_child(Container::new());
    }

    for day in 1..=SimpleDate::days_in(year, month) {
      let this = SimpleDate { year, month, day }.to_string();
      let chosen = {
        let date = Rc::clone(date);

        Rc::new(move || date() == this)
      };
      let change = change.cloned();
      let picked = SimpleDate { year, month, day }.to_string();
      let (prominent, filled, inked) = (Rc::clone(&chosen), Rc::clone(&chosen), chosen);

      // The day shown is Simple's primary button; the others are plain ones.
      grid = grid.add_child(
        SimpleButton::new(day.to_string())
          .derive_prominent(move || prominent())
          .derive_background(move || {
            if filled() {
              SimpleButton::fill()
            } else {
              Value::from(SystemColors::CONTROL)
                .when(State::Hovered, SystemColors::CONTROL_HOVER)
                .when(State::Pressed, SystemColors::CONTROL_PRESSED)
            }
          })
          .derive_color(move || {
            if inked() {
              SimplePalette::ON_ACCENT
            } else {
              SystemColors::INK
            }
          })
          .handle_activate(move || {
            if let Some(change) = &change {
              change(picked.clone());
            }
          }),
      );
    }

    grid
  }
}

impl View for SimpleDatePicker {
  fn build(self, cx: &mut Cx) -> Node {
    let first = untrack(|| (self.date)());
    let start = SimpleDate::parse(&first).map_or((2026, 1), |date| (date.year, date.month));
    let shown = Signal::new(start);
    let (date, change) = (self.date, self.change);

    Container::new()
      .set_label(self.name)
      .set_child(
        Column::new()
          .set_spacing(8)
          .add_child(
            Row::new()
              .set_spacing(8)
              .set_item_alignment(AxisAlignment::Center)
              .add_child(
                Button::new("Previous month")
                  .handle_activate(move || shown.update(|month| *month = Self::moved(*month, -1))),
              )
              .add_child(
                Flexible::new().set_flexibility(1).set_child(
                  Text::derive(move || {
                    let (year, month) = shown.get();

                    format!("{} {year}", MONTHS[usize::from(month - 1)])
                  })
                  .set_font_weight(FontWeight::SEMIBOLD),
                ),
              )
              .add_child(
                Button::new("Next month")
                  .handle_activate(move || shown.update(|month| *month = Self::moved(*month, 1))),
              ),
          )
          .add_child(Derived::new(move || {
            Self::days(shown.get(), &date, change.as_ref())
          })),
      )
      .build(cx)
  }
}

/// What a Simple app imports.
pub mod prelude {
  pub use crate::{
    SimpleButton, SimpleCard, SimpleCheckbox, SimpleDate, SimpleDatePicker, SimplePalette,
    SimpleToggle,
  };
}
