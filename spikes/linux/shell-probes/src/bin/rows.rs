//! `ROWS` rows (1000 unless set) in a scroll, each a box that answers hover and holds three
//! texts: what a frame costs with a thousand of them laid out, and with one restyled.

use skene_gui::prelude::*;
use skene_shell_probes::stateful;

#[skene_gui::main]
fn app() -> impl View {
  let count: usize = std::env::var("ROWS")
    .ok()
    .and_then(|rows| rows.parse().ok())
    .unwrap_or(1000);
  let mut column = Column::new().set_padding(8).set_spacing(2);

  for index in 0..count {
    column = column.add_child(
      Container::new()
        .set_padding(6)
        .set_corner_radius(4)
        .set_background(stateful())
        .set_child(
          Row::new()
            .set_spacing(12)
            .add_child(Text::new(format!("{index:04}")).set_color(Color::rgb(0x8e8e93)))
            .add_child(
              Flexible::new()
                .set_flexibility(1)
                .set_child(Text::new(format!("Item number {index}, a row of the list"))),
            )
            .add_child(Text::new(format!("{:.2}", index as f64 * 1.37))),
        ),
    );
  }

  Scroll::new(ScrollAxes::Vertical).set_child(column)
}
