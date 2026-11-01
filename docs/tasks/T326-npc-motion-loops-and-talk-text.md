# T326 — Townspeople's motions on the executable's clock; white outlined talk text; colour codes

Owner Claude, 2026-11-01, requested by burba in the project thread. Gate: development of field NPCs and message windows in the
test room; no final gate is inferred. Prerequisites: the npc-01 and dialogue-01 research. Build identity: local `main` after
`1972788` plus this change, release launcher with the `skate3` feature, built from a private export because other sessions' uncommitted
edits were in the shared folder. Source: the selected Steam KH2FM build, validated each run; the install was only read. Ghidra 12.1.4
headless on a private read-only project copy (plain Ghidra, no MCP).

Private evidence (outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\npc-talk-20261101` (decompiles `g\m1`, `g\t1`, `g\t2`,
`g\f4`), older `playable-room-20261008\tt-world-01\npc-01`, `dialogue-01`, `text-codes-01`; runs in `%USERPROFILE%\kh2-npctalk` (`b0`,
`n1`, `t2`, `t3`).

## What was wrong

**Motions.** `npcs::build_anim` sampled the motion at `frame_start + tick / scale`, half a key per tick. The installed sets say a tick
is one key: B_EX110_RTN's R00A declares end 60 at 30 fps, its header's length word is 120 and its key times run 0 to 120. Townspeople
played the first half of every motion at half speed, then jumped to the return frame. R30A (end 80, return 20, 160 ticks) never reached
its second half.

They also had no controller: a motion change restarted at once with no blend, the clock ran to the end frame inclusive, and walking
was a linear `travel` per tick.

**Text.** Code 0x04 did nothing, so `{04:0a}this board{03}` showed no highlight, and code 0x07 ignored the text's alpha.

## Changes

- `npcs::Anim`: one key per tick through `test_room::clip_for`, as Sora's and the party's clips.
- `FieldNpc.ctl`: `party_motion::Ctl` (FUN_1403c86a0, FUN_1403c6dc0, FUN_1403c8310, FUN_1403c6740): length from the header, first step
  0 to 2, wrap to the return frame at the length, hold when it is the end, queue.
- `FieldNpc::set_motion`: rtn_default's L1413. Walk to turn keeps the frame with no blend; turn to walk goes through idle with the walk
  queued; anything else asks FUN_1403c6dc0 with the script's 8.
- The talk gesture starts once with no blend (FUN_140400f90), and a set without one keeps its motion. The talk's end starts the idle
  with no blend (FUN_140416d00).
- Root motion follows the frames the clock covered (`Step::spans`, FUN_1403c8520), and none moves during a blend.
- `update_look`: the blend (joints left by the switch toward the new motion's held frame, FUN_1401c3a30 with FUN_1401c3970) and the
  head look-at on one pose, through the new `Skinning::pose_from_worlds_look`. Raw motions have no joints, so their vertices turn.
- `dialogue::Text`: code 0x04 through `Palette` (FUN_1403ea860), codes 0x04 and 0x07 scale by the text alpha (FUN_1403b0280), and a
  glyph fading in has that alpha alone (FUN_1403aff50). `hud::ftst_palette`.
- `--npc-tour` puts Sora by a townsperson with a talk, talks and pages, takes `--capture-at`, runs 420 ticks and logs `npc_motion` with
  the clock, the blend and the furthest a vertex moved. `--autoplay` in the talk room takes `--capture-at`.

## A deliberate difference from the executable

burba asked for talk text white with a black outline, Twilight Town included, after being told the original differs. `WINDOW_COLOUR_ID`
in `dialogue.rs` is the one line.

| | Executable | This build |
| --- | --- | --- |
| Talk, balloon text colour | `ftst` id 5 for the world (FUN_1403bb700, FUN_1403bb270): TT and HB 0x80050520, dark | `ftst` id 3, 0x80f0f0f0, white, as WINDOW_CREDIT (FUN_1403bb520) |
| Font | 3, program 5 where DAT_14074d020 stays 1 (FUN_1403e2d80; ps 027: solid colour, alpha from the texel's red, so the art's edge does not show) | Art times colour (program 3, ps 026), as font 4, which keeps the art's black edge |

So program 5 is not ported: it would remove the outline. The exe's default for DAT_14074d020 is 1 [O]; whether it stays 1 on the PC
depends on `+0x94` and `+0x7c` of the font's texture record, which were not traced [U].

Reference footage of the PS4 remaster (YouTube `uTZTLhIKYKc`) shows the dark text on Twilight Town's windows. No PC footage was found.

## The close

No close was missing. FUN_1403bce30 and FUN_1403bbeb0 were re-read and the rewrite already matched them: state 4 starts the tail's close
(G48, 5 ticks), state 5 the box's (G69, 13 ticks, scale and alpha 1 to 0 on a cubic ease, so most of the change is in its last ticks),
and the text is drawn in states 1 to 5 only, so **it goes at once when the box starts to close**. The footage shows the empty box too.
FUN_1403afd90 gets a fixed colour from the window, so text has no fade of its own in the executable. Nothing was changed here.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | FAIL on `source/game/save/src/state.rs`, from T323, not this change; none in the files of this change |
| `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --locked` | Tests 1174 passed, 0 failed; debug build PASS, on `fcdfc21` plus this change. Clippy FAIL on one finding, `too_many_arguments` at `launcher/install.rs` `write_field_config`, which is not from this change; no finding in its files |
| Synthetic tests: a held talk's clock and return frame, a missing motion, root motion across the wrap, walk to turn, colour codes, the close | PASS |
| Talk room `n1`, Axel, close at ticks 152 to 174 | The tail then the empty box shrink and fade; gone by about tick 172. That run's exit code was 1 because its first capture, tick 100, came out uniformly black: a uniform capture fails an unattended run, and with other sessions' builds on every core the room was still blank then. Its rerun `n2` from tick 40 exits 0 |
| Twilight Town tour `t2`, Station Plaza, the townsperson of serial 131, message 17670 | White outlined text, the gesture reaches its raised arm, open and close seen at 190 to 203 |
| Tour `t4` and talk room `n2` on the committed tree | Same results. `t4` exits 1 on a uniform first capture at tick 40, the same blank room under load; its other captures are fine |
| Twilight Town tour `t3`, pose jumps | Walk seam at clock 71 to 0: 12.2 units against 11.1 before it; the 0 to 2 step: 22.2, the double key FUN_1403c6740 does. Largest: 94.6 at the talk's end (no blend, as FUN_140416d00; the dissolve covers it in play) and 30.6 on a blend's first shown weight 3/8. Median 2.97 |
| World map message windows, also now white | NOT RUN |
| Side by side with the installed PC game | NOT RUN |

## Uncertainty

- Blends and the talk's end are seen in a log and captures only; a townsperson was not watched in motion at full speed.
- `--npc-tour` follows Sora, so the walk's loop was checked by the log, not by eye.
- Text codes 0x06 (window type), 0x08 (tail), 0x09 (inline icon) and 0x13 (window position) are still not carried out on a talk window.
- Code 0x04 in cutscene subtitles has no table.
- `Talk`'s art and palette are read for the area the room starts in and do not follow a jump to another world.

## Verdict

PARTIAL: townspeople's motions, the colour codes and the requested white text are in and seen in Twilight Town; the close already
matched the executable; no comparison against the running PC game.

## Next

Page-start codes 0x06, 0x08 and 0x13 on talk windows, so positioned windows and their tails land where the data says.
