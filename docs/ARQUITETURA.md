# Arquitetura Técnica — TurnRush v0.5.0

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
│  │     ├─ cards.rs           # pool de 16 contratos + draft + LCE
│  │     ├─ combat.rs          # motor de turnos 1v1 + IA greedy + build_team
│  │     ├─ formation.rs       # Formação 4v4 (chakra, cadeias, passivas, DoT)
│  │     ├─ run.rs             # partida 1v1 (9 rodadas, draft, escalação)
│  │     ├─ balance.rs         # DWC v2 (Monte Carlo + calibração 2 fases + gate)
│  │     ├─ json.rs            # parser JSON std-only (contrato v2, relatórios)
│  │     └─ logger.rs          # logs de sessão (.log+.jsonl, panic, sinais)
│  ├─ turnrush-sim/            # binário: simulador, probe, gate de CI
│  │  └─ src/main.rs           # CLI: --quick --gate --out --match --formation*
│  └─ turnrush-game/           # CLIENTE 2D jogável (Bevy 0.20)
│     ├─ src/app.rs            # estados: Menu/ComoJogar/Elenco/Draft/Battle/Reward/GameOver
│     ├─ src/battle_log.rs     # parser tolerante dos eventos da engine
│     ├─ src/main.rs           # modos: janela | --check | --shot <base>
│     └─ assets/DejaVuSansMono.ttf  # fonte embutida (acentos pt-BR)
├─ data/
│  ├─ roster/roster-v2.json    # 16 personagens (fonte da verdade do elenco)
│  └─ naruto-online/           # estudo do banco de dados do Naruto Online
├─ .github/workflows/
│  ├─ build.yml                # testes + gates + smoke --check + build Linux
│  └─ release.yml              # tag v* → AppImage (jogo) + sim musl
└─ docs/                       # GDD, ARQUITETURA, PERSONAGENS, CARTAS, LOGS…
```

## Princípios

1. **Determinismo total** — matemática 100% inteira (per-mille, sem float na
   lógica de jogo) e RNG semeado xoshiro256**. A mesma seed produz a mesma
   partida em qualquer plataforma → replays, anti-cheat e testes estatísticos
   reprodutíveis.
2. **Zero dependências externas** — `std` apenas. CI rápido, build pequeno,
   sem risco de supply chain.
3. **Roster como dados** — o modo 1v1 usa tabela `const`; o 4v4 usa o
   **contrato JSON v2** (`data/roster/roster-v2.json`, embutido no binário via
   `include_str!`) — adicionar personagem = editar JSON + rodar o DWC (o CI
   recalibra sozinho).
4. **Motor headless** — `turnrush-core` não conhece IO/render; o mesmo binário
   lógico servirá servidor autoritativo (M2) e o cliente 2D já o consome (M3).
5. **Cliente como playback** — a batalha no jogo é o playback dos eventos da
   engine real (não há lógica de combate duplicada no cliente): o que você vê
   é exatamente o que o simulador calcula.

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

- 44 testes (21 antigos + logger/roster/formation) em <0.1s (release).
- Monte Carlo 4v4 completo (43 confrontos × 150 sims, 2 threads): ~13s.
- Batalha 4v4 individual: ordem de microssegundos (200 batalhas < 2ms).
- `cargo clippy --all-targets -- -D warnings`: limpo.

---

## Cliente 2D jogável (v0.4–v0.5, `turnrush-game`)

Bevy 0.20 com `default-features = false` + features mínimas de UI. Estados
(`GameState`): `Menu → ComoJogar | Elenco → Draft → Battle → Reward → GameOver`.

- **Raízes persistentes por estado**: cada tela é construída UMA vez no
  `Startup` e mostrada/escondida por `Visibility` — sem despawn recursivo.
- **Seleção por marcador** (`CardSelected`), nunca por cor.
- **Batalha = playback da engine**: `Formation4v4` roda uma vez com log; o
  cliente consome os eventos via parser tolerante (`battle_log.rs` — linhas
  desconhecidas viram `Other` e continuam visíveis). HP, flashes por facção e
  log são derivados dos eventos; velocidade 1×/6× via `FastForward`.
- **Fonte embutida** (DejaVu Sans Mono, licença livre) substitui o subset
  ASCII da fonte default — acentos pt-BR renderizam.
- **Roster embutido** via `include_str!` do contrato v2 — o binário não
  depende de arquivos externos.
- **Logs de sessão**: o cliente usa o MESMO logger do engine
  (`~/TurnRush/logs`) — `game_start`, `battle_start` (com seed e mods),
  `battle_end`, `game_end`, screenshots.

### Lições Bevy 0.20 (registradas para não repetir)

1. `Camera2d` é OBRIGATÓRIO — UI sem câmera = janela em branco.
2. `bevy_ui_render` é crate separado no 0.20 — sem ele a UI roda layout mas
   NÃO DESENHA nada.
3. Queries de `Text` devem ser disjuntas (marcadores) — sobreposição panica.
4. `Node::default()` é Row — telas em coluna precisam `FlexDirection::Column`.
5. `Button`/`Interaction` estão deprecados (removidos no futuro); migração para
   picking + `Pressed` planejada para a v0.6.

### Verificação visual sem interação (repro local + CI)

- `--check`: headless (sem janela/wgpu) — roster + batalha real + parser.
- `--shot <base>`: abre janela, navega Menu → Como Jogar → Elenco → Draft →
  Batalha e salva 5 PNGs (`_menu`, `_como-jogar`, `_elenco`, `_draft`,
  `_battle`), com verificação de arquivo gravado.
- Ambiente sem GPU: Xvfb + mesa-vulkan-drivers (lavapipe) + libxkbcommon-x11
  extraídos de .deb (sem sudo) + `VK_ICD_FILENAMES`/`LD_LIBRARY_PATH`.
  Comprovado no sandbox e no AppImage baixado da release.

---

## Variância tática e DWC 4v4 (v0.5 — o conserto dos contadores categóricos)

**Problema real descoberto pela verificação v0.5**: batalhas 4v4 eram quase
determinísticas (a única aleatoriedade era a moeda de abertura + o alvo
aleatório da Tina) → winrates quantizavam em 0.000/0.500/1.000, a calibração
oscilava (ajuste +95‰ derrubava o wr de 1.000 para 0.000) e confrontos entre
facções puras ficavam categóricos (triângulo Ember>Verdant>Storm>Ember; Tide
perdia de todos).

**Correções na engine** (ambas simétricas e semeadas — determinismo mantido):

1. **Ordem de movimento amostrada** — `Formation4v4::set_orders` + 
   `quick_battle_mods_orders`: o Monte Carlo sorteia a ordem 1º–4º de cada
   lado por batalha. O winrate passa a medir a **média sobre as escolhas
   táticas** (a ordem é decisão do design — a UI de configuração é roadmap).
   No jogo, a IA adversária sorteia a própria ordem por batalha (variância de
   replay).
2. **Jitter de timing de mystery** — 1/4 de chance de segurar a mystery por um
   turno (`act`): modela imperfeição humana de timing, quebra sequências
   ótimas fixas e dá gradiente contínuo aos winrates.

**DWC 4v4 reforçado**: fase 1 com alcance ±550‰/±400‰; fase 2 (ajuste por
confronto) com passos maiores e clamp ±520‰ dano / ±320‰ vida — alcance para
neutralizar assimetrias estruturais entre arquétipos.

**Resultado**: desvio médio **0.077 ≤ 0.15 → PASSOU** (gate completo, 150 sims; meta
v0.4/v0.5 atingida; era 0.214). Espelhos 0.503. Confrontos moderados ficam
intocados; pares ainda categóricos por kit (ex.: comps com dupla cura vs
pressão de Ember) são listados como **alvo de redesign de roster (v0.6)** —
o diagnóstico aponta os pares exatos a cada execução no CI e no cron noturno.
