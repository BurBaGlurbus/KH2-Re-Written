# T176 — World map: the events' character scenes

Owner Claude (thread "World map rewrite"), 2026-10-17. Issue #89 (part of
#81). Gate: none passed; PROGRESS toward the world map work. Prerequisites:
T175 (the event player). Branch `claude/project-thread-9zxl2w-widgets`,
stacked on `claude/project-thread-9zxl2w-events` (T175). Selected KH2FM
Steam app 2552430 build 15194255 English remains a quarantined candidate.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/worldmap-20261016/`
(decompiles `out/wid1`..`out/wid11`, `out/vt-wid`, `out/vt-mot`; disassembly
`out/dis-f270`, `out/dis-wid`, `out/dis-b10`; dumps `out/data-13.txt`..
`out/data-15.txt`, `out/str-orig.txt`). Copies of the event archives, the
event scripts and the captures below are in a private folder outside the
checkout. The installed source stayed read-only.

## Steps and implementation

1. `world_map::widgets`: worldmap::ObjWidget over menu::ObjWidget
   (FUN_140393920, FUN_140535040) in the event object's four slots.
   - Ops 0x102 / 0x10d / 0x112 create and remove widgets
     (FUN_14038ee70, FUN_14038f1f0, FUN_14038f150).
   - The first widget hides the map (FUN_1401625f0). When the last one
     goes, the map shows again, the draw group set applies and the objects
     refresh (FUN_140162460, FUN_1403f0b70, FUN_140394050).
   - The event object's teardown clears the widgets (FUN_14038eda0).
   - Ops 0x109 / 0x10a set a widget's position and rotation.
   - Ops 0x10b / 0x10c set the shared view (FUN_140393ad0 / FUN_140393b10,
     FUN_1403b7410).
   - The widget matrix is rotation X·Y·Z, then the position, then the view
     (FUN_1403939f0, FUN_1401aae60, FUN_14013f850). The rotations are
     FUN_1401411b0 / FUN_140141270 / FUN_140141320.
   - The motion player (FUN_1405359d0 / FUN_140535ac0 / FUN_140535b00 /
     FUN_140535b10) plays an archive entry, queues up to four more, loops
     the last at the motion's return point, and fires the type-8 frame
     triggers as sounds `bank × 1000 + number` (LAB_1405359b0,
     FUN_14040e8a0).
   - Ops 0x103 and 0x111 start and queue motions.
2. `world_map::events`:
   - Op 0x101 names the event's archive `libretto/<lang>/wm<event>.bar`
     (format 0x1405c0798, language strings 0x1405b142c / 0x1405b1444).
     It is not a BGM stop: FUN_14039d460(0) drops unused cached files.
   - Op 0x106 waits for the loads.
   - Op 0x108 plays an archive entry's voice: an "ORIGIN" stub naming the
     voice file (FUN_1403a64f0, FUN_1401dfbb0 → FUN_14013aa60) at volume
     0x3fff, after the last voice stops.
3. `kh2fm wm`:
   - At load, it reads every queued event's archive (motions, clips,
     voices), the models the scripts put in widgets, the voice files and
     the sound banks the motions' triggers name.
   - While widgets show, the map root hides and the main camera becomes the
     widget camera (focal 512 on 640 × 480, near 1, FUN_14019dc50
     arguments).
   - Each widget model draws at its matrix on its motion.
   - Voices and trigger sounds play through the mixer.

## Checks

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked` (763 passed, 0 failed),
  `cargo build --workspace --locked` and the smoke run
  `cargo run -p main --locked -- --frames 180` passed on this branch.
- New unit tests on generated values: the motion queue and loop with
  trigger sounds, the map hiding and returning, the view's turn and move,
  the widget op routing and the voice stub names, and the scene matrix's
  axis flip.
- Installed unattended runs:
  - `--event 0x1f6`: Chip and Dale's first call shows the Gummi hangar with
    Chip and Dale at the console. Seven voices play as the messages
    advance, and their footstep sounds (16010..16012) fire from the
    motions. The widgets go and the map refreshes at tick 388. The camera
    flies to Twilight Town. The cockpit scene with Sora, Donald and Goofy
    follows at tick 598.
  - `--event 0x200`: the cockpit scene and op 0x112.
- NOT RUN: a side-by-side comparison with the installed game. The
  characters' facing is inferred (see below).

## Uncertainty

- Widget model space: the scene uses the plain Y/Z flip of the world. The
  field actors' π − yaw turn faces every character away from the widget
  camera, while the plain flip faces them to it as the scenes are staged
  [I].
- The voice stub's file comes from the observed install
  (`voice/us/event/<name>.win32.scd`). The PC remap in FUN_14012db50 is not
  traced.
- Not carried out:
  - Range triggers (type 0x17, FUN_1401a8a20 on the model, tested by the
    exe against the tick length).
  - The widget's draw parameters other than its neutral colour.
  - What the hidden map leaves behind (FUN_140170fe0's flag). The scene
    shows the clear colour there.
- Hollow Bastion's effect (op 0x110) and the other object effects remain
  for the effects work.

## Verdict

PROGRESS (no gate). Next: the world map object effects (symbol effects,
the new-episode effect FUN_140390460, op 0x110's effect 0x9c).
