# Direção de Arte & Pipeline de Sprites (v2 — referências do dono recebidas)

> Estado: **direção v2 definida a partir das 7 referências enviadas pelo dono
> (2026-10-20)**; 4 conceitos-mestre candidatos gerados e aguardando aprovação
> para eleger o estilo-mestre. A v1 (pixel art 64×64) foi escrita *aguardando
> os esboços* — com as referências em mãos, a direção é revista abaixo. O
> pipeline (folha-mestre → família → auditoria) continua igual.

## 0. As referências e como são usadas

- 7 imagens de direção de arte enviadas pelo dono (armazenadas **localmente,
  fora do repositório** — não versionamos arte de terceiros; o que sobe é
  apenas a **tradução original** para os personagens do TurnRush).
- Uso: **direção estética somente** (linework, paleta, silhueta, efeitos).
  Personagens específicos das referências **não são copiados** — o elenco do
  TurnRush é autoral (16 personagens, `data/roster/roster-v2.json`).
- Prompts **nunca citam nomes de artistas vivos** (regra da v1 mantida).

## 1. DNA de estilo extraído das referências

| Traço | Regra para o TurnRush |
|---|---|
| Linework | Contorno de tinta confiante, espessura 2–3 px @256; hachura/crosshatch só em sombras secundárias |
| Cor | **Fosco e esbatido**, sem gradiente pintado nem metal brilhante; 1 acento saturado por facção |
| Silhueta | Grande e legível em preto absoluto; chapéus/elmos oversized; **rosto oculto** (visor, sombra + olhos acesos, lenço) sempre que o papel permitir |
| Vestuário | Camadas de tecido (capas, saias, rolos de dormir, cordas), armadura remachada com tachas visíveis |
| Efeitos | **Cel chapado** (respingo, fumaça, corte) — formas sólidas, nunca volumétrico |
| Fundo | Pergaminho `#F2E0CC` em conceitos; **100% transparente** em sprite final |

**Paleta base extraída das referências** (rampa global compartilhada):

| Papel | Hex |
|---|---|
| Pergaminho (fundo de conceito) | `#F2E0CC` |
| Tinta (contorno) | `#2A2830` |
| Sombra hachurada | `#787068` |
| Pele | `#C89A78` |
| Osso/tecido claro | `#EFE8D8` |
| Rosa pó | `#C9A2A2` |
| Sálvia | `#8CA08C` |

**Acento por facção** (1 cor dominante + 1 brilho, sobre a base):

| Facção | Domínante | Brilho |
|---|---|---|
| EMBER (fogo) | terracota `#C4703F` | âmbar `#E0A458` |
| TIDE (água) | ardósia `#6E8CA0` | jade `#7FBFAE` |
| VERDANT (pedra/natureza) | sálvia `#8CA08C` | cardo `#A85C7A` |
| STORM (elétrico) | ocre `#D9A648` | cinza-tempestade `#6E6E80` |

## 2. Decisão revista (v1 → v2)

| | v1 (aguardando esboços) | **v2 (com referências)** |
|---|---|---|
| Estilo | Pixel art 64×64 | **Ilustração a tinta, frame 256×256** |
| Motivo | grid/paleta restringem a IA | As referências do dono **definem** tinta+hachura; 64×64 destruiria a hachura |
| Baseline | `y=58` em 64 | `y=232` em 256 (**mesma proporção** 58/64 — integração no cliente vira `scale = 1/4`, posições não mudam) |
| Filtragem | `ImagePlugin::default_nearest()` | **linear** (default do Bevy) na integração dos sprites de tinta |
| Altura do personagem | ~48 px | 150–230 px (brutos ~220+, ágeis ~170) — hierarquia dentro do canvas uniforme |

Todo o resto da v1 **se mantém**: folha-mestre com lock sha256, paleta de clã ×
silhueta de classe, pós-processo determinístico, auditoria executável,
contact sheet para aprovação, timing de frame não-uniforme.

## 3. Status: conceitos-mestre candidatos (Fase 1)

Gerados em `assets/art/concepts/` (provenance em `assets/art/ledger.jsonl`):

| Conceito | Facção × Papel | Leitura | Status |
|---|---|---|---|
| `dalva-tide-conceito.png` | TIDE × guardian | elmo great-helm + coral, ondas cel | ✅ candidato forte |
| `volta-storm-conceito.png` | STORM × controller | chapéu-cone estrelado, olhos acesos, raios cel | ✅ candidato forte |
| `yara-verdant-conceito.png` | VERDANT × support | chapéu de musgo, staff anel, cardos | ✅ candidato |
| `ignia-ember-conceito-v2.png` | EMBER × striker | lenço+brasa, bedroll, chama cel | ✅ candidato (v1 reprovado na cura: metal liso/rosto visível) |

**Próximo passo do dono: aprovar/apontar ajustes** → o aprovado de cada
"vencedor" de cura vira **estilo-mestre** (`assets/art/master/`, lock sha256)
e os outros 12 personagens derivam por família (facção × papel).

## 4. Fluxo de produção (7 etapas, da v1, inalterado)

1. Referência/conceito aprovado → 2. folha-mestre (turnaround 3/4, proporções
em "cabeças", rampas) → 3. contrato técnico (`assets/art/contract.json`:
frame 256×256, baseline 232, fundo transparente, paleta) → 4. produção por
família (novo personagem = **edição da folha-mestre**, nunca geração isolada;
regra por frame: só a pose muda) → 5. pós determinístico (chroma-key → maior
componente conexo → corte uniforme → âncora) → 6. auditoria + contact sheet →
7. integração Bevy.

## 5. Animações e o "60 FPS" (metas da v1 mantidas)

| Animação | Frames | FPS do sprite | Notas |
|---|---|---|---|
| idle (loop) | 4–6 | 6–8 | respiração; 90% do tempo em tela |
| attack (one-shot) | 4–6 | 12–15 | antecipação; impacto segura 120–150 ms |
| hit (one-shot) | 2–3 | 10–12 | + hitstop 60–100 ms |
| cast (one-shot) | 4–6 | 12 | flash de paleta pode derivar de attack |
| move (loop) | 6–8 | 10–12 | deslocamento/lançamento |
| death (one-shot) | 6–10 | 8–12 | hold no último frame |

Camadas de suavidade: **(1)** Transform interpolado a 60 fps (bob/sway
procedural — nunca assado nos frames), **(2)** duração por frame
(`Vec<Duration>`), **(3)** game feel em código (hitstop/shake/squash/flash).

**Stack Bevy 0.20 pesquisada (crates.io, 2026-10):**

| Crate | Ver. | Compatível 0.20? | Papel |
|---|---|---|---|
| `bevy_spritesheet_animation` | 8.0.1 | ✅ | flipbook declarativo (clips/loop/eventos) — escolhido |
| `bevy_easings` | 0.20.0 | ✅ | interpoladores contínuos (bob/sway) — opcional |
| `bevy_asset_loader` | 0.28.1 | ✅ | carga de atlases por estado — opcional |
| `bevy_hanabi` | 0.19.0 | ❌ (aguardar 0.20) | partículas VFX — fase posterior |
| `bevy_trickfilm`/`bevy_tweening` | — | ❌ obsoletos/lag | descartados |

Fallback aceito: `Timer` + `TextureAtlas::index` (padrão oficial
`sprite_sheet.rs`, verificado no source 0.20.1).

## 6. Kit de ferramentas

| Ferramenta | Papel |
|---|---|
| `z-ai image` (CLI) | geração de conceito/folha-mestre; edições por frame a partir do mestre |
| ImageMagick (`montage`, `-remap`, `-sample`) | sheets, paleta fixa, resize |
| Pillow | keying edge-connected, auditoria, âncoras |
| pngquant / `magick -remap` | quantização para a rampa da facção |
| `texture_packer` / grid Bevy | atlas final (grid uniforme cobre nosso caso) |

## 7. Provenance e licença

- Cada PNG → linha em `assets/art/ledger.jsonl` (data, origem, ferramenta,
  sha256, status de cura).
- Referências do dono: arquivadas **fora do repo**; prompts sem nomes de
  artistas; saída sempre re-desenhada (inspirado, nunca traçado).
- Licença dos assets: **decisão do dono** na 1ª entrega (sugestão v1 segue:
  CC-BY-4.0 para `assets/art/`, código MIT).

## 8. Faseamento

1. **Fase 1 (agora)**: 4 conceitos candidatos → aprovação do dono ✅ aguardando
2. **Fase 2**: folhas-mestre dos 4 + `idle/attack/hit` jogável dos
   porta-estandarte (Ignia, Dalva, Yara, Volta) + `contract.json` + auditoria
3. **Fase 3**: os outros 12 por família + `cast/move/death`
4. **Fase 4**: integração Bevy (atlas + flipbook + Transform layer a 60 fps +
   linear filtering) validada com `--shot`

## 9. Definição de pronto (por personagem — da v1, inalterada)

- [ ] Folha-mestre aprovada pelo dono
- [ ] idle + attack + hit (mínimo jogável)
- [ ] Auditoria verde (alpha/dimensões/duplicados/atlas)
- [ ] Contact sheet aprovada
- [ ] Integrado no Bevy, playback a 60 fps sem drift
