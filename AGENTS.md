# KH2 Final Mix rewrite

Read `docs/fidelity-contract.md`, `docs/milestones.md`, `docs/unknowns.md`, and the latest `docs/tasks/` record before work. User authorization: work through the roadmap from P00 using the local Steam files; KH2 Final Mix only. No other collection game belongs in this rewrite.

- Keep the installed source read-only. Never launch a writer/mod manager against it. Put manifests, extracted data, saves, captures and disassembly outside the checkout in private local storage. `.local/` is only for ignored development outputs; backups live outside the checkout.
- Require selected-source validation at every gameplay session. A remembered path, matching cache, Steam manifest or filename is insufficient proof of a supported build. Unknown builds remain quarantined.
- No game data, executable extracts, captures, user saves, secrets or personal paths in Git or distribution. Tests use generated synthetic bytes. Review provenance as well as ignore rules. Do not push or publish private research.
- Separate detection, decoding, cache, authoritative simulation and presentation. Create crates as needed; do not scaffold empty subsystems. Do not invent game rules or advance gameplay gates without reference evidence.
- Use `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, and `cargo build --locked` (the default members leave out the optional Skate 3 crates). When a change touches the `skate3` feature or `source/extras/skateboard`, also run clippy and test with `--workspace --features main/skate3`. Toolchain is pinned; ensure the Cargo bin directory is on PATH. Smoke: `cargo run -p main --locked -- --frames 180`.
- Nontrivial work requires an ID, gate, owner, prerequisites, build/source identity, steps, exact evidence locations, checks, uncertainty, verdict and next task in `docs/tasks/`. Use PASS, FAIL, NOT RUN, BLOCKED honestly. Unknown or skipped checks do not pass a gate.
- Maintain `docs/progress/coverage.json` when reviewed evidence changes coverage: retain every required unknown/failed unit, use opaque public IDs, and link public acceptance records. Only complete, qualified functions count as decompiled; only controlled installed/rewrite comparisons count as 1:1 behavior. Do not infer final gate PASS from task/build PASS or mark an incomplete inventory complete. Run `python tools/progress.py` and its synthetic tests after ledger/report changes; GitHub refreshes the report on pushes and daily. Keep all source bytes, addresses, pseudocode, source hashes and private evidence outside the public ledger.
- Resolve public technical claims against pinned primary sources and installed-build observations. A tool's README is not compatibility evidence. No claim of whole-game equivalence from finite opening tests.
- Preserve user files and saves. Do not switch Steam editions, repair/update the installation, spend money, share data or publish releases without the corresponding user instruction. Local inspection/builds are authorized.

## File placement and naming

- Read `docs/folder-guide.md` for the current layout and historical-name map.
- Use familiar nouns and descriptive snake_case names. Owned packages use `player`, `area`, `effect`, `audio` rather than `_sim`, `_vm` or `_out` suffixes. Preserve established format names and upstream attribution.
- Put gameplay in `source/game/`, display/menu code in `source/visual/`, effect execution in `source/visual/effect/`, application wiring in `source/main/`, readers in `source/data/`, validation in `validation/`, and mixing/output in `source/audio/`. Add folders/packages only for working code; do not scaffold empty systems.
- Update imports, Cargo paths, assets/shaders, scripts, ignore rules and current guides together with moves. Historical records retain original command/path spellings; their index links the migration guide.
