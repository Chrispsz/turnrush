# Arquitetura Técnica — TurnRush

> TurnRush: tactical turn-based roguelike com cadeias de combo (chase),
> draft simétrico e balanceamento Monte Carlo (DWC/LCE). 100% Rust, Linux first.

## Visão geral

```
game/
├─ Cargo.toml                  # workspace (core sem dependências; game usa Bevy)
├─ rust-toolchain.toml         # stable, profile minimal
├─ crates/
│  ├─ turnrush-core/           # motor determinístico (lib, std-only)
│  │  └─ src/
│  │     ├─ rng.rs             # xoshiro256** semeado (splitmix64)
│  │     ├─ types.rs           # Stats, Ability, Unit, Team, Outcome
│  │     ├─ data.rs            # ROSTER 1v1 (12, const) + contrato JSON v2 (Roster)
│  │     ├─ cards.rs           # pool de 16 Contratos + draft + LCE
│  │     ├─ combat.rs          # motor de turnos 1v1 + IA greedy + build_team
│  │     ├─ formation.rs       # Formação 4v4 (chakra, cadeias, passivas, DoT)
│  │     ├─ run.rs             # partida 1v1 (9 rodadas, draft, escalada)
│  │     ├─ balance.rs         # DWC (Monte Carlo + calibração 2 fases + gate)
│  │     ├─ json.rs            # parser JSON std-only (contrato v2, relatórios)
│  │     └─ logger.rs          # logs de sessão (.log+.jsonl, panic, sinais)
│  ├─ turnrush-sim/            # binário: simulador, gate de CI
│  │  └─ src/main.rs           # CLI: --quick --gate --out --match --formation*
│  └─ turnrush-game/           # CLIENTE 2D jogável (Bevy 0.20)
│     ├─ src/app.rs            # estados: Menu/ComoJogar/Elenco/Draft/Battle/Reward/GameOver
│     ├─ src/battle_log.rs     # parser tolerante dos eventos da engine
│     ├─ src/main.rs           # modos: janela | --check | --shot <base>
│     └─ assets/DejaVuSansMono.ttf  # fonte embutida (acentos pt-BR)
├─ data/
│  └─ roster/roster-v2.json    # 16 personagens (fonte da verdade do elenco)
├─ .github/workflows/
│  ├─ build.yml                # testes + gates + smoke --check + build Linux
│  └─ release.yml              # tag v* → AppImage (jogo) + sim musl
└─ docs/                       # GDD, ARQUITETURA, PERSONAGENS, CARTAS, NAMING, LOGS
```

## Princípios

1. **Determinismo total** — matemática 100% inteira (per-mille, sem float na
   lógica de jogo) e RNG semeado xoshiro256**. A mesma seed produz a mesma
   partida em qualquer plataforma → replays, anti-cheat e testes estatísticos
   reprodutíveis.
2. **Zero dependências externas** — `std` apenas no core. CI rápido, build
   pequeno, sem risco de supply chain.
3. **Roster como dados** — o modo 1v1 usa tabela `const`; o 4v4 usa o
   **contrato JSON v2** (`data/roster/roster-v2.json`, embutido no binário via
   `include_str!`) — adicionar personagem = editar JSON + rodar o DWC (o CI
   recalibra sozinho).
4. **Motor headless** — `turnrush-core` não conhece IO/render; o mesmo binário
   lógico servirá servidor autoritativo (M2) e o cliente 2D já o consome (M3).
5. **Cliente como playback** — a batalha no jogo é o playback dos eventos da
   engine real (não há lógica de combate duplicada no cliente): o que você vê
   é exatamente o que o simulador calcula.

## Fluxo de dados (roster → engine → eventos → playback)

```
data/roster/roster-v2.json (contrato v2)
        │  Roster::from_json (validação com erro por campo) / include_str!
        ▼
turnrush-core::Formation4v4        ← comps + seed + FormationMods (DWC)
        │  run() → Outcome + eventos de texto + contadores (elos, mysteries)
        ▼
        ├─► turnrush-sim: Monte Carlo → calibração DWC → gate + relatório JSON
        └─► turnrush-game: battle_log.rs (parser tolerante) → playback na UI
                 (barras de HP, flashes por clã, log, Acelerar 1×/6×)
```

- O contrato v2 define por personagem: `id`, `name`, `faction`, `role`, stats
  (`hp/attack/defense/speed`) e 4 slots (`mystery`, `standard`, `chase`,
  `passive`). Interpretações contratadas do loader: `ally_single`/`self` +
  `damage_pct` = **cura/escudo** em % do ATK; efeitos de texto das passivas
  são interpretados por palavras-chave — **o não modelado fica documentado**
  (`unmodeled()`), nunca silencioso.
- No cliente, linhas desconhecidas viram `Other` no parser e continuam
  visíveis no log — o playback nunca quebra por causa de um evento novo.

## Determinismo

- **RNG**: xoshiro256** semeado via splitmix64 (`rng.rs`) — fluxo único por
  batalha/partida; moeda de abertura, alvos aleatórios (`random_2`), shuffle
  de ofertas e jitter de mystery saem do mesmo stream.
- **Matemática**: inteiros per-mille (1000‰ = 100%) em todos os
  multiplicadores (DWC, escalada, febre, buffs, auras). Zero float no caminho
  quente; sem `unsafe`.
- **Testes de determinismo**: mesma seed ⇒ mesmo `Outcome`, mesmos eventos e
  mesmos contadores de cadeia (cobertos nos 44 testes do workspace).
- **Seed documentada nos logs**: `battle_start` registra seed e mods —
  qualquer partida pode ser reproduzida e auditada.

## Logger de sessão (JSONL + LOG) e crash reports

Subsistema std-only (`logger.rs`): uma sessão = um par de arquivos em
`$HOME/TurnRush/logs/` (`.log` legível + `.jsonl` estruturado), cabeçalho com
ambiente completo (versão da engine, SO, kernel, arquitetura, host, CPUs,
seed), rotação das 20 últimas sessões e API de custo mínimo (buffer + flush
periódico — o Monte Carlo roda sem logging).

- **Término limpo**: `session_end` com tempo total (ms, inteiro — contrato do
  parser).
- **Panic**: hook global grava `crash-YYYYMMDD-HHMMSS.log` com mensagem,
  local e as últimas ~200 linhas da sessão.
- **Sinais**: captura de SIGINT/SIGTERM/SIGHUP (self-pipe + thread monitora,
  handler async-signal-safe) → `session_interrupted` + exit `128+sig`.
- Eventos do jogo (`game_start`, `battle_start` com seed/mods, `battle_end`,
  `reward_picked`, `game_end`) vão para os MESMOS arquivos — o log é o
  roteiro completo da sessão.

Detalhes, formatos e guia de reporte: [LOGS.md](LOGS.md).

## DWC — fluxo de calibração

```
para cada iteração (fase 1):
    simular os confrontos amostrados (threads = núcleos)
    ajustar modificadores por composição (dano/vida, per-mille)
fase 2: ajuste POR CONFRONTO direcionado a extremos (wr > 0.85 / < 0.15)
verificação em seeds novos → relatório JSON + gate
```

- **1v1**: Monte Carlo de TODAS as composições (C(12,3) = 220 → 48.180
  confrontos ordenados); gate: desvio médio ≤ 0.15 (medido 0.119).
- **4v4**: amostragem documentada — composições puras de clã + confrontos
  entre clãs + composições mistas (seed fixa); calibração 2 fases com alcance
  reforçado (fase 1 ±550‰/±400‰; fase 2 clamp ±520‰/±320‰); gate completo
  (150 sims): desvio médio **0.077 ≤ 0.15**. Pares categóricos restantes por
  kit são listados no relatório como alvo de redesign de roster.
- Relatório JSON (`--out`): stats por comp + matrizes de ajuste
  (adj_dmg/adj_hp) — insumo direto do futuro servidor autoritativo.

## Variância tática (v0.5)

Batalhas 4v4 quase determinísticas quantizavam winrates em 0/0.5/1 e faziam a
calibração oscilar. Duas correções simétricas e semeadas na engine:

1. **Ordem de movimento amostrada** (`set_orders`/`quick_battle_mods_orders`):
   o Monte Carlo sorteia a ordem 1º–4º por batalha — o winrate mede a média
   sobre escolhas táticas; a IA do jogo sorteia a própria ordem (variância de
   replay).
2. **Jitter de timing de mystery** (1/4 de segurar por turno em `act`):
   modela imperfeição humana de timing e dá gradiente contínuo aos winrates.

## CI (GitHub Actions)

**Build & Balance CI** (`build.yml`) — dispara em push/PR, `workflow_dispatch`
e **cron noturno `0 7 * * *`** (04:00 em São Paulo; Monte Carlo completo):

| Job | O que faz |
|---|---|
| `test` | `cargo test --workspace --release` (44 testes) |
| `balance` | gate 1v1 (`--quick --gate`) + publica `balance-report.json` |
| `balance-formation` | gate 4v4 (`--formation4v4 --formation-gate --formation-sims 150`) + publica relatório |
| `build-linux` | build release do workspace + smoke `turnrush-game --check` + artifact |

**Release** (`release.yml`) — dispara em tag `v*`: compila o jogo (glibc,
wayland+x11) e o simulador (**musl estático**), roda smoke tests
(`--check` + sim rápido), monta o AppDir e o AppImage (`appimagetool --comp
zstd`), valida AppStream strict e publica na release com os 3 artefatos
(AppImage + 2 tar.gz).

## AppImage

- O **jogo** (`turnrush-game`, Bevy/wgpu) é o executável principal — glibc
  dinâmico, abre com clique duplo (`Exec=turnrush-game`, `Terminal=false`).
- O **simulador** vai embutido como ferramenta secundária (musl estático) e o
  `AppRun` roteia flags do sim.
- AppDir completo: `AppRun`, `.desktop`, metainfo AppStream 1.0
  (`io.github.Chrispsz.turnrush`), ícones hicolor 128/256, `.DirIcon`, LICENSE.
- Smoke test do artefato antes do upload: jogo `--check` + sim `--quick`.
- Compressão **zstd** (obrigatória no `appimagetool` continuous).

## Cliente 2D jogável (`turnrush-game`)

Bevy 0.20 com `default-features = false` + features mínimas de UI. Estados
(`GameState`): `Menu → ComoJogar | Elenco → Draft → Battle → Reward → GameOver`.

- **Raízes persistentes por estado**: cada tela é construída UMA vez no
  `Startup` e mostrada/escondida por `Visibility` — sem despawn recursivo.
- **Batalha = playback da engine**: `Formation4v4` roda uma vez com log; o
  cliente consome os eventos via parser tolerante. HP, flashes por clã e log
  são derivados dos eventos; velocidade 1×/6× via `FastForward`.
- **Fonte embutida** (DejaVu Sans Mono, licença livre) substitui o subset
  ASCII da fonte default — acentos pt-BR renderizam.
- **Roster embutido** via `include_str!` do contrato v2 — o binário não
  depende de arquivos externos.

### Lições Bevy 0.20 (registradas para não repetir)

1. `Camera2d` é OBRIGATÓRIO — UI sem câmera = janela em branco.
2. `bevy_ui_render` é crate separado no 0.20 — sem ele a UI roda layout mas
   NÃO DESENHA nada.
3. Queries de `Text` devem ser disjuntas (marcadores) — sobreposição panica.
4. `Node::default()` é Row — telas em coluna precisam `FlexDirection::Column`.
5. `Button`/`Interaction` estão deprecados (removidos no futuro); migração para
   picking + `Pressed` planejada.

### Verificação visual sem interação (repro local + CI)

- `--check`: headless (sem janela/wgpu) — roster + batalha real + parser.
- `--shot <base>`: abre janela, navega Menu → Como Jogar → Elenco → Draft →
  Batalha e salva 5 PNGs, com verificação de arquivo gravado.
- Ambiente sem GPU: Xvfb + mesa-vulkan-drivers (lavapipe) + libxkbcommon-x11 —
  comprovado no sandbox e no AppImage baixado da release.

## Netcode planejado (M2)

- Servidor autoritativo em tokio + WebSocket (porta própria, mini-service).
- Cliente envia **intenções** (pick de carta, ação de turno); servidor valida,
  simula com `turnrush-core` e difunde snapshots determinísticos.
- Seed única por partida → os dois clientes podem pré-calcular e o servidor
  apenas valida hashes de estado (anti-cheat barato).
- Reconexão: replay a partir do último snapshot (determinismo garante
  sincronia exata).

## Performance medida

- Batalha 4v4 individual: ordem de microssegundos (200 batalhas < 2 ms no
  teste de regressão, build release).
- Monte Carlo 4v4 completo (43 confrontos × 150 sims, 2 threads): ~13 s.
- Batalha 1v1 isolada: ~2–4 µs (sandbox 2 vCPU).
- Testes completos do workspace: < 1 s (release).
- `cargo clippy --all-targets -- -D warnings`: limpo.
