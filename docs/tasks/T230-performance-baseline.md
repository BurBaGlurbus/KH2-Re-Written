# T230 — Performance diagnostics and baseline

Owner Claude, 2026-10-25, requested by burba in the project chat ("a major refactor focused on performance
across the whole game, without changing gameplay behavior or visuals", step 1: measure before changing
anything). Gate: P30 performance engineering groundwork; no gameplay or presentation change, no final
gate inferred. Prerequisites: T110 (area transitions and load times), T127 (mesh storage reuse), T229.
Build identity: branch `perf/high-fps` from local `main` at `36c97bb`, package 0.1.2, Rust 1.99.0
Windows MSVC. Source: the selected Steam KH2FM build, validated at each run; the install was only read.

Private evidence: `C:\Users\ANIMA\kh2-perf-out\` — `base-room-1`, `base-room-2`, `base-room-vsync`,
`base-tt-1`, `base-tt-2` (frame CSVs, run logs, stderr), `trace-room-1`, `trace-tt-1` (Chrome traces and
`summary-all.txt`). Criterion output in the worktree's ignored `target/criterion/`.

## What was added

- `perf-diag` feature (`source/main/src/perf.rs`, off by default): Bevy's `FrameTimeDiagnosticsPlugin`,
  `EntityCountDiagnosticsPlugin` and `SystemInformationDiagnosticsPlugin` logged every 5 s by
  `LogDiagnosticsPlugin`; `--perf <new-dir>` records every frame (real frame time, virtual-time pause,
  fixed ticks run, entities, process memory, heap allocations and bytes) to `frames.csv` at exit;
  `--uncapped` presents without VSync whatever the settings say. New optional dependency `stats_alloc`
  (allocation counts through a wrapping global allocator; the workspace forbids the unsafe code a
  hand-written one needs); `bevy/sysinfo_plugin` brings `sysinfo` for process memory.
- `perf-trace` feature: `bevy/trace_chrome`, per-system spans in a Chrome trace. Tracy was not used: it
  needs the Tracy GUI or capture tool running beside the game, which this unattended setup does not have;
  the Chrome trace gives the same per-system spans.
- `tools/perf_report.py` (averages, 1%/0.1% lows, percentiles, allocations, the log's load times, a tick
  window) and `tools/trace_summary.py` (span totals per frame).
- `source/visual/benches/hud_canvas.rs`: a criterion benchmark of the HUD canvas on generated data. New
  dev-dependency `criterion` 0.7.0 without its default plotting/rayon features, for statistically sound
  timings of single functions.

The numbers, traces and bottlenecks are in [docs/perf/baseline.md](../perf/baseline.md).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo clippy -p main --all-targets --features perf-diag,perf-trace --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS |
| Release build with `skate3,perf-diag` and with `skate3,perf-diag,perf-trace` | PASS |
| `cargo bench -p visual --bench hud_canvas` | PASS (numbers in the baseline) |
| Unattended test room and Twilight Town runs, uncapped and VSync | PASS: exit 0, records written |

## Uncertainty

- The owner's other programs were running; run-to-run variation is large (Twilight Town's field phase
  averaged 27.9 FPS in one run and 51.3 in the next). Before/after comparisons need several runs each.
- The allocation counter and the diagnostics cost a little themselves; they are in every measured build.
- GPU time was not measured separately; the traces show CPU time on the main and render threads.
- The run log's `hitch` events and the autoplay logging (`pose_jump` events, flushed per write) exist
  only in unattended runs.

## Verdict / next

PASS for the measurement setup and the recorded baseline. Next: the owner's go-ahead on the plan, then
the frame-rate settings (step 2), interpolated presentation (step 3) and the optimizations (step 4),
starting with the HUD canvas.
