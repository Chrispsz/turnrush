# Nomenclatura — TurnRush

> **Status**: os nomes de **clãs, classes e personagens** abaixo são
> **OFICIAIS** e já são usados nesta documentação e no roster
> (`data/roster/roster-v2.json`). Os nomes de **habilidades** (mysteries,
> padrões, chases e passivas) seguem em **português por ora** — a proposta em
> inglês na §5 é **DRAFT, pendente de aprovação**, e representa a próxima
> passada de nomenclatura.

---

## 1. Clãs (factions)

| id interno | Nome de exibição | Tema | Personagens |
|---|---|---|---|
| `ember` | **PYREBORN** | fogo · agressão | Ignia, Orson, Amara, Nimbo |
| `tide` | **DUSKMERE** | maré · controle | Nereo, Dalva, Mel, Ligeia |
| `verdant` | **THORNVALE** | natureza · veneno | Tina, Aro, Yara, Ivo |
| `storm` | **SKYREND** | tempestade · velocidade | Reia, Aki, Alísio, Volta |

Racional:

- **PYREBORN** — *pyre* (pira) + *born* (nascidos): "nascidos da pira".
  Identidade de fogo e agressão: dano concentrado, Queimadura, pressão de
  cadeia. O nome carrega a ideia de renascimento pelo calor — força que se
  consome e se reacende.
- **DUSKMERE** — *dusk* (crepúsculo) + *mere* (lago, arcaísmo inglês): "o
  lago do crepúsculo". Identidade de maré e controle: escudos, reflexo,
  cura e negação de turnos — profundidade calma que engole o ritmo inimigo.
- **THORNVALE** — "vale dos espinhos". Identidade de natureza e veneno:
  desgaste contínuo, escudo que cresce, regeneração — beleza hostil que
  protege o próprio terreno e desgasta quem nele entra.
- **SKYREND** — *rend* (rasgar): "aqueles que rasgam o céu". Identidade de
  tempestade e velocidade: iniciativa, AoE de Lançamento, contra-ataque.

## 2. Classes (roles)

| id interno | Nome de exibição | Papel no time |
|---|---|---|
| `striker` | **Reaver** | dano concentrado; abre cadeias com Lançamento |
| `guardian` | **Bastion** | linha de frente; escudo, provocação, Derrubada |
| `support` | **Mender** | cura e economia de chakra; sustenta mysteries |
| `controller` | **Tactician** | controle de estados e timing; punição em área |

Racional: **Reaver** evoca o saqueador que devasta (agressão pura);
**Bastion** é a fortaleza da linha de frente; **Mender** é quem repara e
mantém o time de pé; **Tactician** comanda estados, ordem e timing — o
cérebro que transforma estados de combo em vantagem.

## 3. Personagens (elenco 4v4 — oficial)

| id interno | Nome de exibição | Clã | Classe |
|---|---|---|---|
| `ignia` | **Ignia Emberbright** | PYREBORN | Reaver |
| `orson` | **Orson Moltenwall** | PYREBORN | Bastion |
| `amara` | **Amara Sweetflame** | PYREBORN | Mender |
| `nimbo` | **Nimbo Smokeveil** | PYREBORN | Tactician |
| `nereo` | **Nereo Saltblade** | DUSKMERE | Reaver |
| `dalva` | **Dalva Reefwarden** | DUSKMERE | Bastion |
| `mel` | **Mel Tidemother** | DUSKMERE | Mender |
| `ligeia` | **Ligeia Coldmist** | DUSKMERE | Tactician |
| `tina` | **Tina Twinthorn** | THORNVALE | Reaver |
| `aro` | **Aro Barkshell** | THORNVALE | Bastion |
| `yara` | **Yara Lifesap** | THORNVALE | Mender |
| `ivo` | **Ivo Finewire** | THORNVALE | Tactician |
| `reia` | **Reia Thunderstep** | SKYREND | Reaver |
| `aki` | **Aki Countercut** | SKYREND | Bastion |
| `alisio` | **Alísio Lightgale** | SKYREND | Mender |
| `volta` | **Volta Cyclone** | SKYREND | Tactician |

Os ids internos (`ignia`, `tide`, `striker`…) são **estáveis e não mudam** —
são chaves de dados; os nomes de exibição podem evoluir sem quebrar engine,
roster ou saves/logs.

## 4. Estados de combo (glossário)

| id interno | Termo em jogo (atual, pt) | Proposta EN (draft) | Papel na cadeia |
|---|---|---|---|
| `launch` | Lançamento (arremesso) | Launch | abre cadeias |
| `knockback` | Empurrão | Knockback | intermediário |
| `knockdown` | Derrubada | Knockdown | fecha cadeias |

Os termos **Mystery** e **Chase** já são o vocabulário oficial (em inglês)
dos slots de habilidade e permanecem.

## 5. Habilidades — proposta EN (DRAFT · pendente de aprovação)

> A próxima passada de nomenclatura renomeará mysteries, padrões, chases e
> passivas para inglês. Abaixo, a proposta completa (16 personagens × 4
> slots = 64 nomes). **Nada disto está implementado ainda** — os nomes
> atuais em português continuam valendo em jogo e nos dados até a aprovação.

### PYREBORN

| Personagem | Mystery (atual → proposta) | Padrão | Chase | Passiva |
|---|---|---|---|---|
| Ignia Emberbright | Detonação Solar → **Solar Detonation** | Corte em Brasa → **Ember Slash** | Chute Vulcânico → **Volcanic Kick** | Sede de Brasa → **Ember Thirst** |
| Orson Moltenwall | Escudo de Obsidiana → **Obsidian Bulwark** | Bastão de Lava → **Molten Cudgel** | Contragolpe Fundido → **Molten Counter** | Guardião Incandescente → **Incandescent Guardian** |
| Amara Sweetflame | Fogueira Interior → **Inner Hearth** | Toque Morno → **Warm Touch** | Golpe Fervedor → **Boiling Strike** | Cinzas Que Aquecem → **Warm Embers** |
| Nimbo Smokeveil | Névoa Sonífera → **Slumber Mist** | Cajado Fumegante → **Smoldering Staff** | Lança-Fumaça → **Smoke Lance** | Olhos de Cinza → **Ashen Gaze** |

### DUSKMERE

| Personagem | Mystery (atual → proposta) | Padrão | Chase | Passiva |
|---|---|---|---|---|
| Nereo Saltblade | Corte da Maré Alta → **Hightide Cleave** | Golpe de Sal → **Salt Strike** | Reflexo da Maré → **Tide Reflex** | Lâmina Salmoura → **Brine Blade** |
| Dalva Reefwarden | Barreira de Coral → **Coral Barrier** | Cajado de Coral → **Coral Staff** | Recife Vivo → **Living Reef** | Pele de Coral → **Coral Skin** |
| Mel Tidemother | Maré Curativa → **Mending Tide** | Espuma Firme → **Steadfast Foam** | Contracorrente → **Undertow** | Onda de Chakra → **Rising Tide** |
| Ligeia Coldmist | Prisão de Vapor → **Vapor Prison** | Vapor Cortante → **Cleaving Steam** | Redemoinho → **Maelstrom** | Neblina Densa → **Dense Fog** |

### THORNVALE

| Personagem | Mystery (atual → proposta) | Padrão | Chase | Passiva |
|---|---|---|---|---|
| Tina Twinthorn | Rajada de Espinhos → **Thorn Volley** | Chicote de Cipó → **Vine Whip** | Duplo Espinho → **Twin Thorns** | Peçonha Persistente → **Lingering Venom** |
| Aro Barkshell | Casco de Pedra-Viva → **Living Stone Shell** | Casco Esmagador → **Shell Crusher** | Garras de Raiz → **Root Talons** | Casca Dura → **Hardbark** |
| Yara Lifesap | Seiva Vital → **Vital Sap** | Folha Cortante → **Leaf Blade** | Raízes Ágeis → **Swift Roots** | Broto Renascido → **Reborn Sprout** |
| Ivo Finewire | Marionete Eremita → **Hermit Puppet** | Fio de Aço → **Steel Wire** | Contra-Peso → **Counterweight** | Corda-Fina → **Fine Wire** |

### SKYREND

| Personagem | Mystery (atual → proposta) | Padrão | Chase | Passiva |
|---|---|---|---|---|
| Reia Thunderstep | Impacto do Trovão → **Thunder Impact** | Golpe Trovejante → **Thundering Strike** | Fenda do Raio → **Lightning Rift** | Iniciativa Trovejante → **First Thunder** |
| Aki Countercut | Manto do Redemoinho → **Whirlwind Mantle** | Lâmina Espelhada → **Mirror Blade** | Corte Reverso → **Reverse Cut** | Espelho de Vento → **Wind Mirror** |
| Alísio Lightgale | Brisa Guiadora → **Guiding Breeze** | Rajada Firme → **Steady Gust** | Vento de Cauda → **Tailwind** | Ritmo de Batalha → **Battle Rhythm** |
| Volta Cyclone | Tempestade Circular → **Circling Tempest** | Discos de Vento → **Wind Discs** | Descarga de Acompanhamento → **Follow-Up Discharge** | Carga Estática → **Static Charge** |

## 6. Como aplicar a próxima passada (quando aprovada)

1. Renomear os campos `name`/`effect` de `mystery`/`standard`/`chase`/
   `passive` em `data/roster/roster-v2.json` (os textos de efeito alimentam o
   parser de passivas por palavra-chave — **traduzir os efeitos exige
   atualizar as palavras-chave do loader em `formation.rs` na mesma mudança**).
2. Atualizar as strings da UI (telas Como Jogar/Elenco/log de batalha).
3. Atualizar esta doc + `PERSONAGENS.md` (e remover a nota de "por ora").
4. Rodar `cargo test --workspace` + gates do DWC no CI — nada de números
   muda, mas os testes de contrato garantem que a passada foi completa.
