# MECÂNICAS — Estudo do combate do Naruto Online

> Estudo (Task 8-b) do banco de dados de personagens do usuário (repo `sistema-dominus`, clonado em `/tmp/dominus/`) para alimentar o design de TurnRush.
> Dataset principal: `research/fresh_ninja_list_clean.json` — **255 ninjas estruturados** (grade, elemento, 5 slots de skill com custo/cooldown).
> Complementos: `research/ninja_data_fresh_0930/*.json` (343 árvores de detalhe em chinês), `f5_meta.json` (metas de comp), `f2p_builds.json` (builds F2P), `f6_pets.json` (pets/companheiros), `chase_mechanics.json` (buscas sobre perseguição), busca web 2026 (vocabulário atual de estados).
> Dataset curado gerado a partir deste estudo: **`ninjas.json`** (40 ninjas, 8 por elemento, traduzidos para pt-BR) — produzido por `_gerador_ninjas.py` (reproduzível).

---

## 1. O combate em resumo

- **4v4 em grade 3×3** (9 casas): cada lado posiciona 4 ninjas (1 protagonista + 3) nas 9 casas; há posições de **frente (linha 1)** e **trás (linha 2)**, e as habilidades citam linha/coluna/cruz do esquadrão ("对敌小队[3]体" = 3 corpos do esquadrão, "选定目标的小队十字" = cruz no esquadrão do alvo).
- **Iniciativa por posição de mão** (1手 → 2手 → 3手 → 4手): a fonte de meta (f5_meta) mostra comps descritas por ordem de mão: "【火主】1手：佐助/手鞠；2手：火主；3手：鹿丸；4手：勘九郎/静音". Ninjas têm passivas de **iniciativa** (ex.: Minato — dano escala com vantagem de iniciativa) e de **posição** (ex.: Shikamaru — bônus para quem NÃO está na sua linha/coluna; Chôji — defesa por aliado atrás dele).
- **Rodadas**: dentro da rodada, cada ninja age (Oitiva opcional → ataque padrão → perseguições disparam em cadeia). Suportes e controladores podem **substituir o ataque padrão** por cura/dissipação/buff (ex.: Kabuto, Chiyo, Shikamaru — só 1% do dataset faz isso, é assinatura de suporte).
- **Chakra**: pool compartilhado do time. Custos das Oitivas: 20 / 40 / 60 / 80. Habilidades manipulam o pool: gerar chakra (查克拉汲取 +20/rodada), roubá-lo (查克拉吸收, 查克拉暴食), baratear Oitivas (-20, Konan), punir quem tem chakra (Kisame, Shino). O chakra **não gasto** pode virar recurso: Asuma e Tenten convertem chakra restante em dano/redução de cooldown.
- **Interruptor de Oitiva (打断)**: 20% das mysteries **interrompem** a Oitiva inimiga em carregamento — o "counter" natural de burst; explica por que controladores existem.

## 2. Os 5 slots de habilidade (schema do dataset)

| Slot | Chinês | Papel | Padrão numérico (255 ninjas) |
|---|---|---|---|
| skill_1 | 奥义 **Oitiva/Mystery** | habilidade ativa, gasta chakra, tem cooldown | custo **20 (71%) / 40 (21%) / 60 (2%) / 80 (2%)**; CD **2 (47%) / 3 (47%) / 1 ou 4 (5%)**; dano em modos **70 / 80 / 100 / 120%**; **99%** traz bônus de **Golpe Final (必杀)** (+20–40% dano ou efeito extra) |
| skill_2 | 普攻 **Ataque padrão** | golpe básico por rodada | **70% de dano no inimigo à frente (64% do dataset)**; aplica estado "com certa probabilidade (一定概率)" |
| skill_3 | 追打 **Perseguição (chase)** | reage a um estado e encadeia outro | **25% de dano (71% do dataset)**; 1x/rodada por padrão, **11% podem disparar 2x/rodada** |
| skill_4 | **Passiva** | permanente/condicional | 29% usam crescimento (成长) contínuo; 13% escudo; 22% imunidade |
| skill_5 | **Passiva de chakra** | economia do pool | +20/rodada, roubo, desconto, conversão de chakra não gasto |

**41% das mysteries têm cooldown inicial (进场冷却 1–2 rodadas)**: ninjas caros/fortes não abrem a partida já com a Oitiva — dá ritmo de jogo (rodada 1 = posicionamento + padrão, rodada 2–3 = primeiras Oitivas).

## 3. Estados e cadeias de perseguição

Normalização adotada no projeto (usada em `ninjas.json`):

| Chinês | pt-BR | Papel na cadeia |
|---|---|---|
| 小浮空 | **Arremesso Leve** | início curto da cadeia |
| 大浮空 | **Arremesso** | continuação forte |
| 击退 | **Empurrão** | desloca/reposiciona |
| 击倒 | **Derrubada** | fim típico da cadeia |

- A cadeia clássica: **padrão/Oitiva aplica estado → qualquer aliado cuja perseguição TEM GATILHO naquele estado dispara automaticamente → aplica outro estado → encadeia a próxima perseguição**, tudo dentro da rodada, cada ninja no máximo 1 vez (ou 2, em 11% dos casos). Confirmado por fontes: "浮空、击退、击倒等等连击效果…忍者的多样性，将为玩家们提供不同的连招效果" (9game, via `chase_mechanics.json`); wiki de comp atual: "鸣人普攻起手, 小浮空接风主追打" (游侠手游, 2026); tabela oficial de gatilhos de chase inclui 小浮空/大浮空/击退/倒地/定身/高连击 (simulador de combos, busca web 2026).
- **Gatilhos por contagem de combo** (sem estado): 24 chases disparam a **10连击**, 8 a 5连击, 3 a 20连击 (1 a 15, 1 a 30). Perseguições de "dano puro" ficam nas pontas da cadeia.
- **Gatilhos por condição/tag**: 5 chases disparam contra/via tag 木叶 (Konoha), 3 via 晓 (Akatsuki); 2 exigem "≥N perseguições já disparadas na rodada" (Itachi, Akamaru) — recompensam times que encadeiam.
- **Estados não-motores** (efeito em vez de encadeio): 定身 **Imobilização** (9% das mysteries), 点燃 **Queimadura** (9%), 中毒 **Veneno** (2%), 麻痹 Paralisia, 封穴 **Seladura de Chakra** (bloqueia Oitiva), 目盲 Cegueira, 混乱 Confusão (amigo-atira-em-amigo), 减速 Lentidão, 虚弱 Fraqueza, Marca (+30% dano recebido), 融解 Dissolução.
- **Imunidade/defesa**: 免疫 em 22% das passivas (imunidade total ou parcial a estados, p.ex. Gaara com escudo; A imune exceto Seladura); 反弹 (refletir estado) em 3%; esquivas 1x/rodada em 15%; barreiras de campo (结界, 9%) dão bônus de dano por elemento por 2 rodadas; pets completam estados que faltam no time ("缺倒地选小蛞蝓，缺击退选蓝蛇" — f6_pets).

## 4. Estatísticas do dataset (255 ninjas)

**Elemento (property)**: Água 56 · Fogo 54 · Vento 53 · Raio 47 · Terra 45 (bem distribuído — o jogo é desenhado em volta dos 5 elementos, com bônus de barreira por elemento).
**Grade**: S 104 (41%) · A 63 · SS 63 · B 20 — o power creep é gerenciado por grade, mas os padrões numéricos (70/25/20-chakra) são constantes entre grades; o que muda é a **largura** dos efeitos (alvos, condições, sinergias).

**Custo de chakra da Oitiva**: 20×182 (71%) · 40×53 (21%) · 60×6 · 80×2 · 0×10 (custo 0 quase sempre paga em HP, ex. Rock Lee).
**Cooldown**: 2×120 · 3×119 · 1×6 · 4×7 → **94% entre 2 e 3 rodadas**.
**Dano da Oitiva**: modos 80 (48), 100 (42), 120 (23), 70 (23) — janela típica **70–120%**; extremos (160–200%) vêm com custo alto (60–80 chakra) ou pagamento em vida.
**Dano do ataque padrão**: **70% em 64%** dos ninjas; 60% (20); 50%/40% para suportes/invocadores.
**Dano da perseguição**: **25% em 71%**; 20% (43); 30% (13).
**Alvos da Oitiva**: alvo único escolhido 77 · esquadrão inteiro ~81 · 3 alvos 18 · 4 alvos 15 · N escalado com Golpe Final ("+6 alvos", "+3 alvos").
**Gatilhos de perseguição (dataset todo)**: Derrubada 61 · Arremesso Leve 55 · Empurrão 39 · Arremesso 30 · combo ≥10 37 · condição/tag 10.

**Corpus curado (`ninjas.json`, 40 ninjas)**: 8 por elemento; grades B5/A11/S13/SS11; chakra 0×1, 20×25, 40×10, 60×3, 80×1; CD 1×1, 2×16, 3×22, 4×1; gatilhos Arremesso Leve 20, Arremesso 10, Empurrão 6, especial/combo 4; aplica Empurrão 11, Derrubada 10, Arremesso Leve 9, Arremesso 6.

## 5. Padrões de design de habilidade (o "kit de ferramentas" do jogo)

1. **Âncoras fixas + variação no "largura"**: 70% padrão, 25% chase, 20 chakra, CD 2–3 são âncoras universais. Variam: nº de alvos, probabilidade, condições, tags.
2. **Probabilidade em vez de garantia nos padrões** ("一定概率造成[小浮空]" — chance de aplicar o estado): o atacante *convida* a cadeia, não a garante; a garantia vem de Oitivas/passivas fortes.
3. **Golpe Final (必杀) em 99% das mysteries**: um "segundo andar" (+20–40% dano, +alvos, efeitos especiais) atrelado a um medidor separado — drena recursos de quem spammeia.
4. **Conversão de recursos**: chakra restante → dano/CD (Asuma, Tenten); vida → efeitos (Lee, Kimimaro, Itachi); derrotas → crescimento (Naruto, Orochimaru); marionetes destruídas → crescimento (Chiyo).
5. **Sinergia por tag e por formação**: mulheres no time (Naruto de terno), Akatsuki (Konan), Konoha (Shikamaru), Bijuu (Bee), linha/coluna (Shikamaru, Chôji), posição 1手 (Sai prioriza o "1手" inimigo).
6. **Extra attacks (额外普攻) em 25% dos ninjas**: economia de ação como recompensa — 2 padrões/rodada (Zabuza), ataques por rodada pares (A), ataques do time (Chiyo, Ônoki, Darui).
7. **Suporte substitui o ataque padrão** (cura/dissipa/buff): Kabuto, Chiyo, Shikamaru, Ino — a "ação livre" do slot 2 é o espaço de utilidade.
8. **Defesa ativa**: contra-ataque com esquiva (Hinata/Neji 回天, Pain 神罗天征), refletir debuffs (Itachi, Kurenai), provoco/redirecionamento de Oitiva (Anko, Kiba).
9. **Escalada controlada**: growth %/rodada com cap ("até 2x", "até 3 acúmulos", "8 ataques → libera Bijuu") — nunca crescimento sem teto.
10. **Counter-play de economia**: aumentar custo de chakra do inimigo (Kisame +20), impedir cura de chakra (Yamato), zerar o pool (Shino) — pressão econômica é um eixo de design.

## 6. Lições para TurnRush

1. **Estados como enum fechado** (arremesso_leve/arremesso/empurrao/derrubada) + gatilho/aplica por skill: é o que faz cadeias serem emergentes e legíveis — adotado 1:1 no roster v2 (contrato `launch|knockback|knockdown`).
2. **Chase 1x/rodada por personagem** (com raros 2x): evita o problema já resolvido no motor v1 (chase alimentado por 2 lançadores toda rodada → 60% do efeito).
3. **Âncoras numéricas**: padrão 60–80, chase 20–35, mystery 80–130, chakra 15–40, CD 2–4 — tradução direta dos âncoras 70/25/20/2–3 do jogo original, escalada para rodadas mais curtas do TurnRush.
4. **Probabilidade → determinismo**: TurnRush é determinístico; os "chance de X" do Naruto Online viram **estados garantidos com custo/cooldown maior** (ou chance explicitada como fração por-mille decidida pelo loader).
5. **CD inicial (进场冷却)**: Oitivas fortes começam indisponíveis 1 rodada — mecanismo barato contra "abertura de burst" e ótimo para o DWC variar.
6. **Golpe Final/medidor**: não copiar agora (anti-complexidade), mas as mysteries "condicionais" (ex.: dano dobrado sob 20% de HP, do A) mostram como dar teto alto sem spam.
7. **Pressão econômica** (roubo/desconto/punição de chakra) é o terceiro eixo de design depois de dano/estado — cabe no suporte/controlador de cada facção.
8. **Sinergia por facção/tag** (Akatsuki/Konoha/mulheres) vira **facções do roster** (ember/tide/verdant/storm) com cadeias garantidas por facção.
9. **Defesa ativa > escudo estático**: o original resolve stall com imunidade 1x/rodada e contra-ataque; no TurnRush, escudos têm CD 2 e teto (lição v0.2: "Muralha 55/2t" virava stall infinito).
10. **Pets completam cadeias** (f6_pets): no futuro, uma **carta utilitária de draft** que fornece o estado que falta no time — herda esse padrão sem nova mecânica de campo.

## 7. Fontes

- Locais: `research/fresh_ninja_list_clean.json` (255 ninjas), `research/ninja_data_fresh_0930/` (343 árvores de detalhe: datas de lançamento, sexo, avaliações, atributos), `research/chase_mechanics.json`, `research/f5_meta.json`, `research/f2p_builds.json`, `research/f6_pets.json`, `research/f7_talents.json`.
- Web (2026): simulador de comp do Naruto OL (lista oficial de gatilhos de chase: 小浮空/大浮空/击退/倒地/定身/高连击); 游侠手游 — "普攻起手，小浮空接风主追打" (exemplo de cadeia em comp inicial); 52PK (2014) — diagrama 奥义→追打 com 大小浮空/击退/击倒/高连击; 9game — "浮空、击退、击倒等等连击效果" (via `chase_mechanics.json`); f5_meta — comps por ordem de mão 1手–4手 (youxiao/jb51/9game); f6_pets — pets fornecem estados faltantes (9game) e escudos empilháveis em PvE (taptap).
