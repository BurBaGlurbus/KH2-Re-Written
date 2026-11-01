# T333 — Linux port: everything that can be done from Windows

Owner Claude, 2026-11-01, requested by burba in the project thread ("i want to have this working on linux with no problems how can
we do that"). Gate: portability; no gameplay gate moves. Prerequisites: none. Build identity: local `main` after `e28a49b` plus this
change, package 0.1.2. Source: the installed Steam build was only read, for the names of its 13 required files; no scan or game run was
needed. Plain guide for the owner: [docs/LINUX.md](../LINUX.md).

Private evidence (outside the checkout): check logs in this session's scratch folder (not kept); the Linux-target build tree in
`%USERPROFILE%\kh2-linux-check-target`.

## Steps

1. Audited the workspace for Windows-only paths, crates, process logic, fonts, audio, build scripts and CI.
2. Fixed what code can fix, keeping each Windows path the same.
3. Checked the Linux target from Windows. WSL is not installed (`wsl -l -v`) and Docker is not on `PATH`, so nothing was linked or run
   on Linux.

## Blockers found, and the changes

| Blocker | Where it was | Change |
| --- | --- | --- |
| No window could open: Bevy's `x11` and `wayland` features were off | `source/main/Cargo.toml` | On, for Linux only. Features on a target-specific dependency do not reach the Windows build |
| No sound: `Output::open` returned an error off Windows | `source/audio/src/lib.rs` | `cpal_device.rs` (cpal 0.18.2, ALSA on Linux), behind the same `Device::open` as `winmm`. The stream stays on its own thread because it may not cross threads |
| Launcher refused to start: fonts read from `%WINDIR%\Fonts` only | `launcher/paint.rs:104` | Off Windows, searches the font folders; the Windows path is unchanged |
| Browse listed drive letters, which is empty on Linux | `launcher.rs:275,320` | `browse_roots`: home, `/`, `/mnt`, `/media`, `/run/media` |
| Steam found only under Program Files | `launcher/install.rs:138` | `validation::host::steam_roots` adds native, XDG, Flatpak and Snap folders |
| `%LOCALAPPDATA%` lookups | `install.rs:47,840`, `title.rs:742`, `test_room.rs:14339`, `bin/smoke.rs:24`, `data/examples/texa_graphs.rs:120` | `validation::host::local_data_root`, XDG off Windows |
| Original saves and bindings found only under `%USERPROFILE%` | `save/src/store.rs:107`, `pc_controls.rs:18` | `install::game_documents`: the Proton prefix in each library, then `~/Documents`. `store::original_folder` takes them as an argument, since `save` has no dependencies |
| Skate 3's vgmstream was the Windows zip, `curl.exe` and `tar.exe` | `skate.rs:298-342` | `vgmstream-linux.zip` with its own digest, `curl`, `unzip` and the executable bit |
| `build.bat` and `tools/Build-Launcher.ps1` only | repository root | `build.sh` |
| CI ran only on Windows | `.github/workflows/ci.yml` | A `linux` job with the same checks (not pushed) |
| Test that runs the built launcher named only `KH2RW` on Linux | `install.rs:1399` | Also `KH2RW Launcher` |

Windows-only code left as it is, already behind `cfg`: `windows_subsystem`, `CREATE_NO_WINDOW`, `K32EmptyWorkingSet` (T313's trim has no
Linux counterpart and returns false).

No change needed:

- **Renderer:** no code selects a backend, and Bevy already picks Vulkan on the owner's Windows PC (T237, T313).
- **Input:** gilrs is cross-platform, and no XInput code is reachable.
- **Game files:** the 13 required names were compared with the install's real capitals and match. Everything else is found by hash inside
  the packages.
- **Source identity:** it ignores the root path and file times and stores `/` in relative paths, so a copied install keeps its identity
  by construction. No Linux-made manifest has been compared.
- **Connected:** it has its own engine copy and no path dependency on this repository, so it is untouched and still has the same blockers.

vgmstream's Linux asset, as far as can be checked without running it: digest
`2f98c77f756079f63fbd119939067f1ed461d77e70993bc4cc372736d859c84a` from GitHub's asset metadata, matching a fresh download. It is a static
64-bit ELF named `vgmstream-cli` at the zip's root, mode 755, with FFmpeg and XMA strings in it. The download stayed in scratch storage and
was not executed.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` (Windows) | PASS |
| `cargo test --locked` (Windows) | PASS: 1191 passed, 0 failed |
| `cargo build --locked` (Windows) | PASS |
| Windows, `--workspace --features main/skate3`: clippy and test | PASS: clippy clean, 1213 passed, 0 failed |
| `cargo clippy --target x86_64-unknown-linux-gnu --all-targets --locked -- -D warnings` | PASS, with a stand-in `pkg-config` (below) |
| The same with `--workspace --features main/skate3` | PASS, also with `ZSTD_SYS_USE_PKG_CONFIG=1` |
| Linux-target check with no stand-in | FAIL as expected: only `wayland-sys`, `libudev-sys` and `alsa-sys` stop, because their build scripts need Linux's C libraries. Bevy and the rest of the graph compile |
| Exact-case scan of 832 `include!`, `#[path]` and `mod` references against Git's file list | PASS: 0 wrong case, 0 names differing only by case, no backslash paths |
| `sh -n build.sh` | PASS (syntax only) |
| Unit tests of the Linux path logic, font search and ranking, on Windows | PASS |
| Linking on Linux, `cargo test` on Linux, `build.sh`, smoke run | NOT RUN: no Linux machine, WSL or Docker here |
| Game, launcher, Vulkan rendering, sound, gamepad on Linux | NOT RUN |
| Skate 3 vgmstream on Linux | NOT RUN |
| GitHub `linux` job | NOT RUN: not pushed, on the owner's standing rule |

The stand-in is a script that answers every `pkg-config` question with `1.0.0`, plus `PKG_CONFIG_ALLOW_CROSS=1`. A check does not link, so
this proves our Rust code and the dependencies' Rust code type-check for the target. It proves nothing about the C libraries.

## Uncertainty

- The Linux path has never run, so a clean check says nothing about behavior there. `docs/LINUX.md` lists what to test, in order.
- Shaders and render targets were only exercised on the owner's Windows Vulkan driver. Mesa or another GPU may behave differently.
- The Fedora and Arch package lists are not tested. The Debian/Ubuntu development packages will first be exercised by the CI job.
- cpal asks ALSA for a 512-frame period, the same period as `winmm`, with two periods against waveOut's four. That may crackle under load.
- The launcher's layout was designed around Trebuchet and Segoe UI, and Linux fonts are wider.
- `dependency-inventory.csv` stays the Windows inventory; a Linux release needs its own (`docs/release/provenance.md`).

## Verdict

PASS for what can be done from Windows. BLOCKED on a Linux machine for "runs on Linux": no claim that it does.

## Next

The owner, or the GitHub `linux` job once pushed, runs the list in `docs/LINUX.md`. Fix what the first real build and run find.
