//! Peça 11 — FINALIZAÇÃO GARANTIDA (v0.9.1, playtest 2026-10-20).
//!
//! O playtest relatou: "joguei duas batalhas, uma perdi e outra ganhei e ele
//! não finaliza a batalha após vencer… ficou com vitória 0/3". A causa era a
//! janela de chase manual (ENTER/ESC) que podia pausar para sempre — inclusive
//! no golpe final. Esta peça reasserta, EM CADEIA com as peças 1-10, que:
//!
//! 1. a janela de chase é sempre RESOLVÍVEL (nunca pendurada sem elegível);
//! 2. TODA batalha converge para Over — vitória, derrota, teto de rodadas —
//!    mesmo com janela de chase aberta no golpe final;
//! 3. seguindo a POLÍTICA DO CLIENTE (chase automático: 1º elegível persegue,
//!    recusa quando não há — mesma de `src/controller.rs`, testada também nos
//!    unit tests do binário), nenhuma batalha trava em centenas de decisões;
//! 4. os elos de combo da engine aparecem 1:1 como eventos visíveis.
//!
//! Como as demais peças: só API pública do core, determinístico, sem I/O.
//! (A política do controlador Bevy é unit-testada em `src/controller.rs` —
//! o `cargo test --workspace` e a peça 10b do gate cobrem.)

use turnrush_core::data::Roster;
use turnrush_core::interactive::{
    greedy_decide, BattleEvent, ChosenAction, InteractiveBattle, PromptPhase,
};
use turnrush_core::types::Outcome;

fn roster() -> Roster {
    Roster::from_json(include_str!("../../../data/roster/roster-v2.json")).expect("roster v2")
}

/// A POLÍTICA do cliente v0.9.1 (combo automático), espelhada aqui sobre a
/// API pública: chase → 1º elegível (ou recusa); tática → alterna pela
/// geração; ação → greedy. Nunca pausa: decide imediatamente.
fn decidir(bt: &mut InteractiveBattle, p: &turnrush_core::interactive::TurnPrompt) -> ChosenAction {
    match p.phase {
        PromptPhase::ConfirmChase => match bt.chase_eligible_slots(p.side).first() {
            Some(&s) => ChosenAction::Chase { chaser_slot: s },
            None => ChosenAction::RejectChase,
        },
        PromptPhase::ChooseTactic => ChosenAction::PickTactic { index: (bt.generation() % 2) as usize },
        PromptPhase::ChooseAction => greedy_decide(bt, p),
        PromptPhase::Over => unreachable!("chamou decidir com batalha over"),
    }
}

/// Âncora da peça 10: o contrato de cliente existe (mesma cadeia de encaixe).
/// Batalha interativa completa headless, com elementos, converge para Over.
#[test]
fn p11_ancora_da_peca_anterior() {
    let r = roster();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 20_261_020);
    let mut guardas = 0;
    while !bt.is_over() {
        guardas += 1;
        assert!(guardas < 100_000, "engine não converge — peça 10 quebrada");
        let p = bt.prompt();
        match p.phase {
            PromptPhase::Over => break,
            _ => {
                let acao = decidir(&mut bt, &p);
                bt.execute_action(acao).expect("ação da política é sempre legal");
            }
        }
        bt.drain();
    }
    assert!(bt.outcome().is_some(), "peça anterior não encaixa: sem resultado");
}

/// REGRESSÃO PRINCIPAL do playtest: com a política do cliente (combo
/// automático), TODAS as batalhas terminam — vitória, derrota e teto de
/// rodadas. 16 seeds variadas; limite duro de passos por batalha.
#[test]
fn p11_politica_do_cliente_sempre_finaliza() {
    let r = roster();
    let seeds = [3u64, 7, 11, 42, 77, 123, 404, 999, 1500, 7777, 20_261, 20_261_020, 867_5309, 555, 64, 128];
    for seed in seeds {
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], seed);
        let mut passos = 0;
        while !bt.is_over() {
            passos += 1;
            assert!(passos < 100_000, "seed {seed}: batalha não finaliza (travou?)");
            let p = bt.prompt();
            if p.phase == PromptPhase::Over {
                break;
            }
            let acao = decidir(&mut bt, &p);
            bt.execute_action(acao).expect("ação legal");
            bt.drain();
        }
        assert!(bt.outcome().is_some(), "seed {seed}: fim sem resultado");
    }
}

/// Janela de chase SEMPRE resolvível: em qualquer `ConfirmChase` da engine,
/// `chase_eligible_slots` NÃO pode estar vazio (a engine fecha a janela
/// sozinha quando ninguém pode perseguir) — se estiver vazio e mesmo assim
/// pausar, o jogo trava. Varredura ampla de seeds.
#[test]
fn p11_janela_de_chase_sempre_tem_elegivel_ou_fecha() {
    let r = roster();
    let seeds = [1u64, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233];
    for seed in seeds {
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], seed);
        let mut passos = 0;
        while !bt.is_over() {
            passos += 1;
            assert!(passos < 100_000, "seed {seed}: não converge");
            let p = bt.prompt();
            if p.phase == PromptPhase::ConfirmChase {
                let elegiveis = bt.chase_eligible_slots(p.side);
                assert!(
                    !elegiveis.is_empty(),
                    "seed {seed}: janela de chase pausa sem elegível — TRAVA do playtest"
                );
            }
            if p.phase == PromptPhase::Over {
                break;
            }
            let acao = decidir(&mut bt, &p);
            bt.execute_action(acao).expect("ação legal");
            bt.drain();
        }
    }
}

/// O placar da run conta EXATAMENTE 1 vitória por batalha ganha — a lógica
/// do cliente (`battle_finish`) espelhada em teste: `TeamAWin` ⇔ lado B
/// derrotado, e `prompt()` pós-Over é idempotente (a run não pode somar
/// 2 vitórias numa batalha — regressão do "Vitórias 0/3" eterno).
#[test]
fn p11_placar_conta_uma_vitoria_por_batalha() {
    let r = roster();
    for seed in [5u64, 88, 1234] {
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], seed);
        while !bt.is_over() {
            let p = bt.prompt();
            if p.phase == PromptPhase::Over {
                break;
            }
            let acao = decidir(&mut bt, &p);
            bt.execute_action(acao).expect("ação legal");
            bt.drain();
        }
        let outcome = bt.outcome().expect("resultado");
        let lado_b_derrotado = (0..4).all(|s| !bt.unit(1, s).alive);
        if outcome == Outcome::TeamAWin {
            assert!(lado_b_derrotado, "seed {seed}: vitória sem inimigo derrotado");
        }
        // idempotência do fim: prompt pós-Over continua Over, sempre.
        for _ in 0..3 {
            assert_eq!(bt.prompt().phase, PromptPhase::Over, "fim não é estável");
        }
    }
}

/// Coerência visual: cada elo de combo da engine vira exatamente 1 evento
/// `ChaseResolved` — o jogador VÊ o combo automático que aconteceu.
#[test]
fn p11_combo_automatico_visivel_1_para_1() {
    let r = roster();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 20_261_020);
    let mut eventos: Vec<BattleEvent> = Vec::new();
    while !bt.is_over() {
        let p = bt.prompt();
        if p.phase == PromptPhase::Over {
            break;
        }
        let acao = decidir(&mut bt, &p);
        eventos.extend(bt.execute_action(acao).expect("ação legal"));
        eventos.extend(bt.drain());
    }
    let ev_chases = eventos
        .iter()
        .filter(|e| matches!(e, BattleEvent::ChaseResolved { .. }))
        .count() as u64;
    assert_eq!(
        ev_chases,
        bt.chain_links(),
        "elo de combo sem evento visível (ou evento fantasma)"
    );
}

/// O golpe FINAL com estado aplicado não reabre rodada fantasma: se o último
/// inimigo caiu, `ActionKind` seguinte é Over IMEDIATO (sem turnos de
/// corpos) — o "não finaliza após vencer" do playtest não volta.
#[test]
fn p11_vitoria_encerra_imediatamente() {
    let r = roster();
    // Varredura de seeds: em nenhuma batalha existe decisão de ação APÓS um
    // lado estar derrotado (a engine só pergunta com os dois lados vivos —
    // ou no chase que é resolvido pela política sem rodar rodada nova).
    let seeds = [9u64, 19, 29, 39, 49, 59];
    for seed in seeds {
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], seed);
        let mut passos = 0;
        while !bt.is_over() {
            passos += 1;
            assert!(passos < 100_000);
            let p = bt.prompt();
            if p.phase == PromptPhase::ChooseAction {
                // nenhuma das duas forças pode estar derrotada numa vez real
                let a_vivo = (0..4).any(|s| bt.unit(0, s).alive);
                let b_vivo = (0..4).any(|s| bt.unit(1, s).alive);
                assert!(a_vivo && b_vivo, "seed {seed}: vez de ação com lado derrotado");
            }
            if p.phase == PromptPhase::Over {
                break;
            }
            let acao = decidir(&mut bt, &p);
            bt.execute_action(acao).expect("ação legal");
            bt.drain();
        }
    }
}
