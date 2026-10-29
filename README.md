# TurnRush

**Tactical turn-based roguelike** com cadeias de combo (chase), draft simétrico
e balanceamento Monte Carlo (DWC/LCE).
100% Rust · modo **offline vs IA** · Linux first · CI no GitHub Actions.

> **Fase atual: mecânica e gameplay.** Menu/telas são esboços funcionais
> (arte, sprites e polimento visual vêm depois). Tudo que está documentado
> aqui está implementado e testado na engine.

| | |
|---|---|
| **Download** | [Releases (AppImage)](https://github.com/Chrispsz/turnrush/releases/latest) |
| **Docs** | [GDD](docs/GDD.md) · [Arquitetura](docs/ARQUITETURA.md) · [Personagens](docs/PERSONAGENS.md) · [Cartas](docs/CARTAS.md) · [Nomenclatura](docs/NAMING.md) · [Logs](docs/LOGS.md) |
| **Status** | v0.10.0 — AppImage **autocontido** + portátil sem FUSE + `doctor` (roda em distro mínima/Wayland-only, ex.: Archcraft Elite); elementos, combo automático e UI v0.9; 16 peças do quebra-cabeça verdes, clippy limpo |

## O que é

TurnRush é um jogo de combate tático por turnos em que **a sorte não decide**:
não existem itens, loot ou níveis persistentes — a única vantagem permitida é
a estratégia da composição que você monta. Cada run é uma escalada curta de
batalhas 4v4 com **cadeias de combo** (Lançamento → Empurrão → Derrubada),
**draft simétrico** de cartas e recompensas persistentes — e todo o
equilíbrio é **calibrado por dados** por um simulador Monte Carlo que roda no
CI a cada commit.

## O jogo hoje (v0.9.1)

```
Menu → Elenco (clique = página do kit) → Draft (escolha 4 de 8) → Batalha 4v4 INTERATIVA
     → CARTA (1 de 3, com reroll) → inimigo escala → 3ª luta = BOSS → 3 vitórias
```

- **Modo interativo completo**: você comanda **tudo** — a timeline da rodada é
  visível (SPD), cada unidade tem Fluxo ⚡ (Padrão grátis e gera +15; Skill 30;
  Mystery 60 com desbloqueio R2/R3; Interrupt 35), e o painel mostra a
  **prévia exata** de cada ação antes de confirmar. **Combos são automáticos**
  (v0.9.1): quem tem o gatilho compatível persegue sozinho — o jogo mostra o
  flash "COMBO!" e os elegíveis brilham dourado, sem pausa e sem custo ⚡.
  Unidades com **Interrupt** atordoam ou silenciam um inimigo que ainda vai
  agir — o jogo não avisa: leia a timeline e o fluxo alheio.
- **Roguelike**: entre batalhas você compra 1 de 3 cartas (Equipe/Unidade/
  Tática, raridades e preferência de clã, teto de poder +30% na run). A 3ª
  batalha é o BOSS. Teto de 10 rodadas com desempate por % de vida.
- **Elementos (v0.9)**: fogo, água, elétrico e pedra com ciclo de fraquezas
  (Água > Fogo > Pedra > Elétrico > Água; ±20% de dano). **Ressonância** —
  golpes de mesmo elemento em sequência do seu time dão +15% de dano e +5⚡
  (o "bombo mono-elemento"); **Cadeia elemental** — golpear na progressão do
  ciclo com elemento DIFERENTE dá +10% e abre espaço para passivas de
  sintonia (+20%) formando combos entre elementos. Tudo visível na prévia.
- **IA simétrica**: o adversário decide pela MESMA API que você usa
  (`legal_actions` + `preview_action`) — contrato testado, zero trapaça.
- **Menu funcional**: Elenco com página de detalhe por personagem (as 16
  cartas com kit completo) e **ESPAÇO = 2×** na batalha (modo legado 1×/6×
  preservado via `--playback`).
- **Determinismo total**: RNG xoshiro256** + matemática inteira (per-mille) —
  mesma seed = mesma partida em qualquer plataforma.

## Recursos

- **Engine determinística ultraleve** — batalhas resolvidas em microssegundos
  (200 batalhas 4v4 em < 2 ms no teste de regressão, build release); zero
  dependências externas no `turnrush-core`.
- **4v4 com cadeias de combo (chase)** — três estados de controle
  (Lançamento/Empurrão/Derrubada) alimentam perseguições encadeadas entre
  aliados, com limite de 1 por unidade/rodada e 12 elos por cadeia.
- **Draft simétrico** — os dois lados veem a mesma oferta de cartas; a sorte
  não é assimétrica por construção.
- **DWC/LCE** — calibração Monte Carlo de todas as composições (DWC) e
  compensação de sorte em tempo real (LCE), com gate de equilíbrio no CI.
- **Cliente Bevy 2D** — janela 1280×720, draft visual, batalha manual
  (ou playback automático no botão Auto) com animações, recompensas e
  escalada do inimigo. Roster embutido no binário — **zero assets externos**.
- **Logs de sessão completos** — do iniciar ao fechar, em `~/TurnRush/logs`
  (`.log` legível + `.jsonl` estruturado), incluindo captura de
  SIGINT/SIGTERM/SIGHUP.
- **Crash reports** — hook de panic global grava `crash-*.log` com backtrace
  e as últimas ~200 linhas da sessão.

## Quick start

### Jogar (janela 1280×720)

```bash
cargo run --release -p turnrush-game        # jogo completo (offline vs IA)
cargo run --release -p turnrush-game -- --check   # smoke test headless (CI)
cargo run --release -p turnrush-game -- --shot /tmp/tr   # captura 5 PNGs das telas
```

Ou baixe da [última release](https://github.com/Chrispsz/turnrush/releases/latest):

| Formato | Como rodar | Exige do host |
|---|---|---|
| `TurnRush-*.AppImage` | `chmod +x TurnRush-*.AppImage && ./TurnRush-*.AppImage` | FUSE2 (sem ele: `./TurnRush-*.AppImage --appimage-extract-and-run`) |
| `TurnRush-*-portable.tar.gz` | extrair e `./turnrush.AppDir/turnrush` | nada — **sem FUSE** |
| `turnrush-doctor.sh` | `./turnrush-doctor.sh TurnRush-*.AppImage` | shell POSIX |

O AppImage e o portátil são **autocontidos**: a stack cliente X11/xkb/wayland
vai embutida — do host só precisam de **glibc** e **drivers gráficos
(Vulkan/GL)**. Sessão Wayland-only (Archcraft Elite, sway, GNOME) abre
nativo. Não abriu? → [docs/COMPATIBILIDADE.md](docs/COMPATIBILIDADE.md).

### Ferramentas de balanceamento

```bash
cargo test --workspace --release                        # 51 testes
cargo run --release -p turnrush-sim -- --match          # demonstra uma partida
cargo run --release -p turnrush-sim -- --quick --gate   # gate de equilíbrio (CI)
cargo run --release -p turnrush-sim --                  # calibração completa (Monte Carlo)
```

## Clãs e classes

4 clãs × 4 classes = **16 personagens** (detalhes completos em
[docs/PERSONAGENS.md](docs/PERSONAGENS.md); racional dos nomes em
[docs/NAMING.md](docs/NAMING.md)):

| Clã | Identidade | Reaver | Bastion | Mender | Tactician |
|---|---|---|---|---|---|
| **PYREBORN** | fogo · agressão | Ignia Emberbright | Orson Moltenwall | Amara Sweetflame | Nimbo Smokeveil |
| **DUSKMERE** | maré · controle | Nereo Saltblade | Dalva Reefwarden | Mel Tidemother | Ligeia Coldmist |
| **THORNVALE** | natureza · veneno | Tina Twinthorn | Aro Barkshell | Yara Lifesap | Ivo Finewire |
| **SKYREND** | tempestade · velocidade | Reia Thunderstep | Aki Countercut | Alísio Lightgale | Volta Cyclone |

Por decisão de balanceamento **não existe contador duro entre clãs** — o DWC
calibra todas as composições para ~50% de winrate médio. Clã é identidade
temática/mecânica, não pedra-papel-tesoura.

## Crates

| Crate | Papel |
|---|---|
| **turnrush-core** | Motor determinístico (zero dependências): Formação 4v4 (chakra, cooldowns, cadeias de chase, passivas), draft simétrico + LCE, partida 1v1 de 9 rodadas, calibrador DWC, roster data-driven (contrato JSON v2), logs de sessão |
| **turnrush-sim** | CLI: simulador Monte Carlo de composições (clãs puras + mistas), gate de equilíbrio de CI, diagnóstico de confrontos extremos, relatório JSON |
| **turnrush-game** | Cliente 2D jogável (Bevy 0.20): menu, Elenco + detalhe, draft visual, **batalha manual** (painel Mystery/Padrão/alvo) ou Auto, barras de HP, flashes, log, Acelerar 1×/6×, recompensas, escalada de inimigos, fim de run |

## Documentação

| Doc | Conteúdo |
|---|---|
| [docs/GDD.md](docs/GDD.md) | Game Design Document — visão, loop da run, combate 4v4, clãs, equilíbrio em 3 camadas |
| [docs/ARQUITETURA.md](docs/ARQUITETURA.md) | Arquitetura técnica — crates, fluxo de dados, determinismo, logger, CI/CD, AppImage |
| [docs/PERSONAGENS.md](docs/PERSONAGENS.md) | Os 16 personagens: stats, mystery, padrão, chase, passiva e estilo de jogo de cada um |
| [docs/CARTAS.md](docs/CARTAS.md) | Sistema de cartas: draft de formação, recompensas, escalada, os 16 Contratos do 1v1, LCE e DWC |
| [docs/NAMING.md](docs/NAMING.md) | Nomenclatura oficial (clãs, classes, personagens) e proposta de nomes para habilidades |
| [docs/LOGS.md](docs/LOGS.md) | Logs de sessão completos (`~/TurnRush/logs`): início→fim, panics, sinais (Ctrl+C) |

## Balanceamento (filosofia)

1. **Draft simétrico** — os dois lados veem a mesma oferta (nenhuma assimetria existe).
2. **LCE** — quem está atrás no valor acumulado recebe a próxima carta promovida +1 tier.
3. **DWC** — Monte Carlo de todas as composições calibra dano/vida por composição
   **e por confronto** (neutraliza hard counters); gate de CI: desvio médio ≤ 0.15
   (medido: 0.077 no gate completo do 4v4).
4. **Clãs são identidade — sem contador duro** entre clãs.

## Desenvolvimento

- `cargo test --workspace` — 51 testes (determinismo, simetria, chase, fórmulas, manual, logger).
- `--check` (headless) e `--shot` (screenshots reais das 5 telas) rodam em CI e em
  ambiente sem GPU via **Xvfb + lavapipe** — nunca dependemos da máquina do usuário
  para validar visual.
- Logs de sessão em `~/TurnRush/logs` cobrem **do iniciar ao fechar**: limpo
  (`session_end`), panic (`crash-*.log` com backtrace) e sinais
  (`session_interrupted`, exit 128+sig).
- CI: **Build & Balance CI** a cada push (testes + gates de equilíbrio 1v1 e 4v4 +
  build Linux + smoke headless) com **cron noturno** rodando o Monte Carlo completo;
  **Release** automática por tag `v*` gerando o AppImage.

## Roadmap

- ✅ M1 — motor determinístico + simulador + partida jogável local
- ✅ M3 (1ª versão) — cliente 2D jogável offline vs IA (arte final pendente)
- ⏳ Renomear habilidades/passivas para inglês (ver [docs/NAMING.md](docs/NAMING.md))
- ⏳ Expansão de cartas (boons de cadeia, sinergias de clã) — ver [docs/CARTAS.md](docs/CARTAS.md)
- ⏳ M2 — servidor autoritativo (WebSocket) para o multiplayer online
- ⏳ 1v1 com draft de cartas na UI do jogo (hoje o 1v1 vive no simulador)
- ⏳ Arte, sprites e polimento visual (fase pós-mecânica)
- ⏳ Beta fechada Linux + ranqueado

## Licença

MIT — ver [LICENSE](LICENSE).
