# Performance baseline (T230)

Recorded 2026-10-25 on branch `perf/high-fps` from local `main` at `36c97bb`, before any optimization.
Raw records (frame CSVs, run logs, Chrome traces) stay private in `C:\Users\ANIMA\kh2-perf-out\`; only
aggregate numbers are here.

## Setup

- PC: Intel i7-9700F (8 cores), NVIDIA RTX 2070 SUPER (driver 616.92, Vulkan backend), 32 GB RAM,
  Windows 11, one 1920x1080 monitor at 100 Hz.
- Build: `cargo build -p main --bin KH2RW --release --features skate3,perf-diag` (default release
  profile: opt-level 3, no LTO, 16 codegen units). `perf-diag` counts every heap allocation; its cost is
  the same before and after.
- Window: 1280x720 windowed (the test room's default), render scale 100%, MSAA off (unattended runs use
  the default settings). `--uncapped` presents with `AutoNoVsync` (no limiter); one run with VSync.
- Background load: the owner's other programs (Roblox, Godot, Discord, a browser) were open, so runs
  vary. Run 1 had about 56% system CPU load from other programs, run 2 about 9%. Compare like with like.

## Scenes

Both are the existing unattended `--autoplay` scripts, so the input is the same every run and the
simulation is the fixed 60 Hz tick:

- **Test room** (`room`, arena config 07): ticks 0–1800 are field combat against the Shadow (attacks,
  jumps, all spells, a drive form, blows, guards); from tick 1810 the script works through the camp menu.
  3600 ticks.
- **Twilight Town** (`tt`, the launcher's generated config, Station Plaza, first-visit story): the same
  script while walking toward exits; one area change (to area 6) during the field phase, then the camp
  menu. 5400 ticks.

```
KH2RW.exe <room|tt> <steam-root> <manifest> <config> --log <run.jsonl> --autoplay <capture-dir>
          --capture-at <last-tick> --perf <out-dir> [--uncapped]
python tools/perf_report.py <out-dir>/frames.csv <run.jsonl> [--ticks 0:1800]
```

`--capture-at <last tick>` replaces the scripts' captures with one at the end, so screenshots do not
stall the measured frames.

## Numbers

"1% low" is 1000 / the mean frame time of the slowest 1% of frames. Allocations count every thread.

### Field phase (ticks 0–1800)

| Scene, run | Avg FPS | 1% low | 0.1% low | Frame avg / p50 / p99 / max (ms) | Frames > 33 ms | Allocations per frame (avg / max) | Allocated per frame (avg / max) |
|---|---|---|---|---|---|---|---|
| Test room, run 1 | 44.7 | 23.6 | 14.4 | 22.4 / 21.9 / 35.6 / 69.3 | 25 | 9,649 / 67,969 | 13.0 / 18.3 MiB |
| Test room, run 2 | 54.1 | 22.0 | 6.0 | 18.5 / 17.7 / 28.3 / 165.5 | 7 | 8,965 / 23,817 | 12.7 / 15.8 MiB |
| Test room, VSync 100 Hz | 66.8 | 49.5 | 44.6 | 15.0 / 15.6 / 19.6 / 23.9 | 0 | 8,650 / 24,809 | 11.8 / 15.8 MiB |
| Twilight Town, run 1 | 27.9 | 5.6 | 3.9 | 35.9 / 28.8 / 137.5 / 255.4 | 251 | 9,960 / 218,022 | 14.1 / 54.2 MiB |
| Twilight Town, run 2 | 51.3 | 29.7 | 8.4 | 19.5 / 19.4 / 22.7 / 119.6 | 3 | 8,516 / 61,069 | 13.3 / 44.5 MiB |

### Camp-menu phase (tick 1810 on)

| Scene, run | Avg FPS | 1% low | Frame avg / p99 / max (ms) | Allocated per frame (avg) |
|---|---|---|---|---|
| Test room, run 1 | 93.4 | 34.4 | 10.7 / 23.3 / 80.3 | 4.6 MiB |
| Test room, run 2 | 120.3 | 57.7 | 8.3 / 16.6 / 20.8 | 4.6 MiB |
| Twilight Town, run 1 | 60.2 | 18.7 | 16.6 / 34.8 / 168.7 | 4.3 MiB |
| Twilight Town, run 2 | 141.9 | 48.5 | 7.0 / 19.8 / 23.2 | 3.6 MiB |

### Other measures

| Measure | Test room | Twilight Town |
|---|---|---|
| Entities | 537–617 | 2,206–3,036 |
| Process memory (sysinfo) | 1.87–2.09 GiB | 1.32–1.67 GiB |
| Start to "decoded" (includes the mandatory source validation) | 9.6–9.8 s | 4.2–6.0 s |
| Start to first frame | 11.1–11.7 s | 5.5–7.5 s |
| Area change (worker decode while the screen is black; main-thread part) | — | 2.2–3.1 s; 13 ms |
| Frames over 50 ms in the whole run (`hitch` events) | 5 | 4–116 |

The field phase does not hold 60 FPS in either scene, and with VSync it cannot hold the monitor's
100 Hz. The camp menu, which hides the field HUD, runs about twice as fast.

## Where the time goes

Chrome traces of the field phase (`perf-trace` build, 1800 ticks; per rendered frame, mean). The main
app and the render app run in parallel (pipelined), so the slower of the two sets the frame rate.

| Span | Test room | Twilight Town |
|---|---|---|
| Main app, whole frame | 19.0 ms | 20.3 ms |
| — `test_room::hud_present` | **13.9 ms** | **15.2 ms** |
| — FixedUpdate (all simulation, ~1.3 ticks per frame) | 1.4 ms | 1.5 ms |
| — PostUpdate | 1.2 ms | 1.4 ms |
| Render app, whole frame | 12.4 ms | 12.1 ms |
| — `allocate_and_free_meshes` | 3.7 ms | 3.9 ms |
| — `render_system` (command encoding and submit) | 3.1 ms | 3.6 ms |
| — GpuImage extract + prepare | 2.2 ms | 2.7 ms |
| — mesh extract + prepare | 0.9 ms | 0.7 ms |
| Worst single spans | update 288 ms, prepare_lights 106 ms, GpuImage prepare 65 ms, extract 59 ms | update 177 ms, mesh allocator 87 ms, GpuImage prepare 82 ms |

Criterion (`cargo bench -p visual --bench hud_canvas`, generated textures and a HUD-shaped draw list of
about 180 quads): `Canvas::render` 7.6 ms at texel scale 2 and 28.4 ms at scale 4; copying the
finished canvas adds 1.0 ms and 3.9 ms.

## Top bottlenecks

1. **The field HUD is rasterised in software every frame.** `hud_present` re-draws the whole HUD on the
   CPU whenever `Hud::updated` is set, which `hud_tick` does every tick. The canvas clears and resolves
   the union box of everything drawn; with text at the upper left and the gauge at the lower right that
   box is nearly the whole 1366x832 canvas. The result is then cloned (4.5 MB) into the image, and Bevy
   copies and uploads it again. This is 70–75% of the main thread and most of the 13 MB per frame
   allocated in the field (4.6 MB in the menu, where the HUD is hidden).
2. **Mesh churn in the render world** (`allocate_and_free_meshes`, 3.7–3.9 ms): the 2D overlay rewrites
   every slot's mesh every frame even when unchanged; the spell and drive effect presenters despawn every
   effect entity and add new mesh assets whenever an effect changes; the CPU-skinned actors (Sora, the
   Keyblade, the party, enemies, townspeople) re-upload positions every tick.
3. **Texture uploads** (GpuImage extract + prepare, 2.2–2.7 ms): mainly the 4.5 MB HUD texture each
   tick, plus animated map textures. First uses of effect textures are also the worst single frames.
4. **Hitches:** first-use uploads and pipeline work when a spell or drive effect first appears (single
   spans of 60–290 ms), and the area change's 2.2–3.1 s black hold while a worker decodes the next area.
5. **No parallelism in game systems, and allocation churn:** all of `Update` is one `.chain()` of about
   45 systems, and FixedUpdate's systems are chained as well, so they run one after another on one
   thread; about 9,000 heap allocations per frame. Once the HUD is fixed, the render app's ~12 ms becomes
   the limit, so items 2 and 3 matter next. The release profile also has no LTO and 16 codegen units.

## Frame-rate independence today

The simulation already runs on a fixed 60 Hz `FixedUpdate` (tick counters, the original's two-frame
physics step), and presentation systems read `game.tick`; only the debug menu, the launcher and the
smoke binary use the frame delta. What is missing is interpolation: above 60 FPS, actors, the camera and
effects only change once per tick, so extra frames repeat the same picture. Unlimited FPS therefore
would not look smoother until rendering interpolates between the last two ticks.
