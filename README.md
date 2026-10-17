# TurnRush — motor Rust

Roguelike acelerado 1v1 · Combate por turnos estilo Naruto Online ·
Balanceamento absoluto (DWC v2 + LCE) · Linux first · CI no GitHub Actions.

> Documentação completa em `docs/` (GDD, arquitetura, inspirações, setup GitHub).

## Quick start

```bash
cargo test --workspace --release                          # 21 testes
cargo run --release -p turnrush-sim -- --match            # demonstra uma partida
cargo run --release -p turnrush-sim -- --quick --gate     # gate de equilíbrio (CI)
cargo run --release -p turnrush-sim --                    # calibração completa
```

## Crates

- **turnrush-core** — motor determinístico: RNG xoshiro256**, combate por
  turnos (alternância A-B-A-B, energia, cooldowns, launch/chase, febre de
  batalha), draft simétrico com LCE, partida de 9 rodadas, calibrador DWC v2.
- **turnrush-sim** — CLI: simulador Monte Carlo de TODAS as composições
  (C(12,3) = 220 → 48.180 confrontos), gate de equilíbrio, diagnóstico de
  hard counters, relatório JSON.

## Status v0.1

- ✅ 21 testes verdes (determinismo, espelho ~50%, chase 1×/rodada, fórmulas)
- ✅ Gate de equilíbrio: desvio médio 0.119 ≤ 0.15 (PASSOU)
- ✅ CI pronto para GitHub Actions (testes + gate + build Linux + release musl)
- ⏳ Repositório remoto aguardando token do dono (ver `docs/GITHUB-SETUP.md`)
- ⏳ v0.2: redesign de roster para hard counters (diagnóstico automático já
  aponta os pares), gate de pior confronto, servidor autoritativo


## v0.3 — Formação 4v4, logs de sessão e roster data-driven

```bash
cargo run -p turnrush-sim --release -- --formation4v4 --formation-gate --formation-demo
```

- **Modo Formação 4v4**: 4×4 em formação 2×2, ordem de movimento, cadeias de
  perseguição (arremesso/empurrão/derrubada), mysteries com chakra/cooldown.
- **Roster**: `data/roster/roster-v2.json` (16 personagens, 4 facções) com
  fallback embutido; estudo do Naruto Online em `data/naruto-online/`.
- **Logs de sessão**: `~/TurnRush/logs` (`.log` + `.jsonl`, crash report
  automático, retenção 20) — ver `docs/LOGS.md`.
- **CI**: `cargo test` + gate 1v1 + gate 4v4 a cada push; **cron noturno**
  reexecuta o Monte Carlo de balanceamento.
