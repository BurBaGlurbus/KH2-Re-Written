# T257 — The 3D scene froze under a live HUD after the window changed size

Owner Claude, 2026-10-26, requested by burba in the project chat ("sometimes in game the game frame render will
freeze but everything will still move around, jump and attack, and UI updates, but visually it's frozen. This
happens randomly. Please fix it."). Gate: development of the test room's presentation; no gameplay rule changes and
no final gate is inferred. Prerequisites: T229 (the scene composed by the 2D layer), T236 (the frame-rate work).
Build identity: local `main` after `7405db8`, package 0.1.2, Rust 1.99.0 Windows MSVC, release launcher with the
`skate3` feature. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence: the probe script, both executables' window captures (`cap-old/`, `cap-new/`, each with the run's
`err.txt` log) and the check logs (`checks/`) in this session's private scratch folder outside the checkout.

## Cause

The test room's 3D camera draws into an image (`Compose::scene`) that the 2D layer draws full screen under the HUD.
`resize_scene` resizes that image whenever the window's size (times the Resolution setting) changes. Bevy gives an
image whose descriptor changed a new GPU texture, but a prepared 2D material keeps the texture view it was built
with and is only prepared again when the material itself changes. After any resize the camera drew into the new
texture while the 2D layer kept drawing the old one's last picture: the scene stopped, the HUD (its own image) and
the simulation went on. Window size changes come from resizing the window, maximizing, minimizing and restoring,
alt-tab out of full screen, a monitor or DPI change and the Resolution setting, which is why it looked random. The
Drive afterimage samples the same image through its own materials and froze the same way.

The other suspects were read and ruled out for this symptom: Motion Smoothing only blends transforms (it cannot hold
a picture while actors move), the frame cap and winit update modes stop the whole window (HUD included), and the HUD
tile upload writes only the HUD's own texture.

## What the rewrite does now

- `visual::overlay` (`rebind_resized`): when an image's texture descriptor changes, every `LayoutMaterial` sampling it
  is marked changed so Bevy prepares it again with the new texture. It logs
  `2D layer: N image(s) changed size, M material(s) rebound`.
- `resize_scene` skips a minimized window (no size) instead of shrinking the target to 1×1, resizes the afterimage's
  two pictures with the scene (they kept the startup size) and logs `scene target resized to W×H`.

## Checks

| Check | Result |
| --- | --- |
| Live, pre-fix release exe: room via the launcher's room arguments and the owner's settings; walk with posted keys, resize the window (no focus), minimize/restore; mean pixel difference of the scene area | FAIL as reported: walking moves the scene (15–20) before the resize; after it 0.33 / 0.00 while the Form gauge still changes; still frozen after restore |
| Same run on the fixed release exe | PASS: 19 / 17 before, 18 / 21 after the resize, 38 / 39 after minimize and restore; the log shows the resize and 5 materials rebound |
| New unit test: a resized image rebinds its material, a same-size rewrite and other images do not | PASS |
| `cargo fmt --all --check` | PASS (after formatting the new code) |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS (1116 tests) |
| `cargo clippy --workspace --all-targets --features main/skate3 --locked -- -D warnings` | PASS |
| `cargo build --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release launcher with `skate3` | PASS |

## Uncertainty

- The owner's freeze was not seen on their machine; the reproduced one matches the report (scene still, HUD and play
  live) and has one cause in the code. If the scene freezes with no `scene target resized` line just before it in the
  log, it is another cause.
- Exclusive full screen alt-tab, DPI changes and the Resolution setting reach the same path but were not run.
- Other render-target images (the cutscene scene, the snapshot and menu targets) are never resized; they are covered
  by the same rebind if that changes.

## Verdict

PASS for the reproduced freeze in the test room. No final gate.

## Next

None for this bug unless the owner sees it again without a resize line in the log.
