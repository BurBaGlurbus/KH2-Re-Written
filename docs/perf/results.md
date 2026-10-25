# Performance: before and after (T236)

Measured 2026-10-25 on the same PC, scenes, window and settings as [the baseline](baseline.md): i7-9700F,
RTX 2070 SUPER, 1280x720 windowed, render scale 100%, MSAA off, VSync off (`--uncapped`) unless noted.
Both executables are `--release --features skate3,perf-diag` builds made the same hour: "before" from
the baseline commit `16cec02` (an exported copy of that tree), "after" from local `main` at `1b92156`.
The runs alternated before/after, three of each per scene; Roblox was open throughout, so single runs
swing (one "after" room run averaged 93 FPS, the next two 207 and 195). The tables give the median of the
three runs. Raw frame records stay private in `C:\Users\ANIMA\kh2-perf-out\fin-*`.

## Field play (ticks 0–1800)

| | Test room before | Test room after | Twilight Town before | Twilight Town after |
|---|---|---|---|---|
| Average FPS | 58.8 | **194.8** | 45.5 | **147.3** |
| 1% low FPS | 26.8 | **45.9** | 14.0 | **53.6** |
| 0.1% low FPS | 8.8 | 16.1 | 7.2 | 16.6 |
| Frame time avg / p50 / p99 (ms) | 17.0 / 16.8 / 24.7 | 5.1 / 3.8 / 12.3 | 22.0 / 20.6 / 44.0 | 6.8 / 5.9 / 12.1 |
| Worst frame (ms) | 113 | 80 | 139 | 94 |
| Frames over 33 ms | 6 | 5 | 37 | 6 |
| Heap allocated per frame | 12.5 MiB | 2.5 MiB | 13.4 MiB | 3.1 MiB |
| Heap allocations per frame | 8,816 | 4,659 | 8,658 | 5,210 |

## Camp menu phase (tick 1810 on)

| | Test room before | after | Twilight Town before | after |
|---|---|---|---|---|
| Average FPS | 67.5 | 254.2 | 62.6 | 191.3 |
| 1% low FPS | 32.2 | 67.9 | 27.7 | 68.9 |
| p99 frame (ms) | 20.2 | 8.6 | 25.2 | 10.1 |

## With VSync (100 Hz monitor), test room field phase, one run each

| | Before | After |
|---|---|---|
| Average FPS | 57.2 | 99.8 (holds the refresh rate) |
| 1% low FPS | 24.8 | 62.7 |
| p99 / worst frame (ms) | 30.2 / 90.6 | 11.6 / 74.9 |

## Unchanged

- Entities (537–617 in the room, 2,206–3,036 in Twilight Town) and process memory (1.8–2.0 GiB and
  1.3–1.7 GiB).
- Start to first frame (room 8.6–9.8 s before, 8.8–9.5 s after; Twilight Town 6.1–7.3 s before,
  5.3–5.4 s after; mostly the required source check) and the area change's background decode
  (2.0–2.4 s both).
- Gameplay: the room's unattended run logs the same gameplay events at 30, 60, 144, 240 FPS and
  unlimited; captures taken at the same tick match the previous build's.

## Where the time went

The software HUD canvas (14–15 ms of a 19–20 ms main-thread frame) now redraws only the 16-pixel tiles
whose triangles changed, on several threads, and uploads only those tiles; the main thread's frame fell
to about 5 ms. The render thread (now about 9 ms in the room and the limit) lost the HUD texture
re-upload and most mesh re-uploads; what remains is the CPU-skinned characters' meshes, which change
once per tick (see the task record's deferred list).
