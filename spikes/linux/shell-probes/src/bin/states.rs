//! Three activatable rows, each holding a switch: hovering, pressing and focusing a row
//! restyles it through the states the realization reports; a click or Enter on a row counts,
//! a click on its switch flips the switch alone.

use skene_gui::prelude::*;
use skene_shell_probes::stateful;

fn row(index: usize, clicks: Signal<u32>, last: Signal<String>) -> impl View {
  let on = Signal::new(false);

  Container::new()
    .set_padding(12)
    .set_corner_radius(8)
    .set_background(stateful())
    .handle_click(move || {
      clicks.update(|clicks| *clicks += 1);
      last.set(format!("row {index}"));
    })
    .set_child(
      Row::new()
        .set_spacing(12)
        .set_item_alignment(AxisAlignment::Center)
        .add_child(
          Flexible::new()
            .set_flexibility(1)
            .set_child(Text::new(format!("Row {index}"))),
        )
        .add_child(Toggle::new(format!("Switch {index}")).bind_on(on)),
    )
}

#[skene_gui::main]
fn app() -> impl View {
  let clicks = Signal::new(0u32);
  let last = Signal::new(String::from("none"));

  Container::new()
    .set_background(Color::rgb(0xf2f2f7))
    .set_padding(16)
    .set_alignment(Alignment::new(AxisAlignment::Stretch, AxisAlignment::Start))
    .set_child(
      Column::new()
        .set_spacing(8)
        .add_child(Text::derive(move || {
          format!("Clicks: {} (last {})", clicks.get(), last.get())
        }))
        .add_child(row(1, clicks, last))
        .add_child(row(2, clicks, last))
        .add_child(row(3, clicks, last)),
    )
}
