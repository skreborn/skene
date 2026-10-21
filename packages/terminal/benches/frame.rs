//! The terminal's frame at scale: a scrolled column of sixteen thousand text rows, drawn
//! with nothing changed, after a pointer move, resized, which restyles and lays out
//! every row, with a keyed row added or removed, and mounted afresh.

#[cfg(not(target_arch = "wasm32"))]
mod frame {
  use criterion::Criterion;
  use skene_terminal::{self as terminal, prelude::*, testing};

  /// The rows the column holds.
  const ROWS: usize = 16_000;

  /// Mounts a screen-high scroll box over [`ROWS`] text rows.
  fn mount_rows() {
    let _ = terminal::mount(|| {
      Container::new()
        .set_height(24)
        .set_overflow(Overflow::ScrollIfNeeded)
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

  /// Mounts a screen-high scroll box over the keyed rows `rows` names.
  fn mount_keyed_rows(rows: Signal<Vec<usize>>) {
    let _ = terminal::mount(move || {
      Container::new()
        .set_height(24)
        .set_overflow(Overflow::ScrollIfNeeded)
        .set_child(Column::new().derive_children(
          move || rows.get(),
          |row| *row,
          |row| Text::new(format!("row {row}")),
        ))
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

    testing::with_platform(|| {
      let rows = Signal::new((0..ROWS).collect::<Vec<_>>());

      mount_keyed_rows(rows);

      let _ = testing::render_to_text(80, 24);
      let mut appended = false;

      criterion.bench_function("keyed_row_toggled_frame_16k_rows", |bencher| {
        bencher.iter(|| {
          appended = !appended;
          rows.update(|rows| {
            if appended {
              rows.push(ROWS);
            }
            else {
              rows.pop();
            }
          });

          testing::render_to_text(80, 24)
        });
      });
    });

    let mut group = criterion.benchmark_group("mount");

    group.sample_size(10);
    group.bench_function("mounted_first_frame_16k_rows", |bencher| {
      bencher.iter(|| {
        testing::with_platform(|| {
          mount_rows();

          testing::render_to_text(80, 24)
        })
      });
    });
    group.finish();
  }
}

#[cfg(not(target_arch = "wasm32"))]
criterion::criterion_group!(benches, frame::frames);
#[cfg(not(target_arch = "wasm32"))]
criterion::criterion_main!(benches);

#[cfg(target_arch = "wasm32")]
fn main() {}
