//! Text sizing, in cells: a font size is a scale of the cell, drawn through the
//! text sizing protocol (OSC 66) where the terminal proves it and at one cell
//! elsewhere.
//!
//! A run's `set_font_size` is the shared type metric, in the terminal's unit: a size
//! of two is a heading two cells tall and two cells wide per character, a size below
//! one is a fraction of a cell, and the typography's `size` token sizes every run
//! beneath its theme. Layout counts the scaled cells, a paragraph sets its smaller
//! spans on the baseline of its tallest, and a terminal without the protocol lays
//! the same tree out at one cell, so nothing here costs anything there.

#![cfg(not(target_arch = "wasm32"))]

use skene_simple::terminal::prelude::*;
use skene_terminal::prelude::*;

/// The page: a title at three cells, sections at two, body text at one, and a
/// themed footer.
#[must_use]
pub fn app() -> impl View {
  Theme::new(SimplePalette::default()).set_child_builder(|| {
    Container::new()
      .set_overflow(Overflow::Auto)
      .set_color(SimplePalette::BACKGROUND)
      .set_child(
        Container::new().set_max_width(64).set_child(
          Column::new()
            .set_spacing(1)
            .set_padding(Inset::symmetric(2, 1))
            .set_children((title(), scales_card(), baseline_card(), footer())),
        ),
      )
  })
}

fn title() -> impl View {
  Column::new().set_children((
    Text::new("Text sizing")
      .set_font_size(3.)
      .set_bold(true)
      .set_color(SimplePalette::FOREGROUND),
    Text::new("A size is a scale of the cell; the terminal draws it where it can.")
      .set_color(SimplePalette::MUTED_FOREGROUND),
  ))
}

/// One run per whole scale, and one at a fraction.
fn scales_card() -> impl View {
  SimpleCard::new().set_spacing(1).set_children((
    Text::new("Scales")
      .set_font_size(2.)
      .set_bold(true)
      .set_color(SimplePalette::FOREGROUND),
    Text::new("Two to seven cells; below one, a fraction of a cell at its foot.")
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Row::new().set_spacing(2).set_children((
      Text::new("2")
        .set_font_size(2.)
        .set_color(SimplePalette::FOREGROUND),
      Text::new("3")
        .set_font_size(3.)
        .set_color(SimplePalette::FOREGROUND),
      Text::new("4")
        .set_font_size(4.)
        .set_color(SimplePalette::FOREGROUND),
      Text::new("half")
        .set_font_size(0.5)
        .set_color(SimplePalette::FOREGROUND),
    )),
  ))
}

/// A paragraph mixing sizes: the smaller spans sit on the tallest span's baseline.
fn baseline_card() -> impl View {
  SimpleCard::new().set_spacing(1).set_children((
    Text::new("Baseline")
      .set_font_size(2.)
      .set_bold(true)
      .set_color(SimplePalette::FOREGROUND),
    Text::new("A paragraph's line is as tall as its tallest span.")
      .set_color(SimplePalette::MUTED_FOREGROUND),
    RichText::new().set_spans([
      Span::new("Big ")
        .set_font_size(3.)
        .set_color(SimplePalette::FOREGROUND),
      Span::new("mid ")
        .set_font_size(2.)
        .set_color(SimplePalette::FOREGROUND),
      Span::new("and small, ").set_color(SimplePalette::FOREGROUND),
      Span::new("selectable")
        .set_selectable(true)
        .set_color(SimplePalette::FOREGROUND),
      Span::new(" as one flow.").set_color(SimplePalette::FOREGROUND),
    ]),
  ))
}

/// A theme whose typography sizes everything beneath it.
fn footer() -> impl View {
  Theme::new(Typography::default().set_size(2.))
    .set_child_builder(|| Text::new("Themed at two.").set_color(SimplePalette::MUTED_FOREGROUND))
}
