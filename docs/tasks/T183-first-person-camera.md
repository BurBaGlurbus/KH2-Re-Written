# T183 — The first-person look camera (camera mode 1, Select)

Owner Claude, 2026-10-17, requested by burba in the project chat (the original's
Select-button look-around should be in the room). Gate: development of the test
room only; no final gameplay acceptance is inferred. Prerequisites: T170
(lock-on camera), T105 (movement). Build identity: branch
`claude/combat-feel-first-person` on PR 103's branch (`main` at `554d3bf`),
package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build,
validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/combat-feel-20261017/`
(headless Ghidra 12.1.4 on a read-only project copy: decompiles `g/d37`, `g/d38`,
data `g/dd10.txt`, references `g/r18.txt`, the earlier `ghidra-kh2-a1/cam-01`,
`cam-02`, `camcol-01`) and, in the private `kh2-combat-out/`, the unattended runs
`fp1` (camera trace) and `fp3` (captures). No game data or captures are in Git.

## What the executable does

The camera singleton (0x140718c60) keeps its mode at +0x48 (that is
`DAT_140718ca8`); mode 1 is first person.

- **Entering** (`FUN_140165110`, mode 0): when `FUN_1403ba300(player)` holds and
  logical pad bit 22 (Select) was pressed, the mode becomes 1, the pitch
  (+0xc8) is cleared, and `FUN_1403b7810(player+0xa10, 1.0, 0, 30)` starts
  Sora's fade-out ramp (value 1.0, step -1/30). The rest of that frame is the
  field camera's tail without the follow and manual turn. `FUN_1403ba300` is
  true for a physics mode whose flag word has bit 7 (modes 0 and 1, 4, 0x1e: the
  ground and the air; not Quick Run's mode 0x10 or the glides' 0x11 / 0x12) with
  `obj+0x18c` bit 14 and `obj+0x194` bit 1 clear.
- **Each frame** (`FUN_140166100` case 1, `FUN_140165e30`): R3 (bit 17, pressed)
  levels the pitch. The eye target (+0x64) is the head, the player's position
  with y reduced by the PRTY record's word at +0x30 (`FUN_1403b5c50`). The yaw
  base (+0xc4) is the player's facing (behind him plus pi). If no
  `FUN_1403babf0` flag is set and an up/down camera bit (0xc0000) is held, the
  pitch drops by `right-stick magnitude x right-stick Y x 0.05236` (up is
  negative, so up looks up); it is wrapped and clamped to +-1.2217 (70 degrees).
  The look target (+0x84) is the head plus the vector (0, 0, 350) turned by the
  pitch about X and then by the facing about Y. The ordinary spring
  (`FUN_140165700`) eases the eye and look point to those targets.
- **Collision** (`FUN_140165700`, modes 1, 7, 9, 10): the first surface on the
  segment from the look point to the eye (`FUN_140171210`, the same camera ray)
  replaces the look point.
- **Leaving** (`FUN_140165e30`, when Select is pressed again or the player no
  longer passes `FUN_1403ba300`; `FUN_140164870` when another camera mode such
  as lock-on takes over): if the mode is 1 the fade ramp restarts the other way
  (`FUN_1403b7810(player+0xa10, 0, 1.0, 30)`, value 0, step +1/30), the mode
  becomes 0, the yaw and its target snap behind the player, the look target is
  the player's position 170 up and the eye target comes from the field camera's
  placement (`FUN_140164520`); the spring eases the eye back.
- **Player input** (`FUN_1401650f0` is "camera mode is 1"). Its callers: the
  idle states' per-frame handlers `FUN_1403a70a0` and `FUN_1403a9250` turn Sora
  in place by +-0.05236 radians a frame (`FUN_1403b7a80`) while pad bit 21 or 20
  (right stick right / left) is held and a command is possible
  (`FUN_1403bfb30`: `obj+0x18c & 0x1c`, the range 0 to 2 window); and the two
  "is moving" predicates `FUN_1403a7830` / `FUN_1403a8f00` return false in first
  person, so the left stick starts no walk or run.

## Rewrite

- `follow_camera.rs`: `FollowCamera::first_person` and `pitch`; new
  `CameraInput` fields (`select`, `reset_pitch`, `look`, `first_person_ok`,
  `head_lift`); `tick_first_person`; entry and the lock-on drop in `tick`;
  tests for entry, the head eye and look point, the pitch and its limit, R3, the
  wall stop, and leaving.
- `ObjParams::head` / `Params::head_height` (PRTY word 12).
- `test_room.rs`: Select (logical bit 22) and R3 into the camera input; the
  right stick's vertical axis (or the mouse's vertical drag) as the pitch; Sora's
  turn in place; no walking; `first_person_allowed`; the fade ramp
  (`FadeRamp`, value 1 to 0 over 30 frames and back) applied to Sora's and the
  Keyblade's materials by `first_person_fade`.

## Checks, uncertainty, verdict and next task

PASS unit tests (the four camera tests above). PASS unattended run `fp1` with the
installed source, Select forced at ticks 200 and 520: the eye settled one unit
behind the head at (0, -150, 601) with the look point 350 ahead of it at
(0, -150, 251); the pitch followed up to the 70 degree limit and R3 levelled it;
Sora's alpha ramped 1 to 0 in 30 frames and back; after leaving, the eye returned
to the third-person position. PASS captures `fp3`: Sora translucent mid-fade,
absent in first person, and the view tilted up with the pitch.

Not verified: no installed side-by-side. `[I]` the sign of the turn (a right push
turns the view right; the executable's own yaw sign was not derived); `[I]` that
the left stick starts no movement (read from the two predicates, not captured);
`[I]` that the Keyblade fades with Sora (its attach routine copies the owner's
flag `+0x7b8` bit 6 outside camera mode 0); the entry frame keeps the field
camera's collision where the executable already uses mode 1's; `obj+0x18c` bit 14
and `obj+0x194` bit 1 of `FUN_1403ba300` are not modelled; the party members are
not faded. Which actions the executable lets run while in first person beyond
the turn and the movement were not traced (attacks and jumps are unchanged).

Verdict: PARTIAL. The camera follows the traced routines in the development room;
the input side is read from the predicates only.
Next: compare the look against the installed game; trace what `FUN_1403d65f0`'s
other callers gate.
