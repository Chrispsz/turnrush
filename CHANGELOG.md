# Changelog — TurnRush

Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/);
versões seguem [SemVer](https://semver.org/lang/pt-BR/). O jogo está em
**fase de mecânica**: arte/sprites/nomes finais são esboços conscientes.

## [0.9.1] — playtest de áudio: combo automático, fim de batalha garantido e UI limpa

> Fonte: playtest em áudio (2026-10-20, `gamereview-POLIDA.wav`) + parte
> escrita. Relatos tratados como bugs/requisitos: "joguei duas batalhas, uma
> perdi e outra ganhei e ele não finaliza a batalha após vencer"; "quando vai
> fazer um combo ele pergunta… prefiro que o personagem faça o combo
> automaticamente, até porque combo não é para gastar energia"; "pode ser
> limpo esse texto que fica aqui embaixo de rodada"; "esse botão de
> habilidade pode ser menos confuso"; "o como jogar pode até ser retirado do
> jogo"; "o kit está bem cortado".

### Corrigido
- **Fim de batalha travado ("Vitórias 0/3" eterno)** — causa raiz: a janela
  de chase manual (ENTER/ESC) pausava o jogo e era a única decisão que o
  painel não resolvia; no golpe final ela podia ficar pendurada para sempre.
  A janela manual **deixou de existir**: o combo agora é automático (abaixo) e
  a peça 11 do quebra-cabeça garante que toda batalha converge para `Over`
  (vitória, derrota ou teto de rodadas) — 16 seeds varridas + varredura de
  janela "sempre tem elegível ou fecha sozinha".
- **CI que nunca disparava em push para main** — `build.yml` tinha
  `branches: ain]` (YAML válido, filtro sem sentido; os runs vinham só de
  cron/dispatch/tag). Corrigido para `branches: [main]`.
- **Kit cortado no elenco** — as cartas da grade mostravam o kit inteiro em
  150px com `clip` (texto sumia). Agora a carta é RESUMO (elemento, clã,
  classe, stats, ⚡ da Mystery) e **o clique abre a página do personagem**
  com o kit COMPLETO (Padrão/Mystery/Chase/Passiva, sem corte).

### Adicionado
- **Combo automático (v0.9.1)** — a perseguição resolve sozinha dos DOIS
  lados: quem tem gatilho compatível persegue o 1º elegível; a chance de
  acerto continua na engine; **custa 0 ⚡** (nunca custou — a pausa é que
  parecia cobrar). Feedback visual: faixa "COMBO! A → B" + destaque dourado
  nos perseguidores (0,45–1,2 s), sem pausa, sem ENTER/ESC.
- **Peça 11 do quebra-cabeça** (`tests/p11_finalizacao.rs`, 6 testes) +
  peça 10b (unit tests do binário — política do controlador manual).
  `puzzle-gate.sh` agora roda **16 peças**; CI e comentários atualizados.
- **docs/COMO-JOGAR.md** — o conteúdo da tela "Como Jogar" (que saiu do
  jogo) virou documento, atualizado com elementos (v0.9) e combo automático.

### Alterado
- **Log de batalha enxuto** (pedido direto do áudio): a linha "— Rodada N —"
  sumiu (o header já mostra); caído não repete "vez passada" a cada rodada
  (só o ✖ do momento); janela de perseguição/recusa automática ficaram
  silenciosas; combo no log virou `⛓ combo: A → B (N)`. Teto de 8 linhas.
- **Botões de habilidade menos confusos**: sumiram os prefixos
  `Skill «…»`/`Mystery «…»` — os botões mostram o NOME do poder + forma de
  alvo + custo ⚡ (ex.: `[2] Chama Residual · 1 alvo (livre) · ⚡30`).
- **"Como Jogar" retirado do jogo** (entrada e tela removidas; o jogo
  ensina pela prévia exata e pelos textos de botão — o guia completo está
  em `docs/COMO-JOGAR.md` para voltar num menu de opções no 1.0).
- `_gerador_roster.py` movido de `data/roster/` para `scripts/` (dados e
  ferramentas separados).

### Balanceamento
- Intacto por construção: a política de chase automático escolhe o MESMO
  alvo que a IA já escolhia (1º elegível) e a chance é da engine — os gates
  confirmam: 1v1 desvio **0.1191** (≤ 0.15) ✓; 4v4 desvio **0.0469**,
  espelho **0.5063** ([0.42, 0.58]) ✓.

## [0.9.0] — elementos e cadeias de combo: fogo, água, elétrico e pedra

> Pedido: "vamos trabalhar com elementos nos personagens, sendo elétrico,
> fogo, água, pedra, priorizando os bombos com mesmo elemento, passivas de
> buff que podem combinar com outro elemento desde que forme uma cadeia de
> combos… com elemento X tendo fraqueza com elemento Y". Modelagem estilo
> coleção de criaturas (fraqueza explícita) sobre a arena já existente:
> tudo continua automático exceto as decisões do jogador.

### Adicionado
- **4 elementos de combate**: `fogo | agua | eletrico | pedra` (campo
  `element` no contrato JSON; ausente → deriva do clã: ember→fogo,
  tide→água, storm→elétrico, verdant→pedra). Todo golpe ofensivo
  (padrão/Skill/Mystery/chase) carrega o elemento de quem age.
- **Ciclo de fraquezas** (cada elemento vence EXATAMENTE 1 e perde para
  EXATAMENTE 1; diagonal neutra): Água>Fogo, Elétrico>Água, Pedra>Elétrico,
  Fogo>Pedra. Vantagem **×1200‰** de dano por alvo; desvantagem **×800‰**.
  Matemática inteira (per-mille), determinística — `preview == execução`
  garantido (mesmos helpers nas duas pontas).
- **Ressonância elemental** (mesmo elemento do golpe ofensivo anterior do
  time): **+15% de dano e +5⚡** para quem age — o "bombo de mesmo elemento"
  e o acelerador que liga elementos à economia de fluxo (teto 100 mantido).
  Visível na PRÉVIA da ação (nota + fluxo prometido) e no log
  (`✦ Ressonância elemental de Água: Nereo (+15% de dano)`).
- **Cadeia elemental** (elemento na progressão do ciclo do golpe anterior —
  elemento DIFERENTE): **+10% de dano**; atravessa virada de rodada
  (wrapthrough). Cura/escudo/Interrupt não tocam a cadeia; DoT não é ação.
- **Passiva de sintonia (Alísio)**: "quando age em cadeia elemental (elemento
  diferente…), o golpe causa 20% a mais de dano nessa ação" — o protótipo de
  passiva que combina com OUTRO elemento formando cadeia (+30% no golpe:
  10% da cadeia + 20% da sintonia). Modelada nos DOIS motores e testada com
  número exato; NÃO dispara em Ressonância (é bônus de cadeia).
- **Eventos finos novos**: `ElementalCombo` (elo fechado, com bônus) e
  `ElementalHit` (vantagem no alvo — só quando ≠ 0; informação simétrica).
  Cliente: linha no log de batalha + texto flutuante âmbar
  Ressonância/Cadeia; glifo `[Fg|Ag|El|Pd]` nos cartões de alvo/aliado e no
  painel da vez.
- **Peça 9 do quebra-cabeça** (`tests/p09_elements.rs`, 12 testes): ciclo
  fechado, contrato JSON (explícito/derivado/erro claro), vantagem com
  matemática exata NOS DOIS motores, Ressonância (dano+fluxo+prévia),
  Cadeia com sintonia e wrapthrough, sintonia fora da Ressonância, cura
  neutra, determinismo/simetria com elementos, snapshot expõe elemento.
  Suíte agora tem **10 peças**; cliente E2E virou Peça 10
  (`p10_client_e2e.rs`); `puzzle-gate.sh` e CI atualizados.
- Roster v2 ganha `element` explícito nos 16 personagens (fonte da verdade
  da deriva por clã; teste contratual garante 4 elementos × 4 papéis).

### Alterado
- **Economia**: a Ressonância adiciona +5⚡ por golpe de mesmo elemento em
  sequência — as asserções exatas de banking das peças 3/4/8 foram
  atualizadas com o novo contrato (o resto da economia intacto:
  +10/+15, custos 30/60/35, CDs, teto 100, teto passivo +20).

### Balanceamento
- Gate 1v1: desvio médio **0.119** (≤ 0.15) ✓.
- Gate 4v4: desvio médio **0.047** (≤ 0.15), espelho 0.506 ([0.42, 0.58]) ✓ —
  elementos são simétricos (os dois lados têm) e o espelho não se move.
- Suíte: **112 testes no core** (102 inline + 10 peças = 110 integração) e
  `--check` verde com eventos elementais no fluxo.

## [0.8.1] — suíte quebra-cabeça: testes em cadeia que cobrem tudo

> Pedido: "crie um teste completo para testar tudo desse projeto e em
> cadeia como um quebra-cabeça, que cubra de tudo, tanto da qualidade do
> código quanto uso e etc". Executor mestre: `scripts/puzzle-gate.sh`.

### Adicionado
- **Suíte em cadeia de 9 peças** (`crates/*/tests/p01…p09`): dados →
  determinismo → economia → mira → passivas → kits → corrente de batalha →
  run → cliente E2E. Cada peça reasserta a âncora da anterior e usa só a
  API pública da engine. **89 testes novos** (188 no core + game E2E).
- **`scripts/puzzle-gate.sh`**: executor mestre com ✔/✘ por peça — clippy
  `-D warnings`, 9 peças, testes inline e os gates de Monte Carlo (peça 10).
- **Job de CI `puzzle-gate`** (`build.yml`): roda a cadeia inteira a cada push.
- **`docs/TESTING.md`**: mapa do quebra-cabeça, regras da casa (API pública,
  rosters sintéticos via `CharSpec`, determinismo) e os bugs que a suíte pegou.

### Corrigido
- **🐛 Interrupt morria depois da rodada 1** (achado da Peça 3): `open_round`
  resetava o campo `u.acted` da unidade, mas NUNCA o array `sides[si].acted`
  que a engine usa de verdade — `interrupt_targets` ficava vazio da rodada 2
  em diante e o Interrupt (35⚡/CD3) era inutilizável exatamente quando a
  economia permite pagá-lo. Correção: reset do array na abertura de rodada.

### Verificado (auditado pela suíte, sem mudança de código)
- **Mira**: padrão espelho 1↔1 com fallback ao primeiro vivo; alvo explícito
  vale para qualquer vivo (traseira inclusa); `enemy_single` confirma acesso
  à linha de trás; provocação desvia o automático; prévia = execução.
- **Economia**: +10/+15/30/60/35, CDs 2/3/3, teto 100, teto da passiva de
  chakra (+20), erros não consomem a vez, desconto do Gatilho Rápido chega
  na engine (`mystery_cost`).
- **Passivas**: 13 famílias testadas comportamentalmente (reflexo com número
  exato, curas EoR, DoT, bônus condicional, escudo crescente, aceleração na
  ordem…). `unmodeled` vazio em todas as composições do roster real.
- **Kits**: orçamento por papel respeitado; anti-faz-tudo confirmado
  (guardião = âncora de HP, striker = lâmina, suporte não compete, controlador
  paga com vida); cadeias de combo fecham nos 4 clãs.
- **Run**: carta → `SideEdge` → batalha muda de fato (Fluxo Estável +2,
  Gatilho Rápido R3→R2 e 60→45, Último Suspiro revive 1×, Ambidestria permite
  2ª perseguição com estado pendente, Eco de Combate bônusa do 2º elo em
  diante); reroll é abastecido pela camada de run (1/batalha).

## [0.8.0] — kits com assinatura: alvo único, mecânicas reais e variedade

> Auditoria de gameplay + passe de design inspirado em Into the Breach
> (informação completa), Darkest Dungeon (posições que importam) e
> Wildfrost/Slay the Spire (economia com "banking"). Princípio: cada
> personagem é um ESPECIALISTA complementar — nada de faz-tudo.

### Adicionado
- **Alvo único (`enemy_single`) no contrato**: o jogador mira QUALQUER
  inimigo vivo — linha de trás inclusive. Skill e Mystery dos **strikers**
  (Ignia, Nereo, Reia) viraram single-target: Mystery na banda 200% (foco/
  eliminação), Skill 120%. Tina mantém `random_2` (espalhar veneno é a
  assinatura dela). O modo compacto resolve single como "primeiro da
  frente" (determinístico, fonte do Monte Carlo recalibrada).
- **`skill_targets` opcional no JSON v2**: forma de alvo própria da Skill
  (ausente = herda a mystery — retrocompatível).
- **Mystery inimiga com status público**: a fila de alvos mostra
  `M:R3` (trancada) / `M:CD2` / `M:pronta` — informação simétrica que
  afina interrupt e alvo único.
- **Forma e custo no botão**: `[3] Mystery «…» · 1 alvo · ⚡60` — a
  leitura da ação fica no botão (glanceability), não na memória.
- **Novo evento `Taunted`** no log: "⚔ Provocação: ataques automáticos
  miram X por N rodada(s)".

### Corrigido — assinaturas mecânicas que existiam só no texto
- **Aro**: escudo crescia +25%/rodada — AGORA cresce de verdade (teto 2×
  ATK, antistall da lição v0.2 "Muralha").
- **Volta**: arremessados perdiam 5 de velocidade "até o fim da rodada"
  (sem efeito) — AGORA a lentidão vale na ordem da PRÓXIMA rodada (SPD
  efetiva na timeline).
- **Ivo**: "-10% de dano de inimigos com estado" não modelado — AGORA é
  aura real: inimigos sob Veneno/Queimadura causam -10% (texto explícito).
- **Alísio**: Aceleração não modelada — AGORA o alvo da mystery ganha
  +25 de SPD na ordem da próxima rodada (age antes do oponente).
- **Mel**: Dissipação não modelada — AGORA a mystery limpa
  Veneno/Queimadura do alvo curado.
- **Yara**: Regeneração não modelada — AGORA o alvo regenera 6% do HP
  máximo por rodada durante 2 rodadas.
- **Orson**: Provocação não modelada — AGORA a mystery provoca: ataques
  automáticos inimigos miram Orson por 1 rodada (alvo MANUAL do jogador
  NUNCA é forçado — decidir é do jogador). CD 2→3 para existir janela de
  gap. Passiva nova: **Muro Incandescente** — quem bate com padrão em
  Orson pega Queimadura por 1 rodada (30% do ATK dele).
- **Dalva/Orson**: redução "na linha de frente" era SEMPRE ativa — AGORA
  é condicional de verdade (vale nos slots 0-1).
- **Reia**: removida a promessa "age primeiro na rodada de abertura"
  (a estrutura A-B intercalada não permite); +10% de SPD mantido.
- **Lista `unmodeled` do roster real: VAZIA** (teste contratual garante).

### Alterado — variedade (passivas de-duplicadas)
- **Orson** sai do clone "-15% de dano recebido" (virou muro de brasa);
  **Aro** fica só com reflexo + escudo crescente; **Dalva** mantém a
  redução frontal — três âncoras com identidades distintas.
- **Yara** sai do clone da Amara (5% no mais ferido) — agora TODOS os
  aliados curam 2% no fim da rodada; Amara mantém o foco.
- **Unlock da Mystery = R3 uniforme**: com +10⚡/rodada, 60⚡ só é
  alcançável na R3 — o "R2 por papel" era texto morto.
- **Gatilho Rápido** redesenhada: unlock 1 rodada antes (R3→R2) E
  Mystery custa 15⚡ menos (60→45) — guardar 2 rodadas de básico (50⚡)
  detona a Mystery na R2. A fantasia de "banking" existe de verdade.
- Bônus de cartas de unidade agora aplicam por slot (desconto/unlock
  são da unidade escolhida, não do time todo).

### Balanceamento
- Gate 4v4 recalibrado pós-mudanças: desvio médio **0.048** (≤ 0.15),
  espelho 0.513 ([0.42, 0.58]) — nenhum confronto extremo.
- 102 testes no core (+11: alvo único com preview==result, provocação,
  escudo crescente, lentidão, aura, aceleração, dissipação, regeneração,
  curas EOR distintas, muro de brasa, Mystery na R2); 109 no workspace.

## [0.7.1] — resposta ao playtest 2026-10-18: manual que funciona e roguelike no meio da luta

### Corrigido
- **Modo manual travava ao escolher skill + alvo (B1/B2 do playtest)**: o
  painel de comando era reenchido a cada frame e resetava a fila de alvos
  ~50 ms depois do clique — os botões de alvo piscavam 1 frame e os cliques
  morriam; a batalha ficava esperando uma entrada que nunca chegava (o
  "freeze" relatado era a mesma espera invisível). Correção: geração de
  decisão na engine (`InteractiveBattle::generation`) + chave
  (geração, janela) no painel — a mesma vez nunca é reenchida; recusas da
  engine agora aparecem no painel ("⚠ ação recusada") em vez de falharem
  em silêncio.

### Adicionado
- **Táticas de batalha a cada 2 rodadas (R1 do playtest)**: nas rodadas
  2/4/6/8 o jogo pausa e o jogador adota 1 de 2 táticas de escopo da
  batalha (Reforço de Linha +12% ATK · Pele de Pedra -8% dano recebido ·
  Fluxo Acelerado +4/rodada · Cura de Campo 3% no fim da rodada). O inimigo
  adota a dele **em silêncio** (mesma força, zero UI). Efeito só até o fim
  da batalha — não entra no teto de poder da run nem afeta o Monte Carlo.

### Alterado
- **Ataque padrão com alvo automático (U1 do playtest)**: mira o inimigo
  espelhado (1↔1, 2↔2…); espelhado caído, primeiro vivo da frente para
  trás. Dispara direto no clique, sem fila de alvos — seleção manual só
  para poderes focados (cura aliada/interrupt).
- **Log de batalha enxuto (U2 do playtest)**: só casts especiais
  (Mystery/Interrupt), mortes, estados, perseguições e separadores de
  rodada entram no log; curas/escudos ficam nos números flutuantes; teto
  de 8 linhas.
- **"Como Jogar" atualizado (U3)**: alvo automático do básico, janela de
  táticas e fluxo de decisão clicado descritos passo a passo.

## [0.7.0] — modo INTERATIVO completo: fluxo, tiers, interrupt, cartas e boss

### Adicionado
- **Engine interativa** (`interactive.rs`): máquina de estados determinística
  para controle manual total — `prompt`/`legal_actions`/`preview_action`/
  `execute_action`/`timeline`, eventos finos (`BattleEvent`) consumidos pelo
  cliente sem parse de texto. O modo compacto (`Formation4v4::run`) segue
  intacto como fonte do Monte Carlo.
- **Economia de Fluxo** (substitui chakra no modo interativo): teto 100,
  +10 por rodada, Padrão grátis e gera ⚡+15, **Skill** ⚡30 (CD 2), **Mystery**
  ⚡60 (CD 3, destrava R2/R3 por papel ou `unlock_schedule` do JSON),
  **Interrupt** ⚡35 (CD 3). Geração passiva com teto +20/rodada.
- **Tiers de habilidade derivados do roster**: Skill = forma base da mystery
  (120% alvo único / 100% multi, utilidade do contrato, sem estado); Mystery =
  forma final (200% único / 100% multi, cura/escudo ×1,65, estado + DoT +
  perfuração). Bandas de dano da spec respeitadas e testadas.
- **Interrupt**: atordoa (perde a ação) ou silencia (sem Skill/Mystery) um
  inimigo que AINDA vai agiar na rodada — janela validada pela engine, zero
  aviso na UI (a informação existe: timeline + ⚡ do inimigo, simétrico).
- **Perseguição manual (chase)**: a janela PAUSA o jogo, aliados elegíveis
  brilham dourado; [ENTER] persegue, [ESC] recusa (tática válida). Bônus de
  primeira perseguição do time (passivas Nimbo/Alísio) modelados.
- **Cartas roguelike** (`rewards.rs`): 12 cartas (Equipe/Unidade/Tática) com
  raridade 600/300/100, preferência de clã +10%, sem repetição na run, alvo
  vivo para cartas de unidade e **teto de poder +30%**. Gate Monte Carlo:
  run sortuda vs média ≤ 10 p.p. (medido: 1,0 p.p.).
- **Run v0.7**: 3 batalhas (a 3ª = **BOSS** escalado), 1 carta entre batalhas
  com reroll; tela de vitória reconstruída.
- **Teto de 10 rodadas** com desempate por % de HP total.
- **IA simétrica** (`greedy_decide` no core): decide apenas por
  `legal_actions` + `preview_action` — contrato de simetria testado
  (zero submissão ilegal em batalhas completas; espelho 25–75%).
- **Cliente Bevy v0.7**: controlador de batalha (`controller.rs`), painel de
  comando com prévia EXATA (dano/cura/escudo/estados/fluxo), barras de Fluxo
  nos 8 personagens (⚡ simétrico), timeline da rodada no cabeçalho, teclado
  completo ([1]-[4] ações, Q/W/E/R alvo inimigo, A/S/D/F aliado, ENTER, ESC,
  ESPAÇO 2×, Z Auto), `--playback` preserva o modo legado v0.6.

### Alterado
- `--check` também conduz uma batalha interativa completa headless.
- Tela "Como Jogar" reescrita com as regras v0.7.

## [0.6.0] — identidade própria + batalha MANUAL + partidas 2× mais rápidas

### Adicionado
- **Batalha manual (você comanda cada ação)**: o engine ganhou API por
  passos (`advance_one`/`submit`/`pending_decision`) — na vez de cada
  unidade sua, o painel inferior oferece a **Mystery** (custo de chakra
  visível) ou o **Padrão com alvo clicável** (escolha o inimigo vivo, ou
  Auto-alvo). Botão **Auto** entrega as decisões à IA quando quiser
  assistir. O adversário continua sendo a IA calibrada do engine.
- **Identidade original**: clãs **PYREBORN / DUSKMERE / THORNVALE /
  SKYREND**, classes **Reaver / Bastion / Mender / Tactician**, 16
  personagens renomeados (`docs/NAMING.md`). Zero referências a outros
  jogos em docs, código e UI; sprites procedurais por (clã × classe) e
  cena de batalha animada (lunge, flash, morte, números flutuantes,
  banners ROUND/CHAIN, screen shake, brasas).
- `AGENTS.md`/`CLAUDE.md` — guia para agentes de IA (invariantes, comandos,
  lições do Bevy 0.20, loop de verificação).

### Mudado (partidas mais rápidas e dinâmicas — pedido direto de playtest)
- **Teto de rodadas: 24 → 12** (desempate por % de vida total).
- **Chakra: 30 inicial (era 20) · +14/rodada (era +12)** — mysteries entram
  mais cedo.
- **Ritmo de dano global ×2.5 (`DAMAGE_PACE` 2500‰)** — aplicado
  simetricamente aos dois lados (DWC recalibra; winrates preservados).
  Medição Monte Carlo (300 batalhas): **22.4 rodadas médias → 7.9**, com
  **94% das partidas terminando por derrota real** (antes: guerra de
  atrito até o teto).
- Pacing do playback: passo 0.36s → 0.30s; rodada 0.95s → 0.80s.
- Como Jogar e menu atualizados ao novo modo manual e números.

### Técnico
- Engine: máquina de fases (`Phase`) equivalente ao loop compacto — teste
  de equivalência `run()` vs `advance_one` garante eventos idênticos no
  modo automático; escolhas inválidas caem defensivamente para o padrão
  (nunca trava); decisões manuais não sofrem jitter de timing (o jitter
  modela imperfeição da IA, não do humano).
- Cliente: `process_event` compartilhado entre playback e modo manual
  (mesma fidelidade visual); 48 testes no core (+4 novos) e clippy zero
  warnings nos 3 crates.

## [0.5.0] — mecânica + menu funcional + documentação completa

### Adicionado
- **Menu mais funcional**: telas **Como Jogar** (regras fiéis à engine:
  draft, turnos A-B-A-B, chakra, cadeias de combo, passivas, recompensas,
  escalada, facções) e **Elenco** (as 16 cartas com kit completo — mystery,
  padrão, chase, passiva, stats), com botão Voltar.
- **Botão Acelerar (1×/6×)** no playback da batalha — testar runs fica 6×
  mais rápido quando quiser.
- **Modo `--shot` estendido**: captura 5 telas (menu, como-jogar, elenco,
  draft, batalha) — insumo direto da galeria do site.
- **Documentação baseada na engine**: `docs/PERSONAGENS.md` (16 personagens
  do contrato v2 + 12 do modo 1v1, gerados do código) e `docs/CARTAS.md`
  (draft de formação, recompensas, escalada, os 16 Contratos, LCE, DWC);
  README e GDD/ARQUITETURA reescritos para "como funciona hoje"; CHANGELOG.
- Versão exibida no menu principal; nota "modo offline — adversário IA".

### Mudado
- README agora documenta o jogo jogável (não só a engine) e aponta para as
  releases.

### Corrigido (mecânica — contadores categóricos entre facções)
- **Causa raiz**: batalhas 4v4 quase determinísticas (só a moeda de abertura
  variava) quantizavam winrates em 0.000/0.500/1.000 e faziam a calibração
  oscilar; times de facção pura eram contadores duros (Ember>Verdant>Storm>
  Ember; Tide perdia de todos).
- **Ordem de movimento amostrada por batalha** no Monte Carlo
  (`set_orders`/`quick_battle_mods_orders`) e na IA do jogo — o winrate passa
  a medir a média sobre escolhas táticas (ordem = decisão de design; UI v0.6).
- **Jitter de timing de mystery (1/4 por turno)** na engine — variância
  simétrica e semeada; batalhas do jogo ficam menos robóticas.
- **DWC 4v4 reforçado** (fase 1 ±550‰/±400‰; fase 2 passos maiores, clamp
  ±520‰/±320‰). Gate 4v4: desvio médio 0.214 → **0.077 ≤ 0.15 — PASSOU**
  (meta v0.4/v0.5 cumprida); pares categóricos restantes listados como alvo
  de redesign de roster.

## [0.4.1] — correção da janela em branco

### Corrigido
- Janela abria 100% em branco no Linux/KDE — **três causas somadas**, todas
  invisíveis no `--check` headless: falta de `Camera2d`, falta da feature
  `bevy_ui_render` (o render da UI virou crate separado no Bevy 0.20) e
  queries de `Text` sobrepostas que panicavam ao entrar na batalha.
- Fonte default do Bevy (subset ASCII) trocada por DejaVu Sans Mono embutida
  — acentos pt-BR renderizam.

### Adicionado
- Modo de diagnóstico `--shot <base>` (menu → draft → batalha, PNGs reais)
  e ambiente de repro sem GPU (Xvfb + lavapipe) para validar visual no CI.

## [0.4.0] — o JOGO 2D de verdade (Bevy)

### Adicionado
- Novo crate **turnrush-game** (Bevy 0.20): janela 1280×720, menu clicável,
  draft visual (escolha 4 de 8 cartas), batalha 4v4 com playback animado da
  engine (barras de HP com HP real do roster, flashes de dano, log de
  combate), recompensas (+10% dano / +10% vida / equilíbrio), escalada de
  inimigos por vitória, fim de run (3 vitórias).
- Roster embutido no binário (16 personagens); zero assets externos.
- AppImage passa a abrir o JOGO (Exec=turnrush-game); o sim musl vai embutido
  e roteado pelo AppRun.

### Corrigido
- `turnrush-sim` sem argumentos agora mostra ajuda e sai (não satura CPU com
  calibração); nova flag `--threads` (default educado: min(cpus, 4)).

## [0.3.1] — logs cobrem TODAS as formas de término

### Adicionado
- Captura de SIGINT/SIGTERM/SIGHUP no logger (zero dependências: self-pipe +
  thread monitora): evento `session_interrupted` + exit 128+sig. Valores de
  tempo migrados para inteiros (contrato do parser).

### Corrigido
- Sessões interrompidas (Ctrl+C/fechar terminal) agora registradas — gap
  encontrado analisando logs reais da máquina do usuário (chris-pc/CachyOS).

## [0.3.0] — Formação 4v4, roster data-driven, logs de sessão

### Adicionado
- **Modo Formação 4v4** (`formation.rs`): formação 2×2, ordem de movimento,
  cadeias de perseguição (arremesso/empurrão/derrubada),
  mysteries com chakra/cooldown, passivas, DoT.
- **Contrato JSON v2** para rosters (`Roster::from_json`, validação com erro
  por campo) + `data/roster/roster-v2.json` (16 personagens, 4 facções).
- **Logs de sessão completos**: `~/TurnRush/logs` (.log + .jsonl, cabeçalho de
  ambiente, crash report em panic, retenção 20).
- Calibração 4v4 no simulador (por composição + por confronto) e gate próprio.

## [0.2.0] — DWC v2 e gates de CI

### Adicionado
- **DWC v2**: calibração em 2 fases (composição → confronto) sobre Monte Carlo
  de TODAS as composições (C(12,3) = 220 → 48.180 confrontos), passada de
  verificação em seeds novos, relatório JSON.
- Gates de equilíbrio no CI; diagnóstico automático de hard counters.
- Repositório remoto + GitHub Actions (testes + gate + build Linux).

## [0.1.0] — motor determinístico

### Adicionado
- Workspace Rust com **zero dependências externas**: RNG xoshiro256**,
  combate por turnos 1v1 (alternância A-B-A-B, energia/chakra, cooldowns,
  launch/chase, status, febre de batalha, juízo anti-turtle), draft simétrico
  com LCE, partida de 9 rodadas, roster v0.1 (12 personagens), simulador CLI,
  21 testes.
