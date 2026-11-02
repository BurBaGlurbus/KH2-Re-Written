# T350 — Map objects: their true place, their levels by distance, and the alpha order

Owner Claude, 2026-11-01, requested by burba in the project thread ("work through issues and close them"). Issue #22. Gate: development of the map's draw in the test
room; no final gate is inferred. Prerequisites: T294, T340. Build identity: local `main` after T349 plus this change, package 0.1.2. Source: the selected Steam KH2FM
build, validated each run; the install was only read. Plain Ghidra 12.1.4 headless on a read-only project copy.

Private evidence (outside the checkout): `research\issues-20261102\lod51-spec.md` sections 1 to 4, with its census of all 278 maps; runs `spawn25000`, `spawnm25000`,
`f_*`, `wmf_*`, `l_*`, `g25*`, `n25`, `n7` under `%USERPROFILE%\kh2-issues-out\s162\`; private configurations `tr02-spawn*.json` and `test-room-tr07.json`.

## What the executable does

- An 'out' record's translation is in the player's space, and FUN_1401a5180 negates only the matrix's rows. So in the scene it is (x, -y, -z), with the rotation
  Rz(-rz) Ry(-ry) Rx(rx). The room had used the raw numbers, so every map object with a translation off those axes was in the wrong place.
- FUN_1401879a0, every draw: a container's object measures the distance from the eye to its point and sets its level byte. `d > f3` gives 2, `d > f2` keeps,
  `d > f1` gives 1, `d > f0` keeps, else 0, with f at record +0x38..+0x44. A level past the end is the last. An object with a motion measures from the motion's root.
- The containers are tr02's BOB 0 (2 levels) and tr07's BOB 2 (3 levels). T294 and T340 said both were in tr02.
- The part fade never turns on: `kn::ModelMulti`'s and `kn::ModelBG`'s +0x68 return 0, and all 2237 BOB meshes of all 278 maps are part 0. T294 was wrong about this,
  so there is nothing to build. The 2100/400 far fade stays unbuilt for T294's reason.
- FUN_14019f170's phases: sky, map opaque, skinned opaque, map alpha (8), shadows, skinned alpha (0xb). Inside a batch, class 0 triangles come before class 1.
- Far levels' tiles repeat their image (UVs 0..4, 1..13).

## Changes

- `scenery`: the placement's space, `next_level`, `level_count`, `level_payload(_, n)`, `level_bank(_, n)`, `Bob::far`, `Bob::level`, `BobAnim::root`.
- `test_room`: every level's parts spawn, far ones hidden. `bob_levels` runs each frame. `area_start` logs `bob_levels`, and a change logs `bob_level`.
- `spawn_model`: a part whose UVs leave 0..1 repeats its image. The sampler used to clamp, which drew the far tile as a pale slab.
- `mesh_upload::SortOrder`: a fixed place in the transparent sort, set each frame. T294's fraction of a depth bias only broke ties, because Bevy 0.19.1 sorts by each mesh's
  bounds centre. So its order never took effect. Map alpha now draws in batch order before the objects' alpha, and sky alpha before it.
- `by_class`: an alpha chunk's class 0 triangles before its class 1.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | PASS |
| Unit tests: `next_level`'s bands, levels and banks at 0, 1 and past the end | PASS |
| tr02, spawn z +25000, raw placement (`spawn25000`) | FAIL, which is what showed the bug: no grid at the walkable place, while the grid lay at z -25000 |
| Raw against flipped, tr02, tt area 9, tt area 27, hb area 1 (`f_*`) | PASS for the flip: the Sorcerer's Loft's tall chair stands behind the desk, not in front of it; tt area 9 and hb area 1 show no misplacement |
| World map, raw against flipped (`wmf_*`) | PASS by eye: one keyhole-shaped object moves, from in front of the clock tower towards the pink area. The owner should look at it, as this map was reviewed before |
| tr02 at z +25000 with levels (`n25`) | PASS: the grid runs to the horizon; `bob_level` 2 for far tiles, with d recomputed from the logged eye and point |
| tr07 (`n7`) | PASS: `bob_levels` [1,1,3,1,1,1]; objects log their levels and the scene draws |
| tt area 7, 9 and 27 (`l_*`) | PASS: no level events, and no visible break in the alpha order by eye |
| Alpha order compared with the installed game | NOT RUN |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- [I] The flip was checked by eye and against collision, not against the installed game. Its reasoning is [O] for the matrix and [I] for the scene's axes.
- [I] Part index order among several MAP parts, in `SortOrder`: the submission order was not traced. Effects keep their own key, so map alpha now always draws before them.
- The double-sided split inside a batch needs the strip function, which the merged triangle list no longer has. Only the class split is built.
- The room draws alpha chunks without culling; the executable culls single-sided triangles.
- FUN_1401a4bb0's texture animation per bank level was not traced. It only matters if those banks animate.

## Verdict

PASS for #22's items in the room. No claim about the installed game.

## Next

The rest of the batch.
