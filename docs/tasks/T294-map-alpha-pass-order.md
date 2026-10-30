# T294 — The map's alpha pass in the exe's batch order; level of detail and fade

Owner Claude, 2026-10-30, requested by burba in the project thread ("start working on the github issues and getting them
out of the way"). Issue #22. Gate: development of the map renderer; no final gate is inferred. Prerequisites: T097 (areas).
Build identity: local `main` after `e31a43c`, package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the
install was only read.

Private evidence (outside the checkout): `research\playable-room-20261008\tt-world-01\draw-01\draw-spec.md` §3,
`draw-03\draw3-spec.md` §2; `research\issues-20261030\g\lod\` (FUN_1401a5180, FUN_1401c3330, FUN_1401c3200); a private census of
every map's BOB headers and 'out' distances over the install.

## Findings

- **Alpha order.** FUN_1401b91e0 sorts a map's chunks by layer then a key of texture, flags, blend class and group; equal keys in
  a run form a batch; FUN_1401cf5e0 draws the batches in order of their lowest chunk index. tt07's map also keeps textures 4
  and 0x22 in batches of one (a signature check on its header).
- **Level of detail.** Only a model whose first word is -1 has levels (FUN_1401c3330 picks offset N in the table at +0x10). A
  census of all 278 installed maps: 523 BOB models are plain, **2 are level containers, both in tr02**. No Twilight Town object
  has levels.
- **Part fade.** The per-part distances only act on models that report more than one part, the same container class. In
  Twilight Town they act on nothing, although every 'out' record carries the distances.
- **Far cull 2100 / fade 400** is for the BG-object path, which BOBs never take (draw3-spec §2.5).

So the Twilight Town part of #22 is the alpha order. The two tr02 containers are a separate piece: the room doesn't decode
them yet.

## Change

- `visual::room::draw_order` and `isolated_textures`, with a unit test on the batching.
- `spawn_room`: alpha chunks get their own material with a depth bias of their place in that order. All of a map's chunks sit
  at one origin, so the fraction orders them in Bevy's transparent sort and, as it truncates to 0, doesn't move the depth.

## Checks

| Check | Result |
| --- | --- |
| Unit test: batches in leader order, an isolated texture | PASS |
| `cargo clippy` on the changed files | PASS |
| Release runs in tt01, tt05, tt06, tt08 | PASS: they load and draw; order not compared with the installed game |
| Side by side with the installed game | NOT RUN |

## Uncertainty

- The order holds while the view distance of the map's origin leaves float room for the steps: 500 alpha chunks at under about
  8000 units. tt08 has 33. A bigger map could tie, and then Bevy's stable sort decides.
- Map alpha chunks still sort against BOBs and effects by distance, not by the exe's fixed phases (8, 10, 0xb).
- Sub-buffer order inside a batch (class 0 before class 1) is not kept: a chunk's triangles keep their file order.

## Verdict

PASS for the alpha order in the room. Next: the tr02 level containers.
