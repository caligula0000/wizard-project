# Wizard Engine — cell/body experiment

Rust + SDL3. Requires Rust with edition 2024 support, a native linker, and SDL3
 development libraries discoverable via `pkg-config` (3.4.x recommended).
SDL3 is dynamically linked, not bundled.

```sh
pkg-config --modversion sdl3
cargo run --release
```

## Sandbox

A 320×180 cell world displayed at 3× scale. Eight wood masks rotate over a rock
floor, with an initial patch of falling sand. Holes, edits, and fragments are all
defined by material cells.

- **Space**: pause/resume
- **N**: advance one 1/120-second tick while paused
- **Left drag**: erase cells with a circular brush; disconnected pieces split
- **Right drag**: paint sand into empty space (also works while paused)
- **R**: reset
- **Escape**: quit

Pause before cutting a beam for the clearest demonstration. Fragments retain
world-space placement at the cut, then rotate independently around their new
centres. This is prescribed animation, **not physical fracture**: no gravity,
rigid-body collision response, inherited linear velocity, or water yet. Overlaps
are counted and displayed, not resolved. Bodies can pass through terrain/each other.

Sand falls down, then tries downward diagonals, accumulating on terrain, sand, and
body coverage. Each fixed tick scans bottom-up, so grains move at most one cell;
a deterministic position/tick hash varies diagonal preference and scan direction
alternates. There are no per-grain allocations or update stamps. Solid world edges
retain material. The full-grid scan includes settled sand (sleeping is deferred).

If an animated body covers existing sand, the grain is frozen and hidden until
uncovered, not deleted or displaced. Sand does not exert forces on bodies; thin
rotating masks can still alias. Brush input is applied per display frame, while
sand movement runs at fixed ticks. The interactive loop rebuilds coverage before
each simulation tick and again for presentation; the benchmark measures one
motion/coverage/sand/color sequence without that extra presentation rebuild.

The title reports CPU event/edit/update/raster/color work, excluding texture
upload, presentation, and pacing. Simulation uses a fixed 120 Hz tick, independent
of a nominal 120 Hz presentation cap. At most eight catch-up ticks run per frame;
excess elapsed time is dropped and counted rather than causing a spiral of death.
This overload policy is exploratory, not a future replay contract.

## Data design

`src/world.rs` contains SDL-independent simulation and tests. `src/main.rs`
contains presentation, controls, and a headless benchmark.

- Terrain: contiguous row-major `Vec<Material>`, explicitly one byte per entry.
- Bodies: tight local material grids plus one transform each. Rotation updates
  the transform, not material storage. Empty local cells represent holes.
- Coverage: transient `u32` body index + 1 per world cell (zero = no body).
  These indices are rebuilt each frame, **not stable entity identifiers**.
- Visible material: one derived byte per world cell.
- Output: four RGBA bytes per world cell, uploaded as one streaming texture.

Total dense CPU buffers: **10 bytes/world cell**, plus body masks and edit-time
scratch space. SDL/GPU allocations are additional. Body-local arrays support mixed
materials; shared properties currently consist only of material colors.

Rasterization inverse-samples destination cell centres within each body's clipped
bounding box. Traversal writes rows sequentially, but rotated local-grid reads are
not always contiguous. The first body wins body/body overlaps; bodies render over
terrain. Neither policy deletes underlying material. Coverage is an approximation,
not mass: rotating thin features can alias. Authoritative material stays in local
masks, and edits test those masks rather than the display coverage.

After an edit, four-neighbour connected components become tightly cropped local
grids. Positions are adjusted to preserve world placement. Editing currently scans
the full terrain and all body cells and allocates component scratch buffers; it is
intentionally a reference implementation, not the final spatial-query architecture.
No allocations are needed for steady-state simulation/raster/color passes.

Not implemented: terrain-to-body detachment, bonding/anchors, physical collisions,
material displacement, persistent body IDs, turn rules, or activity scheduling.
The goal is to test storage and representation before committing to those systems.

## Verification and measurements

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo run --release -- --bench
```

The benchmark needs no display. It runs three fixtures with 100 warmup frames and
1,000 measured frames each, including a finite sand patch and a separate sand
phase, printing phase p50/p95/p99/max, buffer addresses,
memory estimates, and one separately timed edit. It checks material conservation.
See `PERFORMANCE.md` for initial results, limitations, and profiling commands.

Headless SDL smoke test (exit 124 is expected):

```sh
SDL_VIDEO_DRIVER=dummy SDL_RENDER_DRIVER=software timeout 3s cargo run --release
```
