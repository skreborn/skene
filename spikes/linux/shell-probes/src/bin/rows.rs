//! `ROWS` rows (1000 unless set) in a scroll, each a box that answers hover and holds three
//! texts: what a frame costs with a thousand of them laid out, and with one restyled.
//!
//! - `CONTROL=checkbox|switch|slider|picker|field` puts that control in each row in place of its
//!   last text, bound to a signal of its own.
//! - `MODE=value` changes row 0's control (or its last text) every 400 ms, twelve times: a
//!   check, a flip, a step, a choice, a character typed.
//! - `MODE=layout` changes row 0's padding (a layout change), `MODE=paint` its background (a
//!   paint change), on the same clock.
//! - `MODE=full` changes the background behind the scroll on the same clock: a repaint of the
//!   whole window.
//! - `MODE=painted` shows one 200 by 100 painted box, red, and nothing else.
//! - `LAZY=1` puts the same rows in a `LazyColumn` in place of a scroll's `Column`: only those
//!   in view and a few beyond are built, whatever `ROWS` says (ten thousand, a hundred
//!   thousand).

use std::time::Duration;

use skene_gui::{AnyView, painted::shapes::Rect, prelude::*};
use skene_shell_probes::stateful;

/// Flips `flip` every 400 ms, `left` more times.
fn tick(flip: Signal<bool>, left: u32) {
  if left == 0 {
    return;
  }

  skene_gui::schedule(Duration::from_millis(400), move || {
    flip.set(!flip.get_untracked());
    tick(flip, left - 1);
  });
}

/// Row `index`'s last child: the control `control` names, its value following `flip` where it
/// is row 0 and the mode changes values.
fn last(index: usize, control: &str, flip: Option<Signal<bool>>) -> AnyView {
  let label = format!("Row {index}");

  match control {
    "checkbox" => {
      let checked = Signal::new(false);

      if let Some(flip) = flip {
        Effect::new(move || checked.set(flip.get()));
      }

      Checkbox::new(label).bind_checked(checked).into_any()
    }
    "switch" => {
      let on = Signal::new(false);

      if let Some(flip) = flip {
        Effect::new(move || on.set(flip.get()));
      }

      Toggle::new(label).bind_on(on).into_any()
    }
    "slider" => {
      let value = Signal::new(0.3_f32);

      if let Some(flip) = flip {
        Effect::new(move || value.set(if flip.get() { 0.4 } else { 0.3 }));
      }

      Slider::new(label)
        .set_step(0.1)
        .bind_value(value)
        .into_any()
    }
    "picker" => {
      let sizes = ["Small", "Medium", "Large"];
      let chosen = Signal::new(sizes[0]);

      if let Some(flip) = flip {
        Effect::new(move || chosen.set(sizes[usize::from(flip.get())]));
      }

      Picker::new(label)
        .bind_chosen(chosen)
        .set_options(sizes.map(|size| (size, size)))
        .into_any()
    }
    "field" => {
      let text = Signal::new(String::from("Ada"));

      if let Some(flip) = flip {
        Effect::new(move || text.set(if flip.get() { "Adab" } else { "Ada" }.to_owned()));
      }

      TextField::new(label).bind_text(text).into_any()
    }
    _ => match flip {
      Some(flip) => Text::derive(move || if flip.get() { "on" } else { "off" }).into_any(),
      None => Text::new(format!("{:.2}", index as f64 * 1.37)).into_any(),
    },
  }
}

#[skene_gui::main]
fn app() -> AnyView {
  let count: usize = std::env::var("ROWS")
    .ok()
    .and_then(|rows| rows.parse().ok())
    .unwrap_or(1000);
  let mode = std::env::var("MODE").unwrap_or_default();
  let control = std::env::var("CONTROL").unwrap_or_default();
  let flip = Signal::new(false);

  if mode == "painted" {
    return Column::new()
      .add_child(
        Painted::new(|canvas| {
          let (width, height) = (canvas.size().width, canvas.size().height);

          canvas.fill(&Rect::new(0., 0., width, height), Color::rgb(0xff0000));
        })
        .set_width(200)
        .set_height(100),
      )
      .into_any();
  }

  if !mode.is_empty() {
    tick(flip, 12);
  }

  let row = {
    let (mode, control) = (mode.clone(), control.clone());

    move |index: usize| {
      let first = index == 0;
      let row = match mode.as_str() {
        "layout" if first => Container::new()
          .derive_padding(move || if flip.get() { 9 } else { 6 })
          .set_background(stateful()),
        "paint" if first => Container::new().set_padding(6).derive_background(move || {
          if flip.get() {
            Color::rgb(0xfde68a)
          } else {
            Color::rgb(0xffffff)
          }
        }),
        _ => Container::new().set_padding(6).set_background(stateful()),
      };
      let changes = (first && mode == "value").then_some(flip);

      row.set_corner_radius(4).set_child(
        Row::new()
          .set_spacing(12)
          .set_item_alignment(AxisAlignment::Center)
          .add_child(Text::new(format!("{index:04}")).set_color(Color::rgb(0x8e8e93)))
          .add_child(
            Flexible::new()
              .set_flexibility(1)
              .set_child(Text::new(format!("Item number {index}, a row of the list"))),
          )
          .add_child(last(index, &control, changes)),
      )
    }
  };

  if std::env::var("LAZY").is_ok_and(|lazy| lazy == "1") {
    return LazyColumn::new()
      .set_padding(8)
      .set_spacing(2)
      .set_item_extent(ItemExtent::Estimated(29.))
      .derive_items(move || (0..count).collect::<Vec<_>>(), |index| *index, row)
      .into_any();
  }

  let column = (0..count).fold(
    Column::new().set_padding(8).set_spacing(2),
    |column, index| column.add_child(row(index)),
  );
  let scroll = Scroll::new(ScrollAxes::Vertical).set_child(column);

  if mode == "full" {
    return Container::new()
      .derive_background(move || {
        if flip.get() {
          Color::rgb(0xe5e7eb)
        } else {
          Color::rgb(0xf3f4f6)
        }
      })
      .set_child(scroll)
      .into_any();
  }

  scroll.into_any()
}
