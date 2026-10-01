# Engine benchmarks

`cargo bench -p skene-reactive --bench engine` measures the reactive engine.

## Baseline

The fastest of five runs on an AMD Ryzen AI 9 HX 370 at rest, pinned to one of its
Zen 5 cores (2026-10-02). Runs of one build differ by up to a fifth, so a change is
measured against its parent built beside it, never against these numbers alone:

| Benchmark | What it measures | Time |
| --- | --- | --- |
| `create_1000_signals` | a thousand signals created and disposed, nothing recording snapshots | 40 µs |
| `get_outside_effect` | a signal read outside an effect | 5.2 ns |
| `set_with_10_effects` | a write reaching ten effects | 237 ns |
| `memo_chain_10_update` | a write through a chain of ten memos | 420 ns |
| `row_edit_among_100` | a store row's edit among a hundred tracked rows | 155 ns |
| `row_edit_among_10000` | the same among ten thousand | 158 ns |
| `keyed_row_edit_among_100` | a row's edit by key among a hundred | 216 ns |
| `keyed_row_edit_among_10000` | the same among ten thousand | 214 ns |
| `row_push_among_100` | a row pushed onto a hundred tracked rows | 225 ns |
| `row_push_among_10000` | the same onto ten thousand | 4.7 µs |

## Method

Two versions compare only when each is built in its own target directory (a shared one
reuses another tree's build) and their bench binaries run alternately, pinned to one
fast core with address randomization off:

```sh
setarch -R taskset -c 2 <binary> --bench
```

Callgrind's instruction counts compare two builds whatever the machine's load, cache
misses aside, on a build that keeps its symbols:

```sh
CARGO_PROFILE_RELEASE_STRIP=none cargo bench -p skene-reactive --bench engine --no-run
valgrind --tool=callgrind <binary> --bench
```
