# Setup do GitHub — TurnRush

> **Status**: aguardando o token GitHub do dono do projeto.
> Quando o token for enviado, o repositório será criado e o CI ativado
> automaticamente seguindo este roteiro.

## O que já está pronto

- `game/.github/workflows/build.yml` — CI completo (testes + gate de
  equilíbrio DWC + build Linux) que roda a cada push/PR.
- `game/.github/workflows/release.yml` — em cada tag `v*`, publica binário
  estático Linux (musl).
- Workspace Rust compila e testa verde localmente (cargo 1.99, edition 2021,
  zero dependências externas → CI rápido e estável).

## Passo a passo (quando o token chegar)

1. **Criar repositório** via API:
   ```
   POST /user/repos { "name": "turnrush", "private": true }
   ```
2. **Push do conteúdo de `game/`** como raiz do repositório (workflows, crates,
   docs).
3. **Secrets**: nenhum necessário — o CI usa apenas `GITHUB_TOKEN` embutido.
4. **Ativar Actions** (já ativo por padrão) e verificar o run verde de:
   - `test` → `cargo test --workspace --release`
   - `balance` → `turnrush-sim --quick --gate` (falha o PR se o equilíbrio
     regredir além do baseline v0.1)
   - `build-linux` → artifact `turnrush-linux-x64`
5. **Proteger `main`**: PRs obrigatórios + checks `test` e `balance`
   obrigatórios.
6. **Primeira release**: tag `v0.1.0` → binário estático anexado.

## Segurança do token

- O token será usado **apenas** para criar o repo e dar push; nunca será
  gravado em arquivo do projeto, log ou código.
- Recomendado: token **fine-grained** com escopo apenas em `Administration
  (read/write)` e `Contents (read/write)` do novo repo, expiração curta, e
  revogação após o setup.

## Comandos locais (equivalentes ao CI)

```bash
cd game
cargo test --workspace --release          # testes
cargo run --release -p turnrush-sim -- --quick --gate   # gate de equilíbrio
cargo run --release -p turnrush-sim -- --match          # partida de demonstração
cargo run --release -p turnrush-sim -- --probe 3 0      # diagnóstico de confronto
cargo run --release -p turnrush-sim -- --sims 300 --iters 8 --gate --out report.json
```
