# TurnRush

**Roguelike acelerado** com combate por turnos estilo **Naruto Online** —
draft simétrico de cartas, cadeias de combo (Lançamento → Empurrão →
Derrubada) e **balanceamento absoluto por dados** (DWC v2 + LCE).
100% Rust · modo **offline vs IA** · Linux first · CI no GitHub Actions.

> **Fase atual: mecânica e gameplay.** Menu/telas são esboços funcionais
> (arte, sprites e nomes finais vêm depois). Tudo que está documentado aqui
> está implementado e testado na engine.

| | |
|---|---|
| **Download** | [Releases (AppImage)](https://github.com/Chrispsz/turnrush/releases/latest) |
| **Docs** | [GDD](docs/GDD.md) · [Arquitetura](docs/ARQUITETURA.md) · [Personagens](docs/PERSONAGENS.md) · [Cartas](docs/CARTAS.md) · [Logs](docs/LOGS.md) · [Inspiracoes](docs/INSPIRACOES.md) |
| **Status** | v0.5.0 — jogo 2D jogável offline vs IA, 44 testes, clippy limpo |

## O jogo hoje (v0.5.0)

```
Menu → Como Jogar / Elenco → Draft (escolha 4 de 8) → Batalha 4v4
     → Recompensa (+10% dano | +10% vida | +5%/+5%) → inimigo escala → 3 vitórias
```

- **Modo offline completo**: o adversário é controlado pela IA do engine
  (a mesma heurística validada pelo simulador Monte Carlo).
- **Batalha 4v4 por turnos**: alternância A-B-A-B, chakra (20 inicial,
  +12/rodada, teto 100), mysteries com cooldown, cadeias de perseguição
  (até 12 elos), passivas, dano contínuo (Queimadura/Veneno).
- **Menu funcional**: Como Jogar (regras fiéis à engine), Elenco (as 16
  cartas com kit completo) e botão **Acelerar (1×/6×)** na batalha.
- **Determinismo total**: RNG xoshiro256** + matemática inteira (per-mille) —
  mesma seed = mesma partida em qualquer plataforma.

## Quick start

### Jogar (janela 1280×720)

```bash
cargo run --release -p turnrush-game        # jogo completo (offline vs IA)
cargo run --release -p turnrush-game -- --check   # smoke test headless (CI)
cargo run --release -p turnrush-game -- --shot /tmp/tr   # captura 5 PNGs das telas
```

Ou baixe o **AppImage** da [última release](https://github.com/Chrispsz/turnrush/releases/latest)
(`chmod +x TurnRush-*.AppImage && ./TurnRush-*.AppImage`).

### Ferramentas de balanceamento

```bash
cargo test --workspace --release                        # 44 testes
cargo run --release -p turnrush-sim -- --match          # demonstra uma partida
cargo run --release -p turnrush-sim -- --quick --gate   # gate de equilíbrio (CI)
cargo run --release -p turnrush-sim --                  # calibração completa (Monte Carlo)
```

## Crates

| Crate | Papel |
|---|---|
| **turnrush-core** | Motor determinístico (zero dependências): combate 1v1 e Formação 4v4 (chakra, cooldowns, cadeias de chase, passivas), draft simétrico + LCE, partida de 9 rodadas, calibrador DWC v2, roster data-driven (contrato JSON v2), logs de sessão |
| **turnrush-sim** | CLI: simulador Monte Carlo de TODAS as composições (C(12,3) = 220 → 48.180 confrontos), gate de equilíbrio de CI, diagnóstico de hard counters, relatório JSON |
| **turnrush-game** | Cliente 2D jogável (Bevy 0.20): menu, Como Jogar, Elenco, draft visual, batalha com playback animado (barras de HP, flashes, log, Acelerar 1×/6×), recompensas, escalada de inimigos, fim de run. Roster embutido no binário — **zero assets externos** |

## Documentação

| Doc | Conteúdo |
|---|---|
| [docs/GDD.md](docs/GDD.md) | Game Design Document — visão, loop, combate, equilíbrio em 3 camadas |
| [docs/ARQUITETURA.md](docs/ARQUITETURA.md) | Arquitetura técnica — crates, determinismo, logger, CI/CD, lições Bevy |
| [docs/PERSONAGENS.md](docs/PERSONAGENS.md) | **Os 16 personagens do jogo + os 12 do modo 1v1**: stats, mystery, padrão, chase, passiva — gerado a partir da engine |
| [docs/CARTAS.md](docs/CARTAS.md) | **Sistema de cartas**: draft de formação, recompensas, escalada, os 16 Contratos do 1v1, LCE e DWC |
| [docs/LOGS.md](docs/LOGS.md) | Logs de sessão completos (`~/TurnRush/logs`): início→fim, panics, sinais (Ctrl+C) |
| [docs/INSPIRACOES.md](docs/INSPIRACOES.md) | Referências: 9 Kings, Naruto Online (banco de dados em `data/naruto-online/`) |

## Balanceamento (filosofia)

1. **Draft simétrico** — os dois lados veem a mesma oferta (nenhuma assimetria existe).
2. **LCE** — quem está atrás no valor acumulado recebe a próxima carta promovida +1 tier.
3. **DWC v2** — Monte Carlo de todas as composições calibra dano/vida por composição
   **e por confronto** (mata hard counters > 0.85/< 0.15); gate de CI: desvio médio ≤ 0.15.
4. **Facções (Ember/Tide/Verdant/Storm)** são identidade — **sem contador duro**
   entre facções.

## Desenvolvimento

- `cargo test --workspace` — 44 testes (determinismo, simetria, chase, fórmulas, logger).
- `--check` (headless) e `--shot` (screenshots reais das 5 telas) rodam em CI e em
  ambiente sem GPU via **Xvfb + lavapipe** — nunca dependemos da máquina do usuário
  para validar visual.
- Logs de sessão em `~/TurnRush/logs` cobrem **do iniciar ao fechar**: limpo
  (`session_end`), panic (`crash-*.log` com backtrace) e sinais
  (`session_interrupted`, exit 128+sig).

## Roadmap

- ✅ M1 — motor determinístico + simulador + partida jogável local
- ✅ M3 (1ª versão) — cliente 2D jogável offline vs IA (arte final pendente)
- ⏳ M2 — servidor autoritativo (WebSocket) para o multiplayer estilo Naruto Online
- ⏳ 1v1 com draft de cartas na UI do jogo (hoje o 1v1 vive no simulador)
- ⏳ Arte, sprites, nomes finais e polimento (fase pós-mecânica)
- ⏳ Beta fechada Linux + ranqueado

## Licença

MIT — ver [LICENSE](LICENSE).
