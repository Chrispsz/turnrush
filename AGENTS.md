# AGENTS.md — TurnRush

Guide for AI agents working in this repo. Read fully before editing. Keep changes small, verified, documented.

## Workspace

- `crates/turnrush-core` — deterministic engine. **ZERO external deps (std only)**, no `unsafe`, integer math only (per-mille, `PER_MILLE = 1000`) in combat paths. No floats in the hot path.
- `crates/turnrush-sim` — Monte Carlo balance CLI (DWC/LCE calibration, JSON reports).
- `crates/turnrush-game` — Bevy 0.20 visual client. Only crate allowed to depend on Bevy. Battle = **playback of engine events**, client never mutates battle state.
- `data/roster/roster-v2.json` — source of truth for the 16 characters; loaded by `data.rs` via keyword interpreter.
- `docs/` — GDD, ARQUITETURA, PERSONAGENS (generated from roster JSON), CARTAS, NAMING, LOGS. **Update docs + `CHANGELOG.md` in the same change as mechanics.**

## Commands

```bash
cargo test --workspace                                    # all tests must pass
scripts/puzzle-gate.sh                                    # FULL chained suite (10 pieces + gates)
PUZZLE_FAST=1 scripts/puzzle-gate.sh                      # chained suite, skip formation MC gate
cargo clippy --workspace --all-targets -- -D warnings     # zero-warnings policy
cargo run -p turnrush-game -- --check                     # smoke: headless battle + event counts
cargo run -p turnrush-sim -- --formation-gate             # 4v4 balance gate (winrates)
cargo run -p turnrush-game -- --shot /tmp/shots           # 5 headless screenshots (needs Xvfb + lavapipe)
cargo run -p turnrush-game                                # playable client (menu → draft → battle)
```

**Puzzle suite (docs/TESTING.md):** 10 chained integration-test pieces
(data → determinism → economy → targeting → passives → kits → battle chain →
run → elements → client E2E) plus `scripts/puzzle-gate.sh` master runner and the
`puzzle-gate` CI job. Integration tests use ONLY the public engine API —
if a test needs new API, the UI probably needs it too.

**Balance gate:** 4v4 mean winrate deviation must stay ≤ **0.15** (mirror ≈ 0.50). If you touch combat/roster/balance math, run the formation gate and report the numbers in your summary.

## Invariants (never break)

1. **Determinism**: same seed ⇒ identical battle on any platform. All randomness via `rng.rs` (xoshiro256**). No `rand::thread_rng`, no `HashMap` iteration order in logic, no system time in combat.
2. **Integer math** (per-mille) for damage/HP/heal/shield scaling. Round toward zero consistently.
3. **Engine emits string events**; the client parses (`battle_log.rs`) and plays back. Unknown lines stay visible, never silently dropped.
4. **Naming**: clans `PYREBORN / DUSKMERE / THORNVALE / SKYREND`; classes `Reaver / Bastion / Mender / Tactician`. Canonical id→display table: `docs/NAMING.md`. Ability names must match roster JSON.
5. **Zero references** to other games/IPs anywhere (docs, comments, UI, metainfo).
6. **Session logs**: `logger.rs` writes to `~/TurnRush/logs` (JSONL + .log + crash reports on panic). Never bypass or suppress.
7. Core stays `std`-only. No new dependencies without explicit discussion.

## Bevy 0.20 gotchas (learned the hard way — details in docs/ARQUITETURA.md)

- UI **renders nothing** without feature `bevy_ui_render` (in 0.20 the UI render is a separate crate). Sprites need `bevy_sprite` + `bevy_sprite_render`.
- `Camera2d` is mandatory, else screen is blank.
- `Node` default flex direction is **Row** — screens in column need explicit `FlexDirection::Column`.
- Two overlapping `Query<&mut Text>` in different systems **panics at system init** — keep text queries disjoint (filter by component marker).
- **B0001 within a single system**: `With<X>` does NOT prove disjointness to the scheduler. Any two `&mut T` queries in the SAME system need explicit `Without` on each side (component-level reasoning only). Caught via `--shot` (headless `--check` does not exercise schedule conflicts).
- Inserting resources via `Commands` between systems needs apply-deferred; simpler: generate assets in `PreStartup` (sprites.rs does this).
- `Anchor` is a tuple struct (e.g. `Anchor::CENTER_LEFT.0`).
- Default font is ASCII-subset; the embedded `turnrush-game/assets/DejaVuSansMono.ttf` provides pt-BR accents — keep using it.
- Sandbox disk is tight: `profile.dev` uses `debug = false`, `incremental = false`. Do not re-enable (target dir balloons to 6+ GB).

## Conventions

- User-facing strings: pt-BR. Identifiers, comments, commit messages, issues: **English**.
- Tests live inline (`#[cfg(test)]`) next to the code; names describe behavior.
- Commits: conventional, English, one logical change each.
- Experimental/scratch work goes in `/tmp`, never in the repo.
- Curated agent skills live in `.claude/skills/` (list + rationale in
  `docs/ARTE-DIRECAO.md`). Any sprite/art task MUST load `turnrush-art-style`
  first — it pins the frame/baseline/palette/silhouette contract (v2: inked
  256×256, direction from the owner's 2026-10-20 references).

## Agent verification loop

1. Edit → `cargo test --workspace` → clippy `-D warnings` → game `--check`.
2. Combat/balance touched → run formation gate, paste numbers.
3. Visuals touched → `--shot`, inspect PNGs (or point user to screenshots).
4. Mechanics changed → update `docs/` + `CHANGELOG.md` in the same commit.
5. Report: what changed, numbers measured, what remains.
