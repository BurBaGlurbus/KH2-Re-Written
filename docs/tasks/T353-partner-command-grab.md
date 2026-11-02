# T353 — Sora takes a script's jump, so the grab and the partner command run

Owner Claude, 2026-11-01, requested by burba in the project thread ("work through issues and close them"). Issue #138. Gate: development of reaction commands in the test room; no
final gate is inferred. Prerequisites: T211, T339. Build identity: local `main` after T352 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run;
the install was only read. Plain Ghidra 12.1.4 headless on read-only project copies.

Private evidence (outside the checkout): M_EX770's disassembly in `research\enemy-1to1-20261027\runs-20261030\lst\`; `research\reaction-1to1-20261023\out\core1\`
(FUN_1403b2340, FUN_1403b22d0, FUN_140402110, FUN_1404081c0); runs `hbj`, `hbk` under `%USERPROFILE%\kh2-issues-out\s162\`; private configuration `area-hb01-770.json`.

## What the executable does

- M_EX770's trigger 11 on command 27 hooks a thread on Sora: `method_obj_jump` and capture 252. Each tick it checks Sora: in the air, within 100 of height and 200 across the enemy.
  Then motion 241, act `rc_catch`, capture 253, attach, jump.
- The room never carried out a script's jump on Sora, and `obj_is_air` on him was false. That is why `rc_catch` never came in T339's 7000 ticks.
- Capture 253's range 0x19 offers command 28 at priority 0xcc, mode 7 (FUN_1404081c0).
- FUN_1403b2340 takes a command only below mode 4 (FUN_1403b22d0) and buzzes modes 5 and 6 (FUN_140402110). So Triangle does nothing at mode 7: the HUD shows the command, and Sora can't
  take it. `rc::press` already says the same.

## Changes

- `Sysobj::take_pending_jump`, `ScriptActor::sora_jumps`, `boss::requests`, `Player::script_jump`: the jump reaches Sora, after the capture that starts with it.
- `Env::player_air`, `HostFacts::player_air`: `obj_is_air` on Sora reads him.
- The area tour presses partner offers too, so the log shows what a press does.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| `KH2RW_AREA_ENEMIES=1`, `--enemy-combat-tour`, hb area 1 (`hbk`) | PASS: 27 offered and taken, `boss_sora_jump` height 100 speed 8, capture 252, then 22 ticks later act `rc_catch` and capture 253; command 28 offered, `source` Partner, mode 7, priority 204; presses log `reaction_ignored`; capture 256 follows at tick 2229 |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The executable's press path is read from code. Nothing here shows the installed game ignoring that press, and if a route to trigger 11's 28 exists elsewhere, I did not find it. L2297 in the script handles 28 and nothing in the room reaches it.
- `obj_attach` on Sora isn't carried out by the room, so he is not held at the enemy's bone during 253.
- `Player::script_jump` is only exercised here. Other scripts that call `method_obj_jump` on Sora now move him too; the Sephiroth and boss tours were not rerun for this.

## Verdict

PASS for #138's list in the room. No claim about the installed game.

## Next

The rest of the batch.
