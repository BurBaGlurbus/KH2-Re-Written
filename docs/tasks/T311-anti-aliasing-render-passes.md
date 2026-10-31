# T311 — Twilight Town quit with anti-aliasing on: the sun's probe and refraction

Owner Claude, 2026-10-31, requested by burba in the project thread: "the game crashes when i enter twlight town for a few seconds". Gate: none of the roadmap's; a crash fix in passes added by T291-T295. Prerequisite: T310. Source: Steam KH2FM build as before, only read; `.local/test-room/test-room-tt.json` with the owner's own `kh2fm-settings.json` (`anti_aliasing: true`).

## Cause

Not the warnings on the error screen. `CommandQueue has un-applied commands` comes from the world being dropped after Bevy has already decided to quit. The first lines of the same output hold the failure:

```
In Device::create_bind_group, label = 'lens probe group'
  Texture binding 0 expects multisampled = false, but given a view with samples = 4
...
Quitting the application due to Validation RenderError
```

The Anti-aliasing setting puts `Msaa::Sample4` on the room's scene camera (`settings_room.rs`), so its depth buffer has 4 samples. `lens_probe.rs` bound that buffer as a one-sample `texture_depth_2d`. The probe runs only when a map's sun program asks for it, and Twilight Town's does, about 3 s after the first frame. Bevy quits on a wgpu validation error.

The unattended runs never saw it: they keep the default settings, where anti-aliasing is off.

`refraction.rs` had the same flaw in another form: a one-sample colour attachment with the scene's 4-sample depth attachment. It would quit the same way the first time a refraction effect drew with anti-aliasing on.

## What changed

- `lens_probe.rs`, `lens_probe.wgsl`: a second pipeline and layout for multisampled depth (`texture_depth_multisampled_2d`, behind `MULTISAMPLED`), chosen by the depth texture's sample count. The block reads sample 0.
- `refraction.rs`: the pipeline key carries the view's samples, and the pass draws through `ViewTarget::get_color_attachment`: the multisampled scene, resolved back into the main texture. With anti-aliasing off that attachment is the main texture, as before.

Glow and its reset read and write the resolved main texture at one sample, and were not changed.

## Checks

The scripted runs used the room's real Twilight Town config. A temporary patch, removed before the commit (the diff was byte-compared afterwards), gave these switches: draw a test triangle through the refraction pass, force anti-aliasing in an unattended run, select the old refraction attachment, and print the probe's answers.

| Check | Verdict |
|---|---|
| Before: the old build, anti-aliasing on, Twilight Town | FAIL as reported: quit about 3 s after the first frame with the `lens probe group` error, then 29 `CommandQueue` warnings |
| After: the committed code, the same run left for 75 s | PASS: still running when stopped; 0 errors, 0 warnings of that kind |
| The probe on 4-sample depth, 45 s | PASS: 628 answers, mostly 0, with 51 of 1 and some near 0.78 |
| Refraction with anti-aliasing on, old attachment | FAIL as predicted: `Attachments have differing sample counts`, then the same quit |
| Refraction with anti-aliasing on, fixed, 45 s | PASS: 3333 passes on 4-sample views, 0 errors |
| A capture of the triangle with anti-aliasing on | PASS by eye: the frame shows displaced inside the triangle, and the scene around it is intact |
| Unattended run with anti-aliasing off (the default path) and the temporary probe print | PASS: exit 0, 0 errors, the probe still answers |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS: 1134 passed, 0 failed |
| `cargo build --release --locked --features main/skate3 -p main --bin KH2RW` | PASS |
| `tools\Build-Launcher.ps1 -Skate3` | PASS |

An unattended run with anti-aliasing forced first exited with code 1. Its capture at tick 120 fell in the black of the fade-in (`uniform: true`), and the same happened with anti-aliasing off. With the capture at tick 220 it exited 0.

Evidence: `%LOCALAPPDATA%\KH2FinalRewrite\logs\T311-anti-aliasing-render-passes\` (private: logs and captures).

## Uncertainty

- No real refraction effect was seen with anti-aliasing on, only the test triangle through the same pass.
- Twilight Town was not walked through: the 75 s stood at the owner's start, Station Plaza, and the scripted runs went only as far as their autoplay does.
- The owner's own launch through the launcher's menu was not run; the same program and arguments were.
- Not compared with the installed game; the exe has no multisampled depth to compare with, so which sample the block reads is this rewrite's choice.

## Open

The launcher's error screen hid the cause: `failure_lines` filters on `" WARN "`, but the child's output reaches it with colour codes (`[33m WARN[0m`), so the warnings pass and the last six lines are all warnings. Not changed here.

## Verdict

PASS for this crash: Twilight Town with anti-aliasing on no longer quits, and refraction with anti-aliasing on no longer can in this way. No gameplay gate moves.

## Next

The launcher's `failure_lines`, so the first error shows instead of the last warnings.
