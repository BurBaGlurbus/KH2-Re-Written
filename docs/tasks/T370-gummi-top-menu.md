# T370 — The Gummi menu's top screen (2D)

Owner Claude, 2026-11-02, requested by burba in the project thread; the coordinator asked for the 2D screens as separate verified commits. Issue #142. Gate: development of the Gummi menu in the world map program; no final gate is inferred.
Prerequisites: T362, T369. Build identity: local `main` after T369 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\gummi142\gummi-menu-layout.md` §1, §2, §5 with its `lay\` folder; runs `gm1` and `gm2` with `gm1sheet.png` and `gm2sheet.png` under `%USERPROFILE%\kh2-issues-out\g142\` (generated save `gummi.sav`).

## What the executable does

A route's Proceed starts the Gummi menu mode on its top screen (FUN_140394690, FUN_140243bc0, FUN_14024e780).
- **Frame:** the frame and title play the "none" → "top" transition, scene 34 then 35.
- **Rows:** `menuentry.bar` records 0, 1 and 4 (Start, Gummi Editor, World Map).
  - Each row appears, then loops. The cursor row goes on to its selected appear and loop, which carry the glow; there's no cursor sprite.
  - Labels sit at the loop group's IS_ACTIVE anchor, in its colour and text size.
- **Help:** a help window (record 0xf with 0x15a's orange groups) shows the cursor row's help text (0x2102, 0x2103 or 0x2119).
- **Banner:** a course banner (record 0x1c4) names the route.
- **Selector** (FUN_140274850):
  - decide wins over cancel;
  - a move repeats, but wraps only on a fresh press;
  - cancel moves the cursor to World Map.
- **Decide:** the rows close. Start leads to mission select, or to ship select while level 1 was never cleared. Gummi Editor leads to the editor. World Map leads to the leave window (0x4ab0), then the map.

## Changes

- `gummi::menu`: the selector, the top rows, `top_choice`, the return entrance, the sound ids.
- `visual::gummi_menu`: `GummiData`, items (appear, loop, selected, close, text at the anchor), `TopMenu`.
- `visual::menu_screen::load_entry_layout`: a 2LD inside a BAR entry, with its `_bar<N>` pictures.
- World map:
  - `ui.rs` loads the menu's files;
  - `world_map.rs` opens the menu on a route's Proceed and draws only the menu while it is up;
  - `KH2RW_GUMMI_KEYS=<tick>:<hex bits>,...` presses keys for runs.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS on the second run. The first failed `frame_limit::locked_frames_run_one_tick_each_and_stay_on_real_time`, a real-time test this change doesn't touch; it passes alone |
| Unit tests: selector wrap on a fresh press only, decide before cancel, row choices, return entrance | PASS |
| `wm --load-image gummi.sav --all-worlds --at 5458,-5300 --autoplay --decide` (`gm1`) | PASS: Proceed at tick 387 opens the menu (items 0, 1, 4, 0x1c4, 0xf); frame, title, rows, Chip's help and the "Asteroid Sweep" banner draw; cancel moves to World Map with its help; Down on a fresh press wraps to Start |
| `KH2RW_GUMMI_KEYS=450:20,700:20,820:4` (`gm2`) | PASS: Gummi Editor selected in green with "Assemble and upgrade Gummi Ships.", World Map in white; decide on World Map closes the rows and the map returns at tick 830 |
| Side by side with the installed game | NOT RUN |

## Gaps

- **Not drawn (3D):** the room and the row panels (G_MN100_BG, G_MN200_TOP_*), whose camera and easing aren't traced. The menu is drawn over black.
- **Silent:** how the sound ids map to se997 entries isn't traced.
- **Leave window:** 0x4ab0 isn't drawn, so World Map returns at once. The map also isn't re-entered at entrance 13 + route; it resumes where it was.
- **Other screens:** mission select, ship select and the editor aren't built. Choosing them ends the run as the route did before.
- **Inferred:** text alignment is taken as centred [I]. Which of the exe's two draw lists comes first isn't known.

## Verdict

PASS in the world map program for the top screen's 2D side.

## Next

#142: mission select (screen 1) on the same items, with its records' best values and the EX badge.
