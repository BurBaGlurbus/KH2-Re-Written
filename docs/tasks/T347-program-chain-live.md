# T347 — The chain between programs runs for real, under the launcher too

Owner Claude, 2026-11-01, requested by burba in the project thread ("work through issues and close them"). Issues #143, #79, #32. Gate: development of the
save and world map hand-off; no final gate is inferred. Prerequisites: T323, T324, T344. Build identity: local `main` after `5cbd52b` plus this change,
package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): runs `c2`, `L2`, `L3`, `T1` under `%USERPROFILE%\kh2-issues-out\s162\`.

## Why

T324 and T325 left "the live chain has not been played": unattended runs stopped before starting the next program, so the spawn itself never ran. Reading
it for this task showed a bug: the launcher waits on its one child and shows itself when that ends, so a room starting the world map left the launcher's
window over it.

## Changes

- `save_files::start_next`: every hand-over (save point to map, landing, door, story event, event's end) goes through it. Under the launcher the program
  prints `kh2fm-next <args>` and the launcher starts the next one and stays hidden. A program run by itself starts it directly.
- `launcher::start_loaded`: a file saved on the world map starts the map with that image, not the room at the config's place. `--all-worlds` is left out.
- Test hooks, off unless set: `KH2RW_CHAIN=1` lets an unattended run hand over; `KH2RW_CHAIN_ARGS_<PROGRAM>` adds arguments to the next run, split on `|`;
  `KH2RW_LAUNCHER_RUN=<program>` starts that entry on reaching the tests screen and closes on coming back.
- A title that hands over during an unattended plan exits 0; its log no longer says "next screen not built".

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Room alone, `--save-point-tour` on the World Map row of tt area 9's WORLD_POINT program, chain on (`c2`) | PASS: `world_map_jump` at tick 441; the map started from the handoff image and landed; `field_start` `started: true`; the room started on it, `save_applied` at 2/9/99, first frame logged |
| The launcher exe, entry `wm`, chain on (`L2`) | PASS: the map ran, `kh2fm-next` reached the launcher, the room started on the landing's image, exit 0, the launcher closed from its tests screen |
| The launcher exe, entry `title`, plan `--autoplay-files` (`L3`) | PASS: title's Load wrote the slot, the launcher read `kh2fm-handoff Load`, and the slot (world 15) started the map on that image |
| The launcher's Load of a file in a field world | NOT RUN as one run: the unattended title's container only loads its world 15 slot. The room's half, `room <config> --load-image <handoff>`, ran in `c2` and `L2` |
| The owner's save file (`saves/KHIIFM_WW.png`) after these runs | PASS: not written; unattended programs use an in-memory container |
| A person clicking through it | NOT RUN |
| Side by side with the installed game | NOT RUN (#144, #82) |

## Uncertainty

- The launcher window's own buttons were not pressed: the hook calls the same `launch` a press does.
- The window changes between programs, so the fades are each program's own, not the one jump the game makes.

## Verdict

PASS for the chain. #143's and #79's own lists are done; the comparison with a save from the installed game stays in #82.

## Next

The rest of the batch.
