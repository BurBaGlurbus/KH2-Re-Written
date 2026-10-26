# T256 — The PC version's keyboard and mouse, and two windows on one PC

Owner Claude, 2026-10-26, requested by burba ("import the pc versions kbam controls so its 1:1 and make it so
testing 2 windows on the same computer they dont control each other at the same time so like 1 controller and 1
keyboard"). Gate: none of the roadmap's for the two-window part (a rewrite option); the bindings and mouse-look
follow the installed executable. Prerequisites: T240-T249 (LAN duel). Build identity: worktree
`KH2FinalRewrite-lan`, branch `feature/lan-duel` after the merge of local main (0f178b4); package 0.1.2, Rust
1.99.0 Windows MSVC. Source: the selected Steam collection's `KINGDOM HEARTS II FINAL MIX.exe`, read only with
headless Ghidra 12.1.4 (project copy `ghidra-kh2-c4`, read-only); the player's own `config1525.dat` read only
(never written).

Private evidence: `%USERPROFILE%\kh2-lan-out\input-research\` (decompiles `out\d1`..`d7`, data dumps `dd1`-`dd3`,
the config initialiser's disassembly `a1.txt`); earlier notes in the private `bgm-input-01` spec (B1.5). Run logs
`kbm1-*`, `kbm2-*`; shots in the worktree's ignored `.local\lan-review\controls\`.

## What the executable does

| Source | Finding |
| --- | --- |
| 0x14014ce50 (a new config's defaults), FUN_1404ea260, FUN_1404e6d10 (the loader) | Two codes per action, DirectInput DIK codes, 0x1000 + n mouse button n. `config1525.dat` holds them at 0x3c (eight bytes a slot, in the order below), the camera's four codes after them, the confirm layout at 0x32 and the mouse sensitivity at 0x38 (u16, 20). The loader keeps each slot's default when the file's code is not a key or mouse code. |
| FUN_14014f010, FUN_1404e5650(1) | At boot: the config is loaded, the mouse enabled, preset 1 applied (the default table), then the config's own codes. |
| FUN_1404e94c0(1, 0x60) | W / A: -128, S / D: +127 on the left stick; with the walk key held -96 / +96. |
| FUN_140102ca0, FUN_1404eb6d0 | Held codes OR their PS2 button into pad 0; a binding to Enter also answers to the numpad's Enter; keys bound to the camera move the right stick. AZERTY and QWERTZ layouts swap a few letters (not done: Bevy's key codes are positions, as DIK's are). |
| FUN_140105810 | The mouse: DirectInput's relative motion over 200 (the wheel over 400), clamped to -1..1, read only while the window is active and the cursor is inside it. |
| FUN_1404e51f0, FUN_14039c180 | A frame with mouse motion puts clamp(24 dx), clamp(20 dy) on the right stick (dead zone one byte unit) and multiplies its magnitude by 1 + sensitivity x 0.05 x 0.4. |
| FUN_140166100 | The camera turns max(10 abs(dx), 1) times its 3 degrees a frame (height: max(10 abs(dy), 1)). |
| FUN_1404ebd30(1) | A scroll is one D-pad press (up or down). |

### The bindings (defaults, also this PC's `config1525.dat`)

| Slot | Button | First | Second |
| --- | --- | --- | --- |
| 0 | walk | Left Alt | - |
| 1 | Cross (decide, attack) | left click | Enter (+ numpad Enter) |
| 2 | Circle (back, jump) | Space | Backspace |
| 3 | Triangle (reaction) | F | Right Ctrl |
| 4 | Square (guard) | right click | Right Shift |
| 5 | L1 (shortcuts) | Left Shift | - |
| 6 | R1 (lock-on) | R | - |
| 7 | L2 | C | - |
| 8 | R2 | E | - |
| 9 | L3 | X | - |
| 10 | R3 (recentre) | middle click | Right Alt |
| 11 | D-pad right | Right arrow | mouse forward (button 4) |
| 12 | D-pad down | Down arrow | wheel down [I] |
| 13 | D-pad up | Up arrow | wheel up [I] |
| 14 | D-pad left | Left arrow | mouse back (button 3) |
| 15 | Select (first person) | N | 3 |
| 16 | Start (menu) | M | 4 |
| 17-20 | left stick up, down, left, right | W, S, A, D | - |
| camera | right stick left, right, up, down | the mouse's motion | - |

[I] the codes 0x1009 / 0x100a: DirectInput reports eight buttons, so these are read as the wheel.

## What changed

- `visual::input`: the bindings above (`Bindings`, with `from_config` for the player's file), a DIK-to-key table,
  the exact stick bytes (W+S cancel to 0x7f as the exe's sum does), the side buttons on the D-pad, 3 and 4 as
  second Select and Start, keys bound to the camera, and `mouse_look` (the model above).
- `pc_controls.rs`: finds the newest `config1525.dat` under Documents\My Games\KINGDOM HEARTS HD 1.5+2.5 ReMIX
  (OneDrive too) and reads its bindings; unattended runs keep the defaults so they read the same everywhere.
- The room's camera: mouse motion counts are kept per frame (only with the window focused and the cursor inside
  it, as the PC version) and each tick turns them into the camera's turn and height (first person: its pitch)
  through `mouse_look`, in place of the old per-pixel turn. The room's own Camera Speed setting now multiplies it,
  5 being the PC version's speed.
- **Two windows**: `--input auto|kbm|pad|pad:N|both` and a launcher Multiplayer row, Controls (Auto, Keyboard and
  mouse, Controller, Both; saved). A window given a controller ignores the keyboard and mouse (keys, buttons,
  mouse-look) and keeps reading its pad when not focused (pads are read whatever the focus); one given the
  keyboard and mouse ignores every pad. Auto (the default): when a duel connects and the other player's address
  is this PC's (loopback or one of its own), the host keeps the keyboard and mouse and the joiner takes the
  controller; otherwise both read everything. The window's title and the lobby's Match page (Controls) say what
  a window reads; a controller-only window shows a pad's button art and prompts from the start. Nothing grabs or
  hides the cursor.
- Tests: the defaults read back from a default file, a rebind and bad codes, every default code valid, mouse-look's
  numbers (10 counts: 1.4; 100 counts: 7; one count up: a small upward turn), `--input` parsing, the config
  search (newest file, others left), the launcher setting's round trip.

## Checks

- fmt, clippy, `cargo test` (all passed): PASS.
- Loopback (`kbm1`, `kbm2`; default `--input auto`): the host logs `input_devices` keyboard and mouse, the joiner
  controller; the joiner's lobby shows Controls: controller with pad art (`joiner-controller-lobby.png`; before
  the fix it showed the keyboard's, `joiner-before-pad-glyphs.png`). PASS.
- Single-player tours against a release build of 0f178b4 (the merged head before this work, built in its own
  detached worktree and target): talk, magic and command deck identical; party-hit identical but for the exit
  code, which is the baseline's (its captures at 6538 and 6788 came out uniformly black, twice; this build's did
  not; the baseline's checkout had no `.local` folder, the likely difference, not traced). A first talk run of this
  build died at startup on a GPU device loss while the baseline compiled; rerun on a quiet machine: identical.
  PASS.
- Physically pressing keys, the mouse or a pad: NOT RUN (the owner's desktop is not driven from here; a game
  controller is attached to this PC). The mouse-look feel against the installed game: NOT RUN.

## Uncertainty and next

- The two wheel codes (0x1009 / 0x100a) are read as the wheel [I]; the four 10-valued words before the bindings
  (file 0x28) are not traced; keyboard presets 2-6 and the AZERTY / QWERTZ swaps are not done.
- Next: a player's check of mouse-look and two windows with a real pad.
EOF
echo ok