#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# TurnRush — Puzzle Gate (suíte quebra-cabeça)
#
# Roda as 10 peças EM CADEIA, na ordem do encaixe:
#   Peça 0  qualidade do código (clippy -D warnings)
#   Peça 1  fundação de dados            (testes p01)
#   Peça 2  determinismo                 (testes p02)
#   Peça 3  economia do fluxo            (testes p03)
#   Peça 4  contrato de mira             (testes p04)
#   Peça 5  passivas e assinaturas       (testes p05)
#   Peça 6  kits e identidade de classe  (testes p06)
#   Peça 7  corrente de batalha          (testes p07)
#   Peça 8  ciclo de run                 (testes p08)
#   Peça 9  elementos e cadeias (v0.9)   (testes p09)
#   Peça 10 cliente E2E (--check)        (testes p10)
#   Peça 11 balanceamento Monte Carlo    (sim --quick --gate [+ formação])
#
# Uso:
#   scripts/puzzle-gate.sh              # cadeia completa
#   PUZZLE_FAST=1 scripts/puzzle-gate.sh # pula o gate de formação (pesado)
# Saída: um ✔/✘ por peça + veredito final. Exit != 0 se qualquer peça falhar.
# ═══════════════════════════════════════════════════════════════════════════
set -u
cd "$(dirname "$0")/.."

export PATH="$HOME/.cargo/bin:$PATH"
FAST="${PUZZLE_FAST:-0}"
PASS=0
FAIL=0
FAILED_PECAS=()

peca() {
  local num="$1" nome="$2"; shift 2
  echo "─────────────────────────────────────────────────────────────"
  printf "Peça %s — %s\n" "$num" "$nome"
  if "$@" >"/tmp/puzzle_peca_${num}.log" 2>&1; then
    echo "  ✔ encaixa"
    PASS=$((PASS + 1))
  else
    echo "  ✘ NÃO ENCAIXA (log: /tmp/puzzle_peca_${num}.log)"
    tail -20 "/tmp/puzzle_peca_${num}.log" | sed 's/^/    /'
    FAIL=$((FAIL + 1))
    FAILED_PECAS+=("$num")
  fi
}

echo "╔════════════════════════════════════════════════════════════╗"
echo "║  TurnRush — Quebra-Cabeça Completo                         ║"
echo "╚════════════════════════════════════════════════════════════╝"

peca 0 "qualidade do código (clippy -D warnings)" \
  cargo clippy --workspace --all-targets -- -D warnings

peca 1 "fundação de dados" \
  cargo test -p turnrush-core --release --test p01_data_foundation
peca 2 "determinismo" \
  cargo test -p turnrush-core --release --test p02_determinism
peca 3 "economia do fluxo" \
  cargo test -p turnrush-core --release --test p03_fluxo_economy
peca 4 "contrato de mira" \
  cargo test -p turnrush-core --release --test p04_targeting
peca 5 "passivas e assinaturas" \
  cargo test -p turnrush-core --release --test p05_passives
peca 6 "kits e identidade de classe" \
  cargo test -p turnrush-core --release --test p06_kit_composition
peca 7 "corrente de batalha interativa" \
  cargo test -p turnrush-core --release --test p07_battle_chain
peca 8 "ciclo de run roguelike" \
  cargo test -p turnrush-core --release --test p08_run_lifecycle
peca 9 "elementos e cadeias de combo (v0.9)" \
  cargo test -p turnrush-core --release --test p09_elements

# Testes inline (as peças NÃO substituem os 100+ testes de módulo).
peca "8b" "testes inline do core (regressão de módulo)" \
  cargo test -p turnrush-core --release --lib

# Peça 9: o binário do jogo precisa existir — cargo compila via test target.
peca 10 "cliente E2E (binário --check)" \
  cargo test -p turnrush-game --release --test p10_client_e2e

if [ "$FAST" = "1" ]; then
  echo "─────────────────────────────────────────────────────────────"
  echo "Peça 11 — balanceamento Monte Carlo: PULADA (PUZZLE_FAST=1)"
  PASS=$((PASS + 1))
else
  peca 11 "balanceamento 1v1 (Monte Carlo, gate ≤ 0.15)" \
    cargo run --release -p turnrush-sim -- --quick --gate --out /tmp/puzzle_balance.json
  peca "11b" "balanceamento 4v4 (formação, desvio ≤ 0.15)" \
    cargo run --release -p turnrush-sim -- --formation4v4 --formation-gate --formation-sims 150 --out /tmp/puzzle_formation.json
fi

echo "═════════════════════════════════════════════════════════════"
if [ "$FAIL" -eq 0 ]; then
  echo "✔ QUEBRA-CABEÇA COMPLETO — $PASS peças encaixam, 0 falhas."
  exit 0
else
  echo "✘ FALTAM PEÇAS — $PASS encaixam, $FAIL falham: ${FAILED_PECAS[*]}"
  exit 1
fi
