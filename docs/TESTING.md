# Suíte Quebra-Cabeça — testes em cadeia do TurnRush

> Cada peça valida uma camada e **encaixa na anterior**: se a Peça 3
> (economia) passa, é porque a Peça 2 (determinismo) passou — e assim por
> diante. O executor mestre é `scripts/puzzle-gate.sh`.

## O quebra-cabeça (10 peças + 2 gabaritos)

| Peça | Arquivo | O que fecha |
|---|---|---|
| 0 | `cargo clippy -D warnings` | qualidade do código (zero-warnings é política) |
| 1 | `tests/p01_data_foundation.rs` | roster v2: 16 personagens, bandas de orçamento, formas de alvo por papel, interrupt só em controladores, cartas com efeito real, **`unmodeled` vazio** |
| 2 | `tests/p02_determinism.rs` | mesma seed = mesma partida (eventos idênticos), observação é pura (`preview` não muta), formação 4v4 e partida 1v1 determinísticas |
| 3 | `tests/p03_fluxo_economy.rs` | fluxo: +10/rodada, +15 no padrão, custos 30/60/35, CDs, teto 100, portão de unlock, erros **não** consomem a vez, teto da passiva de chakra |
| 4 | `tests/p04_targeting.rs` | mira: padrão espelhado 1↔1 (com provocação e fallback), `enemy_single` acessa a traseira, faixas front/all/random_2, cura no mais ferido, `self`, `BadTarget`, **prévia promete = execução entrega** |
| 5 | `tests/p05_passives.rs` | cada família de passiva com assert COMPORTAMENTAL: reflexo (número exato), redução frontal condicional, curas EoR (focada/área), queimadura de retorno, veneno nos ataques, bônus vs DoT, buff de dano, dissipação, regeneração, escudo crescente, aceleração (ordem) |
| 6 | `tests/p06_kit_composition.rs` | kits: orçamento por papel, anti-faz-tudo (âncora/âmbito de cada classe), cadeias de combo fecham nos 4 clãs, gradiente clã↔estilo, controladores distintos |
| 7 | `tests/p07_battle_chain.rs` | corrente do jogador: janela de chase (confirmar/recusar/BadTarget), táticas a cada 2 rodadas (erros e eventos), convergência < 100k passos, espelho sem favoritismo, `BattleOver` limpo |
| 8 | `tests/p08_run_lifecycle.rs` | run: compra com alvo, teto de poder, reroll abastecido por batalha, **carta → SideEdge → batalha muda de fato** (Fluxo Estável, Gatilho Rápido, Último Suspiro, Ambidestria, Eco de Combate), draft 1v1 com LCE |
| 9 | `tests/p09_elements.rs` | **elementos (v0.9)**: ciclo fechado (cada elemento vence exatamente 1 e perde para exatamente 1, diagonal neutra), contrato `element` no JSON (explícito/derivado/erro claro), vantagem ×1200‰/×800‰ exata NOS DOIS motores, **Ressonância** (mesmo elemento: +15% e +5⚡, prévia == execução), **Cadeia** (progressão do ciclo: +10%, wrapthrough de rodada), passiva de sintonia soma SÓ na Cadeia, cura não estende nem quebra a cadeia, determinismo/simetria com elementos, snapshot expõe elemento |
| 10 | `tests/p10_client_e2e.rs` (game) | binário `--check`: roster embutido → motor → parser → controlador → convergência, reprodutível entre execuções |
| 11 | `turnrush-sim --quick --gate` (+ formação) | gabarito de balanceamento: desvio de winrate ≤ 0.15 |

## Regras da casa

- Os testes de integração usam **só a API pública** do `turnrush-core`
  (`legal_actions` + `preview_action` + `execute_action` + snapshots) —
  nada de espiar estado interno. Se um teste precisou de API nova, é sinal
  de que a UI também vai precisar.
- Rosters sintéticos são montados por `tests/common/mod.rs` (`CharSpec` →
  JSON do contrato v2), um mini-roster por família testada — mesmo idioma
  do roster real.
- Testes são **determinísticos** (seeds fixas ou busca determinística de
  seed, ex.: `seed_com_starter`) — nada de sorte em runtime.
- Números exatos quando o contrato é exato (reflexo 15% = dano×150/1000);
  relacional quando a fórmula interna pode evoluir (bônus vs DoT: `>`).

## Como rodar

```bash
scripts/puzzle-gate.sh                 # cadeia completa (~minutos)
PUZZLE_FAST=1 scripts/puzzle-gate.sh   # pula o gate Monte Carlo de formação
cargo test -p turnrush-core            # só as peças 1–8 + testes inline
cargo test -p turnrush-game            # peça 10 (compila o binário do jogo)
```

## Bugs que esta suíte já pegou (motivo de existir)

1. **Interrupt morto da rodada 2 em diante** — `open_round` não resetava o
   array `acted[]` do lado; `interrupt_targets` ficava vazio para sempre e
   o Interrupt (35⚡/CD3) só funcionava na rodada 1, quando não dá para
   pagar. Corrigido em `interactive.rs` (reset de `sides[si].acted`).
2. **Contrato vs. doc**: banda da Mystery era documentada como 80–130
   (v0.7); o v0.8 elevou alvo único para 200% — a suíte fixa o contrato
   REAL por forma de alvo.
3. **Prévia × execução**: prévia de `enemy_single` com slot inválido cai
   no auto silenciosamente enquanto a execução recusa com `BadTarget` —
   documentado no teste (a engine nunca aceita lixo; a prévia mostra o
   auto-alvo que de fato aconteceria).
