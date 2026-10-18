# TURNRUSH — Game Design Document

> Tactical turn-based roguelike · cadeias de combo (chase) · draft simétrico ·
> balanceamento Monte Carlo (DWC/LCE)
> Plataforma inicial: Linux · Motor: Rust · CI: GitHub Actions desde o 1º commit

---

## 1. Visão

TurnRush é um jogo de combate tático por turnos **extremamente balanceado**:
a sorte não decide, itens não existem, e a única vantagem permitida é a
**estratégia e a composição** que o jogador monta. A estrutura é a de um
**roguelike de runs curtas** — draft, batalha, recompensa, escalada — com
combate de formação em que **cadeias de combo** entre aliados são o coração
tático.

Pilares:

1. **Balanceamento absoluto** — todas as composições testadas por simulação;
   nenhuma pode ter vantagem média sobre outra.
2. **A sorte é simétrica e compensada** — os dois lados veem as mesmas
   ofertas; quem está atrás é compensado em tempo real (LCE).
3. **Anti-complexidade** — sem itens, sem loot, sem níveis persistentes.
   Progressão = domínio.
4. **Leve e rápido** — runs de minutos, engine de microssegundos por batalha,
   binário pequeno, 2D.

## 2. Loop da run (modo offline vs IA — o jogo atual)

```
Menu → Draft: 8 cartas na mesa, escolha 4
     → Batalha 4v4 MANUAL (você comanda cada ação)
     → vitória → Recompensa: +10% dano | +10% vida | +5%/+5%
     → próximo inimigo escala → … 3 vitórias = run completa
```

- **Batalha manual (v0.6)**: a engine executa a batalha **por passos** e
  **pausa na vez de cada unidade sua**. O painel inferior mostra a unidade
  ativa (chakra atual) e oferece:
  - **Mystery «nome» · -custo** — conjura a habilidade assinatura (o alvo
    segue a especialização da carta: frente/todos/aliado mais ferido/2
    aleatórios);
  - **Padrão «nome» → alvo…** — ataque grátis com **alvo clicável** entre
    os inimigos vivos (ou Auto-alvo: primeiro vivo);
  - **Auto ▶** — entrega as próximas decisões à IA (assista a run);
  Cadeias de perseguição continuam automáticas (1× por unidade por rodada).
  As decisões do jogador não sofrem o jitter de timing da IA.

- **Draft simétrico de formação**: 8 dos 16 personagens são sorteados como
  cartas — a oferta é única (o inimigo IA monta o time entre os 8 restantes,
  sem duplicar nomes). O jogador escolhe **4**.
- **Batalha 4v4**: todas as regras da §3 (agora comandadas pelo jogador —
  botão Auto entrega o controle à IA quando quiser assistir).
- **Recompensa pós-vitória**: um aprimoramento **persistente da run**
  (modificadores per-mille da formação): **Dano +10%**, **Vida +10%** ou
  **Equilíbrio +5%/+5%**. Curas e escudos escalam junto com o modificador de
  vida — suporte nunca fica obsoleto.
- **Escalada do inimigo**: cada vitória do jogador fortalece o adversário
  seguinte — **+11% de dano e +7% de vida por vitória**. Completar a run
  (3 vitórias) significa derrotar times até **+22% dano / +14% vida**.
- **Roguelike**: derrota (ou empate) encerra a run.

## 3. Combate 4v4 — regras fiéis à engine (`formation.rs`)

- **Formação 2×2** por lado (frente/trás × esquerda/direita), HP individual
  por personagem.
- **Ordem de movimento 1º–4º** por lado (escolha tática de design): quem age
  antes decide foco de fogo e timing de mystery.
- **Iniciativa justa**: as ações são **intercaladas A-B-A-B por passo** — no
  passo 1 agem os primeiros de cada lado, no passo 2 os segundos, etc. Uma
  moeda decide quem abre a 1ª rodada e a abertura **alterna a cada rodada**:
  a iniciativa vale no máximo a 1ª ação, nunca uma vantagem composta.
- **Chakra (por unidade)**: começa em **30**, regenera **+14 por rodada**,
  teto **100**. Mysteries custam **15–40** e têm **cooldown de 2–4 rodadas**
  (decrementa no início da rodada). O ataque padrão é **grátis** — sempre há
  opção.
- **Ritmo de dano (×2.5)**: todo dano direto do 4v4 é multiplicado por
  `DAMAGE_PACE = 2500‰` — simétrico para os dois lados (o DWC recalibra por
  composição, winrates preservados). Existe para que as partidas terminem
  por **derrota real** dentro do teto de rodadas, não por guerra de atrito
  (medido: 22.4 rodadas médias → 7.9 com 94% de K.O.).
- **Variância tática da engine**: 1/4 de chance (semeada, simétrica) de
  segurar a mystery por um turno — modela imperfeição de timing e dá
  gradiente contínuo aos winrates do Monte Carlo.
- **Fórmula de dano** (inteira, sem float): `ATK × dmg% × 100/(100+DEF)`,
  com reduções percentuais aplicadas depois (passivas de redução, auras).
- **Escudos**: absorvem dano antes do HP; mysteries com **perfuração**
  ignoram parte do escudo (ex.: 50%) ou da defesa (ex.: 30%).
- **Reflexo/contra-ataque** acontece em resposta a ataques padrão.
- **Fim de batalha**: lado sem unidades vivas perde; após **12 rodadas**,
  vence o maior **% de vida total** — empate é empate (Draw).

### 3.1 Cadeias de combo (chase) — o coração tático

Três estados de controle alimentam as cadeias:

| Estado | Rótulo em jogo | O que faz |
|---|---|---|
| **Launch** | Lançamento (arremesso) | Alvo arremessado — abre cadeias |
| **Knockback** | Empurrão | Alvo empurrado |
| **Knockdown** | Derrubada | Alvo derrubado — a fila de ação dele é adiada (efeito narrativo; modelado como estado) |

Regras exatas da engine:

1. Mysteries e ataques padrão podem **aplicar um estado** aos alvos sobreviventes.
2. Estado fresco **abre perseguição**: o primeiro aliado vivo cujo chase tem
   o **gatilho correspondente** executa o golpe de perseguição (20–35% do ATK).
3. **Máx. 1 perseguição por unidade por rodada** (`max_per_round`).
4. A cadeia continua enquanto estados frescos encontrarem perseguidores —
   limite de **12 elos** por cadeia (prova contra loops).
5. Passivas e auras reagem a estados: bônus de dano contra alvos com estado,
   auras de equipe ("Inimigos Derrubados recebem +15% de dano"), debuffs.

Exemplo real do log de batalha (nomes oficiais atuais):

```
[mystery] Ignia Emberbright usa «Detonação Solar» (chakra -20)
  Nereo Saltblade sofre 95 e recebe Lançamento
[chase] Chute Vulcânico persegue Lançamento de Nereo Saltblade → 23 de dano
  Nereo Saltblade recebe Derrubada
[chase] Golpe Fervedor persegue Derrubada de Nereo Saltblade → 14 de dano
```

### 3.2 Dano contínuo (DoT) e efeitos de rodada

- **Queimadura/Veneno**: 10% do ATK do aplicador **por rodada, durante 2
  rodadas**, sem stack (reaplicar não renova enquanto durar). O tick ocorre
  no início da rodada e pode derrubar a unidade.
- **Início de rodada**: chakra regenera (+passivas de chakra), cooldowns
  decrementam, buffs expiram, DoT tica.
- **Fim de rodada**: curas de passiva ("cura 5% do HP do aliado com menos HP").

### 3.3 Passivas (modelagem honesta)

Passivas são interpretadas da texto do contrato por palavras-chave: bônus de
dano simples e condicionais (contra Queimadura/Veneno), redução de dano
recebido, reflexo/contra-ataque, cura de fim de rodada, chakra de início de
rodada, aplicação de DoT por ataque. **O que não é modelado fica visível** —
a engine lista efeitos não interpretados em `Formation4v4::unmodeled()` nos
logs; a simulação nunca assume um efeito que não existe.

## 4. Clãs e classes (identidade do elenco)

4 clãs × 4 classes = 16 personagens. Nomes oficiais — tabela completa e
racional em [NAMING.md](NAMING.md); kits completos em [PERSONAGENS.md](PERSONAGENS.md).

| Clã | Identidade mecânica | Personagens |
|---|---|---|
| **PYREBORN** | fogo e agressão — dano concentrado, Queimadura, pressão de cadeia | Ignia Emberbright, Orson Moltenwall, Amara Sweetflame, Nimbo Smokeveil |
| **DUSKMERE** | maré e controle — escudos, reflexo, cura, negação de turnos | Nereo Saltblade, Dalva Reefwarden, Mel Tidemother, Ligeia Coldmist |
| **THORNVALE** | natureza e veneno — desgaste, escudo que cresce, regeneração | Tina Twinthorn, Aro Barkshell, Yara Lifesap, Ivo Finewire |
| **SKYREND** | tempestade e velocidade — iniciativa, AoE, contra-ataque | Reia Thunderstep, Aki Countercut, Alísio Lightgale, Volta Cyclone |

Classes (papéis):

| Classe | Papel no time | Assinatura típica |
|---|---|---|
| **Reaver** (striker) | dano concentrado | Mystery 115–130% com Lançamento; chase 30–35% |
| **Bastion** (guardian) | defesa e provocação | HP 1100–1150, DEF 56–62; escudo/Derrubada no padrão |
| **Mender** (support) | cura e economia | Cura 90–100% do ATK; +chakra ou regeneração |
| **Tactician** (controller) | controle e timing | Derrubada em área/Lançamento AoE; auras de punição |

Orçamento de balanceamento do contrato v2: HP 850–1150 · padrão 60–80% ·
mystery 80–130% (chakra 15–40, CD 2–4) · chase 20–35% · `max_per_round` 1.
O gradiente entre clãs é intencional (SKYREND troca HP por velocidade;
DUSKMERE/THORNVALE trocam ataque por tank/utilidade) — o DWC calibra as
diferenças residuais. **Clã não é contador duro de outro clã.**

## 5. Modo 1v1 legado (simulador — `run.rs`/`combat.rs`)

O modo 1v1 vive hoje no simulador (e nos testes de equilíbrio); a UI do jogo
usará a mesma engine no futuro:

- **Partida de 9 rodadas (I–IX)**; primeiro a **5 vitórias de rodada** vence.
- Times de **3 personagens**; **escalada +8% por rodada** (HP+ATK, composta,
  igual para os dois lados — a pressão sobe, a simetria permanece).
- **Febre de batalha**: o dano cresce **+2% por rodada de combate** (igual
  para os dois lados) — toda batalha se resolve; tartarugar não é estratégia
  vencedora. Após 30 rodadas, juízo anti-turtle: vence quem **causou mais
  dano**; empate cai para HP%.
- **Energia (chakra)**: teto 10, regeneração 2/turno.
- **Perseguição (chase)**: máx. 1 por rodada **por time**, a **60%** do
  efeito.
- **Draft de Contratos por rodada** (§6) com LCE.

## 6. Cartas

"Carta" tem duas famílias implementadas: os **personagens-cartas** do draft
de formação (§2) e os **Contratos** — pool de 16 aprimoramentos (tiers 1–4)
do 1v1, com oferta simétrica e LCE. Detalhes, números e planos de expansão em
[CARTAS.md](CARTAS.md).

## 7. Equilíbrio em 3 camadas (o coração do jogo)

### 7.1 Draft simétrico (por construção)
Mesma oferta para ambos os lados → nenhuma assimetria de sorte existe.

### 7.2 LCE — Luck Compensation Engine (em tempo real)
O valor nominal acumulado de cartas de cada jogador é rastreado; quem está
atrás por ≥ 2 tiers recebe a próxima carta **promovida +1 tier**. A sorte
nunca se acumula em vantagem real.

### 7.3 DWC — Dynamic Win-rate Calibration (por temporada)
Simulador Monte Carlo em Rust que joga as composições e calibra
modificadores de dano/vida em per-mille:

1. **Fase de composição**: ajusta mods globais por composição até o winrate
   médio convergir a 50%.
2. **Fase de confronto**: corrige **hard counters categóricos** (wr > 0.85 ou
   < 0.15) com ajuste por par, espelhos intocados.
3. **Verificação**: re-simula em seeds novos e emite o relatório JSON.

No 4v4 a amostragem é documentada: composições puras de clã + confrontos
entre clãs + composições mistas (seed fixa). Gate de CI: **desvio médio ≤
0.15** — **medido: 0.077** no gate completo (150 sims; espelho ~0.50). Pares
ainda categóricos por kit são listados automaticamente como **alvo de
redesign de roster** — o diagnóstico aponta os pares exatos a cada execução.

**Decisão de matemática**: tudo em **inteiros (per-mille)** e RNG xoshiro256**
semeado — mesma seed = mesma partida em qualquer plataforma (replay/anti-cheat
prontos para o netcode).

## 8. Anti-complexidade

- Sem itens, sem equipamentos, sem loot, sem XP/nível persistente.
- Progressão = **domínio** (maestria de composições) e cosméticos.
- Expansão por **novos personagens e cartas**, sempre seguida de recalibração
  automática do DWC no CI.

## 9. Arte e estilo

- **2D**; estilo final a definir (pixel art é o favorito para leveza e custo).
  Composição da arena: times em formação, cartas flutuando entre eles.
- Cliente 2D implementado com **Bevy 0.20** (ECS, UI nativa, fonte embutida,
  zero assets externos). Menu/telas são **esboços conscientes** — a fase atual
  é mecânica; cores de clã, cartas e log de combate já dão a leitura tática.
- Verificação visual sem interação: `--shot` navega e captura as 5 telas —
  roda no CI com Xvfb + lavapipe.

## 10. Roadmap técnico

- **M1 (✅)**: motor determinístico + simulador + partida jogável local.
- **M3 (✅ 1ª versão)**: cliente gráfico 2D (Bevy) consumindo o mesmo
  `turnrush-core` — modo offline vs IA jogável.
- **M2**: servidor autoritativo (tokio, WebSocket) — o cliente envia
  intenções (escolha de carta/ação), o servidor simula e difunde o estado.
  Determinismo = replay + validação anti-cheat.
- **M4**: beta fechada Linux, ranqueado por Elo.

## 11. Métricas de sucesso

- Winrate médio de TODAS as composições dentro de 46–54% (gate ≤ 0.15 de
  desvio — medido 0.077 no 4v4; 0.119 no 1v1).
- Zero hard counters > 90% sem correção ativa (fase 2 do DWC corrige;
  restantes listados como alvo de redesign).
- Run completa em minutos; batalha em microssegundos na engine.
