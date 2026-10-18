# Personagens — TurnRush (esboço v0.5.0)

> **Fase atual: mecânica e gameplay.** Nomes, textos e números são
> **PROVISÓRIOS** — a arte, sprites e nomes finais virão depois. Este documento
> é gerado a partir da engine: o elenco jogável vem de
> `data/roster/roster-v2.json` (contrato v2, carregado por
> `turnrush-core::data::Roster` e embutido no binário do jogo via
> `include_str!`); o elenco do modo 1v1 é a tabela constante `ROSTER` em
> `crates/turnrush-core/src/data.rs`. **Nada aqui existe fora da engine** —
> se está documentado, está implementado e testado.

## Como ler uma carta

Cada personagem tem **4 slots** (vocabulário herdado do estudo do Naruto
Online — `data/naruto-online/MECANICAS.md`):

| Slot | O que é |
|---|---|
| **Mystery** | A "definitiva": custo de **chakra** + **cooldown**, dano em % do ATK, alvo e **estado de combo** que aplica |
| **Padrão** | Ataque básico (sem chakra, sempre disponível quando a Mystery está em cooldown) |
| **Chase** | Perseguição automática: dispara quando um aliado aplica o **estado-gatilho** (Lançamento/Empurrão/Derrubada); 1× por rodada por unidade |
| **Passiva** | Efeito sempre ativo (reflexo, cura de rodada, chakra extra, bônus contra estados…) |

**Estados de combo** que alimentam as cadeias: **Lançamento (arremesso)** →
**Empurrão** → **Derrubada** — aliados com chase do gatilho correspondente
encadeiam golpes no mesmo turno (até 12 elos por cadeia).

**Fórmula de dano (4v4):** `ATK efetivo × dmg% × 100/(100+DEF)`, tudo em
inteiros (per-mille) — determinístico em qualquer plataforma.

---

## Elenco do jogo — Formação 4v4 (16 cartas, 4 facções × 4 papéis)

| Facção | Identidade | Striker | Guardian | Support | Controller |
|---|---|---|---|---|---|
| **Ember** | agressão + Queimadura | Ígnia | Orson | Amara | Nimbo |
| **Tide** | controle + escudos/reflexo | Nereo | Dalva | Mel | Ligeia |
| **Verdant** | desgaste + Veneno | Tina | Aro | Yara | Ívo |
| **Storm** | velocidade + ação | Reia | Aki | Alísio | Volta |

**Por decisão de balanceamento NÃO existe contador duro entre facções** — o
DWC v2 calibra todas as composições para ~50% de winrate médio. Facção é
identidade temática/visual (e futuramente bônus de sinergia internos à própria
facção), não pedra-papel-tesoura.

### Facção Ember (Brasa)

*Identidade: agressão e dano contínuo (Queimadura).*

#### Ígnia Brasa-Viva · `ignia`

**Striker (atacante)** — HP **920** · ATQ **112** · DEF **38** · VEL **62**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Detonação Solar** | 120% de dano · alvo: frente · chakra 20 · cooldown 3 · estado: Lançamento (arremesso) |
| Padrão | **Corte em Brasa** | 70% de dano · estado: — |
| Chase | **Chute Vulcânico** | gatilho: Lançamento (arremesso) · 30% · aplica: Derrubada · 1×/rodada |
| Passiva | **Sede de Brasa** | +15% de dano contra alvos em Queimadura. |

> Golpe devastador que arremessa o alvo e o deixa em Queimadura (dano contínuo por 2 rodadas).

#### Orson Muralha-Fundida · `orson`

**Guardian (guardião)** — HP **1140** · ATQ **96** · DEF **58** · VEL **42**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Escudo de Obsidiana** | 90% de dano · alvo: aliado (mais ferido) · chakra 20 · cooldown 2 · estado: — |
| Padrão | **Bastão de Lava** | 60% de dano · estado: Derrubada |
| Chase | **Contragolpe Fundido** | gatilho: Lançamento (arremesso) · 25% · aplica: Empurrão · 1×/rodada |
| Passiva | **Guardião Incandescente** | -15% de dano recebido enquanto estiver na linha de frente. |

> Concede escudo de 90% do ataque ao aliado e provoca os inimigos da frente.

#### Amara Chama-Doce · `amara`

**Support (suporte)** — HP **980** · ATQ **100** · DEF **44** · VEL **55**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Fogueira Interior** | 100% de dano · alvo: aliado (mais ferido) · chakra 15 · cooldown 2 · estado: — |
| Padrão | **Toque Morno** | 60% de dano · estado: Lançamento (arremesso) |
| Chase | **Golpe Fervedor** | gatilho: Derrubada · 20% · aplica: — · 1×/rodada |
| Passiva | **Cinzas Que Aquecem** | No fim da rodada, cura 5% do HP do aliado com menos HP. |

> Cura 100% do ataque e gera +10 de chakra para a equipe.

#### Nimbo Véu-de-Fumaça · `nimbo`

**Controller (controlador)** — HP **900** · ATQ **104** · DEF **40** · VEL **58**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Névoa Sonífera** | 85% de dano · alvo: frente · chakra 25 · cooldown 3 · estado: Derrubada |
| Padrão | **Cajado Fumegante** | 60% de dano · estado: — |
| Chase | **Lança-Fumaça** | gatilho: Empurrão · 25% · aplica: Derrubada · 1×/rodada |
| Passiva | **Olhos de Cinza** | A primeira perseguição da equipe em cada rodada causa +10% de dano. |

> Dano nos inimigos da frente e Derrubada (a fila de ação deles é adiada).


### Facção Tide (Maré)

*Identidade: controle, escudos e reflexo.*

#### Nereo Lâmina-de-Sal · `nereo`

**Striker (atacante)** — HP **950** · ATQ **110** · DEF **40** · VEL **60**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Corte da Maré Alta** | 115% de dano · alvo: frente · chakra 20 · cooldown 3 · estado: Lançamento (arremesso) |
| Padrão | **Golpe de Sal** | 70% de dano · estado: — |
| Chase | **Reflexo da Maré** | gatilho: Lançamento (arremesso) · 30% · aplica: Empurrão · 1×/rodada |
| Passiva | **Lâmina Salmoura** | +10% de dano; ataques contra alvos com escudo causam +25%. |

> Corte pesado que arremessa o alvo e ignora 50% do escudo dele.

#### Dalva Recife · `dalva`

**Guardian (guardião)** — HP **1150** · ATQ **94** · DEF **60** · VEL **40**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Barreira de Coral** | 90% de dano · alvo: aliado (mais ferido) · chakra 20 · cooldown 2 · estado: — |
| Padrão | **Cajado de Coral** | 60% de dano · estado: Derrubada |
| Chase | **Recife Vivo** | gatilho: Lançamento (arremesso) · 25% · aplica: Empurrão · 1×/rodada |
| Passiva | **Pele de Coral** | -15% de dano recebido na linha de frente. |

> Escudo de 90% do ataque no aliado alvo; reflete 20% do dano que ele absorver.

#### Mel Maré-Mãe · `mel`

**Support (suporte)** — HP **1000** · ATQ **98** · DEF **46** · VEL **52**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Maré Curativa** | 100% de dano · alvo: aliado (mais ferido) · chakra 15 · cooldown 2 · estado: — |
| Padrão | **Espuma Firme** | 60% de dano · estado: Lançamento (arremesso) |
| Chase | **Contracorrente** | gatilho: Derrubada · 20% · aplica: — · 1×/rodada |
| Passiva | **Onda de Chakra** | No início da rodada, gera +8 de chakra para a equipe. |

> Cura 100% do ataque e dissipa 1 estado negativo do alvo.

#### Ligeia Névoa-Fria · `ligeia`

**Controller (controlador)** — HP **890** · ATQ **102** · DEF **38** · VEL **57**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Prisão de Vapor** | 85% de dano · alvo: frente · chakra 25 · cooldown 3 · estado: Derrubada |
| Padrão | **Vapor Cortante** | 60% de dano · estado: — |
| Chase | **Redemoinho** | gatilho: Empurrão · 25% · aplica: Derrubada · 1×/rodada |
| Passiva | **Neblina Densa** | Inimigos Derrubados recebem +15% de dano dos ataques da equipe. |

> Dano e Derrubada nos inimigos da frente (prisão de vapor).


### Facção Verdant (Verde)

*Identidade: desgaste, veneno e regeneração.*

#### Tina Espinho-Gêmeo · `tina`

**Striker (atacante)** — HP **930** · ATQ **108** · DEF **40** · VEL **58**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Rajada de Espinhos** | 85% de dano · alvo: 2 aleatórios · chakra 25 · cooldown 3 · estado: Lançamento (arremesso) |
| Padrão | **Chicote de Cipó** | 70% de dano · estado: — |
| Chase | **Duplo Espinho** | gatilho: Lançamento (arremesso) · 30% · aplica: Empurrão · 1×/rodada |
| Passiva | **Peçonha Persistente** | Os ataques de Tina aplicam Veneno (dano contínuo por 2 rodadas). |

> 85% de dano em 2 inimigos aleatórios, arremessando-os; aplica Veneno (dano contínuo por 2 rodadas).

#### Aro Cascão · `aro`

**Guardian (guardião)** — HP **1150** · ATQ **92** · DEF **62** · VEL **38**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Casco de Pedra-Viva** | 100% de dano · alvo: si · chakra 25 · cooldown 3 · estado: — |
| Padrão | **Casco Esmagador** | 60% de dano · estado: Derrubada |
| Chase | **Garras de Raiz** | gatilho: Lançamento (arremesso) · 25% · aplica: Empurrão · 1×/rodada |
| Passiva | **Casca Dura** | -15% de dano recebido; reflete 10% do dano de ataque padrão recebido. |

> Escudo de 100% do ataque que ganha +25% de valor ao fim de cada rodada enquanto existir.

#### Yara Seiva-Viva · `yara`

**Support (suporte)** — HP **1020** · ATQ **96** · DEF **46** · VEL **50**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Seiva Vital** | 100% de dano · alvo: aliado (mais ferido) · chakra 15 · cooldown 2 · estado: — |
| Padrão | **Folha Cortante** | 60% de dano · estado: Lançamento (arremesso) |
| Chase | **Raízes Ágeis** | gatilho: Derrubada · 20% · aplica: — · 1×/rodada |
| Passiva | **Broto Renascido** | No fim da rodada, cura 5% do HP do aliado com menos HP. |

> Cura 100% do ataque; o alvo regenera 6% do HP máximo por rodada durante 2 rodadas.

#### Ívo Corda-Fina · `ivo`

**Controller (controlador)** — HP **880** · ATQ **100** · DEF **40** · VEL **54**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Marionete Eremita** | 80% de dano · alvo: frente · chakra 25 · cooldown 3 · estado: Derrubada |
| Padrão | **Fio de Aço** | 60% de dano · estado: — |
| Chase | **Contra-Peso** | gatilho: Empurrão · 25% · aplica: Derrubada · 1×/rodada |
| Passiva | **Corda-Fina** | Inimigos com estado aplicado pela equipe causam -10% de dano. |

> A marionete investe: dano e Derrubada nos inimigos da frente.


### Facção Storm (Tempestade)

*Identidade: velocidade e ganância de ação.*

#### Reia Passo-Trovão · `reia`

**Striker (atacante)** — HP **930** · ATQ **115** · DEF **36** · VEL **70**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Impacto do Trovão** | 130% de dano · alvo: frente · chakra 30 · cooldown 3 · estado: Lançamento (arremesso) |
| Padrão | **Golpe Trovejante** | 75% de dano · estado: Lançamento (arremesso) |
| Chase | **Fenda do Raio** | gatilho: Lançamento (arremesso) · 35% · aplica: Empurrão · 1×/rodada |
| Passiva | **Iniciativa Trovejante** | Age primeiro na rodada de abertura; +10% de velocidade. |

> Dano devastador que arremessa o alvo e ignora 30% da defesa dele.

#### Aki Contra-Corte · `aki`

**Guardian (guardião)** — HP **1100** · ATQ **95** · DEF **56** · VEL **46**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Manto do Redemoinho** | 80% de dano · alvo: si · chakra 20 · cooldown 2 · estado: — |
| Padrão | **Lâmina Espelhada** | 60% de dano · estado: Derrubada |
| Chase | **Corte Reverso** | gatilho: Lançamento (arremesso) · 25% · aplica: Empurrão · 1×/rodada |
| Passiva | **Espelho de Vento** | Ao ser atingida por ataque padrão, contra-ataca com 40% do ataque (1x por rodada). |

> Escudo de 80% do ataque em si; enquanto durar, contra-ataca quem a atingir.

#### Alísio Sopro-Ligeiro · `alisio`

**Support (suporte)** — HP **980** · ATQ **97** · DEF **44** · VEL **60**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Brisa Guiadora** | 90% de dano · alvo: aliado (mais ferido) · chakra 15 · cooldown 2 · estado: — |
| Padrão | **Rajada Firme** | 60% de dano · estado: Lançamento (arremesso) |
| Chase | **Vento de Cauda** | gatilho: Derrubada · 20% · aplica: — · 1×/rodada |
| Passiva | **Ritmo de Batalha** | A primeira perseguição da equipe por rodada gera +5 de chakra. |

> Cura 90% do ataque e concede Aceleração (o alvo age antes dos inimigos na próxima rodada).

#### Volta Ciclone · `volta`

**Controller (controlador)** — HP **900** · ATQ **103** · DEF **38** · VEL **59**

| Slot | Habilidade | Números |
|---|---|---|
| Mystery | **Tempestade Circular** | 80% de dano · alvo: todos · chakra 30 · cooldown 4 · estado: Lançamento (arremesso) |
| Padrão | **Discos de Vento** | 60% de dano · estado: — |
| Chase | **Descarga de Acompanhamento** | gatilho: Empurrão · 25% · aplica: Derrubada · 1×/rodada |
| Passiva | **Carga Estática** | Inimigos Arremessados perdem 5 de velocidade até o fim da rodada. |

> 80% de dano em todos os inimigos, arremessando-os.


---

## Roster do modo 1v1 (12 personagens — tabela `ROSTER`, `data.rs`)

Modo "roguelike acelerado" puro (partida de 9 rodadas, primeiro a 5; chakra
cap 10, regen 2/turno; febre de batalha +2%/rodada). Cada personagem tem
**básico (grátis) + habilidade + ultimate**:

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
(diagnóstico nos logs). Hoje estão modeladas: bônus de dano simples e
condicionais (contra Queimadura/Veneno/estados), redução de dano recebido,
reflexo/contra-ataque, cura de fim de rodada (aliado mais ferido), chakra de
início de rodada, DoT aplicado por ataque. Efeitos puramente narrativos
(ex.: "adiar a fila de ação") ficam visíveis nos logs como não-modelados.
