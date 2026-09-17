//! Enhancement, in cells: a view in the form the terminal can carry.
//!
//! An enhancement waits on a signal of the app's own: the clock's caption stands until
//! the first tick, then the clock counts. A painted box waits on the terminal: where
//! the kitty graphics protocol is proven the stripes are painted over the box's cells,
//! and elsewhere its fallback stands, so the picture is its description.

#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use skene_simple::tui::prelude::*;
use skene_tui::{ActivePlatform, core::Platform, kurbo::Rect, prelude::*};

/// The page: a clock over its caption, and a picture over its description.
#[must_use]
pub fn app() -> impl IntoView {
  Theme::new(SimplePalette::default()).set_child_builder(|| {
    Container::new()
      .set_overflow(Overflow::Auto)
      .set_color(SimplePalette::BACKGROUND)
      .set_child(
        Column::new()
          .set_item_alignment(ItemAlignment::Center)
          .set_child(
            Container::new().set_max_width(56).set_child(
              Column::new()
                .set_spacing(1)
                .set_padding(Inset::symmetric(2, 1))
                .set_children((heading(), clock_card(), picture_card())),
            ),
          ),
      )
  })
}

fn heading() -> impl IntoView {
  Column::new().set_children((
    Text::new("ENHANCE")
      .set_bold(true)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Text::new("A view declares its baseline and its enhancement.")
      .set_color(SimplePalette::FOREGROUND),
  ))
}

/// A clock the loop keeps, over the caption standing until its first tick.
fn clock_card() -> impl IntoView {
  let ticked = Signal::new(false);
  let seconds = Signal::new(0u32);

  tick(ticked, seconds);

  SimpleCard::new().set_spacing(1).set_children((
    Text::new("Clock")
      .set_bold(true)
      .set_color(SimplePalette::FOREGROUND),
    Text::new("The caption stands until the clock's first tick.")
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Enhance::new(ticked.read_only())
      .set_fallback(|| {
        Text::new("Waiting for the first tick.").set_color(SimplePalette::FOREGROUND)
      })
      .set_child_builder(move || {
        Text::derive(move || format!("{} s since the first tick.", seconds.get()))
          .set_color(SimplePalette::FOREGROUND)
      }),
  ))
}

/// Marks the first tick, then advances `seconds` once a second on the loop's clock.
fn tick(ticked: Signal<bool>, seconds: Signal<u32>) {
  let _ = ActivePlatform::default().schedule(
    Duration::from_secs(1),
    Box::new(move || {
      if ticked.get_untracked() {
        seconds.update(|count| *count += 1);
      }
      else {
        ticked.set(true);
      }

      tick(ticked, seconds);
    }),
  );
}

/// A picture painted where the terminal draws pixels, over the description standing
/// elsewhere.
fn picture_card() -> impl IntoView {
  SimpleCard::new().set_spacing(1).set_children((
    Text::new("Picture")
      .set_bold(true)
      .set_color(SimplePalette::FOREGROUND),
    Text::new("The description stands until the terminal proves it draws pixels.")
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Painted::new(|canvas, size| {
      let width = f64::from(size.width);
      let height = f64::from(size.height);

      for stripe in 0..8u8 {
        let shade = 0.35 + 0.08 * f32::from(stripe);
        let left = width * f64::from(stripe) / 8.;

        canvas.fill(
          &Rect::new(left, 0., left + width / 8., height),
          Color::srgb(0.2, shade, 0.9),
        );
      }
    })
    .set_height(3)
    .set_fallback(|| {
      Text::new("Eight blue stripes, lightening from left to right.")
        .set_color(SimplePalette::FOREGROUND)
    }),
  ))
}
