# Cartas & Contratos — TurnRush

> "Carta" tem hoje duas famílias **implementadas na engine**: (1) os
> **personagens-cartas** do draft de formação (o jogo vira cada personagem do
> elenco em uma carta clicável) e (2) os **Contratos** — pool de
> aprimoramentos do modo 1v1 com compensação de sorte (LCE). A estrutura é
> definitiva; valores e o escopo do pool são ponto de expansão (ver §6).

---

## 1. No jogo (Formação 4v4) — draft de personagens

Fluxo (`crates/turnrush-game/src/app.rs`):

1. **Oferta**: 8 dos 16 personagens são sorteados (RNG semeado da run) e
   apresentados como cartas clicáveis — **a oferta é única**: os dois lados
   veem o mesmo conjunto (simetria por construção).
2. **Escolha**: o jogador clica em 4 cartas (clique de novo desmarca) e
   confirma em **LUTAR**.
3. **Inimigo (IA offline)**: montado entre os personagens restantes do elenco
   — jamais duplica nomes com o time do jogador (garantia do playback).
4. **Escalação**: cada time joga na ordem dos slots 1º–4º (frente-esq,
   frente-dir, trás-esq, trás-dir), intercalado A-B-A-B por passo; a IA
   adversária sorteia a própria ordem por batalha.

### Recompensas pós-vitória (modificadores de formação)

Após cada vitória o jogador escolhe **um** aprimoramento persistente da run
(`FormationMods`, em per-mille):

| Recompensa | Efeito | Código |
|---|---|---|
| **Dano +10%** | dano do time ×1.10 | `dmg_permille += 100` |
| **Vida +10%** | vida (e curas/escudos) ×1.10 | `hp_permille += 100` |
| **Equilíbrio +5%/+5%** | ambos ×1.05 | `+= 50 / += 50` |

Os multiplicadores aplicam-se a **dano, cura e escudo** (na engine, curas e
escudos escalam pelo modificador de vida do time — suporte nunca fica
obsoleto).

### Escalada do inimigo (anti-facilitação)

O adversário da próxima batalha fica mais forte **por vitória sua**:

```
inimigo.dmg = 1000‰ + 110‰ × vitórias   (+11% dano por vitória)
inimigo.hp  = 1000‰ +  70‰ × vitórias   (+7% vida por vitória)
```

Vencer 3 batalhas (run completa) significa derrotar times com até
+22% dano / +14% vida — a pressão sobe, o jogador escala junto com as
recompensas. Empates/derrota encerram a run (roguelike).

> **Este sistema de recompensas é a versão v1** — 3 opções fixas, projeto
> deliberadamente mínimo. A expansão planejada está em §6.

---

## 2. Contratos — pool do modo 1v1 (16 cartas)

Fonte: `CARD_POOL` em `crates/turnrush-core/src/cards.rs`. Tiers 1–4; o
**valor nominal** alimenta o LCE (§4). Todos os efeitos são **aditivos aos
stats** e aplicados na montagem do time (`build_team`).

### Tier 1 — refinamentos (valor 10)

| ID | Nome | Efeito |
|---|---|---|
| `atk1` | **Afiar** | +6 ATK para uma unidade |
| `hp1` | **Vigor** | +22 HP para uma unidade |
| `def1` | **Couraçar** | +6 DEF para uma unidade |
| `spd1` | **Agilizar** | +2 SPD para uma unidade |

### Tier 2 — vantagens (valor 20)

| ID | Nome | Efeito |
|---|---|---|
| `atk2` | **Açoite** | +13 ATK para uma unidade |
| `hp2` | **Fortalecer** | +50 HP para uma unidade |
| `def2` | **Reforçar** | +13 DEF para uma unidade |
| `spd2` | **Sombra Ligeira** | +4 SPD para uma unidade |

### Tier 3 — poderes (valor 30)

| ID | Nome | Efeito |
|---|---|---|
| `atk3` | **Titã** | +22 ATK para uma unidade |
| `hp3` | **Colosso** | +95 HP para uma unidade |
| `all1` | **Bandeira de Guerra** | +4 ATK, +2 DEF e +30 HP para o time |
| `wall1` | **Muralha Viva** | +8 DEF para o time |

### Tier 4 — épicas (valor 40)

| ID | Nome | Efeito |
|---|---|---|
| `dmg8` | **Ressonância Ofensiva** | +8% de dano para o time |
| `hp8` | **Ressonância Vital** | +8% de vida para o time |
| `hybrid` | **Coroa do Estrategista** | +4% de dano e +4% de vida para o time |
| `spd3` | **Passo do Vento** | +7 SPD para uma unidade |

**Oferta**: por rodada da partida (9 no total), `cards::offer` sorteia **3
contratos distintos** — a MESMA oferta para os dois jogadores (mesmo stream de
RNG). Cada um escolhe 1; cartas de unidade vão para a unidade mais compatível
(ATK/SPD → maior atacante; HP/DEF → maior tanque).

---

## 3. Draft simétrico — a regra que governa tudo

Toda oferta de cartas no TurnRush é **simétrica**: os dois lados veem
exatamente as mesmas opções, sorteadas do mesmo stream de RNG. Sorte
assimétrica **não existe por construção** — o que existe é diferença de
escolha, e é isso que as camadas seguintes compensam e calibram.

## 4. LCE — Luck Compensation Engine (compensação em tempo real)

Como a oferta é idêntica, escolhas podem desviar valor. O LCE rastreia o
**valor nominal acumulado** de cada jogador:

- Se estou **atrás por ≥ 20 pontos** (2 tiers), minha próxima carta é
  **promovida +1 tier** (`effective_card` → `promote`).
- Promoção é silenciosa e só acontece para quem está atrás — nunca pune quem
  está na frente.
- O valor acumulado usa a carta **efetiva** (pós-promoção), então a dívida se
  liquida sozinha.

## 5. DWC — a calibração por trás das cartas

O **DWC (Dynamic Win-rate Calibration)** é o que permite dar cartas fortes sem
quebrar o jogo: o simulador Monte Carlo joga as composições e calibra
modificadores de dano/vida **por composição** (fase 1) e **por confronto**
(fase 2 — corrige pares com winrate > 0.85 ou < 0.15), com passada de
verificação em seeds novos. Gate de CI: desvio médio ≤ 0.15 (medido: 0.119 no
1v1; 0.077 no 4v4). Relatório JSON em cada execução (`--out`).

---

## 6. Expansão futura (placeholders explícitos)

O que está listado aqui **ainda não existe na engine** — está marcado como
expansão para não confundir leitor nem simulador:

- **Mais cartas de recompensa no 4v4** — a estrutura `FormationMods` aceita
  qualquer par dano/vida; novas famílias (ex.: efeitos por classe, custo de
  chakra inicial) exigem campos novos no contrato + recalibração do DWC.
- **Boons de cadeia** — aprimoramentos que interagem com as cadeias de combo
  (ex.: "a primeira perseguição do time causa +X%", "chase aplica estado
  extra"). Dependem de extensão do contrato v2.
- **Sinergias de clã** — bônus por 2+ cartas do mesmo clã na formação
  (identidade sem contador duro entre clãs).
- **Raridades visuais** (comum/raro/épico) — hoje o tier já define o valor
  nominal do LCE; visual viria depois.
- **Draft de Contratos na UI do 4v4** — hoje o 4v4 usa as 3 recompensas fixas
  e o 1v1 vive no simulador; unificar os dois sistemas de draft é passo
  natural da v0.6+.


---

## v0.7 — cartas de run (roguelike)

Entre batalhas vencidas: escolha **1 de 3** (1 reroll por batalha). Teto de
poder acumulado da run: **+30%** (300‰). Cartas de Unidade exigem um alvo
vivo na compra. Peso por raridade: comum 600 / incomum 300 / rara 100;
preferência de clã multiplica o peso por 1,1.

| Carta | Classe | Raridade | Poder | Efeito |
|---|---|---|---|---|
| Chama Interna | Equipe | comum | 80 | +8% ATK do time |
| Casca de Ferro | Equipe | comum | 100 | +10% HP do time |
| Fluxo Estável | Equipe | incomum | 60 | +2 ⚡/rodada por unidade |
| Eco de Combate | Equipe | rara | 100 | chase +10% por elo da cadeia (máx +30%) |
| Fúria Interior | Unidade | comum | 70 | +15% ATK, -5% HP (1 unidade viva) |
| Manto Denso | Unidade | comum | 60 | +12% HP, -3% ATK (1 unidade viva) |
| Gatilho Rápido | Unidade | incomum | 50 | Mystery destrava 1 rodada antes |
| Veia de Aço | Unidade | rara | 110 | +10% ATK e +10% HP (1 unidade viva) |
| Ambidestria | Tática | rara | 90 | 2 chases por unidade por rodada |
| Último Suspiro | Tática | rara | 100 | 1ª morte do time revive com 20% do HP |
| Sangue por Poder | Tática | incomum | 60 | -10% HP, +15% ATK do time |
| Respiração Ritmada | Equipe | comum | 30 | +1 ⚡/rodada por unidade |

**Gate de sorte** (Monte Carlo, 200 runs): winrate da estratégia sortuda vs
aleatória difere ≤ 10 p.p. (medido: ~1 p.p.).
