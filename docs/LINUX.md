# Running on Linux

KH2 Re:Written builds for Linux, and [T333](tasks/T333-linux-port.md) records how far that is proven. **Nothing has
run on a Linux machine yet.** The code passes clippy for the Linux target from Windows, but the build has never been
linked, the tests have never run there, and the game has never opened a window there. Expect to find problems on the
first run; [what to test](#what-to-test-first) lists them in order.

No game files are distributed. You need your own Steam copy of Kingdom Hearts HD 1.5 + 2.5 ReMIX.

## 1. System packages

Debian or Ubuntu:

```bash
sudo apt install build-essential pkg-config curl unzip libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-0 libx11-6 libxcursor1 libxrandr2 libxi6 libvulkan1 mesa-vulkan-drivers fonts-dejavu-core
```

Fedora:

```bash
sudo dnf install gcc gcc-c++ pkgconf-pkg-config curl unzip alsa-lib-devel systemd-devel wayland-devel libxkbcommon-devel libxkbcommon-x11 libX11 libXcursor libXrandr libXi vulkan-loader mesa-vulkan-drivers dejavu-sans-fonts
```

Arch:

```bash
sudo pacman -S --needed base-devel pkgconf curl unzip alsa-lib systemd-libs wayland libxkbcommon libxkbcommon-x11 libx11 libxcursor libxrandr libxi vulkan-icd-loader ttf-dejavu
```

The Fedora and Arch names come from those distributions' usual packaging and are not tested. Only the Debian/Ubuntu
development packages are exercised, by the CI job. `curl` and `unzip` are used only for Skate 3's sounds. With an NVIDIA
card, install its proprietary driver's Vulkan support instead of Mesa's; on Arch, also install your card's Vulkan driver
(`vulkan-radeon`, `vulkan-intel` or `nvidia-utils`).

Install Rust with [rustup](https://rustup.rs). `rust-toolchain.toml` then installs the pinned version on the first build.

## 2. Game files

Use either of these. Both work the same way, because the launcher checks the files themselves on every launch.

- **Steam on Linux.** Install the collection through Steam (Proton). The launcher looks in `~/.steam/steam`,
  `~/.local/share/Steam`, the Flatpak and Snap Steam folders, and every library they list.
- **A copy from Windows.** Copy the whole `KINGDOM HEARTS -HD 1.5+2.5 ReMIX-` folder to any Linux folder, then browse to
  it in the launcher. Keep the original file names and capitals. Only `KINGDOM HEARTS II FINAL MIX.exe` and
  `Image/dt/kh2_*.hed` / `.pkg` are required, but the launcher accepts the full folder.

Files copied from Windows are recognized as the same build, because a build's identity ignores the folder and file
times. The first launch on Linux still runs a full scan, because manifests are made per folder. It is read-only, like
the Windows scan. The game folder must stay outside the launcher's data folder.

## 3. Build and run

```bash
./build.sh
```

```bash
"target/release/KH2RW Launcher"
```

`./build.sh --debug` uses the faster development profile (`target/debug/`), and `--no-skate3` leaves Skate 3 out. If a
system package is missing, the script says which one before compiling.

The launcher's private files are in `~/.local/share/KH2FinalRewrite` (or under `XDG_DATA_HOME`), unless a
`.local/test-room` folder sits beside the checkout. Saves are in its `saves` folder. On first use, the original game's
save is read, never written, from the Proton prefix (`steamapps/compatdata/2552430/pfx/.../Documents/My Games/...`) or
from `~/Documents/My Games/...`. Key bindings from `config1525.dat` are found in the same places.

## Skate 3

Set the disc in the launcher as on Windows, and **choose the ISO**. An extracted disc folder needs its file names to
match the capitals the code expects, which Linux enforces and Windows does not, so that route is less safe.

The first ride downloads the pinned vgmstream r2117 Linux build into the Skate 3 cache, after checking its SHA-256
against the digest GitHub publishes. It is a static program, and its symbols include FFmpeg's XMA decoder, but it has
not been run on the disc's sounds.

## If something goes wrong

| Problem | Try |
| --- | --- |
| No window, or a Wayland error | Run with `WAYLAND_DISPLAY` unset to use X11: `env -u WAYLAND_DISPLAY "target/release/KH2RW Launcher"` |
| No GPU adapter found | Install your card's Vulkan driver, then run `vulkaninfo --summary` (from `vulkan-tools`) to check it |
| "Sound output unavailable" in the output | Your sound server needs its ALSA route: `pipewire-alsa`, or `libasound2-plugins` for PulseAudio |
| "no usable system font found" | Install `fonts-dejavu-core`, or any `.ttf` under `/usr/share/fonts` or `~/.fonts` |
| Pad not seen | Your user needs read access to `/dev/input/event*`; most desktops grant it, and `input` group membership is the usual fix |

Run the launcher from a terminal to see these messages. A failed test's last lines also appear in the launcher.

## What works differently from Windows

- **Wayland:** the launcher cannot hide itself while a test runs, because winit's `set_visible` does nothing there, so
  it stays on screen. Dropping a folder on the window does not work either; use Browse. Both work under X11.
- **Memory:** the Windows trim after a load (T313) has no Linux counterpart, so the resident size stays higher after a
  load. The game runs the same otherwise.
- **Launcher text:** without Trebuchet and Segoe UI it uses the nearest installed fonts (Liberation, Noto, then DejaVu).
  Those are wider, so long rows may fit differently.

## What was changed, and what still blocks

| Was blocking | Fix | State |
| --- | --- | --- |
| Bevy had no X11 or Wayland backend, so no window could open | `x11` and `wayland` features on Linux only (`source/main/Cargo.toml`) | Compiles; not run |
| Sound was waveOut only, so the game was silent | cpal device (`source/audio/src/cpal_device.rs`), ALSA on Linux | Compiles; not run |
| Launcher refused to start without Windows fonts | Searches Linux's font folders (`launcher/paint.rs`) | Search is unit tested; not run on Linux |
| Browse listed drive letters, so nothing appeared | Home, `/`, `/mnt`, `/media`, `/run/media` (`launcher.rs`) | Compiles; not run |
| `%LOCALAPPDATA%` lookups | `validation::host::local_data_root`, XDG on Linux | Path logic unit tested |
| Steam found only under Program Files | `validation::host::steam_roots`: native, Flatpak, Snap, XDG | Path logic unit tested |
| Saves and bindings found only under `%USERPROFILE%` | `install::game_documents`: Proton prefix, then `~/Documents` | Prefix path unit tested |
| vgmstream was the Windows zip, with `curl.exe` and `tar.exe` | Linux asset, its own digest, `curl`, `unzip`, executable bit (`skate.rs`) | Asset and digest verified; not run |
| `build.bat` and `tools/Build-Launcher.ps1` are Windows only | `build.sh` | Syntax only; not run |
| Smoke test required `LOCALAPPDATA` | `local_data_root` | Compiles; not run |
| CI only ran on Windows | `linux` job in `.github/workflows/ci.yml` | Not pushed, so not run |

These needed no change:

- **Rendering:** Bevy's default backend on Linux is Vulkan, the same backend it picks on the owner's Windows PC (T237,
  T313). No code selects DX12.
- **Gamepads:** gilrs is cross-platform, and no XInput code is reachable.
- **File names:** the install's 13 required names match the code's exact capitals, and every other game file is found
  inside the packages by hash, not on the filesystem. A check of 832 `include!`, `#[path]` and `mod` references found no
  wrong case.
- **Child programs:** the launcher restarts itself through `current_exe`, whatever the file is called. Its Windows flags
  are already behind `cfg(windows)`.

`tools/*.ps1` are private research tools and stay Windows only. `tools/progress.py` is cross-platform.
Kingdom Hearts: Connected has its own copy of the engine with the same blockers, and is not part of this work.

## What to test first

1. `./build.sh --no-skate3` links. This has never happened.
2. `cargo test --locked` passes on Linux.
3. `cargo run -p main --locked -- --frames 180` opens a window and exits cleanly, with no game files needed.
4. The launcher opens with readable text, finds the install or browses to it, and finishes the scan.
5. The title screen and test room render correctly under Vulkan on your GPU. Compare colors, the HUD, effects and the
   glow pass with Windows.
6. Sound plays without crackling. If it crackles, the 512-frame period in `cpal_device.rs` is the first thing to raise.
7. A gamepad works, including its button layout.
8. A save written on Linux loads, and the Proton save is found.
9. `./build.sh`, then the Skate 3 first ride: the vgmstream download, then decoding the disc's sounds.
