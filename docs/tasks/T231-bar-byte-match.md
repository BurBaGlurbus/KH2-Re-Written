# T231 — BAR archives written back byte for byte

Owner Claude, 2026-10-25, requested by burba in the project chat ("i wanted you to add that feature": a byte-for-byte
match check like decompilation projects use, after a README section describing one was added and reverted).
Gate: P07 (packages/archives) and P12 (traceable decoded representation) development; no gate acceptance is
inferred. Prerequisites: T005 (storage decoding), the BAR table reader. Build identity: branch
`feature/byte-match-bar` from local `main` at `4b4ded6`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the
selected Steam KH2FM build, admitted each run by the session check (executable and indexes rehashed, every package
byte read verified against its recorded chunk digest); the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/byte-match-20261025/` (`bar-roundtrip-01.json`
first rule-only writer, `-02` with layout facts, `-03` the final run).

## What a byte match means here

The rewrite is Rust and cannot rebuild the original executable byte for byte. What it can match exactly is the
game's data: a reader that keeps every byte of a format, and a writer that puts them back, must give the original
file again. This task does that for BAR, the archive most installed files are packed in.

## What the installed BARs look like

- Half of them are not packed by one rule. A writer that places each payload after the table on the next 16-byte
  boundary and pads with zeros, the file to a 16-byte length, reproduced 27,000 of 54,202 BARs.
- The rest have payloads at offsets that are not 16-byte aligned, records sharing one offset, files that end unpadded (many nested BARs end exactly at their last payload), padding
  that is not zero, and blocks after the last payload that no record points to (up to about 1.5 KiB).
- No BAR failed to parse and no record was unreadable.

## What the rewrite does now

- `data::bar::parse_bar` reads a whole BAR: header words, each record's type, index, name and payload (or the size
  of an absent record), and a `BarLayout` with each record's offset and every run of bytes after the table that no
  payload covers.
- `data::bar::write_bar` writes it back. A layout that still fits the records (absent records still absent, every
  payload inside the recorded length) is reproduced exactly, table last. A BAR built in code, or one whose payload
  grew, is packed by the plain rule instead.
- `data::path_lookup::Library::all` lists every record of the six indexes.
- `bar_roundtrip` (data crate) reads every installed record, round-trips each BAR and each BAR nested inside one
  through `parse_bar` and `write_bar`, and compares SHA-256 digests. It prints totals; the private report keeps the
  per-record results and, for files the plain rule does not reproduce, their layout facts.

## Checks

| Check | Result |
| --- | --- |
| `bar_roundtrip` against the selected install (private `bar-roundtrip-03.json`) | PASS: 46,076 records read, 0 unread; 36,854 BARs and 17,348 nested BARs; 54,202 of 54,202 written back with the same SHA-256; 27,000 also reproduced by the plain packing rule alone |
| New unit tests on synthetic bytes (`bar::tests`): aligned, unaligned, unpadded, shared, empty-at-0, absent-with-size, non-zero padding, unreferenced tail, out-of-order and overlapping payloads, nested BARs, new BARs packed by rule, a grown payload repacked | PASS |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked` | PASS |
| `cargo build --locked` | PASS |
| `cargo clippy --workspace --all-targets --features main/skate3 --locked -- -D warnings` | PASS |
| `cargo test --workspace --features main/skate3 --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |

(Run once on the tree with T231 and T232 together, in the branch's own target directory.)

## Uncertainty

- [O] The counts cover the six KH2 indexes of the selected build. Records present in more than one package are
  counted once per package.
- [I] Non-zero padding and unreferenced tails look like leftovers of the original packing tools; the game is not
  known to read them. They are kept so the files match, not because they mean anything.
- Only BAR is checked. The payloads inside (models, motions, scripts, images) are carried as opaque bytes, so this
  says nothing yet about those readers.
- The public progress ledger has no measure for format round-trips (it counts functions and behavior scenarios), so
  the result is recorded here and not in `coverage.json`.

## Verdict

PASS for the BAR round-trip: every installed BAR, nested ones included, is written back with the original SHA-256.
No gate is accepted by this.

## Next

Round-trip the save container and save data the same way, then match movement-physics outputs against recorded
inputs and outputs of the original game.
