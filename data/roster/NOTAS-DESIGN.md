# NOTAS DE DESIGN — Roster v2 do TurnRush

> IP original, 4 clãs × 4 classes (16 personagens). Identidade própria do
> jogo: cada personagem tem uma assinatura mecânica única dentro do orçamento
> de balanceamento — sem nomes, imagens ou textos de terceiros.
> Contrato de engine: `roster-v2.json` (schema v2). Interpretações contratadas para o loader:
> - `targets: ally_single` + `mystery.damage_pct` = **% do ATTACK** aplicado como **CURA** (Mender) ou **ESCUDO** (Bastion). `applies_state` só tem efeito em alvos inimigos.
> - `enemies_front` = inimigos da linha de frente; `enemies_all` = todos; `random_2` = 2 aleatórios.
> - Estados: `launch` (Lançamento/arremesso) → `knockback` (Empurrão) → `knockdown` (Derrubada); `none`.

## 1. Mapa clã → arquétipo

| Clã | Arquétipo | Ideia de jogo |
|---|---|---|
| **ember** (PYREBORN) | Agressão / pressão | Dano concentrado + Queimadura; cadeias rápidas de Lançamento; suporte que gera chakra |
| **tide** (DUSKMERE) | Controle / sustentação | Cura, escudo com reflexo, prisão/derrubada para negar turnos, punição anti-escudo |
| **verdant** (THORNVALE) | Desgaste / crescimento | Veneno, escudo que cresce, marionete que trava; vence partidas longas |
| **storm** (SKYREND) | Velocidade / combo | Iniciativa, AoE de Lançamento, contra-ataque; mais papéis agressivos, menos HP |

## 2. Os 16 personagens (assinatura mecânica)

| ID | Nome | Clã | Classe | Assinatura mecânica (única) |
|---|---|---|---|---|
| `ignia` | Ignia Emberbright | ember | striker | Mystery 120% + Queimadura por 2 rodadas; +15% de dano contra Queimados |
| `orson` | Orson Moltenwall | ember | guardian | Escudo + **Provocação** na frente; padrão com Derrubada |
| `amara` | Amara Sweetflame | ember | support | Cura que **gera +10 de chakra** para a equipe |
| `nimbo` | Nimbo Smokeveil | ember | controller | Mystery com Derrubada em frente (adiando fila) + chase Empurrão→Derrubada |
| `nereo` | Nereo Saltblade | tide | striker | Mystery que **ignora 50% do escudo**; +25% de dano contra escudados |
| `dalva` | Dalva Reefwarden | tide | guardian | Escudo que **reflete 20%** do dano absorvido |
| `mel` | Mel Tidemother | tide | support | Cura + **dissipação**; passiva +8 chakra/rodada |
| `ligeia` | Ligeia Coldmist | tide | controller | Prisão de Vapor (Derrubada em frente); +15% de dano em Derrubados |
| `tina` | Tina Twinthorn | verdant | striker | Mystery `random_2` + Veneno; ataques aplicam Veneno (desgaste) |
| `aro` | Aro Barkshell | verdant | guardian | Escudo que **cresce +25%/rodada** enquanto existir |
| `yara` | Yara Lifesap | verdant | support | Cura + **regeneração 6%/rodada por 2 rodadas** |
| `ivo` | Ivo Finewire | verdant | controller | Marionete (Derrubada em frente); -10% de dano de inimigos com estado da equipe |
| `reia` | Reia Thunderstep | storm | striker | Mystery 130% **ignora 30% de defesa**; padrão que arremessa; age primeiro |
| `aki` | Aki Countercut | storm | guardian | Escudo próprio com **contra-ataque** enquanto durar; padrão com Derrubada |
| `alisio` | Alísio Lightgale | storm | support | Cura + **Aceleração** (alvo age antes dos inimigos na próxima rodada) |
| `volta` | Volta Cyclone | storm | controller | **AoE em todos** (80%) com Arremesso; inimigos arremessados perdem 5 de velocidade |

## 3. Cadeias garantidas por clã (validadas por script)

Padrão por clã: **striker/suporte aplicam Lançamento → guardian/controlador perseguem (Empurrão) → controlador/suporte fecha com Derrubada**.

| Clã | Pares Lançamento→perseguição | Pares Derrubada→perseguição |
|---|---|---|
| ember | Ignia (mystery) → Orson; Amara (padrão) → Orson | Nimbo (mystery) → Amara; Orson (padrão) → Amara |
| tide | Nereo (mystery) → Dalva; Mel (padrão) → Dalva | Ligeia (mystery) → Mel; Dalva (padrão) → Mel |
| verdant | Tina (mystery) → Aro; Yara (padrão) → Aro | Ivo (mystery) → Yara; Aro (padrão) → Yara |
| storm | Volta (mystery AoE) → Reia **e** Aki; Reia (mystery e padrão) → Aki | Aki (padrão) → Alísio |

Cada chase: `max_per_round: 1` (contrato — 1 perseguição por unidade/rodada).

## 4. Orçamento de balanceamento

**Ranges do contrato (obrigatórios):** HP 850–1150 · standard 60–80 · mystery 80–130 (chakra 15–40, CD 2–4) · chase 20–35 · `max_per_round` 1.

**Orçamento por papel (o que o DWC pode esperar):**

| Papel | HP | Attack | Defense | Speed | Mystery típica | Chase |
|---|---|---|---|---|---|---|
| striker | 900–950 | 108–115 | 36–40 | 58–70 | 115–130 dmg | 30–35 |
| guardian | 1100–1150 | 92–96 | 56–62 | 38–46 | escudo 80–100 (ou 90+provocar) | 25 |
| support | 960–1020 | 96–100 | 44–46 | 50–60 | cura 90–100 (ou +chakra) | 20 |
| controller | 880–900 | 100–104 | 38–40 | 54–59 | 80–85 + Derrubada (ou AoE 80) | 25 |

**Somas medidas por clã** (geradas por `_gerador_roster.py`): HP 3910 (storm) / 3940 (ember) / 3980 (verdant) / 3990 (tide); Attack 396–412; Speed 200 (verdant) → 235 (storm); soma de chase 100–105. O gradiente é intencional: **storm troca HP por velocidade; tide/verdant trocam attack por tank/utilidade** — o DWC calibra as diferenças residuais.

**Pontos de atenção para o balanceamento (para o agente da engine):**
1. **Escudos são o maior risco de stall** (lição v0.2 do motor: "Muralha 55/2t"): todos os escudos têm CD ≥2 e valor ≤100% do attack; o de Aro **cresce** (+25%/rodada) — se o DWC detectar stall em comps verdant, primeiro nerf é limitar a duração do escudo, não o valor.
2. **Cura % do attack + +chakra** (Amara) pode criar loop ofensivo; se explodir, o alvo do nerf é o +10 de chakra, não a cura.
3. **`random_2` (Tina) e `enemies_all` (Volta) têm variância/valor em área**: em comps de 4, Volta 80% × 4 alvos = 320% de dano bruto, mas diluído; a contrapartida é CD 4 e chakra 30 (o teto do contrato).
4. **Iniciativa** (Reia/Alísio) decide chains; se "age primeiro" for difícil de implementar no loader, tratar Aceleração como +X de velocidade (X sugerido: 15) e documentar a escolha no loader.
5. **Estados são determinísticos** (sem "chance de"): o design do TurnRush troca probabilidade por custo/cooldown garantidos. Não reintroduzir RNG sem passar pelo DWC.
6. **Perfuração** (Nereo −50% escudo, Reia −30% defesa) existe para punir stacking defensivo; manter os dois valores atrelados ao chase correspondente nos testes de confronto.
7. Toda passiva de dano contínuo (Queimadura/Veneno) está em texto e precisa de interpretação do loader: sugestão 10% do attack/rodada, 2 rodadas, sem stack — anotar como decisão v0.3.

## v0.7 — modo interativo (fluxo, tiers e interrupt)

Campos **opcionais** no contrato v2 (migração: ausência = default por papel):

- `"unlock_schedule": {"mystery_round": N}` — rodada em que a Mystery destrava
  no modo interativo. Defaults: striker **3**, guardian/support/controller **2**.
  A Skill (forma base da mystery) está disponível desde o início.
- `"interrupt": {"type": "stun"|"silence", "window": 1}` — reação usada na
  própria vez contra inimigo que AINDA vai agiar na rodada. Custa 35 de fluxo,
  CD 3. Presente nos **4 controllers** (identidade de controle); os demais não
  têm interrupt (default `None`).

Economia do modo interativo: fluxo teto 100, +10/rodada, padrão grátis e gera
+15; Skill 30 (CD 2), Mystery 60 (CD 3). Bandas de dano derivadas na engine:
Skill 120% (único/frente→100% multi) e Mystery 200% (único) / 100% (multi);
cura/escudo da Mystery ×1,65. DoT do modo interativo: 30% do ATK/rodada, 2
rodadas (o modo compacto v0.6 mantém 10% — fonte do Monte Carlo inalterada).

## v0.8 — kits com assinatura (alvo único + mecânicas 100% modeladas)

### Formas de alvo por papel (agora explícitas no JSON)
| Papel | Skill | Mystery | Identidade |
|---|---|---|---|
| striker (Ignia/Nereo/Reia) | `enemy_single` 120% | `enemy_single` 200% | foco/eliminação — linha de trás acessível |
| striker (Tina) | `random_2` | `random_2` | espalhar Veneno é a assinatura dela |
| controller | herda | frente/todos + estado | negação de turno por faixa |
| support | cura base | cura ×1,65 + utilidade | dissipar/regenerar/acelerar/energia |
| guardian | escudo base | escudo final (+provocação/reflexo) | âncora com mecânica própria |

- `skill_targets` (opcional, nível do personagem) permite forma própria da
  Skill; ausente = herda a mystery (retrocompatível).
- Unlock da Mystery: **R3 uniforme** (60⚡ não dá antes com +10/rodada —
  o R2 por papel era texto morto). A carta **Gatilho Rápido** antecipa p/
  R2 E dá -15⚡ de custo (banking real: 2 básicos guardados = 50⚡ → cast).
- Orson: mystery CD 2→3 — a Provocação cobre o resto da rodada N + a N+1;
  com CD 3 existe gap honesto (a provocação não é permanente).

### Assinaturas que ERAM só texto e agora têm efeito
Aro (escudo crescente, teto 2×ATK) · Volta (lentidão -5 SPD na próxima
ordem) · Ivo (aura: DoT no atacante = -10% dano) · Alísio (Aceleração +25
SPD) · Mel (Dissipação) · Yara (Regeneração 6%/2r) · Orson (Provocação +
Muro Incandescente) · redução "na linha de frente" condicional de verdade.
Reia: "age primeiro na abertura" REMOVIDA do texto (A-B intercalado não
permite; SPD +10% mantém a identidade). Lista `unmodeled` do roster: VAZIA
(teste contratual `unmodeled_fica_visivel`).

### De-duplicação de passivas (variedade > uniformidade)
Orson → Muro Incandescente (queima quem bate); Aro → reflexo 15% (sem
redução); Dalva mantém -15% frontal. Amara mantém cura focada 5% no mais
ferido; Yara vira cura em área 2% (todos). Três âncoras, três identidades;
dois suportes de sustain com formas distintas.
