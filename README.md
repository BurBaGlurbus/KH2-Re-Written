# Kingdom Hearts II Re:Written

<p align="center">
  <img src="source/visual/assets/kingdom-hearts-final-rewrite-logo.png" alt="Kingdom Hearts Final Re:WRITE logo" width="850">
</p>

A native Windows engine rewrite of **Kingdom Hearts II Final Mix** in Rust, using [Bevy](https://bevy.org). It reads your own Steam installation at runtime. No game assets are distributed.

> **Status: very early.** All worlds playable and explorable and works as a sandbox.

> **DISCLAIMER:** If you haven't noticed, this is entirely vibecoded using Claude AI. If you do not like it then you can simply move on with your life.

## What works

- Sora walks, runs, jumps, lands and attacks with his own animations and Kingdom Key, in a Twilight Town room.
- Movement numbers are read from Sora's installed parameter records. Some values, such as gravity and body size, are still test-room choices, so the room is not an exact copy of the game.
- Enemies, damage, sound, saves and the rest of the campaign are not implemented.

For scope and progress, see the [fidelity contract](docs/fidelity-contract.md), [milestones](docs/milestones.md) and [unknowns](docs/unknowns.md).

<!-- KH2FM-PROGRESS:START -->
### Verified progress

| Measure | Accepted / tracked | Percentage |
|---|---:|---:|
| Complete functions decompiled and checked | 0 / 228 (inventory incomplete) | Unmeasured |
| Required behavior scenarios verified 1:1 | 0 / 9 (inventory incomplete) | Unmeasured |
| Final roadmap gates accepted | 0 / 32 | 0.00% |

Whole-game 1:1 coverage is **unmeasured** until the complete function and behavior inventories are qualified. Partial work, synthetic tests and successful builds do not count as 1:1 passes. Gate percentage measures accepted gates, not game size or implementation effort.

[Progress details](docs/progress/README.md) · [Coverage ledger](docs/progress/coverage.json) · [Automatic updates](.github/workflows/progress.yml)
<!-- KH2FM-PROGRESS:END -->

## Find your way around

Start with the [folder guide](docs/folder-guide.md) for common edits and naming rules.
`source/game/` holds gameplay, `source/visual/` holds rendering and menus, `source/visual/effect/` runs effects,
and `source/main/` holds the launcher and application. `source/data/` reads game formats,
`validation/` validates the installation, and `source/audio/` handles sound output.
Each main folder has a short README. See [ARCHITECTURE.md](ARCHITECTURE.md)
for package boundaries and current limitations.

## Quick start

You need Windows, your own **legally obtained** Steam copy of Kingdom Hearts HD 1.5 + 2.5 ReMIX, and the pinned Rust toolchain. See [CONTRIBUTING.md](CONTRIBUTING.md#setup).

```powershell
powershell -NoProfile -File tools/Build-Launcher.ps1
& '.\target\release\KH2RW Launcher.exe'
```

The first launch opens a short quick start guide (F1 in the launcher brings it back). The launcher then finds your Steam install (or lets you browse to it), checks that it is a supported build, makes the test configs from your installed files, and offers the title screen and test room. Each launch re-checks your files.

Command-line runs also work:

```powershell
cargo run -p main --bin KH2RW --locked -- room '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/test-room.json'
```

`title` and `talk` take the same arguments with their own configuration. The configuration format is in [T056](docs/tasks/T056-playable-test-room.md).

## Controls

| Action | Keyboard | Controller |
|---|---|---|
| Move | WASD | Left stick |
| Walk | Hold Left Alt | Tilt the stick slightly |
| Camera | Mouse | Right stick |
| Lock on / release | R | RB |
| Cycle locked target | | Hold RT and tilt right stick left/right |
| Attack | Enter, left click | A |
| Jump | Space | B |
| Guard | Right Shift, right click | X |
| Talk | F | Y |
| Shortcuts | Hold Left Shift | LB |
| Skateboard (needs the Skate 3 ISO) | J | |
| Collision overlay | F1 | |
| Debug options, summons and Scan | F3 | |
| Leave the test | Esc | |

In the generated arena, F3 → Summon enemies → Summoned actors lets you enable
AI and combat for ordinary Shadow, Dusk and Soldier. Other enemies currently
support animation inspection. F3's Scan option toggles the learned ability;
equipping it shows the selected enemy's live HP in the original Scan gauge at the
top right. While locked on, the camera follows the original lock-on camera
(right stick still turns and zooms it). The lock-on reticle sprite is not drawn
yet. Full roster combat and installed-game comparisons remain unfinished.

## LAN duel (in progress)

Two players on the same network can each run the game and fight each other's Sora in the test room. This is
the first step of a 1v1 duel mode: the lobby, the duel loadout, the countdown and the match result are still to
come. Each player needs their own Steam copy of Kingdom Hearts II Final Mix, because the game reads its data
from the install and checks it; the launcher on its own is not enough. Both players need the same launcher
build: another one is refused with a message.

**Host or join from the launcher:** choose *Multiplayer*. *Host a duel* waits for a player. Hosts on the same
network appear in the list by name: pick one to join. Or choose *Join by address* and type the host's IP
address. *Name* is the name the other player sees (your Windows user name until you change it; the launcher
keeps it).

**Firewall:** the first time the game hosts or joins, Windows asks whether to allow it on networks. Allow it on
private networks (your home Wi-Fi), or the other player cannot reach you.

**From a command line**, add these to a test-room command (`room <Steam-root> <manifest> <config>`):

| Option | What it does |
|---|---|
| `--host` | Host and wait for a player (UDP port 47620) |
| `--port N` | Host on, or join, another port |
| `--bind IP` | Host on one address only (`127.0.0.1` for two instances on one PC) |
| `--join HOST[:PORT]` | Join the host at that address |
| `--name NAME` | The name the other player sees |
| `--net-sim LATENCY,JITTER,LOSS` | Add latency (ms), jitter (ms) and loss (%) to what this instance sends |
| `--net-overlay` | Show the network overlay |
| `--duel-damage N` | Host: multiply the damage of attacks (hits on Strength) by N (0.5 to 10, default 3) |
| `--duel-magic-damage N` | Host: multiply the damage of magic (spells, Reflect's counter, form magic) by N (0.5 to 10, default 1.5) |
| `--duel-hit-cap PCT` | Host: no single hit takes more than PCT % of max HP (20, 25, 30, 35, 40, 50, 60, 75; 100 for none; default 40) |
| `--duel-arena mirage\|room` | Host: fight in the Mirage Arena (default) or the test room's arena |
| `--rival-script` | No network: a second Sora run by a test script, to try the duel alone. With `--join`: the script presses the joiner's buttons (attacks, Fire, Thunder, a Potion, Valor Form) |

In the room, F3 → Sora has *Network overlay* (ping, loss, data rates, buffered snapshots and warnings, top
right), *Simulated network* (Off, Wi-Fi, Busy Wi-Fi, Bad link) and, outside a session, *Scripted rival Sora*.

**Two instances on one PC:** start one with `--host --bind 127.0.0.1` and the other with `--join 127.0.0.1`,
or host from one launcher window and join *127.0.0.1* from a second. The window in the background draws at 30
FPS, but its game still runs at 60 ticks a second. To try Wi-Fi conditions, choose the same *Simulated network*
on both.

**How it works:** the host runs the game for both Soras on the fixed 60 Hz tick. The joiner sends its buttons
every tick (repeating the last six, so a lost packet costs nothing) and draws the host's snapshots two ticks
behind to ride out jitter. Hit sounds, sparks, guard clashes and knock-outs reach the joiner as reliable events
and play there; nothing cosmetic is streamed. Spells, items and Drive forms work the same way: the joiner's
command goes to the host with its buttons, the host runs it, and the effects it starts reach the joiner as events
naming the effect file, so each side draws them from its own install. Esc leaves; the other side sees "Opponent
left" (or a timeout after five seconds) and the host waits for the next player.

**The lobby:** when the other player joins, the game's own pause menu opens on its *Duel* pages (L2/R2 turn
the pages, Up/Down choose a row, Left/Right change it; the help line says what a row does, and where the other
player is shows on each page):

1. *Match*: the host sets the mode (*One KO*, *Best of 3*, *Time limit*, where the larger share of HP left wins
   at time up), *Damage* (attacks), *Magic damage*, *Hit cap* and the time limit. The joiner sees them and
   cannot change them.
2. *Bans*: each player bans up to two abilities for this match and confirms (no ban is a choice too). A ban
   takes the ability away from both players; the other player's bans show below.
3. *Ready*: each player picks a Keyblade (any of the 24, each with its own stats and ability) and sees the AP
   used against the limit. While the builds are made the menu's own *Abilities* (up to 166 AP, level 99 with
   every AP Boost the game allows) and *Items* (Equip and the item slots) are on the top level. The host checks
   every build; a refusal shows on the help line.
4. *Ready* on both starts the match. A rule change takes Ready back. Start toggles Ready from anywhere in the
   lobby's menu while the builds are made.

The lobby's menu also has *Leave* (back to the Multiplayer screen), and the other player's row shows where they
are and their round trip in ms. The host's rules and each player's last readied build (Keyblade, abilities,
items) are kept for the next duel in `duel-prefs.json` beside the source manifest; anything the next duel does
not allow is left out.

**The arena:** by default the match is fought in the **Mirage Arena** from Birth by Sleep Final Mix, read
from your own install (the same Steam collection holds both games): its round stage, its sky and its walls,
scaled to KH2's size. The host can pick the test room's arena on the lobby's Match page instead. Both players
need Birth by Sleep in their install for the Mirage Arena; if either install cannot show it, the duel stays in
the test room's arena and the lobby says why. The first time, only the parts of Birth by Sleep's package the
arena reads are recorded (a few megabytes, no full scan); later sessions check them. Both Soras start 600 units apart, facing each
other, with the camera behind each player's Sora. There are no enemies, no talk and no exits during a duel.

**The music:** while you are in the duel's arena (the lobby, the countdown and the fight) one of the game's
battle themes plays, a new one each match and the same on both PCs; it fades out at the result.

**The match:** a short shot of both Soras, then 3, 2, 1 and GO on both screens at the same moment; nobody
moves before GO. A knock-out ends a round (a knocked-out Sora stays down). *One KO* is one round; *Best of 3*
goes until someone wins two (the round and score show at the top); *Time limit* shows the clock, and at time
up the larger share of HP left wins. Between rounds both Soras are refilled and put back at the start. At the
end each screen shows VICTORY, DEFEAT or DRAW and how it was decided over the field, and the pause menu with
*Rematch*, *Leave* and *Results* (the rounds, the match time, the damage dealt and the hits landed); the
winner's camera pushes in, the loser's rises behind his Sora. *Rematch* on both
goes back to the lobby (new bans, the builds where they were); *Quick rematch* on both starts the next match
at once with the same builds and bans (either player's plain *Rematch* goes to the lobby); *Leave* closes the duel, and the launcher comes
back on its Multiplayer screen. If the other player leaves in the lobby or the camp, the camp closes without
keeping the duel's build and the room's own comes back. If the other player
leaves during a match, it is your win. Start during a match opens *Keep fighting* / *Forfeit* (the fight goes
on meanwhile); *Forfeit* asks once more, then gives the match to the other player. From a command line the host can set `--duel-mode stock|best3|time` and
`--duel-time 60|90|120|180|300`.

F3 → Sora → Duel → *Test vote* (host) tries the vote both players will use for later choices, in the pause
menu: different answers go to a coin flip on the host that both screens show on the help line.

**The duel build:** for a session both Soras fight at level 99 with every bonus level's HP, MP, Drive bars,
item slots and abilities, the top level of every spell, Valor, Wisdom, Master and Final Form at their level cap
(Limit Form is off), a full Drive gauge, and 99 of every consumable (Potion to Megalixir; the stat boosts, Tents
and Drive Recoveries are left out). Nothing is read from or written to a save; the room's own build comes back
when the session ends. Damage between the two Soras is the game's formula times the host's rules: attacks ×3
by default (`--duel-damage N` or F3 → Sora → Duel), magic ×1.5 (`--duel-magic-damage N`), and no single hit
takes more than 40 % of max HP (`--duel-hit-cap PCT`). The formula is Strength or Magic minus Defence, so at
level 99 the Keyblade's stats count for a lot: with the defaults a Thundaga does 24 on the Kingdom Key and 90 on
the Keyblade with the most Magic, and neither takes a full bar in one cast. The other Sora's HP stays on the target gauge (top right) for
the whole duel. The lobby's Keyblade, abilities and item slots go on top of it.

**Known limitations:**

- Limits, summons, Limit Form, Donald and Goofy, and the room's Shadow are switched off in a session.
- The other player's weapon trail is not drawn yet; Wisdom Form's shots show their effect on the host only.
- The joiner sees its own Sora about one round trip late (one or two ticks on home Wi-Fi): there is no
  prediction yet. The host has no delay.
- Sora has no victory motion in the duel yet; the result's camera frames him as he stands.
- LAN only: no internet play. Discovery uses a broadcast that guest networks and some routers block; join by
  address then.

## Build and check

`build.bat` builds the optimized launcher (`KH2RW Launcher.exe`, with the model and image viewers inside it)
with the `skate3` feature. Only `KH2RW Launcher.exe` is left in `target\release`. Compact development symbols and Rust's bundled
LLVM linker reduce build-output overhead; see
[local build options and cleanup](CONTRIBUTING.md#local-builds-and-disk-use).

```powershell
cargo build --locked
cargo test --locked
cargo run -p main --locked -- --frames 180
```

The full list of checks, and the rules for contributing, are in [CONTRIBUTING.md](CONTRIBUTING.md). Developer tools for reading the installed game are in [docs/developer-tools.md](docs/developer-tools.md).

## Private data

Source manifests, package records, captures and saves stay outside this repository, in `%LOCALAPPDATA%\KH2FinalRewrite\`. See [ARCHITECTURE.md](ARCHITECTURE.md) for the layout.

## Credits

- **Project owner:** TheGlurbus ([@BurBaGlurbus](https://github.com/BurBaGlurbus)), who maintains the project.
- **Code:** written with [Claude](https://claude.com/claude-code), an AI coding assistant, under the owner's direction.
- **Format research:** [OpenKH](https://git.openkh.dev/) (Apache-2.0) documents the game's file formats, and the project's reader follows it. The attribution list is in [NOTICE](NOTICE).
- **Other third-party code:** [Bevy](https://bevy.org), the `image` and `png` crates, zlib-rs, Microsoft DirectXTex (MIT), the .NET runtime (MIT), and `bevy_mikktspace` (Morten Mikkelsen). Release licences are in [docs/release/](docs/release/).
- **Skate 3 crates** (`source/extras/skateboard/engine/`, GPL-3.0): adapted from [SK8-ENGINE/skate-3-rust-engine](https://github.com/SK8-ENGINE/skate-3-rust-engine). See [its README](source/extras/skateboard/engine/README.md).
- **Games:** *Kingdom Hearts* is by Square Enix and Disney. This project is not affiliated with or endorsed by either company.

## License

Apache-2.0, see [LICENSE](LICENSE). Third-party licences are listed in [NOTICE](NOTICE).
