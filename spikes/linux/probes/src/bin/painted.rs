//! A painted box on real GTK: a vello scene rendered by vello_cpu into a GdkMemoryTexture in
//! the widget's snapshot, in device pixels, redrawn when what it read changes or its size does,
//! and described for a reader. `painted scale` runs under `GDK_SCALE=2`.

use std::{cell::Cell, rc::Rc};

use probes::{atspi, hex, line, near, result, run, settle, show, widget};
use skene_gui::{
  painted::shapes::{Rect, Shape},
  prelude::*,
};

fn body() {
  let scaled = std::env::args().nth(1).as_deref() == Some("scale");
  let left = Signal::new(Color::rgb(0xff0000));
  let paints = Rc::new(Cell::new(0));
  let widths = Rc::new(Cell::new(0.));
  let [chart, decoration] = [NodeHandle::new(), NodeHandle::new()];
  let shown = {
    let (paints, widths) = (Rc::clone(&paints), Rc::clone(&widths));

    show(400, -1, move || {
      Column::new()
        .add_child(
          Painted::new(move |canvas| {
            let (width, height) = (f64::from(canvas.width()), f64::from(canvas.height()));

            paints.set(paints.get() + 1);
            widths.set(width);
            canvas.fill(&Rect::new(0., 0., width / 2., height), left.get());
            canvas.fill(
              &Rect::new(width / 2., 0., width, height).to_path(0.1),
              Color::rgb(0x0000ff),
            );
          })
          .set_width(Dimension::Fraction(0.5))
          .set_height(50)
          .set_description("Sales by half")
          .set_handle(&chart),
        )
        .add_child(
          Painted::new(|canvas| canvas.fill(&Rect::new(0., 0., 10., 10.), Color::BLACK))
            .set_width(10)
            .set_height(10)
            .set_handle(&decoration),
        )
    })
  };
  let (red, blue) = (
    shown.pixel_in(&chart, 50., 25.),
    shown.pixel_in(&chart, 150., 25.),
  );
  let first_paints = paints.get();

  result(
    "a canvas drawn by vello_cpu shows in the widget",
    near(red, 0xff0000) && near(blue, 0x0000ff),
    format!("left {} right {}", hex(red), hex(blue)),
  );

  let device = skene_gui::__private::painted(chart.node().expect("built"));
  let expected = if scaled {
    (400, 100, 2.)
  } else {
    (200, 50, 1.)
  };

  result(
    "its texture is in device pixels",
    device == Some(expected),
    format!("{device:?} (box 200x50)"),
  );

  left.set(Color::rgb(0x00ff00));
  settle();

  let green = shown.pixel_in(&chart, 50., 25.);

  result(
    "a signal it read repaints it, once",
    near(green, 0x00ff00) && paints.get() == first_paints + 1,
    format!(
      "left {}, paints {first_paints} -> {}",
      hex(green),
      paints.get()
    ),
  );

  shown.resize(600, 200);

  let (wide, right) = (widths.get(), shown.pixel_in(&chart, 290., 25.));

  result(
    "a new size repaints it at that size",
    wide == 300. && near(right, 0x0000ff),
    format!("canvas width {wide}, pixel at 290 {}", hex(right)),
  );

  let dump = atspi();

  for line in &dump {
    println!("ATSPI {line}");
  }
  let image = line(&dump, &["image", "\"Sales by half\""])
    .unwrap_or("(none)")
    .to_owned();
  let images = dump.iter().filter(|line| line.starts_with("image")).count();

  result(
    "a described canvas is an image to a reader; an undescribed one is skipped",
    image != "(none)" && images == 1,
    format!("{image}; {images} image in the tree"),
  );

  let _ = widget(&decoration);

  shown.screenshot(if scaled {
    "results/painted-scale.png"
  } else {
    "results/painted.png"
  });
  shown.close();
}

fn main() -> std::process::ExitCode {
  run("painted", body)
}
