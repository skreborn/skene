//! Text drawn at sizes that keep changing, on the real platform: the glyph caches stay within
//! their budgets however many sizes a transition draws.

use skene_gui::{
  Curve, Transition, keys,
  prelude::*,
  testing::{self, Driver as _, Role, Screen},
};

/// The process's resident memory, in kilobytes, as the kernel reports it.
fn resident() -> u64 {
  std::fs::read_to_string("/proc/self/status")
    .ok()
    .and_then(|status| {
      status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|kb| kb.parse().ok())
    })
    .unwrap_or(0)
}

/// A button that flips a text's size between `small` and `large` under a linear transition
/// of `duration` milliseconds, and the text.
fn growing(small: f32, large: f32, duration: u32) -> impl View {
  let big = Signal::new(false);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Button::new("Grow").handle_activate(move || big.update(|big| *big = !*big)))
    .add_child(
      Container::new()
        .derive_font_size(move || if big.get() { large } else { small })
        .add_transition(Transition::new(keys::FONT_SIZE, duration).set_curve(Curve::Linear))
        .set_child(Text::new(
          "The quick brown fox jumps over the lazy dog 0123456789 QWERTYUIOPASDFGHJKLZXCVBNM",
        )),
    )
}

/// Flips the text's size `times` times, each flip's transition run out: the memory the process
/// holds before the first and after each.
fn flips(screen: &Screen, times: u32, duration: u32) -> Vec<u64> {
  let grow = screen.find_by_role(Role::Button, "Grow");
  let mut held = vec![resident()];

  for _ in 0..times {
    grow.click();
    screen
      .driver()
      .wait(std::time::Duration::from_millis(u64::from(duration) + 100));
    held.push(resident());
  }

  held
}

#[skene_gui::test]
fn a_font_size_transition_keeps_the_glyph_caches_within_their_budgets() {
  let screen = testing::render(|| growing(200., 400., 1500));
  let held = flips(&screen, 6, 1500);
  let (first, last) = (held[1], held[held.len() - 1]);

  assert!(
    last < first + 200_000,
    "six transitions between 200 and 400 pixels grow the process by less than 200 MB after the first: {held:?} kB"
  );
}
