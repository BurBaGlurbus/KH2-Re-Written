# T313 — Fullscreen no longer quits; Task Manager's memory falls after a load; the Steam game measured

Owner Claude, 2026-10-31, requested by burba in the project thread: "the ps2 had fucking 32 mb of ram and 4 mb of vram like bro it should not be taking this much. also full screening the game crashes". Gate: P30 performance engineering; no gameplay gate moves. Prerequisite: T312. Build: local `main`, `4b197aa` and `28eb6a1`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated at each run; the install was only read, and the Steam game was run unmodified and closed afterwards.

Numbers are in [the results](../perf/results.md#memory-after-a-load-and-the-steam-game-t313). Private evidence stays in an output folder outside the checkout (`n-*`, `cap-*`, `steam-*`, heap dumps).

## Causes and changes

| Cause, as measured | Change |
|---|---|
| T312 regression. Every window size change quit the game: `room::render_target` images kept Bevy's `copy_on_resize`, so resizing copied the old texture without `COPY_SRC`, a validation error. Fullscreen and borderless resize as they open | `copy_on_resize = false`. A resized target starts empty and a camera draws it again that frame |
| A load touches far more memory than the game reads again, and Windows leaves those pages resident | `audio::memory::trim_working_set` (`K32EmptyWorkingSet`) once at frame 30 of each program, at the end of each area change, and when the launcher hides. Pages in use return at their next touch, without a disk read. This lowers physical memory only: commit does not change |
| Sora's baked body poses held one hash map per sample | One sorted bone list per clip and a flat matrix array (`BoneMatrices`). About 20 MiB expected from the heap's 27 MiB entry; the saving is within the commit noise and was not measured alone |

Tried and not kept:

| Tried | Result |
|---|---|
| Lower upload limit (`RenderAssetBytesPerFrame` 32 and 8 MB) | No change in commit, so the 344 MiB of write-combined mappings is not upload staging. The code was removed |
| DX12 instead of Vulkan, 100% scale | Private working set 893, commit 1,310, GPU 372 + 220 shared, against 757, 1,139, 333 + 66. CPU 4.4% against 7.1% in one noisy sample. More memory and not capture-checked |

## Not done

These only lower commit, because the trim already takes cold pages out of physical memory, and each one has a cost:

| Item | Size | Why not this round |
|---|---|---|
| The world's ambient sound bank | 51 + 7 MiB | Streaming changes when sounds become ready; it needs a listening check |
| Party limits' HUD sheets | 36 + 9 + 3 MiB | Loading on first use adds a hitch on first use |
| HUD tile cache | 46 + 3 MiB, up to 64 | It holds the HUD canvas at about 2% of a core instead of 18% |
| The canvas worker's duplicate sheets | 24 MiB | `hud_present` was being edited by another session |
| `load_enemy`'s baked clips | 23 MiB | Same treatment as T312's poses; not started |
| The driver's write-combined mappings | 344 MiB, 32 MiB blocks | The source was not attributed |

Heap before the body change, 624 MiB in all: ambient bank 58, HUD tile cache 49, limit HUD sheets 48, model HD textures 35, Sora's body maps 27, music PCM 27, canvas worker sheets 24, baked enemy clips 23, HD decodes on threads 20, mesh allocator nodes 20, HUD sheets 18, `load_bank` 17, camp 16, sound clips 10, the rest smaller.

## Checks

| Check | Verdict |
|---|---|
| `cargo fmt --all --check` on the exported commit | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` on the exported commit | PASS |
| `cargo test --locked` on the exported commit | PASS: 1,139 passed, 0 failed, with a new test that a render target does not copy on resize |
| Release build with Skate 3 from an export of `28eb6a1` | PASS: only `KH2RW Launcher.exe` left |
| Fullscreen and borderless on the owner's settings, this build | PASS: 0 errors, clean exit |
| Live window resize, `4b197aa` | PASS: 0 errors |
| Render Scale change and the Graphics menu's switch during play | NOT RUN: they need input in the menu |
| Captures at 7 ticks, test room, 6 runs of the trim's code against the build before it | PASS: 3 runs identical at all 7; 1 differs at one tick (5,006 px); 1 took the other combat branch at tick 1500 and matches elsewhere; 1 differs at 3 ticks with large areas, the same scene a few frames of animation apart |
| The same scripted combat in 6 runs of an older build | Tick 1500 took the same other branch in 1 of 6, so this divergence comes before the trim |
| Frame pacing, 50 s with 3 area changes, before and after the trim | PASS with a cost: p50 16.66 and p99 17.16 ms in both. New: one 74 ms frame at frame 30, and one of 46 to 53 ms at each area change. Frames with 2 ticks 11 against 4 |
| Station Plaza for 8 minutes | PASS: Task Manager's memory 141, 176, 183, 190 MiB at 30, 150, 300, 480 s; 0 errors |

## Uncertainty

- The Steam game was measured on its KH2 menu only (`NEW GAME` / `LOAD` / `BACK`), at 1280x720 in a window. A field area was NOT RUN: it needs input, and input was not sent to the owner's desktop. Field-to-field CPU and memory are not compared.
- The Steam numbers are one run; its menu is not the same screen as this game's title.
- The other 1 run in 6 with 3 differing ticks was not explained beyond its look; whether the trim's stall makes such runs more frequent was not separated.
- The launcher hidden behind a running game was not measured, so its trim on hiding is unverified beyond a clean build.
- The owner's settings in fullscreen commit about 1,700 MiB; their GPU memory there was not read.
- Other programs ran during every measurement (system about 30% busy).
- Kingdom Hearts: Connected's local engine-path build also needs `BoneMatrices` where it reads a body pose, as well as T312's `posed` and `weapon_frame`.

## Verdict

PASS for the crash and for physical memory: Station Plaza on the owner's settings from 771 to 190 MiB in Task Manager after 2.5 minutes, title 289 to 111, launcher 226 to 53. FAIL against the owner's stated aim of a PS2 budget, and it does not lower commit, which stays at 1,120 to 1,330 MiB, about the Steam game's 1,293. CPU in the field stays near 5.8%. No gameplay gate moves.

## Next

The largest remaining commit the game itself holds is the heap table above. Attribute the driver's 344 MiB with a Vulkan allocation trace before cutting it.
