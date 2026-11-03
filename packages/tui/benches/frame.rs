//! The terminal's frame at scale: a scrolled column of sixteen thousand text rows, drawn
//! with nothing changed, after a pointer move, resized, which restyles and lays out
//! every row, with a keyed row added or removed, and mounted afresh; a scrolled paragraph
//! of a thousand inline boxes among its words, resized; and a scrolled column of a
//! thousand rows whose boxes and runs are all in motion, a frame later.

#[cfg(skene_platform = "terminal")]
mod frame {
  use std::time::Duration;

  use criterion::Criterion;
  use skene_foundation::{BoxKeyframe, RunKeyframe};
  use skene_tui::{
    Color,
    platform::terminal::{Terminal, mount},
    prelude::*,
    testing,
  };

  /// The rows the column holds.
  const ROWS: usize = 16_000;

  /// The inline boxes the paragraph holds, a word before each.
  const INLINE_BOXES: usize = 1_000;

  /// The rows in motion the column holds.
  const MOVING_ROWS: usize = 1_000;

  /// The color a box's animation starts from and a run's ends at.
  fn red() -> Color {
    Color::srgb(1., 0., 0.)
  }

  /// The color a box's animation ends at and a run's starts from.
  fn blue() -> Color {
    Color::srgb(0., 0., 1.)
  }

  /// A box keyframe of one background color.
  fn frame(color: Color) -> BoxKeyframe<Terminal> {
    BoxKeyframe::<Terminal> {
      background: Some(color.into()),
      ..Default::default()
    }
  }

  /// A run keyframe of one text color.
  fn ink_frame(color: Color) -> RunKeyframe<Terminal> {
    RunKeyframe::<Terminal> {
      color: Some(color.into()),
      ..Default::default()
    }
  }

  /// Mounts a screen-high scroll box over [`ROWS`] text rows.
  fn mount_rows() {
    let _ = mount(|| {
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
    let _ = mount(move || {
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

  /// Mounts a screen-high scroll box over a paragraph of [`INLINE_BOXES`] inline boxes,
  /// each after a word.
  fn mount_inline_boxes() {
    let _ = mount(|| {
      Container::new()
        .set_height(24)
        .set_overflow(Overflow::ScrollIfNeeded)
        .set_child(
          RichText::new().add_spans((0..INLINE_BOXES).flat_map(|index| {
            [
              Span::new(format!("word{index} ")),
              Span::view(Container::new().set_width(2).set_height(1)),
            ]
          })),
        )
        .build()
    });
  }

  /// Mounts a screen-high scroll box over [`MOVING_ROWS`] rows, each a box whose background
  /// runs an endless animation from red to blue around a text whose color runs one from
  /// blue to red.
  fn mount_moving_rows() {
    let _ = mount(|| {
      Container::new()
        .set_height(24)
        .set_overflow(Overflow::ScrollIfNeeded)
        .set_child(
          Column::new().set_children(
            (0..MOVING_ROWS)
              .map(|index| {
                Container::new()
                  .set_background(red())
                  .add_animation(
                    Animation::new(1., [(0., frame(red())), (100., frame(blue()))])
                      .set_iterations(Iterations::Infinite),
                  )
                  .set_child(
                    Text::new(format!("row {index}")).add_animation(
                      Animation::new(1., [(0., ink_frame(blue())), (100., ink_frame(red()))])
                        .set_iterations(Iterations::Infinite),
                    ),
                  )
                  .into_any()
              })
              .collect::<Vec<_>>(),
          ),
        )
        .build()
    });
  }

  /// Registers every case of the frame bench.
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

    testing::with_platform(|| {
      mount_inline_boxes();

      let _ = testing::render_to_text(80, 24);
      let mut wide = false;

      criterion.bench_function("inline_boxes_resized_frame_1k_boxes", |bencher| {
        bencher.iter(|| {
          wide = !wide;

          testing::render_to_text(if wide { 81 } else { 80 }, 24)
        });
      });
    });

    testing::with_platform(|| {
      mount_moving_rows();

      let _ = testing::render_to_text(80, 24);

      criterion.bench_function("moving_frame_1k_rows", |bencher| {
        bencher.iter(|| {
          testing::trigger_frame(Duration::from_millis(16));
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

#[cfg(skene_platform = "terminal")]
criterion::criterion_group!(benches, frame::frames);
#[cfg(skene_platform = "terminal")]
criterion::criterion_main!(benches);

#[cfg(not(skene_platform = "terminal"))]
fn main() {}
