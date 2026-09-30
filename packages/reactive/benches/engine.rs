//! Engine microbenchmarks: creation, reads, writes through effects, memo chains, a store
//! row's edit among a hundred tracked rows and among ten thousand, by position and by
//! key, and a row pushed onto as many.

#![expect(
  missing_docs,
  reason = "criterion's macros generate the harness's functions"
)]

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use skene_reactive::{
  Effect,
  Get,
  IndexedField,
  KeyedField,
  Memo,
  Owner,
  Signal,
  Store,
  StoreField,
  Update,
};

fn bench_create(c: &mut Criterion) {
  c.bench_function("create_1000_signals", |bencher| {
    bencher.iter_batched(
      Owner::new,
      |owner| {
        owner.enter(|| {
          for _ in 0..1000 {
            let _ = Signal::new(0u64);
          }
        });

        owner.dispose();
      },
      BatchSize::SmallInput,
    );
  });
}

fn bench_get(c: &mut Criterion) {
  let owner = Owner::new();
  let signal = owner.enter(|| Signal::new(0u64));

  c.bench_function("get_outside_effect", |bencher| {
    bencher.iter(|| signal.get());
  });
}

fn bench_set_with_effects(c: &mut Criterion) {
  let owner = Owner::new();
  let signal = owner.enter(|| Signal::new(0u64));

  owner.enter(|| {
    for _ in 0..10 {
      Effect::new(move |_| {
        let _ = signal.get();
      });
    }
  });

  c.bench_function("set_with_10_effects", |bencher| {
    bencher.iter(|| signal.update(|value| *value += 1));
  });
}

fn bench_memo_chain(c: &mut Criterion) {
  let owner = Owner::new();
  let source = owner.enter(|| Signal::new(0u64));

  let last = owner.enter(|| {
    let mut memo = Memo::new(move |_| source.get() + 1);

    for _ in 0..9 {
      let previous = memo;

      memo = Memo::new(move |_| previous.get() + 1);
    }

    memo
  });

  owner.enter(|| {
    Effect::new(move |_| {
      let _ = last.get();
    });
  });

  c.bench_function("memo_chain_10_update", |bencher| {
    bencher.iter(|| source.update(|value| *value += 1));
  });
}

#[derive(Store, Clone, Default)]
struct Row {
  label: u32,
  checked: bool,
}

/// One row's label edited among `rows` rows whose label and checked cells an effect
/// each tracks.
fn bench_row_edit(c: &mut Criterion, rows: usize) {
  let owner = Owner::new();
  let store = owner.enter(|| {
    let store = Store::transient(vec![Row::default(); rows]);

    for index in 0..rows {
      Effect::new(move |_| {
        let _ = store.at(index).label().get();
      });
      Effect::new(move |_| {
        let _ = store.at(index).checked().get();
      });
    }

    store
  });
  let mut edit = 0;

  c.bench_function(&format!("row_edit_among_{rows}"), |bencher| {
    bencher.iter(|| {
      edit += 1;
      store.at(rows / 2).label().set(edit);
    });
  });
}

fn bench_row_edits(c: &mut Criterion) {
  bench_row_edit(c, 100);
  bench_row_edit(c, 10_000);
}

#[derive(Store, Clone, Default)]
struct KeyedRow {
  id: u32,
  label: u32,
}

/// One row's label edited by key among `rows` rows whose label an effect each tracks
/// by key.
fn bench_keyed_row_edit(c: &mut Criterion, rows: u32) {
  let owner = Owner::new();
  let keyed = owner.enter(|| {
    let store = Store::transient(
      (0..rows)
        .map(|id| KeyedRow { id, label: 0 })
        .collect::<Vec<_>>(),
    );
    let keyed = store.keyed_by(|row| row.id);

    for id in 0..rows {
      Effect::new(move |_| {
        let _ = keyed.key(id).label().get();
      });
    }

    keyed
  });
  let mut edit = 0;

  c.bench_function(&format!("keyed_row_edit_among_{rows}"), |bencher| {
    bencher.iter(|| {
      edit += 1;
      keyed.key(rows / 2).label().set(edit);
    });
  });
}

fn bench_keyed_row_edits(c: &mut Criterion) {
  bench_keyed_row_edit(c, 100);
  bench_keyed_row_edit(c, 10_000);
}

/// A row pushed onto `rows` rows whose label an effect each tracks, and one effect the
/// length; each run starts from the same `rows`.
fn bench_row_push(c: &mut Criterion, rows: usize) {
  c.bench_function(&format!("row_push_among_{rows}"), |bencher| {
    bencher.iter_batched(
      || {
        let owner = Owner::new();
        let store = owner.enter(|| {
          let store = Store::transient(vec![Row::default(); rows]);

          for index in 0..rows {
            Effect::new(move |_| {
              let _ = store.at(index).label().get();
            });
          }

          Effect::new(move |_| {
            let _ = store.len();
          });

          store
        });

        (owner, store)
      },
      |(owner, store)| {
        store.push(Row::default());

        owner
      },
      BatchSize::LargeInput,
    );
  });
}

fn bench_row_pushes(c: &mut Criterion) {
  bench_row_push(c, 100);
  bench_row_push(c, 10_000);
}

criterion_group!(
  benches,
  bench_create,
  bench_get,
  bench_set_with_effects,
  bench_memo_chain,
  bench_row_edits,
  bench_keyed_row_edits,
  bench_row_pushes
);

criterion_main!(benches);
