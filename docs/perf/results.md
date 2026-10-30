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

## HD textures, memory and loads (T237)

Measured 2026-10-26 on the same PC, scenes, window and settings, uncapped. "Before" is local `main` at
`1b92156` (the end of T236 above), "after" is `8b2985a`, and "no 3D HD" is `8b2985a` with a temporary,
uncommitted patch that keeps the models', maps', map objects' and effects' original textures (the HUD's
HD images are required by the source check). Three alternating runs per build and scene; medians unless
a range is given. Discord, a browser and other sessions were running; the no-HD batch ran later and was
slower across the board, so its frame rates are not compared. Raw records stay private in the output folder outside the checkout (`f2-*`).

| | Test room before | after | no 3D HD | Twilight Town before | after | no 3D HD |
|---|---|---|---|---|---|---|
| Data decoded (ms after start) | 7,039 | 2,870 | — | 4,161 | 2,205 | — |
| First frame (ms after start) | 8,450 | 3,525 | — | 5,342 | 2,903 | — |
| Door change, background decode (ms) | — | — | — | 2,150 | 777 | 829 |
| Process memory, peak (GiB) | 1.92–2.08 | 1.39–1.57 | 1.43–1.46 | 1.61–1.63 | 1.24–1.27 | 1.28–1.29 |
| Field play FPS (ticks 0–1800) | 221.7 | 205.5 | (171) | 147.1 | 157.4 | (131) |
| Field 1% low / p99 (FPS / ms) | 91.9 / 9.9 | 87.1 / 10.6 | | 59.4 / 12.6 | 62.4 / 11.7 | |
| Camp menu FPS | 282.0 | 259.4 | (234) | 195.6 | 195.0 | |

- Start: the room's data decodes in 2.9 s instead of 7.0 s and the first frame shows at 3.5 s instead of
  8.5 s; Twilight Town 2.2 s and 2.9 s instead of 4.2 s and 5.3 s. The GPU device is now created during
  the decode; most other stages overlap on threads.
- A door change in Twilight Town decodes in 0.78 s instead of 2.15 s, so the screen holds black for less
  time.
- Memory: 0.4–0.5 GiB less at peak. The HD textures add no measurable process memory (uploaded once,
  kept only on the GPU; GPU memory not measured).
- Frame rate: unchanged within noise; the HD textures did not measurably slow drawing.

## A long session and the slow rooms (T298)

Measured 2026-10-30 on the same PC with other sessions' compilers running, so frame times swing about 2x between runs. "Before" is
`f69c1b4` with only the census added; "after" the same tree with [T298](../tasks/T298-session-cleanup-and-map-merge.md). Raw records
stay private in the output folder outside the checkout (`b-*`, `a-*`).

| | Before | After |
|---|---|---|
| Scripted duel, 5 min: running effects at the end, host / joiner | 121 / 99 | 0 / 0 |
| Walk through all 278 rooms: effect containers kept | 212 | 2 |
| Walk: heap, first to last sixth (MiB) | 1,407 to 1,485 | 1,386 to 1,397 |
| Rooms over 50 ms median, of 276 | 81 | 21 |
| Median room: frame (ms), meshes drawn | 39.7, 1,285 | 26.4, 303 |
| lm09, hb34, eh22 (ms, three alternating runs) | 196, 157, 145 | 31, 32, 31 |

The worst rooms were rewriting one material per UV-scrolled chunk every tick, allocating 30 to 80 MiB a frame. The room's five-minute
soak with no duel or doors was already flat in both.
