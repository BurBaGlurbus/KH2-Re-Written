# T190 — Frame triggers 16 and 31: the command lockout and controller vibration (issue #100)

Owner Claude, 2026-10-18, requested by burba in the project chat (issue #100, the
frame triggers left open from T180). Gate: development of the test room only; no
final gameplay acceptance is inferred. Prerequisites: T094, T136, T180. Build
identity: branch `claude/combat-feel-command-lockout` on PR 106's branch (`main` at
`554d3bf`), package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM
build, validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/combat-feel-20261017/`
(decompiles `g/d1`, `g/d6`, `g/d7`, `g/d41` to `g/d46`, disassembly `g/a11`, `g/a12`,
references `g/r20` to `g/r24`); unattended runs `lk1` to `lk3` and `vb1` to `vb3` in the
private `kh2-combat-out/`.

## Frame trigger 16: the command lockout

- `FUN_14040dd70` case 0x10 stores its parameter (a u16) as a float in `obj+0xb94`.
  In Sora's motions it sits on the last frame of the attack and ability motions with 8
  ticks (the basic and ground attacks, motions 151 to 171), 14 (motions 14, 15), 20 to 26
  (the heavier ones: 43, 58, 64, 67, 70, 73), 24 (the ability air moves 191, 193, 194,
  204) or 32 (197).
- `FUN_1403b4980` (each frame): while `obj+0xb94` is above zero it counts down by one
  when a range 0, 1 or 2 is active (`obj+0x18c & 0x1c`) and is cleared when none is.
- `FUN_1403ba070`, the command gate (called by the attack, Drive, item and ability
  commands): its main path needs `obj+0xb94 <= 0` besides an active range 0 to 2.

So an unchained attack or ability that runs to its end leaves Sora in Idle with
commands refused for that many ticks.

## Frame trigger 31: vibration

- `FUN_14040dd70` case 0x1f → `FUN_1403eefc0(id)`: a binary search of the id (a byte)
  in the `vibr` table (two-byte rows: id, wave number; header u32 version, u32 count) of
  the 00battle archive's `vbrt` block (`FUN_1403eef80` stores it at load), then the n-th
  type 47 entry (`FUN_1403a6420(block, 0x2f, n)`) is started by `FUN_14019ae60`.
- A wave (`VIBD`, version 0x100): counts at +8 and +0x10, key offsets at +0xc and +0x14;
  two tracks of `(time, value)` i32 keys. The player (`FUN_14019b1d0`, once a frame):
  state 1 starts at time 0, state 2 adds 0.5 a frame, a time at or past the largest key
  time stops it (the time goes back to length - 1); each track with two keys or more and
  a first key not after the time writes its motor byte (`pad+0x48`, `pad+0x49`): the
  value of the first key at or after the time, times how far the time is through the
  segment from the key before it.
- The PC layer (`FUN_14039bbe0` → `FUN_140135080` → `FUN_140132500` →
  `XInputSetState`) clears both bytes each frame, takes byte 1 as the left (strong)
  motor, makes the right (weak) motor fully on when byte 0 is above 0x7f, and does all
  of it only with the config's vibration bit (`FUN_1402e53f0`, default on).
- The block holds nine waves named auto, bliz, smal, blow, refl, bliz, v0lo and two
  l_bo; ids 1 to 5 take the first five, 6 and 7 the two l_bo waves, 8 the second bliz and
  9 v0lo.

## Rewrite

- `player::vibration` (`Wave`, `Library`, `Playback`, `motors`) with tests on synthetic
  bytes; `Clip::vibrations` and `Clip::command_lockout` from the frame triggers.
- `Player::command_lock`, counted and cleared by `FUN_1403b4980`'s rule and gating every
  command (`command_window`) while steering keeps the plain range.
- The room reads the `vbrt` block of the battle archive, starts waves from Sora's
  motions (log event `vibration_start`) and sends the motors to the first gamepad through
  Bevy's rumble requests once per simulation tick.

## Checks, uncertainty, verdict and next task

PASS unit tests (`frame_trigger_16_refuses_commands_for_its_ticks_inside_a_range`, the
wave ramp, motor mapping, table search). PASS unattended runs with the installed source:
the lockout of 8 ticks appears in Idle after the unchained attacks (`lk3`); the combat
tour's action sequence is unchanged because its presses are 20 ticks apart (`lk1`);
waves 3 and 4 played from Sora's motions with ramps up to 140 and 122 (`vb3`).

Not verified: no pad was attached, so the rumble itself (the effect Bevy and gilrs
produce from the requests, one short effect per tick) was not felt or measured; the
order of the countdown against the trigger inside a frame is inferred; the gate's other
branches (a flag-bit bypass, the object-flag cases) are not modelled; the room has no
Config page, so the vibration bit is taken as on; other actors' motions that carry
trigger 31 are not read, nor are the two other callers of `FUN_14019ae60`
(`FUN_1402da350`, `FUN_14020fdc0`).

Verdict: PARTIAL. Both triggers execute the traced routines in the room.
Next: the other starters of vibration; owner feels it on a pad.
