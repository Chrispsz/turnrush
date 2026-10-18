# Changelog — TurnRush

Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/);
versões seguem [SemVer](https://semver.org/lang/pt-BR/). O jogo está em
**fase de mecânica**: arte/sprites/nomes finais são esboços conscientes.

## [0.7.1] — resposta ao playtest 2026-10-18: manual que funciona e roguelike no meio da luta

### Corrigido
- **Modo manual travava ao escolher skill + alvo (B1/B2 do playtest)**: o
  painel de comando era reenchido a cada frame e resetava a fila de alvos
  ~50 ms depois do clique — os botões de alvo piscavam 1 frame e os cliques
  morriam; a batalha ficava esperando uma entrada que nunca chegava (o
  "freeze" relatado era a mesma espera invisível). Correção: geração de
  decisão na engine (`InteractiveBattle::generation`) + chave
  (geração, janela) no painel — a mesma vez nunca é reenchida; recusas da
  engine agora aparecem no painel ("⚠ ação recusada") em vez de falharem
  em silêncio.

### Adicionado
- **Táticas de batalha a cada 2 rodadas (R1 do playtest)**: nas rodadas
  2/4/6/8 o jogo pausa e o jogador adota 1 de 2 táticas de escopo da
  batalha (Reforço de Linha +12% ATK · Pele de Pedra -8% dano recebido ·
  Fluxo Acelerado +4/rodada · Cura de Campo 3% no fim da rodada). O inimigo
  adota a dele **em silêncio** (mesma força, zero UI). Efeito só até o fim
  da batalha — não entra no teto de poder da run nem afeta o Monte Carlo.

### Alterado
- **Ataque padrão com alvo automático (U1 do playtest)**: mira o inimigo
  espelhado (1↔1, 2↔2…); espelhado caído, primeiro vivo da frente para
  trás. Dispara direto no clique, sem fila de alvos — seleção manual só
  para poderes focados (cura aliada/interrupt).
- **Log de batalha enxuto (U2 do playtest)**: só casts especiais
  (Mystery/Interrupt), mortes, estados, perseguições e separadores de
  rodada entram no log; curas/escudos ficam nos números flutuantes; teto
  de 8 linhas.
- **"Como Jogar" atualizado (U3)**: alvo automático do básico, janela de
  táticas e fluxo de decisão clicado descritos passo a passo.

## [0.7.0] — modo INTERATIVO completo: fluxo, tiers, interrupt, cartas e boss

### Adicionado
- **Engine interativa** (`interactive.rs`): máquina de estados determinística
  para controle manual total — `prompt`/`legal_actions`/`preview_action`/
  `execute_action`/`timeline`, eventos finos (`BattleEvent`) consumidos pelo
  cliente sem parse de texto. O modo compacto (`Formation4v4::run`) segue
  intacto como fonte do Monte Carlo.
- **Economia de Fluxo** (substitui chakra no modo interativo): teto 100,
  +10 por rodada, Padrão grátis e gera ⚡+15, **Skill** ⚡30 (CD 2), **Mystery**
  ⚡60 (CD 3, destrava R2/R3 por papel ou `unlock_schedule` do JSON),
  **Interrupt** ⚡35 (CD 3). Geração passiva com teto +20/rodada.
- **Tiers de habilidade derivados do roster**: Skill = forma base da mystery
  (120% alvo único / 100% multi, utilidade do contrato, sem estado); Mystery =
  forma final (200% único / 100% multi, cura/escudo ×1,65, estado + DoT +
  perfuração). Bandas de dano da spec respeitadas e testadas.
- **Interrupt**: atordoa (perde a ação) ou silencia (sem Skill/Mystery) um
  inimigo que AINDA vai agiar na rodada — janela validada pela engine, zero
  aviso na UI (a informação existe: timeline + ⚡ do inimigo, simétrico).
- **Perseguição manual (chase)**: a janela PAUSA o jogo, aliados elegíveis
  brilham dourado; [ENTER] persegue, [ESC] recusa (tática válida). Bônus de
  primeira perseguição do time (passivas Nimbo/Alísio) modelados.
- **Cartas roguelike** (`rewards.rs`): 12 cartas (Equipe/Unidade/Tática) com
  raridade 600/300/100, preferência de clã +10%, sem repetição na run, alvo
  vivo para cartas de unidade e **teto de poder +30%**. Gate Monte Carlo:
  run sortuda vs média ≤ 10 p.p. (medido: 1,0 p.p.).
- **Run v0.7**: 3 batalhas (a 3ª = **BOSS** escalado), 1 carta entre batalhas
  com reroll; tela de vitória reconstruída.
- **Teto de 10 rodadas** com desempate por % de HP total.
- **IA simétrica** (`greedy_decide` no core): decide apenas por
  `legal_actions` + `preview_action` — contrato de simetria testado
  (zero submissão ilegal em batalhas completas; espelho 25–75%).
- **Cliente Bevy v0.7**: controlador de batalha (`controller.rs`), painel de
  comando com prévia EXATA (dano/cura/escudo/estados/fluxo), barras de Fluxo
  nos 8 personagens (⚡ simétrico), timeline da rodada no cabeçalho, teclado
  completo ([1]-[4] ações, Q/W/E/R alvo inimigo, A/S/D/F aliado, ENTER, ESC,
  ESPAÇO 2×, Z Auto), `--playback` preserva o modo legado v0.6.

### Alterado
- `--check` também conduz uma batalha interativa completa headless.
- Tela "Como Jogar" reescrita com as regras v0.7.

## [0.6.0] — identidade própria + batalha MANUAL + partidas 2× mais rápidas

### Adicionado
- **Batalha manual (você comanda cada ação)**: o engine ganhou API por
  passos (`advance_one`/`submit`/`pending_decision`) — na vez de cada
  unidade sua, o painel inferior oferece a **Mystery** (custo de chakra
  visível) ou o **Padrão com alvo clicável** (escolha o inimigo vivo, ou
  Auto-alvo). Botão **Auto** entrega as decisões à IA quando quiser
  assistir. O adversário continua sendo a IA calibrada do engine.
- **Identidade original**: clãs **PYREBORN / DUSKMERE / THORNVALE /
  SKYREND**, classes **Reaver / Bastion / Mender / Tactician**, 16
  personagens renomeados (`docs/NAMING.md`). Zero referências a outros
  jogos em docs, código e UI; sprites procedurais por (clã × classe) e
  cena de batalha animada (lunge, flash, morte, números flutuantes,
  banners ROUND/CHAIN, screen shake, brasas).
- `AGENTS.md`/`CLAUDE.md` — guia para agentes de IA (invariantes, comandos,
  lições do Bevy 0.20, loop de verificação).

### Mudado (partidas mais rápidas e dinâmicas — pedido direto de playtest)
- **Teto de rodadas: 24 → 12** (desempate por % de vida total).
- **Chakra: 30 inicial (era 20) · +14/rodada (era +12)** — mysteries entram
  mais cedo.
- **Ritmo de dano global ×2.5 (`DAMAGE_PACE` 2500‰)** — aplicado
  simetricamente aos dois lados (DWC recalibra; winrates preservados).
  Medição Monte Carlo (300 batalhas): **22.4 rodadas médias → 7.9**, com
  **94% das partidas terminando por derrota real** (antes: guerra de
  atrito até o teto).
- Pacing do playback: passo 0.36s → 0.30s; rodada 0.95s → 0.80s.
- Como Jogar e menu atualizados ao novo modo manual e números.

### Técnico
- Engine: máquina de fases (`Phase`) equivalente ao loop compacto — teste
  de equivalência `run()` vs `advance_one` garante eventos idênticos no
  modo automático; escolhas inválidas caem defensivamente para o padrão
  (nunca trava); decisões manuais não sofrem jitter de timing (o jitter
  modela imperfeição da IA, não do humano).
- Cliente: `process_event` compartilhado entre playback e modo manual
  (mesma fidelidade visual); 48 testes no core (+4 novos) e clippy zero
  warnings nos 3 crates.

## [0.5.0] — mecânica + menu funcional + documentação completa

### Adicionado
- **Menu mais funcional**: telas **Como Jogar** (regras fiéis à engine:
  draft, turnos A-B-A-B, chakra, cadeias de combo, passivas, recompensas,
  escalada, facções) e **Elenco** (as 16 cartas com kit completo — mystery,
  padrão, chase, passiva, stats), com botão Voltar.
- **Botão Acelerar (1×/6×)** no playback da batalha — testar runs fica 6×
  mais rápido quando quiser.
- **Modo `--shot` estendido**: captura 5 telas (menu, como-jogar, elenco,
  draft, batalha) — insumo direto da galeria do site.
- **Documentação baseada na engine**: `docs/PERSONAGENS.md` (16 personagens
  do contrato v2 + 12 do modo 1v1, gerados do código) e `docs/CARTAS.md`
  (draft de formação, recompensas, escalada, os 16 Contratos, LCE, DWC);
  README e GDD/ARQUITETURA reescritos para "como funciona hoje"; CHANGELOG.
- Versão exibida no menu principal; nota "modo offline — adversário IA".

### Mudado
- README agora documenta o jogo jogável (não só a engine) e aponta para as
  releases.

### Corrigido (mecânica — contadores categóricos entre facções)
- **Causa raiz**: batalhas 4v4 quase determinísticas (só a moeda de abertura
  variava) quantizavam winrates em 0.000/0.500/1.000 e faziam a calibração
  oscilar; times de facção pura eram contadores duros (Ember>Verdant>Storm>
  Ember; Tide perdia de todos).
- **Ordem de movimento amostrada por batalha** no Monte Carlo
  (`set_orders`/`quick_battle_mods_orders`) e na IA do jogo — o winrate passa
  a medir a média sobre escolhas táticas (ordem = decisão de design; UI v0.6).
- **Jitter de timing de mystery (1/4 por turno)** na engine — variância
  simétrica e semeada; batalhas do jogo ficam menos robóticas.
- **DWC 4v4 reforçado** (fase 1 ±550‰/±400‰; fase 2 passos maiores, clamp
  ±520‰/±320‰). Gate 4v4: desvio médio 0.214 → **0.077 ≤ 0.15 — PASSOU**
  (meta v0.4/v0.5 cumprida); pares categóricos restantes listados como alvo
  de redesign de roster.

## [0.4.1] — correção da janela em branco

### Corrigido
- Janela abria 100% em branco no Linux/KDE — **três causas somadas**, todas
  invisíveis no `--check` headless: falta de `Camera2d`, falta da feature
  `bevy_ui_render` (o render da UI virou crate separado no Bevy 0.20) e
  queries de `Text` sobrepostas que panicavam ao entrar na batalha.
- Fonte default do Bevy (subset ASCII) trocada por DejaVu Sans Mono embutida
  — acentos pt-BR renderizam.

### Adicionado
- Modo de diagnóstico `--shot <base>` (menu → draft → batalha, PNGs reais)
  e ambiente de repro sem GPU (Xvfb + lavapipe) para validar visual no CI.

## [0.4.0] — o JOGO 2D de verdade (Bevy)

### Adicionado
- Novo crate **turnrush-game** (Bevy 0.20): janela 1280×720, menu clicável,
  draft visual (escolha 4 de 8 cartas), batalha 4v4 com playback animado da
  engine (barras de HP com HP real do roster, flashes de dano, log de
  combate), recompensas (+10% dano / +10% vida / equilíbrio), escalada de
  inimigos por vitória, fim de run (3 vitórias).
- Roster embutido no binário (16 personagens); zero assets externos.
- AppImage passa a abrir o JOGO (Exec=turnrush-game); o sim musl vai embutido
  e roteado pelo AppRun.

### Corrigido
- `turnrush-sim` sem argumentos agora mostra ajuda e sai (não satura CPU com
  calibração); nova flag `--threads` (default educado: min(cpus, 4)).

## [0.3.1] — logs cobrem TODAS as formas de término

### Adicionado
- Captura de SIGINT/SIGTERM/SIGHUP no logger (zero dependências: self-pipe +
  thread monitora): evento `session_interrupted` + exit 128+sig. Valores de
  tempo migrados para inteiros (contrato do parser).

### Corrigido
- Sessões interrompidas (Ctrl+C/fechar terminal) agora registradas — gap
  encontrado analisando logs reais da máquina do usuário (chris-pc/CachyOS).

## [0.3.0] — Formação 4v4, roster data-driven, logs de sessão

### Adicionado
- **Modo Formação 4v4** (`formation.rs`): formação 2×2, ordem de movimento,
  cadeias de perseguição (arremesso/empurrão/derrubada),
  mysteries com chakra/cooldown, passivas, DoT.
- **Contrato JSON v2** para rosters (`Roster::from_json`, validação com erro
  por campo) + `data/roster/roster-v2.json` (16 personagens, 4 facções).
- **Logs de sessão completos**: `~/TurnRush/logs` (.log + .jsonl, cabeçalho de
  ambiente, crash report em panic, retenção 20).
- Calibração 4v4 no simulador (por composição + por confronto) e gate próprio.

## [0.2.0] — DWC v2 e gates de CI

### Adicionado
- **DWC v2**: calibração em 2 fases (composição → confronto) sobre Monte Carlo
  de TODAS as composições (C(12,3) = 220 → 48.180 confrontos), passada de
  verificação em seeds novos, relatório JSON.
- Gates de equilíbrio no CI; diagnóstico automático de hard counters.
- Repositório remoto + GitHub Actions (testes + gate + build Linux).

## [0.1.0] — motor determinístico

### Adicionado
- Workspace Rust com **zero dependências externas**: RNG xoshiro256**,
  combate por turnos 1v1 (alternância A-B-A-B, energia/chakra, cooldowns,
  launch/chase, status, febre de batalha, juízo anti-turtle), draft simétrico
  com LCE, partida de 9 rodadas, roster v0.1 (12 personagens), simulador CLI,
  21 testes.
