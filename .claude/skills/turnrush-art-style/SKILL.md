---
name: turnrush-art-style
description: Contrato de estilo e produção de sprites do TurnRush. Usar SEMPRE que gerar, editar, cortar, animar ou empacotar qualquer sprite/arte 2D do jogo (personagens, efeitos, UI de jogo).
---

# TurnRush — Contrato de estilo de sprites (v2 — tinta 256)

Fonte da verdade: `docs/ARTE-DIRECAO.md` (v2) + folha-mestre aprovada
(`assets/art/master/`). Este skill existe para que qualquer sessão de agente
produza arte **coerente** com o que já existe. Direção v2 extraída das
referências do dono (2026-10-20): **ilustração a tinta com hachura, cores
foscas esbatidas, silhuetas grandes, rostos ocultos, efeitos cel chapados**.

## Regras duras (nunca violar)

1. **Frame 256×256**, baseline dos pés em `y=232` (mesma proporção do
   contrato antigo 58/64 — o cliente posiciona por aí; integração = scale 1/4).
2. **Estilo tinta**: contorno de tinta 2–3 px, hachura só em sombras
   secundárias, cores **foscas e esbatidas** (sem gradiente pintado, sem metal
   brilhante), fundo 100% transparente no sprite final.
3. **Paleta base global** (rampa compartilhada, ver hex em
   `docs/ARTE-DIRECAO.md` §1) + **1 acento por facção**:
   EMBER terracota/âmbar, TIDE ardósia/jade, VERDANT sálvia/cardo, STORM
   ocre/cinza-tempestade.
4. **Silhueta da classe** é o identificador primário — Reaver, Bastion,
   Mender, Tactician reconhecíveis em preto absoluto; rosto oculto (visor,
   sombra com olhos acesos, lenço) sempre que o papel permitir.
5. Visão **3/4 fixa**; direções: down, up, side (side espelha via flip).
6. Nomes: tabela canônica `docs/NAMING.md` (clãs/classes em inglês, exibição
   pt-BR). **Zero referência a outros jogos/IPs; zero nomes de artistas vivos
   em prompts; nunca traçar referência externa.**
7. Origem permitida: conceitos/folha-mestre aprovados pelo dono
   (`assets/art/concepts/` → `assets/art/master/`). Novo personagem = família
   derivada do mestre, **nunca geração isolada**.

## Pipeline obrigatório (na ordem)

1. Ler `docs/ARTE-DIRECAO.md` (DNA de estilo, paleta, fps, faseamento).
2. Produzir a partir da folha-mestre aprovada — restrição por frame: mesmo
   tamanho, proporção, traje, cores e arma em TODOS os frames; muda só a
   pose; nada extra vaza no frame.
3. Pós-processo determinístico por sheet: chroma-key → maior componente
   conexo (preserva claros internos) → corte uniforme → âncora no baseline.
4. Auditoria (alpha, dimensões, frames duplicados, contrato de atlas) +
   contact sheet para aprovação humana. Provenance no `ledger.jsonl`.
5. Só depois: integração Bevy (atlas `TextureAtlasLayout`, flipbook com
   duração **por frame**, camada procedural de Transform a 60 fps,
   filtragem **linear** — não nearest — para o estilo tinta).

## Animações (ordem de produção)

`idle` (loop 4–6 @6–8 fps) → `attack` (4–6 @12–15, impacto segura 120–150 ms)
→ `hit` (2–3) → `cast` → `move` (loop) → `death` (hold). A suavidade 60 fps
vem de Transform interpolado + timing não-uniforme + hitstop/shake/flash —
NÃO de 60 frames por segundo.

## Arquivos

- `assets/art/concepts/` — conceitos candidatos à aprovação do dono.
- `assets/art/master/<char>-master.png` — folha-mestre (lock sha256).
- `assets/art/work/<char>/<action>_<dir>_<frame>.png` — frames de trabalho.
- `assets/sprites/<char_id>/<char_id>.png` + `.ron`/`json` — sheet final
  auditada (grid uniforme, contrato em `assets/art/contract.json`).
- `assets/art/ledger.jsonl` — provenance de cada PNG gerado.
