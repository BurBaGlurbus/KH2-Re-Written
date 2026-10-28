# T268 — A model texture taller than its upload (Port Royal's world map symbol)

Owner Claude, 2026-10-28, requested by burba in the project thread ("work on more issues and closing them").
Issues #86 / #144 (world map symbols); T172 recorded `WM_SYMBOL_CA` as untextured. Gate: development of the
model texture reader; no final gate is inferred. Prerequisites: T172. Build identity: local `main` after `1fa5051`,
package 0.1.2. Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence (outside the checkout): runs in `C:\Users\ANIMA\kh2-issues-out\` (`wm1`, `wm-ca1`, `ca-model2`).

## What was wrong

`obj/WM_SYMBOL_CA.mdlx`'s texture set has six images. Environments 0 and 1 are 128×256 8-bit textures (32768 bytes)
on transfer 5, which uploads 1536 quadwords (24576 bytes, 128×192). That transfer is the last image before the file's
end. The declared size ran past the end of the payload, `read_layout` refused the whole set ("texture range outside
payload"), and the world map drew the symbol untextured. OpenKH's GetData zero-fills such a tail; the reader's
profile had left that case out.

## Change

- `model_texture::read_layout`: when an image's declared size runs past the payload, it keeps the bytes its transfer
  uploads (`quad_word_count × 16`). A missing upload or an empty one is still refused. Images that already fit are
  unchanged.
- `model_texture_pixels::decode_images`: the stored bytes are padded with zeros to the declared size before the page
  swizzle.
- Unit test `an_image_taller_than_its_upload_keeps_the_uploaded_bytes`.

## Checks

| Check | Result |
| --- | --- |
| Throwaway probe on the installed WM_SYMBOL_CA / WM_SYMBOL_NM (deleted) | CA: before "texture range outside payload", after 6 images decoded; NM unchanged (9 images) |
| Release `wm` autoplay (`wm1` before, `wm-ca1` after) | `untextured` was `[WM_SYMBOL_CA]`, now `[]` |
| Release `model_inspector --tour` on WM_SYMBOL_CA (`ca-model2`) | 4 captures: ship, skull, swords and the blue backdrop fully textured |
| `cargo fmt --all --check`; `cargo clippy -p data --all-targets --locked -- -D warnings`; `cargo test --locked` | PASS |
| `cargo clippy --all-targets --locked -- -D warnings` (workspace) | FAIL, only in `test_room/enemy_battle.rs`, another session's uncommitted work, not touched here |
| The symbol on the world map at a story point that opens Port Royal | NOT RUN (no option or save opens it in the run) |

## Uncertainty

- What the GS shows in the rows past the upload is whatever VRAM holds there; zeros follow OpenKH, not the PC renderer.
  The model's UVs were not checked to stay inside the uploaded rows.

## Verdict

PASS for the reader fix; the world map comparison stays with #144.
