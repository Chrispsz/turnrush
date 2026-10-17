# Arquitetura Técnica — TurnRush v0.1

## Visão geral

```
game/
├─ Cargo.toml                  # workspace (zero dependências externas)
├─ rust-toolchain.toml         # stable, profile minimal
├─ crates/
│  ├─ turnrush-core/           # motor determinístico (lib)
│  │  └─ src/
│  │     ├─ rng.rs             # xoshiro256** semeado (splitmix64)
│  │     ├─ types.rs           # Stats, Ability, Unit, Team, Outcome
│  │     ├─ data.rs            # roster de 12 personagens (const, zero-alloc)
│  │     ├─ cards.rs           # pool de 16 cartas + draft + LCE
│  │     ├─ combat.rs          # motor de turnos + IA greedy + build_team
│  │     ├─ run.rs             # partida 1v1 (9 rodadas, draft, escalação)
│  │     └─ balance.rs         # DWC v2 (Monte Carlo + calibração + gate)
│  └─ turnrush-sim/            # binário: simulador, probe, gate de CI
│     └─ src/main.rs           # CLI: --quick --gate --out --match --probe
├─ .github/workflows/
│  ├─ build.yml                # testes + gate de equilíbrio + build Linux
│  └─ release.yml              # tag v* → binário estático musl
└─ docs/                       # GDD, arquitetura, inspirações, setup GitHub
```

## Princípios

1. **Determinismo total** — matemática 100% inteira (per-mille, sem float na
   lógica de jogo) e RNG semeado xoshiro256**. A mesma seed produz a mesma
   partida em qualquer plataforma → replays, anti-cheat e testes estatísticos
   reprodutíveis.
2. **Zero dependências externas** — `std` apenas. CI rápido, build pequeno,
   sem risco de supply chain.
3. **Roster como dados** — personagens são `const` em `data.rs`; adicionar um
   personagem = adicionar uma entrada + rodar o DWC (o CI recalibra sozinho).
4. **Motor headless** — `turnrush-core` não conhece IO/render; o mesmo binário
   lógico servirá servidor autoritativo (M2) e cliente 2D (M3).

## Decisões de balanceamento embutidas no motor

| Mecânica | Decisão | Motivo |
|---|---|---|
| Multiplicadores | per-mille inteiros | determinismo entre plataformas |
| Iniciativa | alternância A-B-A-B, início sorteado | iniciativa deixa de ser vantagem composta |
| Chase | 60% do efeito, 1×/rodada/time | combos existem sem quebrar a economia |
| Escalada | +8%/rodada de partida (HP+ATK) | pressão crescente estilo 9 Kings |
| Suporte (cura/escudo/veneno) | escala com DWC **e** com a escalada | suporte nunca fica obsoleto |
| Febre de batalha | dano +2%/rodada de combate | toda batalha se resolve; anti-turtle |
| Juízo | dano total causado → HP% | agressividade é recompensada |
| Escudo | custo/cd elevados + IA anti-spam | stall de escudo era explorável |

## DWC v2 — fluxo

```
para cada iteração (6× no modo rápido):
    simular 48.180 confrontos ordenados (threads = núcleos)
    ajustar modificadores por composição (ganho 350/175, amortecimento 0.6)
calcular ajuste por confronto (só extremos: wr > 0.85 ou < 0.15)
verificar em seeds novos → relatório + gate
```

- Gate v0.1 no CI: desvio médio ≤ 0.15 (rápido) / ≤ 0.12 (completo).
- O pior confronto é impresso como **diagnóstico de redesign** (gate de hard
  counters planejado para v0.2).
- Relatório JSON: stats por comp + matrizes de ajuste (adj_dmg/adj_hp) —
  insumo direto do futuro servidor autoritativo.

## Netcode planejado (M2)

- Servidor autoritativo em tokio + WebSocket (porta própria, mini-service).
- Cliente envia **intenções** (pick de carta, ação de turno); servidor valida,
  simula com `turnrush-core` e difunde snapshots determinísticos.
- Seed única por partida → os dois clientes podem pré-calcular e o servidor
  apenas valida hashes de estado (anti-cheat barato).
- Reconexão: replay a partir do último snapshot (determinismo garante
  sincronia exata).

## Performance medida (sandbox 2 vCPU)

- Batalha isolada: ~2–4 µs
- Calibração rápida (60 sims × 7 passadas): ~95–105 s
- Testes completos: < 1 s


---

## Logs e Diagnóstico de Sessão (v0.3)

Subsistema std-only (`logger.rs`, ~900 linhas): uma sessão = um par de arquivos
em `$HOME/TurnRush/logs/` (`.log` legível + `.jsonl` estruturado), cabeçalho com
ambiente completo (versão, SO, kernel, arq., host, CPUs, seed), hook de panic
que grava `crash-*.log` com as últimas 200 linhas, e rotação das 20 últimas
sessões. API de custo mínimo (buffer + flush periódico) — o Monte Carlo roda
sem logging. Detalhes: [LOGS.md](LOGS.md).

Racional: auditoria mínima (zero dependências ⇒ zero surface de leaks de
terceiros) e diagnóstico reprodutível — mesma seed + log completo = replay
determinístico de qualquer bug reportado.

## Modo Formação 4v4 (v0.3, `formation.rs`)

- **Formação 2×2** por lado; ordem de movimento 1º–4º configurável por time.
- **Iniciativa justa**: moeda decide quem abre a 1ª rodada; alterna a cada rodada.
- **Estados de combo** estilo Naruto Online: `launch` (arremesso) →
  `knockback` (empurrão) → `knockdown` (derrubada); perseguições (chase)
  disparam sobre estados frescos, **máx. 1 por unidade por rodada**
  (`max_per_round`) e cadeia limitada a 12 elos (prova contra loops).
- **Mystery** com chakra (início 20, +12/rodada, teto 100) e cooldown por
  unidade. `ally_single`/`self` + `damage_pct` = **cura/escudo** em % do ATK.
- **DoT** (Queimadura/Veneno): 10% do ATK/rodada, 2 rodadas, sem stack.
- **Roster data-driven**: `Roster::from_json` (contrato v2, validado com erros
  apontando o campo exato) → `game/data/roster/roster-v2.json` (16 personagens,
  4 facções × 4 papéis) com fallback embutido de 12 personagens.
- **Determinismo**: mesma seed ⇒ mesma batalha (testes cobrem).
- **Calibração 4v4 (simulador)**: fase 1 por composição (±500‰/±350‰) + fase 2
  **por confronto** direcionado a extremos (±280‰/±180‰, espelhos intocados).
  Corrigiu um bug real: o índice de matchup era local do chunk multithread —
  espelhos mostravam 1.000; com índice global, espelho = 0.502.
- **Gate v0.3**: desvio médio por composição ≤ 0.25 (meta v0.4: 0.15 após
  redesign dos hard counters entre facções — cadeia detectada:
  tide→ember→verdant→storm→tide). Extremos listados automaticamente no
  relatório como "alvo de redesign".

### Performance medida

- 42 testes (21 antigos + logger/roster/formation) em <0.1s (release).
- Monte Carlo 4v4 completo (43 confrontos × 150 sims, 2 threads): ~13s.
- Batalha 4v4 individual: ordem de microssegundos (200 batalhas < 2ms).
- `cargo clippy --all-targets -- -D warnings`: limpo.
