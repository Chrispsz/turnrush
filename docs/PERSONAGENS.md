# Personagens — TurnRush

> **Fonte da verdade**: `data/roster/roster-v2.json` (contrato v2, carregado
> por `turnrush-core::data::Roster` e embutido no binário do jogo via
> `include_str!`). **Nada aqui existe fora da engine** — se está documentado,
> está implementado e testado. Nomes de exibição de clãs/classes/personagens
> são oficiais ([NAMING.md](NAMING.md)); os nomes de habilidades seguem em
> português **por ora** — a tradução/renomeação é a próxima passada de
> nomenclatura (proposta draft em [NAMING.md](NAMING.md)).

## Como ler uma carta

Cada personagem tem **4 slots**:

| Slot | O que é |
|---|---|
| **Mystery** | A habilidade assinatura: custo de **chakra** + **cooldown**, dano em % do ATK, alvo e **estado de combo** que aplica |
| **Padrão** | Ataque básico (sem chakra, sempre disponível quando a Mystery está em cooldown) |
| **Chase** | Perseguição automática: dispara quando um aliado aplica o **estado-gatilho** (Lançamento/Empurrão/Derrubada); 1× por rodada por unidade |
| **Passiva** | Efeito sempre ativo (reflexo, cura de rodada, chakra extra, bônus contra estados…) |

**Estados de combo** que alimentam as cadeias: **Lançamento** (arremesso) →
**Empurrão** → **Derrubada** — aliados com chase do gatilho correspondente
encadeiam golpes (até 12 elos por cadeia, 1 perseguição por unidade/rodada).

**Elementos (v0.9)** — cada personagem TEM um elemento (o do clã, campo
`element` do contrato; deriva automática se ausente) e TODO golpe ofensivo o
carrega. Ciclo de fraquezas, cada elemento vence exatamente um e perde para
exatamente um (a diagonal fica neutra):

| Elemento | Clã | Vence | Perde para |
|---|---|---|---|
| **Fogo** | PYREBORN | Pedra | Água |
| **Água** | DUSKMERE | Fogo | Elétrico |
| **Elétrico** | SKYREND | Água | Pedra |
| **Pedra** | THORNVALE | Elétrico | Fogo |

- **Vantagem elemental**: ×1200‰ de dano (e ×800‰ quando desvantagem) —
  por alvo, visível no log (`⚡ Elétrico vs Água: +20%`).
- **Ressonância** (mesmo elemento em sequência ofensiva do time): +15% de
  dano e **+5⚡** para quem age — o acelerador de energia de composições
  mono-elemento.
- **Cadeia elemental** (elemento na progressão do ciclo do golpe anterior):
  +10% de dano — a recompensa de combinar elementos DIFERENTES em cadeia;
  passivas de sintonia (ex.: Alísio) somam bônus extra AQUI.
- Cura, escudo e Interrupt não tocam a cadeia; DoT não é ação.

**Fórmula de dano (4v4):** `ATK efetivo × dmg% × 100/(100+DEF)`, tudo em
inteiros (per-mille) — determinístico em qualquer plataforma. Curas e escudos
usam % do ATK; escudos absorvem dano antes do HP.

---

## Tabela geral — 16 personagens (4 clãs × 4 classes)

| Personagem | Clã | Classe | HP | ATQ | DEF | VEL |
|---|---|---|---|---|---|---|
| Ignia Emberbright | PYREBORN | Reaver | 920 | 112 | 38 | 62 |
| Orson Moltenwall | PYREBORN | Bastion | 1140 | 96 | 58 | 42 |
| Amara Sweetflame | PYREBORN | Mender | 980 | 100 | 44 | 55 |
| Nimbo Smokeveil | PYREBORN | Tactician | 900 | 104 | 40 | 58 |
| Nereo Saltblade | DUSKMERE | Reaver | 950 | 110 | 40 | 60 |
| Dalva Reefwarden | DUSKMERE | Bastion | 1150 | 94 | 60 | 40 |
| Mel Tidemother | DUSKMERE | Mender | 1000 | 98 | 46 | 52 |
| Ligeia Coldmist | DUSKMERE | Tactician | 890 | 102 | 38 | 57 |
| Tina Twinthorn | THORNVALE | Reaver | 930 | 108 | 40 | 58 |
| Aro Barkshell | THORNVALE | Bastion | 1150 | 92 | 62 | 38 |
| Yara Lifesap | THORNVALE | Mender | 1020 | 96 | 46 | 50 |
| Ivo Finewire | THORNVALE | Tactician | 880 | 100 | 40 | 54 |
| Reia Thunderstep | SKYREND | Reaver | 930 | 115 | 36 | 70 |
| Aki Countercut | SKYREND | Bastion | 1100 | 95 | 56 | 46 |
| Alísio Lightgale | SKYREND | Mender | 980 | 97 | 44 | 60 |
| Volta Cyclone | SKYREND | Tactician | 900 | 103 | 38 | 59 |

**Por decisão de balanceamento NÃO existe contador duro entre clãs** — o DWC
calibra todas as composições para ~50% de winrate médio. Clã é identidade
temática/mecânica, não pedra-papel-tesoura.

---

## Clã PYREBORN — fogo e agressão

*Identidade: dano concentrado, Queimadura e pressão de cadeia.*

### Ignia Emberbright · `ignia` · Reaver

**HP 920 · ATQ 112 · DEF 38 · VEL 62**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Detonação Solar** | 120% de dano · alvo: frente · chakra 20 · cooldown 3 · estado: Lançamento |
| Padrão | **Corte em Brasa** | 70% de dano · estado: — |
| Chase | **Chute Vulcânico** | gatilho: Lançamento · 30% · aplica: Derrubada · 1×/rodada |
| Passiva | **Sede de Brasa** | +15% de dano contra alvos em Queimadura. |

> Golpe devastador que arremessa o alvo e o deixa em Queimadura (dano contínuo por 2 rodadas).

**Estilo de jogo:** pressão direta — abre a cadeia do clã com Lançamento +
Queimadura e pune alvos queimados.

### Orson Moltenwall · `orson` · Bastion

**HP 1140 · ATQ 96 · DEF 58 · VEL 42**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Escudo de Obsidiana** | escudo de 90% do ATK no aliado mais ferido e provoca os inimigos da frente · chakra 20 · cooldown 2 |
| Padrão | **Bastão de Lava** | 60% de dano · estado: Derrubada |
| Chase | **Contragolpe Fundido** | gatilho: Lançamento · 25% · aplica: Empurrão · 1×/rodada |
| Passiva | **Guardião Incandescente** | -15% de dano recebido enquanto estiver na linha de frente. |

> Concede escudo de 90% do ataque ao aliado e provoca os inimigos da frente.

**Estilo de jogo:** âncora da linha de frente — protege, provoca e derruba
invasores.

### Amara Sweetflame · `amara` · Mender

**HP 980 · ATQ 100 · DEF 44 · VEL 55**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Fogueira Interior** | cura 100% do ATK e gera +10 de chakra para a equipe · alvo: aliado mais ferido · chakra 15 · cooldown 2 |
| Padrão | **Toque Morno** | 60% de dano · estado: Lançamento |
| Chase | **Golpe Fervedor** | gatilho: Derrubada · 20% · aplica: — · 1×/rodada |
| Passiva | **Cinzas Que Aquecem** | No fim da rodada, cura 5% do HP do aliado com menos HP. |

> Cura 100% do ataque e gera +10 de chakra para a equipe.

**Estilo de jogo:** economizadora de chakra — sustenta as mysteries do time
desde cedo e fecha cadeias por Derrubada.

### Nimbo Smokeveil · `nimbo` · Tactician

**HP 900 · ATQ 104 · DEF 40 · VEL 58**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Névoa Sonífera** | 85% de dano · alvo: frente · chakra 25 · cooldown 3 · estado: Derrubada |
| Padrão | **Cajado Fumegante** | 60% de dano · estado: — |
| Chase | **Lança-Fumaça** | gatilho: Empurrão · 25% · aplica: Derrubada · 1×/rodada |
| Passiva | **Olhos de Cinza** | A primeira perseguição da equipe em cada rodada causa +10% de dano. |

> Dano nos inimigos da frente e Derrubada (a fila de ação deles é adiada).

**Estilo de jogo:** controla o ritmo — derruba a frente inimiga e encadeia a
partir de Empurrão.

---

## Clã DUSKMERE — maré e controle

*Identidade: escudos, reflexo, cura e negação de turnos.*

### Nereo Saltblade · `nereo` · Reaver

**HP 950 · ATQ 110 · DEF 40 · VEL 60**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Corte da Maré Alta** | 115% de dano · alvo: frente · chakra 20 · cooldown 3 · estado: Lançamento · ignora 50% do escudo |
| Padrão | **Golpe de Sal** | 70% de dano · estado: — |
| Chase | **Reflexo da Maré** | gatilho: Lançamento · 30% · aplica: Empurrão · 1×/rodada |
| Passiva | **Lâmina Salmoura** | +10% de dano; ataques contra alvos com escudo causam +25%. |

> Corte pesado que arremessa o alvo e ignora 50% do escudo dele.

**Estilo de jogo:** assassino anti-defesa — perfura escudos e pune times que
dependem deles.

### Dalva Reefwarden · `dalva` · Bastion

**HP 1150 · ATQ 94 · DEF 60 · VEL 40**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Barreira de Coral** | escudo de 90% do ATK no aliado mais ferido; reflete 20% do dano que ele absorver · chakra 20 · cooldown 2 |
| Padrão | **Cajado de Coral** | 60% de dano · estado: Derrubada |
| Chase | **Recife Vivo** | gatilho: Lançamento · 25% · aplica: Empurrão · 1×/rodada |
| Passiva | **Pele de Coral** | -15% de dano recebido na linha de frente. |

> Escudo de 90% do ataque no aliado alvo; reflete 20% do dano que ele absorver.

**Estilo de jogo:** fortaleza reflexiva — escuda o mais ferido e devolve
parte do dano absorvido.

### Mel Tidemother · `mel` · Mender

**HP 1000 · ATQ 98 · DEF 46 · VEL 52**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Maré Curativa** | cura 100% do ATK e dissipa 1 estado negativo do alvo · alvo: aliado mais ferido · chakra 15 · cooldown 2 |
| Padrão | **Espuma Firme** | 60% de dano · estado: Lançamento |
| Chase | **Contracorrente** | gatilho: Derrubada · 20% · aplica: — · 1×/rodada |
| Passiva | **Onda de Chakra** | No início da rodada, gera +8 de chakra para a equipe. |

> Cura 100% do ataque e dissipa 1 estado negativo do alvo.

**Estilo de jogo:** a economizadora suprema — +8 de chakra por rodada mantêm
o time sempre com resposta disponível.

### Ligeia Coldmist · `ligeia` · Tactician

**HP 890 · ATQ 102 · DEF 38 · VEL 57**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Prisão de Vapor** | 85% de dano · alvo: frente · chakra 25 · cooldown 3 · estado: Derrubada |
| Padrão | **Vapor Cortante** | 60% de dano · estado: — |
| Chase | **Redemoinho** | gatilho: Empurrão · 25% · aplica: Derrubada · 1×/rodada |
| Passiva | **Neblina Densa** | Inimigos Derrubados recebem +15% de dano dos ataques da equipe. |

> Dano e Derrubada nos inimigos da frente (prisão de vapor).

**Estilo de jogo:** punição em área — derruba a frente e transforma o estado
em bônus de dano para todo o time.

---

## Clã THORNVALE — natureza e veneno

*Identidade: desgaste contínuo, escudo que cresce e regeneração.*

### Tina Twinthorn · `tina` · Reaver

**HP 930 · ATQ 108 · DEF 40 · VEL 58**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Rajada de Espinhos** | 85% de dano em 2 inimigos aleatórios · chakra 25 · cooldown 3 · estado: Lançamento · aplica Veneno |
| Padrão | **Chicote de Cipó** | 70% de dano · estado: — |
| Chase | **Duplo Espinho** | gatilho: Lançamento · 30% · aplica: Empurrão · 1×/rodada |
| Passiva | **Peçonha Persistente** | Os ataques de Tina aplicam Veneno (dano contínuo por 2 rodadas). |

> 85% de dano em 2 inimigos aleatórios, arremessando-os; aplica Veneno (dano contínuo por 2 rodadas).

**Estilo de jogo:** desgaste em massa — alcança a linha de trás e envenena
tudo o que toca.

### Aro Barkshell · `aro` · Bastion

**HP 1150 · ATQ 92 · DEF 62 · VEL 38**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Casco de Pedra-Viva** | escudo próprio de 100% do ATK que ganha +25% de valor ao fim de cada rodada enquanto existir · chakra 25 · cooldown 3 |
| Padrão | **Casco Esmagador** | 60% de dano · estado: Derrubada |
| Chase | **Garras de Raiz** | gatilho: Lançamento · 25% · aplica: Empurrão · 1×/rodada |
| Passiva | **Casca Dura** | -15% de dano recebido; reflete 10% do dano de ataque padrão recebido. |

> Escudo de 100% do ataque que ganha +25% de valor ao fim de cada rodada enquanto existir.

**Estilo de jogo:** o muro mais duro do elenco — escudo que engorda com o
tempo, redução e reflexo.

### Yara Lifesap · `yara` · Mender

**HP 1020 · ATQ 96 · DEF 46 · VEL 50**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Seiva Vital** | cura 100% do ATK; o alvo regenera 6% do HP máximo por rodada durante 2 rodadas · alvo: aliado mais ferido · chakra 15 · cooldown 2 |
| Padrão | **Folha Cortante** | 60% de dano · estado: Lançamento |
| Chase | **Raízes Ágeis** | gatilho: Derrubada · 20% · aplica: — · 1×/rodada |
| Passiva | **Broto Renascido** | No fim da rodada, cura 5% do HP do aliado com menos HP. |

> Cura 100% do ataque; o alvo regenera 6% do HP máximo por rodada durante 2 rodadas.

**Estilo de jogo:** regeneração — cura instantânea + regeneração seguram
partidas longas.

### Ivo Finewire · `ivo` · Tactician

**HP 880 · ATQ 100 · DEF 40 · VEL 54**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Marionete Eremita** | 80% de dano · alvo: frente · chakra 25 · cooldown 3 · estado: Derrubada |
| Padrão | **Fio de Aço** | 60% de dano · estado: — |
| Chase | **Contra-Peso** | gatilho: Empurrão · 25% · aplica: Derrubada · 1×/rodada |
| Passiva | **Corda-Fina** | Inimigos com estado aplicado pela equipe causam -10% de dano. |

> A marionete investe: dano e Derrubada nos inimigos da frente.

**Estilo de jogo:** negação — trava a frente inimiga com Derrubada e
enfraquece quem está sob estado.

---

## Clã SKYREND — tempestade e velocidade

*Identidade: iniciativa, AoE de Lançamento e contra-ataque.*

### Reia Thunderstep · `reia` · Reaver

**HP 930 · ATQ 115 · DEF 36 · VEL 70**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Impacto do Trovão** | 130% de dano · alvo: frente · chakra 30 · cooldown 3 · estado: Lançamento · ignora 30% da defesa |
| Padrão | **Golpe Trovejante** | 75% de dano · estado: Lançamento |
| Chase | **Fenda do Raio** | gatilho: Lançamento · 35% · aplica: Empurrão · 1×/rodada |
| Passiva | **Iniciativa Trovejante** | Age primeiro na rodada de abertura; +10% de velocidade. |

> Dano devastador que arremessa o alvo e ignora 30% da defesa dele.

**Estilo de jogo:** ponta de lança — maior ATQ e VEL do elenco; abre cadeias
sozinha com o padrão que arremessa.

### Aki Countercut · `aki` · Bastion

**HP 1100 · ATQ 95 · DEF 56 · VEL 46**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Manto do Redemoinho** | escudo próprio de 80% do ATK; enquanto durar, contra-ataca quem a atingir · chakra 20 · cooldown 2 |
| Padrão | **Lâmina Espelhada** | 60% de dano · estado: Derrubada |
| Chase | **Corte Reverso** | gatilho: Lançamento · 25% · aplica: Empurrão · 1×/rodada |
| Passiva | **Espelho de Vento** | Ao ser atingida por ataque padrão, contra-ataca com 40% do ataque (1× por rodada). |

> Escudo de 80% do ataque em si; enquanto durar, contra-ataca quem a atingir.

**Estilo de jogo:** guardiã ativa — punir é defender; contra-ataca e fecha
cadeias com Derrubada no padrão.

### Alísio Lightgale · `alisio` · Mender

**HP 980 · ATQ 97 · DEF 44 · VEL 60**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Brisa Guiadora** | cura 90% do ATK e concede Aceleração (o alvo age antes dos inimigos na próxima rodada) · alvo: aliado mais ferido · chakra 15 · cooldown 2 |
| Padrão | **Rajada Firme** | 60% de dano · estado: Lançamento |
| Chase | **Vento de Cauda** | gatilho: Derrubada · 20% · aplica: — · 1×/rodada |
| Passiva | **Ritmo de Batalha** | A primeira perseguição da equipe por rodada gera +5 de chakra. Quando age em cadeia elemental (elemento diferente do golpe anterior do time, na progressão do ciclo), o golpe dela causa 20% a mais de dano nessa ação. |

> Cura 90% do ataque e concede Aceleração (o alvo age antes dos inimigos na próxima rodada).

**Estilo de jogo:** maestro da iniciativa — cura + Aceleração e economia de
chakra alimentada por cada perseguição; com os elementos (v0.9), é a **passiva
de sintonia**: acertar a progressão do ciclo com um elemento DIFERENTE rende
+30% de dano naquela ação (10% da cadeia + 20% da sintonia).

### Volta Cyclone · `volta` · Tactician

**HP 900 · ATQ 103 · DEF 38 · VEL 59**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Tempestade Circular** | 80% de dano em TODOS os inimigos · chakra 30 · cooldown 4 · estado: Lançamento |
| Padrão | **Discos de Vento** | 60% de dano · estado: — |
| Chase | **Descarga de Acompanhamento** | gatilho: Empurrão · 25% · aplica: Derrubada · 1×/rodada |
| Passiva | **Carga Estática** | Inimigos Arremessados perdem 5 de velocidade até o fim da rodada. |

> 80% de dano em todos os inimigos, arremessando-os.

**Estilo de jogo:** tempestade total — arremessa o time inimigo inteiro e
prepara cadeias em massa (a única Mystery em `enemies_all` do elenco).

---

## Cadeias garantidas por clã (validadas por script)

Padrão por clã: **Reaver/Mender aplicam Lançamento → Bastion/Tactician
perseguem (Empurrão) → Tactician/Mender fecha com Derrubada**.

| Clã | Pares Lançamento→perseguição | Pares Derrubada→perseguição |
|---|---|---|
| PYREBORN | Ignia (mystery) → Orson; Amara (padrão) → Orson | Nimbo (mystery) → Amara; Orson (padrão) → Amara |
| DUSKMERE | Nereo (mystery) → Dalva; Mel (padrão) → Dalva | Ligeia (mystery) → Mel; Dalva (padrão) → Mel |
| THORNVALE | Tina (mystery) → Aro; Yara (padrão) → Aro | Ivo (mystery) → Yara; Aro (padrão) → Yara |
| SKYREND | Volta (mystery AoE) → Reia **e** Aki; Reia (mystery e padrão) → Aki | Aki (padrão) → Alísio |

---

## Roster do modo 1v1 legado (12 personagens — tabela `ROSTER`, `data.rs`)

Modo "roguelike de runs curtas" puro (partida de 9 rodadas, primeiro a 5;
chakra teto 10, regeneração 2/turno; febre de batalha +2%/rodada; escalada
+8%/rodada). Cada personagem tem **básico (grátis) + habilidade + ultimate**:

| # | Nome | Papel | HP | ATK | DEF | SPD | Habilidade | Ultimate |
|---|------|-------|----|-----|-----|-----|------------|----------|
| 0 | Golem | Tanque | 150 | 20 | 32 | 8 | Muralha de Pedra (escudo 30, chakra 3, cd 3) | Terremoto (70% AoE, launch, chakra 6, cd 4) |
| 1 | Bruto | Berserker | 115 | 24 | 26 | 9 | Golpe Esmagador (150%, launch, chakra 4, cd 3) | Fúria (+30% ATK 2 rodadas, chakra 3, cd 4) |
| 2 | Sombra | Assassino | 85 | 26 | 14 | 15 | Lâmina Traspassa (130%, **chase**, chakra 3, cd 2) | Execução (190%, launch, chakra 6, cd 4) |
| 3 | Arqueira | Atiradora | 92 | 26 | 20 | 12 | Tiro Certeiro (125%, chakra 3, cd 2) | Chuva de Flechas (70% AoE, launch, chakra 6, cd 4) |
| 4 | Piromante | Mago | 88 | 28 | 15 | 10 | Bola de Fogo (125% + veneno 6×2, launch, chakra 4, cd 3) | Deflagrar (90% AoE, chakra 7, cd 5) |
| 5 | Oráculo | Curandeiro | 95 | 19 | 26 | 10 | Bênção (cura 26, chakra 3, cd 3) | Círculo Sagrado (cura 14 + escudo 10 AoE, chakra 6, cd 4) |
| 6 | Comandante | Suporte | 105 | 22 | 26 | 10 | Ordem de Ataque (+35% ATK time, chakra 4, cd 3) | Contra-Ataque (115% + escudo 22, chakra 3, cd 2) |
| 7 | Feiticeira | Maldição | 90 | 23 | 20 | 10 | Maldição (−25% ATK + veneno 6×3, chakra 4, cd 3) | Vórtice (55% AoE −15% ATK, launch, chakra 6, cd 4) |
| 8 | Duelista | Duelista | 90 | 25 | 20 | 13 | Estocada Dupla (135%, **chase**, chakra 3, cd 2) | Postura Precisa (+35% ATK 1 rodada, chakra 2, cd 3) |
| 9 | Monge | Lutador | 100 | 24 | 20 | 11 | Palma Sônica (115% + atordoamento 1, **chase**, chakra 4, cd 3) | Fluxo Interior (cura 20 aliado, chakra 2, cd 3) |
| 10 | Necromante | Evocador | 90 | 26 | 20 | 10 | Colheita (125% + cura 15, chakra 3, cd 2) | Enxame de Corvos (65% AoE + veneno 4×2, launch, chakra 6, cd 4) |
| 11 | Guardiã | Paladino | 125 | 23 | 30 | 10 | Escudo Divino (cura 8 + escudo 26, chakra 3, cd 3) | Julgamento (145% + escudo 15, launch, chakra 4, cd 3) |

> O básico universal é **"Ataque"**: 90% do ATK, custo 0, cooldown 0.
> Chase no 1v1: máx. **1 perseguição por rodada por time**, a **60%** do efeito.

---

## O que NÃO está modelado (honestidade da simulação)

A engine nunca assume efeito que não existiu no design — passivas cujo texto
não é interpretável por palavra-chave aparecem em `Formation4v4::unmodeled()`
(diagnóstico nos logs). Modeladas hoje: bônus de dano simples e condicionais
(contra Queimadura/Veneno/estados), redução de dano recebido,
reflexo/contra-ataque, cura de fim de rodada (aliado mais ferido), chakra de
início de rodada, DoT aplicado por ataque, escudos (incluindo o de Aro, que
cresce +25%/rodada) e perfurações (Nereo −50% escudo, Reia −30% defesa).
Efeitos puramente narrativos (ex.: "adiar a fila de ação", "Aceleração",
"provocar") ficam visíveis nos logs como não-modelados — quando entrarem no
motor, entram para os dois lados por igual.
