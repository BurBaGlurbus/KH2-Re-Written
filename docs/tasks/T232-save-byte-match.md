# T232 — Save files written back byte for byte

Owner Claude, 2026-10-25, requested by burba in the project chat (the byte-match feature, "Both, round-trip first":
BAR first, then saves, then movement outputs). Gate: P23 (native saves) development; no gate acceptance is inferred.
Prerequisites: T231, the save container and save image code (`source/game/save`). Build identity: branch
`feature/byte-match-bar` from local `main` at `4b4ded6`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the
selected Steam KH2FM build (its executable, read for the cipher's S-box) and the owner's own KH2FM save container
written by the original game; both were only read, and the container's SHA-256 was the same before and after.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/byte-match-20261025/` (`save-roundtrip-02.json`).

## What is checked

`save_roundtrip` (save crate) takes the install root, a container the original game wrote and a new private report.
It checks that:

1. the header mask made from the Steam id (the container's folder name) and the executable's S-box is the mask the
   file was written with (compared with the mask recovered from the file itself);
2. `Container::open` then `to_png` gives the file again, byte for byte;
3. for every save image in the container: its stored checksum is the one `data::checksum` computes, `Persistent::read`
   then `write` gives the image again, and after every byte `write` sets is overwritten with junk, `write` restores
   the image from the values it read alone. The bytes `write` sets are found by writing over an all-0x00 and an
   all-0xff image: a byte that comes out the same both times is set by `write`.

It prints totals; the report lists each entry's name and results.

## Checks

| Check | Result |
| --- | --- |
| `save_roundtrip` on the owner's original-game container | PASS for the container: mask matches, written back byte for byte. NOT RUN for save images: the container holds no save files (no KH2FM save was made in the original game on this account) |
| New unit tests on synthetic bytes: an opened container (with chunks the reader does not know) written back byte for byte with the Steam-id key and with a recovered mask; a fully populated save image read and written back, and rebuilt after the bytes `write` sets are scrambled | PASS |
| Workspace checks (fmt, clippy and tests with and without `main/skate3`, build, smoke) | PASS, run with T231 |

## Uncertainty

- The save image checks have only run on synthetic images. The checksum, field offsets and the read/write pair are
  not yet confirmed on a save the original game wrote.
- `Persistent` models part of the image; bytes it does not model are carried unchanged, so this does not show that
  the rewrite understands them.

## Verdict

PARTIAL. The container format round-trips on a real original-game file; the save image round-trip passes on
synthetic images only.

## Next

Make one save in the original game (any slot) and run `save_roundtrip` again; then match movement-physics outputs
against the original game.
