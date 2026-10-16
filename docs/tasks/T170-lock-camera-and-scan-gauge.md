# T170 — Lock-on camera (mode 2) and the Scan target gauge

Owner Claude (thread "Lock-on camera and Scan 1:1"), 2026-10-16. Gates P18/P21
presentation work (development test room); no final gameplay gate inferred.
Prerequisites: fidelity contract, milestones, unknowns, T060 (follow camera),
T125 (lock-on holder, Scan ability binding), the player HUD gauge (T10x).
User request: recreate the lock-on camera and the Scan health bar 1:1 with the
installed executable, starting from its decompiled routines, keeping controller
controls as they are and adding no letterbox bars to any cutscene. Branch
`claude/project-thread-1jp40h` from `main` at `1e60690` (package 0.1.2).
Selected KH2FM Steam app 2552430 build 15194255 English remains a quarantined
candidate; the executable hash used for every decompile below was checked equal
to the one bound by the manifest before the passes.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/lock-scan-01/`
(`lock-scan-spec.md`, `g/` decompiles, disassembly, vtable dumps, constants,
cross-references, `gaug-groups-38-46.txt`). Verification captures and logs are
in a private folder outside the checkout (`kh2-lockon-out/`). The installed
source stayed read-only; no vendor save, update or publication is involved.

## Steps and implementation

1. Headless Ghidra 12.1.4 (read-only copy of the existing private project, no
   analysis) decompiled the camera update (mode dispatch, the mode 2 routine and
   its screen test, zoom, bearing, eye target, radius return, eye collision, the
   spring, lock enter and leave), the lock manager and its Scan trigger, the
   damage events, the GAUGE_ENEMY object with its sprite groups, and the
   dk::LockOn reticle. Disassembly settled the control flow where the decompiler
   lost arguments. Addresses stay in the private spec and in code comments.
2. `source/game/player/src/follow_camera.rs`: the lock camera is now the
   translated mode 2. The look point blends 30 % from the player's upper body to
   the target's camera anchor unless the head is more than 300 away and the
   direction points more steeply than 0.5 downward (then it stays on the
   player). The target's and the player's head's screen positions (512x416
   layout, bounds 32..480 / 8..408, depth 1..4,000,000) set the yaw target to
   the player/old-look bearing with a 20 degree bias at a side edge, and each
   point below the bottom costs 2 radius per frame; the radius otherwise
   returns to the lock radius one unit per frame. Right-stick turn and zoom and
   the recentre press act in lock mode; the eye target stays on the player, its
   clip comes from the player's upper body, and the post-spring clip moves only
   the eye. The projection is derived from the executable's projection setup
   (focal 320/tan(fov/2) in the 640x480 virtual screen, 0.8 and 0.8667
   scales). MANUAL_TURN is now the executable's float (0.05235988).
3. `source/visual/src/enemy_gauge.rs`: the native GAUGE_ENEMY object on the
   `gaug` sheet (groups 38-46, root at layout (498, 31)). HP shows in rows of
   200: bars scaled by the row remainder, a full 200 bar when lower rows are
   complete, dark/lit tiles for completed rows stepping left by the tile
   group's text X, the damage trail and the fading tile of an emptied row.
4. `source/main/src/test_room/enemy_target.rs` and `test_room.rs`: the debug
   target panel is removed. A `ScanTracker` drives the gauge like the lock
   manager: a new target, or Scan being equipped, re-creates it from the live
   primary HP pool; the same target's falling HP is a damage event, rising HP a
   heal; no target or no Scan hides it.
5. `source/visual/src/lock_on.rs`: the native dk::LockOn reticle on the `fiel`
   sheet (groups 1/5 auto, 23/27 manual by HP pool), a 3D sprite at the held
   target's TARGET shape centre projected through the camera, truncated to
   integers, scale clamp(depth * 1e-5 + 0.4, 0.5, 1), drawn before the command
   menu (priority 4000). The lock plays se000 #7 as it engages (also when it
   moves to another actor) and #8 when the toggle releases it.
6. The combat tour keeps its three actors and lock schedule; it gains captures
   at 200, 250 and 320.
7. Camera collision now is the executable's camera ray: `collision::World::
   camera_ray` (the COCT segment query with the polygon filter cleared, so
   nibble-4 polygons count) feeds `follow_camera::clip`, which pushes the end
   5 units off the polygon's stored plane normal and, for a surface-5 polygon,
   re-casts toward the end's horizontal position at the hit height. Rooms keep
   their type-11 COCT records (`LoadedRoom::camera_collisions`); the generated
   arena keeps plain triangles.
8. `source/visual/src/hud_slots.rs`: the HUD slot-height table. The Scan gauge
   is slot 18 / sub-slot 1; its base y follows the sum of the lower slots'
   heights with the HUD spring (gain 0.4, spring 1.2, damping 0.5, rest 0.1).
   No popup registers a height in this build, so the base stays at 0.

## Checks, uncertainty, verdict and next task

PASS `cargo fmt --all --check`, warnings-denied locked workspace/all-targets
Clippy, locked workspace tests (all passing, including 12 follow-camera, 5
enemy-gauge, 4 reticle and 1 Scan-tracker unit tests), locked workspace build
and the 180-frame smoke (existing default controller-mapping warning).
PASS the three-actor combat tour with the installed source (private
`kh2-lockon-out/run-04*`): verdict pass, 12 damaging contacts to enemies and 2 to
Sora, Scan hidden then equipped, manual lock held and released. Captures were
visually inspected: the Scan gauge appears at the top right from the `gaug`
sprites, its bar shrinks with the target's HP, the blue (manual, primary pool)
and yellow (auto) reticles sit on the locked actor, and the camera pulls in from
420 to the lock radius 380 while locked. A separate offline render of the real
`gaug` sheet (650/1000 HP, then a hit to 420) shows the row tiles, the full
200 bar, the red trail and the fading emptied-row tile as the decompile
predicts.

Closed in the follow-up pass: the camera anchor is now the kind-7 record whose
group equals the held TARGET record's group (else the TARGET record), and the
release sound (#8) also plays when the held actor still exists but has become
untargetable. A Scan equip change while locked matches the executable: the
equip happens in the menu, whose global state releases the lock manager, and
the gauge is created at the next acquisition event.

Not reachable with the supported development actors (no observable difference
today, kept as open units): the reticle's side panel (group 20; needs the
target's counters at +0xd1c / +0xd20 above zero and a flag bit) and its hide
flag, secondary HP pools (such actors are rejected at load), camera modes
1/6/8 (area event cameras), the zoom gate (an actor flag that is never set).

Closed after that: camera collision uses the executable's camera ray (surface-5
re-cast, nibble-4 polygons, stored plane normals; unit tests on a synthetic
COCT, and a Twilight Town autoplay of 8520 ticks, 679 camera polygons, 126
clipped frames, no errors, framing inspected), and the Scan gauge follows the HUD
slot stack (spring and table ported with tests). The only objects that
register slot heights are popup classes (item and info windows, radar) that
the development room does not create, so the stack stays at zero here; nothing
else is modelled for them.

Still open: no controlled installed/rewrite side-by-side capture exists (needs
the installed game run in a controlled scene; the owner chose to continue
without one); the physical controller and R1/right-stick chords were not
exercised; the dynamic camera-collision object list (spheres of flagged
actors; the supported rooms create none) is not cast against. The decompiled
routines are private, single-pass and not independently audited, so coverage
counts do not change and no complete function or 1:1 unit is marked PASS.

Verdict: PARTIAL. The lock-on camera, Scan gauge and reticle execute translated
executable logic with real sprite data in the development room; the installed
side-by-side comparison and the open items above remain.
Next: when a reference capture is allowed, compare the installed game's lock-on
camera and Scan gauge numerically; port the dynamic camera-collision objects
and the popups that register HUD slots when those objects exist.
