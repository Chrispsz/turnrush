# NOTAS DE DESIGN — Roster v2 do TurnRush

> IP original, 4 facções × 4 papéis (16 personagens). Inspirado nos padrões de habilidade do Naruto Online documentados em `../naruto-online/MECANICAS.md` — sem nomes, imagens ou textos da franquia.
> Contrato de engine: `roster-v2.json` (schema v2). Interpretações contratadas para o loader:
> - `targets: ally_single` + `mystery.damage_pct` = **% do ATTACK** aplicado como **CURA** (support) ou **ESCUDO** (guardian). `applies_state` só tem efeito em alvos inimigos.
> - `enemies_front` = inimigos da linha de frente; `enemies_all` = todos; `random_2` = 2 aleatórios.
> - Estados: `launch` (arremesso) → `knockback` (empurrão) → `knockdown` (derrubada); `none`. No dataset do Naruto Online, 小浮空 (arremesso leve) colapsa para `launch`.

## 1. Mapa facção → arquétipo

| Facção | Arquétipo | Ideia de jogo | Herança Naruto Online |
|---|---|---|---|
| **ember** | Agressão / pressão | Dano concentrado + Queimadura; cadeias rápidas de arremesso | 点燃 (Queimadura em 9% das mysteries), Oitivas 100–120% (modos do dataset), suporte que gera chakra (padrão 查克拉给予) |
| **tide** | Controle / sustentação | Cura, escudo com reflexo, prisão/derrubada para negar turnos | Kits de Água: cura do time 水主, Dissolução/prisão, punição de economia (Kisame: custo de chakra +20) |
| **verdant** | Desgaste / crescimento | Veneno, escudo que cresce, marionete que trava; vence partidas longas | Veneno (志乃), escudo empilhável (Gaara/千代 — f6_pets cita "叠加护盾"), marionetes (勘九郎/蝎), regen (百豪) |
| **storm** | Velocidade / combo | Iniciativa, AoE de arremesso, contra-ataque; mais papeis agressivos, menos HP | Iniciativa (Minato, 雷主 meta), contra-ataque por esquiva (Hinata/Neji 回天, Pain), AoE de Vento com arremesso leve (手鞠) |

## 2. Os 16 personagens (assinatura + inspiração)

| ID | Nome | Facção | Papel | Assinatura mecânica (única) | Origem (padrão do Naruto Online) |
|---|---|---|---|---|---|
| `ignia` | Ígnia Brasa-Viva | ember | striker | Mystery 120% + Queimadura por 2 rodadas; +15% de dano contra Queimados | Oitiva 100–120% + 点燃 (ex.: Jiraiya 100% + Queimadura) |
| `orson` | Orson Muralha-Fundida | ember | guardian | Escudo + **Provocação** na frente; padrão com Derrubada | Escudo 傀儡·乌鸦 (Kankurô) + guardian que derruba invasores |
| `amara` | Amara Chama-Doce | ember | support | Cura que **gera +10 de chakra** para a equipe | 查克拉汲取/查克拉恢复 (econômica de pool) |
| `nimbo` | Nimbo Véu-de-Fumaça | ember | controller | Mystery com Derrubada em frente (adiando fila) + chase knockback→knockdown | 定身/打断 em área frontal (controle de turno) |
| `nereo` | Nereo Lâmina-de-Sal | tide | striker | Mystery que **ignora 50% do escudo**; +25% de dano contra escudados | 水遁 de Água + anti-escudo (punição estilo Kisame/由木人) |
| `dalva` | Dalva Recife | tide | guardian | Escudo que **reflete 20%** do dano absorvido | 护盾 + 反弹 (refletir efeito, Itachi/Kurenai) |
| `mel` | Mel Maré-Mãe | tide | support | Cura + **dissipação**; passiva +8 chakra/rodada | 掌仙术 (Kabuto: cura + dissipação) + pool 水主 |
| `ligeia` | Ligeia Névoa-Fria | tide | controller | Prisão de Vapor (Derrubada em frente); +15% de dano em Derrubados | 水牢术/雾隐 (Zabuza) + bônus contra estado próprio |
| `tina` | Tina Espinho-Gêmeo | verdant | striker | Mystery `random_2` + Veneno; ataques aplicam Veneno (desgaste) | 志乃/勘九郎: Veneno como pressão contínua |
| `aro` | Aro Cascão | verdant | guardian | Escudo que **cresce +25%/rodada** enquanto existir | 砂之守护 (Gaara) — escudo como identidade, com cap por duração |
| `yara` | Yara Seiva-Viva | verdant | support | Cura + **regeneração 6%/rodada por 2 rodadas** | 治活再生 (Tsunade/百豪) |
| `ivo` | Ívo Corda-Fina | verdant | controller | Marionete (Derrubada em frente); -10% de dano de inimigos com estado da equipe | 傀儡师 (Chiyo/Kankurô) + sinergia por estado |
| `reia` | Reia Passo-Trovão | storm | striker | Mystery 130% **ignora 30% de defesa**; padrão que arremessa; age primeiro | 艾 (dano concentrado) + iniciativa de Minato |
| `aki` | Aki Contra-Corte | storm | guardian | Escudo próprio com **contra-ataque** enquanto durar; padrão com Derrubada | 回天 (Hinata/Neji) — defesa que devolve dano |
| `alisio` | Alísio Sopro-Ligeiro | storm | support | Cura + **Aceleração** (alvo age antes dos inimigos na próxima rodada) | 手鞠/风主: manipulação de iniciativa; chase por rodada gera chakra |
| `volta` | Volta Ciclone | storm | controller | **AoE em todos** (80%) com Arremesso; inimigos arremessados perdem 5 de velocidade | 旋风轰壁 (手鞠: AoE + arremesso leve) + debuff de iniciativa |

## 3. Cadeias garantidas por facção (validadas por script)

Padrão por facção: **striker/suporte aplicam Arremesso → guardian/controlador perseguem (Empurrão) → controlador/suporte fecha com Derrubada**.

| Facção | Pares arremesso→perseguição | Pares derrubada→perseguição |
|---|---|---|
| ember | Ígnia (mystery) → Orson; Amara (padrão) → Orson | Nimbo (mystery) → Amara; Orson (padrão) → Amara |
| tide | Nereo (mystery) → Dalva; Mel (padrão) → Dalva | Ligeia (mystery) → Mel; Dalva (padrão) → Mel |
| verdant | Tina (mystery) → Aro; Yara (padrão) → Aro | Ívo (mystery) → Yara; Aro (padrão) → Yara |
| storm | Volta (mystery AoE) → Reia **e** Aki; Reia (mystery e padrão) → Aki | Aki (padrão) → Alísio |

Cada chase: `max_per_round: 1` (contrato), replicando a regra do Naruto Online (1x/rodada; 11% têm 2x — não adotado por ora).

## 4. Orçamento de balanceamento

**Ranges do contrato (obrigatórios):** HP 850–1150 · standard 60–80 · mystery 80–130 (chakra 15–40, CD 2–4) · chase 20–35 · `max_per_round` 1.

**Orçamento por papel (o que o DWC pode esperar):**

| Papel | HP | Attack | Defense | Speed | Mystery típica | Chase |
|---|---|---|---|---|---|---|
| striker | 900–950 | 108–115 | 36–40 | 58–70 | 115–130 dmg | 30–35 |
| guardian | 1100–1150 | 92–96 | 56–62 | 38–46 | escudo 80–100 (ou 90+provocar) | 25 |
| support | 960–1020 | 96–100 | 44–46 | 50–60 | cura 90–100 (ou +chakra) | 20 |
| controller | 880–900 | 100–104 | 38–40 | 54–59 | 80–85 + Derrubada (ou AoE 80) | 25 |

**Somas medidas por facção** (geradas por `_gerador_roster.py`): HP 3910 (storm) / 3940 (ember) / 3980 (verdant) / 3990 (tide); Attack 396–412; Speed 200 (verdant) → 235 (storm); soma de chase 100–105. O gradiente é intencional: **storm troca HP por velocidade; tide/verdant trocam attack por tank/utilidade** — o DWC v2 calibra as diferenças residuais.

**Pontos de atenção para o balanceamento (para o agente da engine):**
1. **Escudos são o maior risco de stall** (lição v0.2 do motor: "Muralha 55/2t"): todos os escudos têm CD ≥2 e valor ≤100% do attack; o de Aro **cresce** (+25%/rodada) — se o DWC detectar stall em comps verdant, primeiro nerf é limitar a duração do escudo, não o valor.
2. **Cura % do attack + +chakra** (Amara) pode criar loop ofensivo; se explodir, o alvo do nerf é o +10 de chakra, não a cura.
3. **`random_2` (Tina) e `enemies_all` (Volta) têm variância/valor em área**: em comps de 4, Volta 80% × 4 alvos = 320% de dano bruto, mas diluído; a contrapartida é CD 4 e chakra 30 (o teto do contrato).
4. **Iniciativa** (Reia/Alísio) decide chains; se "age primeiro" for difícil de implementar no loader, tratar Aceleração como +X de velocidade (X sugerido: 15) e documentar a escolha no loader.
5. **Estados são determinísticos** (sem "chance de"): o Original usa probabilidade; aqui o valor vem do custo/cooldown. Não reintroduzir RNG sem passar pelo DWC.
6. **Perfuração** (Nereo −50% escudo, Reia −30% defesa) existe para punir stacking defensivo; manter os dois valores atrelados ao chase correspondente nos testes de confronto.
7. Toda passiva de dano contínuo (Queimadura/Veneno) está em texto e precisa de interpretação do loader: sugestão 10% do attack/rodada, 2 rodadas, sem stack — anotar como decisão v0.3.
