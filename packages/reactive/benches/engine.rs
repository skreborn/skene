//! Engine microbenchmarks: creation, reads, writes through effects, memo chains.

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use skene_reactive::{Effect, Memo, RwSignal, Scope, SignalGet, SignalUpdate};

fn bench_create(c: &mut Criterion) {
  c.bench_function("create_1000_signals", |bencher| {
    bencher.iter_batched(
      Scope::new,
      |scope| {
        scope.enter(|| {
          for _ in 0..1000 {
            let _ = RwSignal::new(0u64);
          }
        });

        scope.dispose();
      },
      BatchSize::SmallInput,
    );
  });
}

fn bench_get(c: &mut Criterion) {
  let scope = Scope::new();
  let signal = scope.create_rw_signal(0u64);

  c.bench_function("get_outside_effect", |bencher| {
    bencher.iter(|| signal.get());
  });
}

fn bench_set_with_effects(c: &mut Criterion) {
  let scope = Scope::new();
  let signal = scope.create_rw_signal(0u64);

  scope.enter(|| {
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
  let scope = Scope::new();
  let source = scope.create_rw_signal(0u64);

  let last = scope.enter(|| {
    let mut memo = Memo::new(move |_| source.get() + 1);

    for _ in 0..9 {
      let previous = memo;

      memo = Memo::new(move |_| previous.get() + 1);
    }

    memo
  });

  scope.enter(|| {
    Effect::new(move |_| {
      let _ = last.get();
    });
  });

  c.bench_function("memo_chain_10_update", |bencher| {
    bencher.iter(|| source.update(|value| *value += 1));
  });
}

criterion_group!(
  benches,
  bench_create,
  bench_get,
  bench_set_with_effects,
  bench_memo_chain
);

criterion_main!(benches);
