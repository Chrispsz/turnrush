# Jogos para se inspirar — TurnRush

Lista curada a partir do contexto dos áudios (roguelike acelerado tipo 9 Kings +
combate por turnos estilo Naruto Online + balanceamento absoluto) e de
pesquisa web (out/2025). Para cada jogo: **o que extrair** para o TurnRush.

## 1. 9 Kings (2025) — Sad Socket ⭐ referência direta do usuário
"Fast-paced roguelike kingdom builder": rodadas curtas, cartas dos reis
rivais, escalação de dificuldade até o Rei IX (e modo infinito).
- **Extrair**: estrutura de run acelerado, draft de cartas por rodada,
  escalação por "níveis de rei" (nossas Rodadas I–IX).
- **Corrigir**: a comunidade aponta que 9 Kings é "wildly unbalanced" —
  exatamente o oposto da nossa tese. O DWC v2 é a nossa resposta.

## 2. Naruto Online (2013) — Oasis Games ⭐ referência direta do usuário
Combate tático por turnos: iniciativa, chakra, cooldowns, formações de 4 e
**combos de perseguição (chase)** entre os ninjas do time.
- **Extrair**: o vocabulário do combate (iniciativa, chakra/cooldown,
  lançamento + chase em cadeia, formação que define a build).

## 3. Slice & Dice (2021) — tann
Habilidades fixas por herói, faces aleatórias por rodada, e uma reputação de
balanceamento obsessivo por patches — sem itens, sem loot.
- **Extrair**: filosofia "poder fixo + aleatoriedade controlada", pureza
  anti-RPG, patches guiados por dados de partida.

## 4. Slay the Spire (2019) — Mega Crit
Deckbuilder roguelike: cartas fluem aleatoriamente durante o combate, mas a
matemática do RNG (peso por turno, pity implícito) faz o jogo parecer justo.
- **Extrair**: como desenhar ofertas aleatórias que "parecem justas" e o
  ritmo de decisão por turno com recursos limitados (energia).

## 5. Teamfight Tactics (2019) — Riot Games
Auto-battler com **pool compartilhado** de campeões e drops simétricos.
- **Extrair**: o padrão-ouro de "aleatoriedade simétrica": se os dois lados
  pescam do mesmo pool com as mesmas regras, a sorte não é assimétrica.
  Base conceitual do nosso draft simétrico + LCE.

## 6. Into the Breach (2018) — Subset Games
Táticas por turnos com **informação perfeita**: o inimigo anuncia o que fará;
cada turno é um quebra-cabeça solúvel.
- **Extrair**: previsibilidade tática (telegraphing) como opção futura de
  modo/arena — reduz a sensação de "sorte" mesmo com RNG presente.

## 7. Duelyst (2016) — Counterplay Games
Híbrido táticas-em-grid + card game com economia generosa.
- **Extrair**: leitura de tabuleiro, posição como recurso estratégico; caso
  de estudo de ecossistema competitivo (e de seus problemas de balance).

## 8. Legends of Runeterra (2020) — Riot Games
O card game mais justo do gênero: economia sem mana screw e **ações
alternadas** entre os jogadores.
- **Extrair**: alternância A-B-A-B de ações — adotamos diretamente no motor
  (a iniciativa deixa de ser vantagem composta e valem no máximo a 1ª ação).

## 9. Xadrez & Go
O padrão de simetria perfeita: mesmo material, mesmas regras; a vantagem de
iniciar é medida e **compensada por regra** (komi no Go; estudos de tempo no
xadrez).
- **Extrair**: a mentalidade "vantagem de lado se compensa por dado, não por
  sorte" — nosso sorteio de início + escalada simétrica seguem essa escola.

## 10. Pokémon competitivo (Smogon) — comunidade
Não é um jogo, é uma **instituição de balanceamento**: tiers por uso medido,
bans por winrate, suspect tests.
- **Extrair**: balancear por dados de winrate medidos, não por opinião — a
  filosofia exata do DWC (nosso "Smogon automático" roda no CI a cada commit).

---

## Bônus (monitorar)
- **Backpack Battles (2024)** — PvP por composição contra "espelho fantasma";
  referência de build-centric fairness.
- **Shogun Showdown (2024)** — tática minimalista com informação perfeita;
  prova que poucas unidades bastam para profundidade.
- **The Bazaar (2024/25)** — runs rápidas PvP com draft; observar como lidam
  (e deixam de lidar) com balance em tempo real.
