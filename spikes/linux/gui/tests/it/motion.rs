//! Motion on the real platform: a transition of a property that lays out runs on the frame
//! clock, through in-betweens, as CSS runs it.

use skene_gui::{
  Curve, Transition, keys,
  prelude::*,
  testing::{self, Driver as _, Role},
};

/// A black box 100 pixels wide under a 600 ms transition of its width, and a button that
/// makes it 300.
fn widening() -> impl View {
  let wide = Signal::new(false);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .set_item_alignment(AxisAlignment::Start)
    .add_child(
      Container::new()
        .set_label("Widening")
        .set_height(40)
        .set_background(Color::BLACK)
        .derive_width(move || if wide.get() { 300 } else { 100 })
        .add_transition(Transition::new(keys::WIDTH, 600).set_curve(Curve::Linear)),
    )
    .add_child(Button::new("Widen").handle_activate(move || wide.set(true)))
}

#[skene_gui::test]
fn a_width_transition_runs_through_in_betweens() {
  let screen = testing::render(widening);
  let at = screen.find_by_role(Role::Region, "Widening").read().bounds;
  let y = (at.y + at.height / 2.).trunc();

  screen.driver().set_recording(true);
  screen.find_by_role(Role::Button, "Widen").click();

  let frames = screen.driver().take_frames();

  screen.driver().set_recording(false);

  let widths: Vec<u32> = frames
    .iter()
    .map(|frame| {
      let mut width = 0;

      #[expect(clippy::cast_precision_loss, reason = "window pixels")]
      while frame.shot.color_at(at.x + width as f32 + 0.5, y) == 0x0000_0000 && width < 400 {
        width += 1;
      }

      width
    })
    .collect();
  let between = widths
    .iter()
    .filter(|width| **width > 105 && **width < 295)
    .count();

  assert!(
    (screen
      .find_by_role(Role::Region, "Widening")
      .read()
      .bounds
      .width
      - 300.)
      .abs()
      < 0.5,
    "it ends 300 wide"
  );
  assert!(
    between >= 3,
    "the width moves through in-betweens: {widths:?}"
  );
  assert!(
    widths.windows(2).all(|pair| pair[0] <= pair[1] + 1),
    "each frame wider than the one before: {widths:?}"
  );
}
