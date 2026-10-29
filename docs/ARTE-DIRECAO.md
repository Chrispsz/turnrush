# Direção de Arte & Pipeline de Sprites (v1 — aguardando os esboços autorais)

> Estado: **estrutura pronta, produção não iniciada**. O dono vai enviar
> esboços/direção de arte AUTORAIS (não-2D). Este documento fixa o pipeline
> para convertê-los em sprites 2D **originais** (inspirados, nunca traçados),
> com consistência entre os 16 personagens e animações integradas ao cliente
> Bevy. Nada é gerado sem passar pelas etapas abaixo.

## 0. Decisões-base (racional)

| Decisão | Escolha | Por quê |
|---|---|---|
| Estilo | **Pixel art 64×64** | O cliente já renderiza procedural 64×64 com nearest-neighbor; leitura tática exige silhueta legível; é o estilo que IA acerta mais consistente (paleta + grid restringem o modelo); rota determinística/diffável |
| Origem | Esboços autorais do dono | IP próprio — base legítima; saída SEMPRE re-desenhada no estilo do jogo (inspirado, não traçado) |
| Consistência | **Folha-mestre (model sheet) primeiro** | 1 personagem aprovado vira o estilo-mestre, travado por sha256 ("master-lock"); todo o resto deriva dele |
| Família | **Paleta do clã × silhueta da classe** | Reaproveita a lógica que o jogo já tem (PYREBORN/DUSKMERE/THORNVALE/SKYREND × Reaver/Bastion/Mender/Tactician): 16 personagens = 1 família + variações paramétricas |
| Verificação | Auditoria executável + contact sheet | Consistência vira TESTE (alpha, dimensões, duplicados, atlas), não julgamento visual em ~640 frames |

## 1. Fluxo em 7 etapas

1. **Esboço autoral recebido** → análise estrutural (pose, volumetria, paleta
   sugerida, elementos de design) + tradução para a linguagem 2D do jogo.
2. **Folha-mestre do 1º personagem** (turnaround 3/4 fixo, proporções em
   "cabeças", rampas de paleta) → **aprovação do dono** → vira o
   estilo-mestre (`assets/art/master/<char>-master.png` + lock sha256).
3. **Contrato técnico** (`assets/art/contract.json`): frame 64×64, baseline
   `y=58` (os pés onde o cliente espera hoje), direções (down/up/side com
   flip), fundo transparente, paleta global do clã (16–32 cores
   compartilhadas).
4. **Produção por família**: cada novo personagem usa a folha-mestre como
   referência de edição — NUNCA geração isolada. Restrição obrigatória de
   prompt/frame: *mesmo tamanho, proporção, traje, cores e arma em todos os
   frames; muda só a pose; sem personagens/efeitos extras vazando no frame*.
5. **Pós-processo determinístico** (por sheet inteira): chroma-key → maior
   componente conexo (preserva brancos internos como olhos) → corte uniforme
   → âncora no baseline. Ferramentas: ImageMagick/Pillow (offline).
6. **Auditoria + contact sheet**: checagens automáticas (RGBA/alpha,
   dimensões, frames duplicados, contrato de atlas) + uma folha de contato
   por personagem para aprovação humana antes de importar.
7. **Integração Bevy**: atlas por `TextureAtlasLayout` + animação por
   `Timer` com **duração por frame** (não-uniforme), `ImagePlugin::
   default_nearest()` (já em uso). Nada sobe para o jogo sem passar pela
   auditoria.

## 2. Animações: o que cada personagem precisa (ordem de produção)

1. `idle` (loop) — a base; aparece 90% do tempo.
2. `attack` (one-shot) — padrão/Skill.
3. `hit` (one-shot curto) — feedback de dano.
4. `cast` (one-shot) — Mystery (pode derivar de attack com paleta-swap de
   flash).
5. `move` (loop curto) — deslocamento/lançamento.
6. `death` (one-shot, hold no último frame).

## 3. FPS — o "60 FPS" bem-feito (sem 60 frames por segundo)

O jogo roda a 60 FPS; **a suavidade não vem de 60 desenhos por segundo**,
vem de três camadas:

| Animação | Frames | FPS do sprite | Notas |
|---|---|---|---|
| Idle (loop) | 4–6 | 6–8 | ciclo ~0,8 s; respiração/detalhe sutil |
| Movimento (loop) | 6–8 | 10–12 | ciclo ~0,6 s |
| Ataque (one-shot) | 4–6 | 12–15 | antecipação nos frames 1–2; **impacto segura 120–150 ms** |
| Hit | 2–3 | 10–12 | ~0,2 s + hitstop 60–100 ms |
| Morte/ultimate | 6–10 | 8–12 | hold no último frame |

- **Camada 1 (engine, 60 fps)**: deslocamento/escala/rotação interpolados no
  `Transform` (lerp/exponential smoothing) — o sprite só dá o ciclo.
- **Camada 2 (frame timing não-uniforme)**: durações por frame via
  `Vec<Duration>` (o impacto do ataque é o frame mais longo).
- **Camada 3 (game feel em código)**: hitstop, shake, squash de 1–2 px,
  flash de palette-swap — já é linguagem da batalha atual (flashes por clã).

Resultado: leitura de "peso e impacto" a 60 FPS com 4–8 desenhos por ação —
viável para 1 pessoa + IA (16 chars × ~40 frames ≈ 640 frames totais).

## 4. Kit de ferramentas (offline/local)

| Ferramenta | Papel no pipeline |
|---|---|
| ImageMagick (`montage -tile`, `-remap`, `-sample`) | montar sheets, paleta fixa, resize pixel-perfect |
| Pillow (Python) | keying edge-connected, auditoria, âncoras |
| pngquant / `magick -remap` | quantização para a paleta do clã |
| rembg (opcional) | remoção de fundo ML para esboços complexos |
| free-tex-packer-cli | atlas final (se sairmos do grid uniforme) |
| ffmpeg | previz/timing a partir de referência |

## 5. Skills instaladas (`.claude/skills/`)

| Skill | Uso |
|---|---|
| `turnrush-art-style` (própria) | **contrato de estilo do jogo** — carrega em qualquer tarefa de sprite |
| `create-game-assets` (gamedev-skills, Apache-2.0) | direção de arte: brief técnico, famílias de assets, QA |
| `ai-game-art-pipeline` (ybuild-ai, MIT) | keyframe-first → grid → key → âncora → contact sheet |
| `ai-sprite-production-audit` (Kein-chronicle, MIT) | auditoria determinística (master-lock, ledger, manifest) |
| `pixel-art-studio` (Gamezxz, MIT) | rota code-drawn (Pillow) quando quisermos determinismo total |
| Rust/processo: `test-driven-development`, `systematic-debugging`, `writing-plans`, `verification-before-completion` (superpowers, MIT) | disciplina de dev |
| `rust-best-practices`, `rust-review` (BeautifulRust, MIT) | idioms + review Rust |
| `bevy-testing`, `bevy-ecs-systems`, `bevy-ecs-queries` (bevy-skills, MIT) | Bevy 0.20 por subsistema |

## 6. Provenance e licença

- Todo PNG gerado ganha linha no **ledger** (`assets/art/ledger.jsonl`):
  data, origem (esboço autoral nº X), ferramenta, modelo, sha256.
- Direitos: base = esboços autorais do dono; saída = re-desenho original;
  sem nomes de artistas vivos em prompts ("inspirado, não traçado").
- Licença dos assets a definir na 1ª entrega (sugestão: CC-BY-4.0 para
  `assets/art/`, mantendo o código MIT) — **decisão do dono**.

## 7. Definição de pronto (por personagem)

- [ ] Folha-mestre aprovada pelo dono
- [ ] idle + attack + hit (mínimo jogável) nas direções contratadas
- [ ] Auditoria verde (alpha/dimensões/duplicados/atlas)
- [ ] Contact sheet aprovada
- [ ] Integrado no Bevy com timing por frame + playback a 60 fps sem drift
