# Initial performance notebook

## Sand addition (current benchmark)

The historical results below precede sand. The benchmark now seeds a finite sand
patch and measures an additional full-grid sand pass after coverage. Same CPU and
release settings; one run:

| World / bodies | Sand p50 | Total p50 | Total p99 |
|---|---:|---:|---:|
| 320×180 / 8 | 0.018 ms | 0.065 ms | 0.072 ms |
| 1024×512 / 64 | 0.156 ms | 0.585 ms | 0.682 ms |
| 2048×1024 / 256 | 0.625 ms | 2.402 ms | 2.902 ms |

This is not a sand-filled-world stress test: a finite patch falls and settles
throughout measurement. No per-cell memory was added. Each grain moves at most
one cell per 120 Hz tick, rather than using continuous acceleration. Bodies block
movement destinations, but overlapping sand is preserved/frozen, not displaced.
Interactive frames may do multiple ticks and an extra presentation rasterization;
these totals measure one motion/coverage/sand/color sequence, not a display frame.
No new hardware counter or assembly conclusions were drawn for the sand pass.


## Environment and method

AMD Ryzen 7 9800X3D, Linux x86-64, rustc 1.97.0, default Cargo release profile.
Portable compiler target: no `target-cpu=native`, unsafe indexing, threading, or
manual SIMD. Timings below are one local run, not a hardware requirement or a
controlled statistical study. CPU frequency, scheduling, and warm caches matter.

Each fixture runs 100 warmup and 1,000 measured iterations: one 120 Hz rotation
step, coverage rebuild, and RGBA conversion. Every body starts with a 61×15 mask
containing 865 wood cells. Final RGBA output is passed through `black_box`.
The headless benchmark excludes SDL, texture transfers, presentation, real physics,
material rules, and edits from its steady-state totals. It varies size and body
count together; this does not isolate their individual scaling costs.

## Results after removing inner-loop floor calls

| World | Bodies | Dense CPU buffers | Total p50 | p95 | p99 | Max |
|---|---:|---:|---:|---:|---:|---:|
| 320×180 | 8 | 0.55 MiB | 0.046 ms | 0.049 | 0.052 | 0.066 |
| 1024×512 | 64 | 5 MiB | 0.425 ms | 0.435 | 0.638 | 0.644 |
| 2048×1024 | 256 | 20 MiB | 1.767 ms | 1.781 | 1.807 | 2.065 |

Coverage p50: 0.037 / 0.339 / 1.425 ms.
RGBA p50: 0.009 / 0.085 / 0.340 ms.
One centre cut through the first body: 0.058 / 0.339 / 1.335 ms.
That edit scans all material and includes reconstruction. It is not a fragmentation
stress test. Benchmark assertions verify removed + remaining material = initial.

A 120 Hz frame is 8.33 ms. A provisional goal is to keep this representation/render
preparation below 1 ms at 1024×512, leaving room for future systems. This is not an
established world size or whole-game budget. The large case already exceeds that
sub-budget. Noita's historical performance does not prove ours at unknown workloads.

## Assembly investigation

```sh
cargo rustc --release -- --emit=asm
rg -n 'rasterize|floorf|sincosf' target/release/deps/wizard_engine-*.s
```

The original raster loop used `.floor()` on both local coordinates before checking
bounds. Inspection showed `floorf` calls in the baseline x86-64 rasterizer. Since
negative local coordinates are rejected, conversion to an integer is already floor
for accepted coordinates. Removing these two calls preserves the centre-sampling
rule. A full-grid reference test checks rotations and clipping at world edges.
Bounding-box floor/ceil operations remain, once per body rather than per cell.

Before: total p50 about 0.093 / 0.811 / 3.313 ms.
After: total p50 about 0.046 / 0.425 / 1.767 ms.

## Hardware counters

```sh
cargo build --release
perf stat -e cycles,instructions,cache-references,cache-misses,branches,branch-misses \
  ./target/release/wizard-engine --bench
```

Whole-process counters (all fixtures, warmup, setup, edits and printing included):

| Counter | Before | After |
|---|---:|---:|
| Cycles | 23.89 billion | 12.84 billion |
| Instructions | 93.12 billion | 48.39 billion |
| Cache references | 1.301 billion | 1.317 billion |
| Cache misses | 91.20 million | 98.72 million |
| Branches | 18.08 billion | 8.85 billion |
| Branch misses | 70.59 million | 52.51 million |

Counters were multiplexed (~83% time enabled). Generic cache events have
architecture-specific meanings; these do not establish an L1/L2/LLC miss rate.
The edit location changed between these runs to guarantee actual destruction in
all fixtures, so whole-process counters are not an exactly controlled comparison.
The primary evidence is lower raster time and instructions, not better cache use.

The benchmark prints allocation addresses for inspection. Those are virtual
addresses, not physical cache placement. Contiguous row traversal and compact
storage are deliberate; pointers alone cannot demonstrate locality. The largest
fixture's working set can fit this CPU's large L3, so testing smaller-cache CPUs
and cold/stressed workloads remains necessary.

## Next useful experiments

- Isolate world size, occupied area, body count, and fragment count independently.
- Stress long thin rotating masks, checkerboard fragmentation, and simultaneous cuts.
- Measure presentation/upload separately on the actual display backend.
- Compare compact cell records vs separate state arrays when material rules exist.
- Test terrain-to-body transfers, then bounded collision/displacement semantics.
- Profile edit scans before adding spatial indexing; profile rasterization before
  adding tiling or SIMD. Preserve reference tests when optimizing.
