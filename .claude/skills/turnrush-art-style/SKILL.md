---
name: turnrush-art-style
description: Contrato de estilo e produção de sprites do TurnRush. Usar SEMPRE que gerar, editar, cortar, animar ou empacotar qualquer sprite/arte 2D do jogo (personagens, efeitos, UI de jogo).
---

# TurnRush — Contrato de estilo de sprites

Fonte da verdade: `docs/ARTE-DIRECAO.md` + folha-mestre aprovada
(`assets/art/master/`). Este skill existe para que qualquer sessão de agente
produza arte **coerente** com o que já existe.

## Regras duras (nunca violar)

1. **Frame 64×64**, baseline dos pés em `y=58` (o cliente posiciona por aí).
2. **Pixel art**, fundo 100% transparente, contorno escuro legível.
3. **Paleta do clã** (rampas fixas, 16–32 cores globais compartilhadas):
   - PYREBORN (fogo/agressão), DUSKMERE (maré/controle),
     THORNVALE (natureza/veneno), SKYREND (tempestade/velocidade).
4. **Silhueta da classe** é o identificador primário — Reaver, Bastion,
   Mender, Tactician devem ser reconhecíveis em preto absoluto.
5. Visão **3/4 fixa**; direções: down, up, side (side espelha via flip).
6. Nomes: usar a tabela canônica `docs/NAMING.md` (clãs/classes em inglês,
   exibição pt-BR). **Zero referência a outros jogos/IPs.**
7. Origem permitida: esboços AUTORAIS do dono → re-desenho original 2D.
   Nunca traçar referência externa; nunca citar artistas vivos em prompts.

## Pipeline obrigatório (na ordem)

1. Ler `docs/ARTE-DIRECAO.md` (decisões, fps, ferramentas).
2. Produzir a partir da folha-mestre aprovada — nunca gerar personagem
   isolado; novo personagem = edição/família do estilo-mestre.
3. Restrição por frame: mesmo tamanho, proporção, traje, cores e arma em
   TODOS os frames; muda só a pose; nada extra vaza no frame.
4. Pós-processo determinístico por sheet: chroma-key → maior componente
   conexo (preserva brancos internos) → corte uniforme → âncora baseline.
5. Auditoria (alpha, dimensões, duplicados, contrato de atlas) + contact
   sheet para aprovação humana.
6. Só depois: integração Bevy (atlas `TextureAtlasLayout`, `Timer` com
   duração por frame, `ImagePlugin::default_nearest()`).

## Animações (ordem de produção)

`idle` (loop 4–6 frames @6–8 fps) → `attack` (4–6 @12–15, impacto segura
120–150 ms) → `hit` (2–3) → `cast` → `move` (loop) → `death` (hold).
Suavidade 60 fps vem de interpolação no `Transform` + timing não-uniforme +
hitstop/shake/flash — NÃO de 60 frames por segundo.

## Arquivos

- `assets/art/master/<char>-master.png` — folha-mestre (lock sha256).
- `assets/art/work/<char>/<action>_<dir>_<frame>.png` — frames de trabalho.
- `assets/sprites/<char_id>/<char_id>.png` + `.ron`/`json` — sheet final
  auditado (formato AtlasGrid 4×2/4×4, contrato em `assets/art/contract.json`).
- `assets/art/ledger.jsonl` — provenance de cada PNG gerado.
