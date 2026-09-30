//! The terminal's frame at scale: a scrolled column of sixteen thousand text rows, drawn
//! with nothing changed, after a pointer move, and resized, which restyles and lays out
//! every row.

#![cfg_attr(
  not(target_arch = "wasm32"),
  expect(
    missing_docs,
    reason = "criterion's macros generate the harness's functions"
  )
)]

#[cfg(not(target_arch = "wasm32"))]
mod frame {
  use criterion::Criterion;
  use skene_tui::{self as tui, prelude::*, testing};

  /// The rows the column holds.
  const ROWS: usize = 16_000;

  /// Mounts a screen-high scroll box over [`ROWS`] text rows.
  fn mount_rows() {
    let _ = tui::mount(|| {
      Container::new()
        .set_height(24)
        .set_overflow(Overflow::Auto)
        .set_child(
          Column::new().set_children(
            (0..ROWS)
              .map(|index| Text::new(format!("row {index}")).into_any())
              .collect::<Vec<_>>(),
          ),
        )
        .build()
    });
  }

  pub(crate) fn frames(criterion: &mut Criterion) {
    testing::with_platform(|| {
      mount_rows();

      let _ = testing::render_to_text(80, 24);

      criterion.bench_function("unchanged_frame_16k_rows", |bencher| {
        bencher.iter(|| testing::render_to_text(80, 24));
      });

      let mut row = 0;

      criterion.bench_function("pointer_move_frame_16k_rows", |bencher| {
        bencher.iter(|| {
          row = (row + 1) % 20;
          testing::trigger_mouse_move(1, row);

          testing::render_to_text(80, 24)
        });
      });

      let mut wide = false;

      criterion.bench_function("resized_frame_16k_rows", |bencher| {
        bencher.iter(|| {
          wide = !wide;

          testing::render_to_text(if wide { 81 } else { 80 }, 24)
        });
      });
    });
  }
}

#[cfg(not(target_arch = "wasm32"))]
criterion::criterion_group!(benches, frame::frames);
#[cfg(not(target_arch = "wasm32"))]
criterion::criterion_main!(benches);

#[cfg(target_arch = "wasm32")]
fn main() {}
