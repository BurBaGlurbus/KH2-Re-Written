# T339 — The partner field's writer; M_EX770 fights in a room; Twilight Town's enemies on the map

Owner Claude, 2026-11-01, requested by burba in the project thread ("get more issues on github done"). Issues #138 and #26 (research and
runs, one comment changed). Gate: development of combat in the test room; no final gate is inferred. Prerequisites: T211, T266, T269, T283.
Build identity: local `main` after `c4f64bc` plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the
install was only read. Plain Ghidra 12.1.4 headless on read-only project copies.

Private evidence (outside the checkout): `research\issues-20261101\g\e3\`, `e4\`, `e5\` (FUN_1403c6cc0, FUN_1403c64a0, FUN_1403c65e0, FUN_1404081c0);
runs `hb1`, `hb2`, `tt13` and `placements.txt` under `%USERPROFILE%\kh2-issues-out\s161\`; private configurations `area-hb01-770.json` and
`area-tt13-01.json` under `.local\test-room\`.

## What the executable does

A capture writes the partner: FUN_1403c6cc0 → FUN_1403c64a0 → FUN_1403c65e0 stores its last argument at ctl+0x158, and the motion controller
sits at obj+0x158, so that is obj+0x2b0. The argument is built from the capturing object. The reaction search reads +0x2b0 only while Sora's
+0x18c bit 20 is set (range 0x19), for command +0x638 (FUN_1404081c0). So the room's rule, "the object holding Sora's capture is the partner",
stands on code. The exact field at captor+0x5b8 is a reference of that object whose form was not read.

## Change

`boss.rs`'s comment says so. No behaviour changes.

## Checks

| Check | Result |
| --- | --- |
| `KH2RW_AREA_ENEMIES=1`, `--enemy-combat-tour`, hb area 1, programs [261, 1, -1], 2400 ticks (`hb1`) | PASS: M_EX770 stands on its spawn with its bank; its command 27 is offered and taken, and capture 252 plays on Sora. #138's loader blocker is gone since T266 |
| The same, 7000 ticks (`hb2`) | M_EX770's `rc_catch` (slot 253, command 28) did not come in this run: NOT RUN for the partner command itself |
| The same setup, tt area 13, programs [235, 4, -1], 1500 ticks (`tt13`) | PASS: M_EX010, M_EX020 and M_EX700 run on the Twilight Town map with no faults; no enemy died in that length |
| `enemy_placements` | The TT spawn sets by battle program, for the record: tt03..tt41 hold 24 enemy models |
| Side by side with the installed game | NOT RUN |

## Uncertainty

`KH2RW_AREA_ENEMIES` is still a switch, so a normal run has no standing enemies. T298's eviction of enemy archives is unbuilt, and the owner
paused the enemy work (#163), so the default was left alone. That is the owner's decision.

## Verdict

PASS for the trace and the runs. BLOCKED for #138's command 28 on getting the grab in a tour. #26 waits on the default.

## Next

A tour that forces M_EX770's `rc_catch`; the owner's decision on turning standing enemies on.
