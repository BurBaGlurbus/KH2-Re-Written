# T356 — Effect programs 229 and 82, and who fills the user-control lists

Owner Claude, 2026-11-02, requested by burba in the project thread ("work through issues and close them"). Issue #65. Gate: P22 (field and combat presentation), progress only. Prerequisites: T150. Build
identity: local `main` after T355 plus this change, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless on a read-only
project copy.

Private evidence (outside the checkout): `research\issues-20261102\g\h1`, `h2`, `h3`; the census at `%USERPROFILE%\kh2-issues-out\census.tsv`; runs `he5`, `he6` under `%USERPROFILE%\kh2-issues-out\s162\`;
private configuration `area-he05-n.json`.

## What the executable does

- The census had five kinds left, of 1060 containers: 82, 229, 239, 242 and the ribbon's object space (flag 0x40000243). Tail2 and RyjMatrixWorldXYZ, named in the issue's title, were already built.
- 229, pppRyjMegaBirthModelFilter, is a ball of screen-facing models around the camera that falls and comes back. The one element is the mist in Olympus area 5.
- 82, pppSRandUpHCV, is 81's sibling with the positive draw.
- 239 and 242 draw records from a list on the instance (`+0x48`, `+0x50`, flags 0x40000 and 0x80000, set by FUN_1402bebe0 and FUN_1402bebd0). The ribbon's space 2 reads a matrix from the same place. B_EX170_LAST's elements 14, 69 and 70 are given theirs by FUN_14053a3f0, a native driver of that boss.

## Changes

- `effect::model_filter`, with create, destroy, update and draw. `extra::s_rand_up_hcv`.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit tests: 82 adds, 229 stays within its ball | PASS |
| Census after the change | PASS for the two: 82 and 229 gone, 239 and 242 and the ribbon's flag left |
| he area 5 (`he6`), with a temporary trace since removed | PASS: the program runs, ten slots filling one by one, about half behind the camera, fading in; radius 1900, near fade 1000 less 800. The captures show the mist over the cave walls |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The camera's position and forward are taken from the inverse view, and a three-component dot is assumed. Flag 0x200 on the draw and `inst+0xc3` are not interpreted.
- The mist's look in the captures was only judged by eye, with no reference picture.
- 239 and 242 aren't translated: nothing here feeds them, so they couldn't be tested. The other callers of the two list setters aren't read, including those behind the ribbon elements of B_NM110, N_HB580, B_LK110 and N_CM000. With an empty list the ribbon uses the raw sample, as the rewrite does.

## Verdict

PASS for 229 and 82. FAIL for the issue's whole list: 239 and 242 remain.

## Next

Those two with B_EX170_LAST's driver, in the paused enemy work.
