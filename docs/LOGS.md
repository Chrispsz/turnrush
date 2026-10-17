# Logs e Diagnóstico de Sessão — TurnRush

> "Um script de tudo que aconteceu desde iniciar o jogo até fechar" — com
> diretório fácil na home e anexo pronto para reportar problemas.

## Onde ficam os logs

```
$HOME/TurnRush/logs/          ← diretório padrão (fácil na home)
├── session-20261017-184151-2f5d0cb7.log   ← log legível (humano)
├── session-20261017-184151-2f5d0cb7.jsonl ← eventos estruturados (JSON por linha)
└── crash-20261017-180602.log              ← relatório de crash (quando ocorre)
```

> Exemplo real em produção: sessões de `chris-pc` (CachyOS, kernel 7.2.9,
> 12 CPUs) geradas pelo AppImage da release — o cabeçalho identifica a
> máquina de quem reportou o problema automaticamente.

Variável `TURNRUSH_LOG_DIR` sobrescreve o diretório (útil para CI: defina
`TURNRUSH_LOG_DIR=$PWD/.turnrush-logs`). Se `$HOME` não existir, o fallback é
`./turnrush-logs`.

## O que cada formato contém

### `.log` (legível)
Cronologia completa da sessão em texto: cabeçalho com ambiente (versão da
engine, SO, kernel, arquitetura, host, CPUs, seed), cada subsistema
inicializado, cada decisão de jogo (draft, turno, habilidade, dano, estado,
cadeia de perseguição) e o encerramento com tempo total.

### `.jsonl` (estruturado)
Um evento JSON por linha — ideal para filtrar/analisar:

```json
{"ts":"0.000","level":"info","event":"session_start","msg":"sessão iniciada",
 "data":{"engine":"turnrush-core","version":"0.1.0","id":"20261017-184151-2f5d0cb7",
         "utc":"2026-10-17 18:41:51","seed":0,"os":"linux",
         "kernel":"5.10.134-…","arch":"x86_64","host":"…","cpus":2}}
{"ts":"0.003","level":"info","event":"roster_load","msg":"roster carregado de data/roster/roster-v2.json",
 "data":{"personagens":16,"versao":2}}
{"ts":"12.751","level":"info","event":"formation_gate","msg":"PASSOU"}
{"ts":"12.751","level":"info","event":"session_end","msg":"encerramento limpo da sessão",
 "data":{"tempo_total_ms":12751,"eventos":4}}
```

Campos: `ts` (segundos desde o início da sessão), `level` (debug/info/warn/error),
`event` (identificador estável), `msg` (texto humano) e `data` (pares chave-valor).
> Contrato: `data` usa **inteiros** (tempos em ms) — o parser JSON interno aceita
> inteiros por design; nada de float serializado.

## Sessão interrompida (Ctrl+C, kill, terminal fechado)

O logger captura `SIGINT`, `SIGTERM` e `SIGHUP` (zero dependências: FFI mínima
de `signal`/`pipe`/`writev` + self-pipe + thread monitora — handler async-signal-safe
por design). Ao receber o sinal, a sessão registra o evento final antes de sair:

```json
{"ts":"2.000","level":"info","event":"session_interrupted","msg":"sessão interrompida por sinal",
 "data":{"sinal":"SIGINT","sig":2,"tempo_total_ms":1999,"eventos":1,"exit_code":130}}
```

Código de saída convencional `128+sig` (130 Ctrl+C, 143 kill, 129 HUP).
A instalação em si também é logada no início da sessão (`signal_capture`),
com warn explícito se a captura não puder ser ativada.

Verificação E2E (binário real, não só testes de unidade):

```bash
turnrush-sim &  PID=$!; sleep 2; kill -INT $PID; wait $PID   # → exit 130
grep session_interrupted ~/TurnRush/logs/session-*.jsonl | tail -1
```

## Relatório de crash

Um panic hook global intercepta qualquer panic e escreve
`crash-YYYYMMDD-HHMMSS.log` com: mensagem do panic, local, e as **últimas ~200
linhas do log da sessão** — o contexto imediatamente anterior ao problema nunca
se perde.

## Retenção e variáveis de ambiente

| Variável | Padrão | Significado |
|---|---|---|
| `TURNRUSH_LOG_DIR` | `$HOME/TurnRush/logs` | Diretório dos logs |
| `TURNRUSH_LOG_LEVEL` | `info` | `debug`, `info`, `warn` ou `error` |
| `TURNRUSH_LOG_KEEP` | `20` | Sessões antigas mantidas (rotação automática) |

## Como reportar um problema

1. Reproduza o problema (se possível);
2. Pegue o arquivo de sessão mais recente em `$HOME/TurnRush/logs/`
   (`ls -t ~/TurnRush/logs/ | head`);
3. Se houver `crash-*.log`, inclua-o também;
4. Abra uma issue em <https://github.com/Chrispsz/turnrush/issues> descrevendo
   o que esperava × o que aconteceu, e anexe os arquivos;
5. Se for balanceamento, rode
   `turnrush-sim --formation4v4 --formation-gate --out relatorio.json` e anexe
   o relatório.

## Por que zero dependências ajuda aqui

O logger é std-only: sem `tracing`/`serde` na árvore de dependências, a
superfície de auditoria é mínima (sem transitive deps = sem leaks de
terceiros), o binário é pequeno e o custo por evento é um `push` em buffer de
memória + flush periódico — nunca bloqueia o loop de jogo.
