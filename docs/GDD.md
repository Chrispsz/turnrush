# TURNRUSH — Game Design Document (v0.1)

> Roguelike acelerado 1v1 · Combate por turnos estilo Naruto Online ·
> Balanceamento absoluto por dados (DWC v2 + LCE)
> Plataforma inicial: Linux · Motor: Rust · CI: GitHub Actions desde o 1º commit

---

## 1. Visão

Um jogo de combate tático por turnos **extremamente balanceado**: a sorte não
decide, itens não existem, e a única vantagem permitida é a **estratégia e a
build** que o jogador monta. A estrutura é de **roguelike acelerado** (rodadas
curtas que escalam, draft de cartas entre rodadas — inspirado em 9 Kings) com
**combate estilo Naruto Online** (iniciativa, energia/chakra, cooldowns, combos).

Pilares:
1. **Balanceamento absoluto** — todas as composições testadas por simulação;
   nenhuma pode ter vantagem média sobre outra.
2. **A sorte é simétrica e compensada** — os dois jogadores veem as mesmas
   ofertas de cartas; quem está atrás é compensado em tempo real (LCE).
3. **Anti-complexidade** — sem itens, sem loot, sem níveis persistentes.
4. **Leve e rápido** — partidas de 5–12 min, binário pequeno, 2D.

## 2. Loop da partida (1v1)

```
Draft de cartas → Formação → Batalha por turnos → Escalação → (repete)
```

- **9 rodadas (I–IX)**; primeiro a **5 vitórias de rodada** vence a partida.
- A cada rodada, todos os stats escalam **+8% compostos, igualmente para os
  dois lados** (a pressão sobe, a simetria permanece — estilo 9 Kings).
- **Draft simétrico**: os dois jogadores recebem EXATAMENTE as mesmas 3 cartas
  (mesmo stream de RNG). Cada um escolhe 1 e aplica à sua composição.
- **LCE (Luck Compensation Engine)**: o valor nominal acumulado de cartas de
  cada jogador é rastreado; quem estiver atrás por ≥ 2 tiers recebe a próxima
  carta com tier promovido (+1). A sorte nunca se acumula em vantagem real.

## 3. Combate (estilo Naruto Online)

- Times de **3 personagens** com habilidades **fixas** (básico + habilidade +
  ultimate).
- **Alternância de ações** (A-B-A-B, estilo Legends of Runeterra): cada time
  joga suas unidades em ordem de SPD desc; quem começa é sorteado. A iniciativa
  vale no máximo a 1ª ação — nunca uma vantagem composta.
- **Energia (chakra)**: cap 10, regen 2/turno; custos de 2 a 7.
- **Cooldowns** por habilidade; o básico é grátis (cd 0) — sempre há opção.
- **Lançamento (launch)**: habilidades marcadas iniciam combo.
- **Perseguição (chase)**: aliados com habilidade de chase encadeiam o combo —
  **máx. 1 por rodada por time, a 60% do efeito**.
- **Status**: atordoamento (pula ação), veneno (dano/rodada, escala com a febre),
  escudo (absorve dano), buff/debuff de ATK.
- **Febre de batalha**: o dano cresce **+2% por rodada de combate** (igual para
  os dois lados) — toda batalha se resolve; tartarugar não é estratégia vencedora.
- **Juízo anti-turtle** (rodada 30): quem **causou mais dano** vence; empate cai
  para HP% (margem 2%).

## 4. Equilíbrio em 3 camadas (o coração do jogo)

### 4.1 Draft simétrico (por construção)
Mesma oferta para ambos → nenhuma assimetria de sorte existe.

### 4.2 LCE — Luck Compensation Engine (em tempo real)
Compensa desvios de valor de draft durante a partida.

### 4.3 DWC v2 — Dynamic Win-rate Calibration (por temporada)
Simulador Monte Carlo em Rust que joga **TODAS as composições possíveis**
(C(12,3) = 220 → 48.180 confrontos ordenados):

1. **Fase de composição**: ajusta modificadores globais (dano/vida em
   per-mille, ±22%/±15%) até o winrate médio de cada comp convergir a 50%.
2. **Fase de confronto**: corrige **hard counters categóricos** (pares com
   wr > 0.85 ou < 0.15) com penalidade ao lado dominante e reforço ao lado
   fraco — aplicados por par no início de cada luta.
3. **Verificação**: re-simula tudo em seeds novos e emite o relatório.

**Resultados atuais (v0.1, modo rápido 60 sims × 6 iters):**
- Desvio médio máximo de 50%: **0.119** (gate ≤ 0.15 — PASSOU)
- Pior confronto isolado: 1.000 → **diagnóstico automático** identifica o par
  (ex.: sustain duplo vs triple squishy) para redesign de roster no v0.2.

**Decisão de matemática**: tudo em **inteiros (per-mille)** e RNG xoshiro256**
semeado — mesma seed = mesma partida em qualquer plataforma (replay/anti-cheat
prontos para o netcode).

## 5. Anti-complexidade

- Sem itens, sem equipamentos, sem loot, sem XP/nível persistente.
- Progressão = **domínio** (maestria de composições) e cosméticos.
- Expansão por **novos personagens e cartas**, sempre seguida de recalibração
  automática do DWC no CI.

## 6. Arte e estilo

- **2D** (decisão dos áudios); estilo final a definir (pixel art é o favorito
  para leveza e custo). Referência de composição: arena dividida, cartas
  flutuando entre os times.
- Cliente 2D implementado com **Bevy 0.20** (ECS, UI nativa, fonte embutida,
  zero assets externos).

## 7. Multiplayer (roadmap técnico)

- **M1 (✅)**: motor determinístico + simulador + partida jogável local.
- **M2**: servidor autoritativo (tokio, WebSocket) — o cliente envia
  intenções (escolha de carta/ação), o servidor simula e difunde o estado.
  Determinismo = replay + validação anti-cheat.
- **M3 (✅ 1ª versão)**: cliente gráfico 2D (Bevy) consumindo o mesmo
  `turnrush-core` — modo offline vs IA jogável; netcode vem depois.
- **M4**: beta fechada Linux, ranqueado por Elo.

## 8. Métricas de sucesso

- Winrate médio de TODAS as comps dentro de 46–54% (v0.2: 47–53%).
- Zero hard counters > 90% sem correção ativa (v0.2).
- Partida completa < 12 min; input-to-action < 100 ms no servidor.

## 9. Roster v0.1 (12 personagens)

| # | Nome | Papel | HP | ATK | DEF | SPD | Assinatura |
|---|------|-------|----|----|-----|-----|------------|
| 0 | Golem | Tanque | 150 | 20 | 32 | 8 | Muralha de Pedra · Terremoto (launch AoE) |
| 1 | Bruto | Berserker | 115 | 24 | 26 | 9 | Golpe Esmagador (launch 150%) · Fúria |
| 2 | Sombra | Assassino | 85 | 26 | 14 | 15 | Lâmina Traspassa (chase) · Execução 190% |
| 3 | Arqueira | Atiradora | 92 | 26 | 20 | 12 | Tiro Certeiro · Chuva de Flechas (launch AoE) |
| 4 | Piromante | Mago | 88 | 28 | 15 | 10 | Bola de Fogo (veneno) · Deflagrar AoE |
| 5 | Oráculo | Curandeiro | 95 | 19 | 26 | 10 | Bênção · Círculo Sagrado (cura+escudo AoE) |
| 6 | Comandante | Suporte | 105 | 22 | 26 | 10 | Ordem de Ataque (+35% time) · Contra-Ataque |
| 7 | Feiticeira | Maldição | 90 | 23 | 20 | 10 | Maldição (debuff + veneno) · Vórtice |
| 8 | Duelista | Duelista | 90 | 25 | 20 | 13 | Estocada Dupla (chase) · Postura Precisa |
| 9 | Monge | Lutador | 100 | 24 | 20 | 11 | Palma Sônica (stun, chase) · Fluxo Interior |
| 10 | Necromante | Evocador | 90 | 26 | 20 | 10 | Colheita (roubo de vida) · Enxame de Corvos |
| 11 | Guardiã | Paladino | 125 | 23 | 30 | 10 | Escudo Divino · Julgamento (launch) |

## 10. Pool de cartas do draft (16 contratos)

Tiers 1–4: refinamentos de unidade (ATK/HP/DEF/SPD), bônus de time
(Bandeira de Guerra, Muralha Viva) e épicas percentuais (Ressonância Ofensiva
+8% dano, Ressonância Vital +8% vida, Coroa do Estrategista +4%/+4%,
Passo do Vento +7 SPD). O valor nominal alimenta o LCE.


---

## Atualização v0.3 — Formação 4v4 e cadeias de combo

### Combate 4v4 (audio 4 do usuário)

4 personagens por lado em formação 2×2, ordem de movimento (1º–4º) definida
pelo time, ações intercaladas por passo (iniciativa alternada por rodada,
moeda na 1ª). HP individual por personagem (áudio 3). Sem RPG/itens.

### Cadeias de perseguição (o coração do Naruto Online)

Estados: **arremesso (launch)**, **empurrão (knockback)** e **derrubada
(knockdown)**. Cada ataque pode aplicar um estado; aliados com perseguição de
gatilho correspondente perseguem automaticamente (dano 20–35% do ATK), no
máximo 1 perseguição por unidade por rodada. Exemplo real do log:

```
[mystery] Ígnia Brasa-Viva usa «Detonação Solar» (chakra -20)
  Nereo sofre 95 e recebe lançamento
[chase] Ígnia persegue lançamento de Nereo → 23 de dano
  Nereo recebe derrubada
[chase] Amara persegue derrubada de Nereo → 14 de dano
```

### Roster v2 (16 personagens, IP original)

Derivado do estudo do banco de dados do Naruto Online (255 ninjas, 5 slots,
vocabulário de estados normalizado — ver `data/naruto-online/MECANICAS.md`).
4 facções × 4 papéis (striker/guardian/support/controller):

| Facção | Arquétipo | Personagens |
|---|---|---|
| ember | agressão/DoT | Ígnia, Orson, Amara, Nimbo |
| tide | controle/escudos | Nereo, Dalva, Mel, Ligeia |
| verdant | desgaste/veneno | Tina, Aro, Yara, Ívo |
| storm | velocidade/ação | Reia, Aki, Alísio, Volta |

Orçamento de balanceamento: HP 850–1150, padrão 60–80%, mystery 80–130%
(chakra 15–40, CD 2–4), perseguição 20–35%; todas as facções com ≥2 cadeias
arremesso→perseguição e ≥1 derrubada→perseguição.

### Cartas espelhadas (áudio 3)

O draft continua simétrico: os dois jogadores veem as mesmas cartas; a LCE
compensa sorte em tempo real; o DWC calibra por composição E por confronto.

---

## Atualização v0.5.0 — o jogo jogável offline (estado atual)

### Loop do jogo (cliente Bevy — o que existe HOJE)

```
Menu ──→ Como Jogar (regras)  ─┐
      ──→ Elenco (16 cartas)  ─┤
      ──→ Draft: 8 cartas na mesa, escolha 4
              ↓
          Batalha 4v4 (playback da engine, Acelerar 1×/6×)
              ↓ vitória
          Recompensa: +10% dano | +10% vida | +5%/+5%
              ↓ (inimigo seguinte: +11% dano / +7% vida POR vitória)
          … 3 vitórias = run completa; derrota encerra (roguelike)
```

- **Modo offline vs IA**: o adversário é montado entre os personagens não
  escolhidos e jogado pela heurística greedy do engine (a mesma do simulador
  Monte Carlo — o que você enfrenta é o jogo "de verdade").
- **Menu genérico mas funcional**: Como Jogar e Elenco documentam as regras
  a partir da própria engine; nada de manual desatualizado.
- **Esboço consciente**: cores de facção, cartas e log de combate já dão a
  leitura tática; sprites/animações/nomes finais são a próxima fase.
- **Verificação de visual sem interação**: `--shot` navega e captura as 5
  telas (menu/como-jogar/elenco/draft/batalha) — roda no CI com Xvfb+lavapipe.

### Métricas v0.5.0

- 44 testes no workspace (determinismo, espelho ~50%, chase, logger, contrato
  JSON, parser de eventos) · clippy zero warnings.
- Gate 1v1 (desvio ≤ 0.15) + gate 4v4 no CI a cada push; cron noturno roda o
  Monte Carlo completo.
- Release: AppImage com o JOGO como executável principal + sim musl embutido.

### Equilíbrio v0.5 — variância tática (correção dos contadores categóricos)

A verificação v0.5 expôs e corrigiu uma falha real: sem variância tática, as
batalhas 4v4 eram categóricas (winrates 0.000/1.000). Duas correções na
engine — **ordem de movimento amostrada por batalha** (escolha tática do
design) e **jitter de timing de mystery (1/4)** — deram gradiente contínuo
aos winrates; o DWC 4v4 ganhou alcance (±550‰/±400‰ por comp; ±520‰/±320‰
por confronto). Gate completo (150 sims): desvio médio **0.077 ≤ 0.15 — PASSOU**. Pares ainda
categóricos por kit ficam listados como alvo de redesign de roster (v0.6).
