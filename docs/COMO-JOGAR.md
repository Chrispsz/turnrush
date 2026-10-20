# COMO JOGAR — TurnRush

> Este guia saiu do jogo na v0.9.1 (feedback de playtest: "o como jogar pode
> ser retirado do jogo — fica melhor quando o jogo já estiver concluído").
> As regras aqui são FIÉIS ao engine (`crates/turnrush-core`).

## 1 · DRAFT — 8 cartas na mesa, escolha 4

A oferta é a mesma para os dois lados (simetria por construção).
A IA monta o time inimigo entre os personagens restantes do elenco.

## 2 · BATALHA 4v4 (você comanda tudo)

Timeline visível (SPD); A-B intercalado; quem abre a 1ª rodada é sorteado.
Painel de comando: **Padrão [1]** grátis (⚡+15) · **Skill [2]** ⚡30/CD 2 ·
**Mystery [3]** ⚡60/CD 3 (destrava R3; a carta Gatilho Rápido antecipa) ·
**Interrupt [4]** ⚡35/CD 3. O PADRÃO tem alvo automático: mira o inimigo na
sua frente (1↔1, 2↔2…; caiu, o primeiro vivo da frente). STRIKERS têm
ALVO ÚNICO (skill/mystery): mira QUALQUER inimigo — linha de trás inclusive.
Fluxo: +10/rodada, teto 100; guardar ⚡ antecipa a Mystery. Prévia EXATA
antes de confirmar. Máximo de 10 rodadas — desempate por % de vida.

## 3 · ELEMENTOS (v0.9) — fogo, água, elétrico e pedra

Cada personagem tem um elemento (`[Fg] [Ag] [El] [Pd]` nos cartões):

- **Ciclo de fraquezas**: Água > Fogo > Pedra > Elétrico > Água. Golpe com
  vantagem causa **+20% de dano**; com desvantagem, **−20%**. A diagonal
  (ex.: fogo vs água) é neutra.
- **Ressonância elemental** (mesmo elemento do golpe anterior do SEU time):
  **+15% de dano e +5⚡** para quem age — times "mono-elemento" aceleram sozinhos.
- **Cadeia elemental** (elemento na progressão do ciclo do golpe anterior —
  elemento DIFERENTE): **+10% de dano**. Passivas de sintonia (ex.: Alísio)
  ampliam cadeias com OUTRO elemento: +10% (cadeia) + 20% (sintonia).
- Tudo aparece na PRÉVIA da ação e no log — o que você vê é o que rola.

## 4 · CHAIN COMBOS — AUTOMÁTICOS (v0.9.1)

Mysteries e ataques aplicam Arremessado/Empurrado/Derrubado. Quem tem o
gatilho compatível persegue SOZINHO (o jogo mostra o flash "COMBO!" e os
elegíveis brilham dourado). **Combo não custa energia.** Até 12 elos;
1 perseguição por unidade por rodada (a carta certa amplia). A chance de
acerto é da engine — combo é oportunidade, não garantia.

## 5 · INTERRUPT — leia o inimigo

Os 4 Tacticians têm Interrupt: atordoam ou silenciam um inimigo que AINDA
vai agir nesta rodada. O jogo NÃO avisa — leia a timeline e o ⚡ do inimigo
(mostrado dos dois lados). Atordoado perde a ação; silenciado não conjura.

## 6 · TÁTICAS E CARTAS (roguelike)

A cada 2 rodadas (2/4/6/8) o jogo pausa: adote 1 de 2 TÁTICAS de batalha
[1]/[2] — efeito até o fim da batalha. O inimigo escolhe a dele em silêncio
(mesma força, zero pista). Vitória → 1 de 3 cartas de RUN
(Equipe/Unidade/Tática) — 1 reroll por batalha. Teto de poder da run: +30%.
A 3ª batalha é o BOSS. 3 vitórias completam a run.

## 7 · CLÃS E BALANCEAMENTO

Pyreborn, Duskmere, Thornvale e Skyrend dão identidade — NÃO existe contador
duro entre clãs. O calibrador DWC v2 mantém todas as composições em ~50% de
winrate médio: a vantagem vem da sua leitura do confronto (e agora, dos elementos).

## 8 · CONTROLES DA BATALHA

MOUSE: clique na ação — o Padrão dispara direto (alvo automático); poderes
focados e alvos únicos abrem a fila de alvos (o Auto da fila usa o primeiro
da frente). TECLADO: [1]-[4] ações · Q/W/E/R alvo inimigo · A/S/D/F alvo
aliado · ENTER confirma/auto-alvo · ESC cancela · ESPAÇO 2× · Z modo Auto
(IA joga por você). Os botões mostram a FORMA (· 1 alvo / frente / todos /
2 aleat.) e o custo ⚡. O status M: da Mystery inimiga é público
(R/CD/pronta). A IA inimiga usa EXATAMENTE as mesmas informações que você.
