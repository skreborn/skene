//! Enhancement: a view in the form the page can carry, upgraded once the bundle
//! arrives.
//!
//! A server render carries every fallback as markup, so the page reads without a
//! script: the clock is a caption, the picture a description. The bundle hydrates the
//! fallbacks in place and then builds each enhancement over them: a clock the bundle
//! keeps, and a picture painted through vello. Where the bundle never arrives, the
//! fallbacks stand.

use std::time::Duration;

use skene_simple::web::prelude::*;
use skene_web::{ActivePlatform, HasChild, core::Platform, kurbo::Rect, prelude::*};

#[main]
fn app() -> impl View {
  Theme::new(SimplePalette::default()).set_child_builder(|| {
    Container::new()
      .set_color(SimplePalette::BACKGROUND)
      .set_min_height(Dimension::Fraction(1.))
      .set_child(
        Column::new()
          .set_item_alignment(ItemAlignment::Center)
          .set_padding(Inset::symmetric(24., 48.))
          .add_child(
            Container::new().set_max_width(460.).set_child(
              Column::new().set_spacing(24.).set_children((
                heading(),
                clock_card(),
                picture_card(),
              )),
            ),
          ),
      )
  })
}

fn heading() -> impl View {
  Column::new().set_spacing(6.).set_children((
    Text::new("ENHANCE")
      .set_font_size(13.)
      .set_font_weight(500.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Text::new("A view declares its baseline and its enhancement.").set_font_size(15.),
  ))
}

/// A clock the bundle keeps, over a caption the server carries.
fn clock_card() -> impl View {
  SimpleCard::new().set_spacing(12.).set_children((
    Text::new("Clock").set_font_weight(600.),
    Text::new("The server renders the caption; the bundle counts from its arrival.")
      .set_font_size(13.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Enhance::new(skene_web::media::hydrated())
      .set_fallback(|| Text::new("Waiting for the bundle."))
      .set_child_builder(|| {
        let seconds = Signal::new(0u32);

        tick(seconds);

        Text::derive(move || format!("{} s since the bundle arrived.", seconds.get()))
      }),
  ))
}

/// Advances `seconds` once a second on the platform's clock.
fn tick(seconds: Signal<u32>) {
  let _ = ActivePlatform::schedule(
    Duration::from_secs(1),
    Box::new(move || {
      seconds.update(|count| *count += 1);
      tick(seconds);
    }),
  );
}

/// A painted stripe, over a description the server carries.
fn picture_card() -> impl View {
  SimpleCard::new().set_spacing(12.).set_children((
    Text::new("Picture").set_font_weight(600.),
    Text::new("The server renders the description; the bundle paints the stripes.")
      .set_font_size(13.)
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
    .set_height(Dimension::Length(48.))
    .set_fallback(|| Text::new("Eight blue stripes, lightening from left to right.")),
  ))
}
