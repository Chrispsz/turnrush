//! Peça 7 do quebra-cabeça: **a corrente de batalha interativa**.
//!
//! Encaixa na Peça 6: com kits confiáveis, agora a MÃO DO JOGADOR fecha o
//! circuito: janela de perseguição (confirmar/recusar — o ENTER/ESC da UI),
//! oferta de tática a cada 2 rodadas, erros que nunca travam, convergência
//! com a IA simétrica e vitórias dos dois lados no espelho.

mod common;

use common::*;
use turnrush_core::interactive::{
    ActionError, ActionKind, BattleEvent, ChosenAction, MAX_ROUNDS,
};
use turnrush_core::interactive::{InteractiveBattle, PromptPhase};
use turnrush_core::types::Outcome;

#[test]
fn peca_07_encaixa_na_peca_06() {
    ancora_dados();
}

/// Lançamento abre janela; perseguidor elegível executa o elo (60% do pacto).
#[test]
fn cadeia_de_perseguicao_flui_da_ponta_a_ponta() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    // s1 banking até a r3 (60 ⚡) e mystery single com Lançamento no b1.
    let _p = vez_de(&mut bt, 0, 0, 3);
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: Some(0) })
        .expect("mystery de lançamento");
    assert!(evs.iter().any(|e| matches!(e, BattleEvent::StateApplied { side: 1, slot: 0, .. })));
    // Janela aberta para o lado A: s2 é o perseguidor elegível.
    let p = bt.prompt();
    assert_eq!(p.phase, PromptPhase::ConfirmChase, "janela de chase não abriu");
    let elegiveis = bt.chase_eligible_slots(0);
    assert_eq!(elegiveis, vec![1], "s2 deveria ser o único elegível (chase de Lançamento)");
    // Tentar perseguir com quem não tem o gatilho → BadTarget, janela segue aberta.
    let erro = bt.execute_action(ChosenAction::Chase { chaser_slot: 0 }).unwrap_err();
    assert_eq!(erro, ActionError::BadTarget);
    let p2 = bt.prompt();
    assert_eq!(p2.phase, PromptPhase::ConfirmChase, "erro de chase deve preservar a janela");
    // Executar com o elegível → elo resolvido.
    let evs = bt
        .execute_action(ChosenAction::Chase { chaser_slot: 1 })
        .expect("chase do perseguidor");
    let elo = evs.iter().find_map(|e| match e {
        BattleEvent::ChaseResolved { chaser, target, amount, killed } => {
            Some((*chaser, *target, *amount, *killed))
        }
        _ => None,
    });
    let ((cs, cslot), (ts, tslot), amount, _) = elo.expect("elo de chase ausente");
    assert_eq!((cs, cslot), (0, 1));
    assert_eq!((ts, tslot), (1, 0));
    assert!(amount > 0, "chase sem dano");
    assert!(bt.unit(0, 1).chased, "flag de perseguição da rodada não marcada");
    // Fila vazia → o turno de s1 é consumido; a vez segue o timeline.
    let p3 = bt.prompt();
    assert_eq!(p3.phase, PromptPhase::ChooseAction);
    assert_ne!((p3.side, p3.slot), (0, 0), "turno não foi consumido após a cadeia");
}

/// Recusar a janela (ESC da UI) é tática válida e consome o turno com evento claro.
#[test]
fn recusar_perseguicao_e_tatica_valida() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let _p = vez_de(&mut bt, 0, 0, 3);
    bt.execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: Some(0) })
        .expect("mystery de lançamento");
    let p = bt.prompt();
    assert_eq!(p.phase, PromptPhase::ConfirmChase);
    let evs = bt.execute_action(ChosenAction::RejectChase).expect("recusa");
    assert!(evs.iter().any(|e| matches!(e, BattleEvent::ChaseRejected { side: 0 })));
    let p2 = bt.prompt();
    assert_eq!(p2.phase, PromptPhase::ChooseAction, "recusa deve consumir o turno");
}

/// Oferta de tática: cadência 2/4/6/8, índices válidos, erro fora do cardápio,
/// evento do jogador visível e escolha silenciosa do inimigo.
#[test]
fn taticas_chegam_de_dois_em_dois() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    for rodada in [2usize, 4] {
        // Antes da rodada par: vez de s1 na rodada ímpar, SEM oferta.
        let p = vez_de(&mut bt, 0, 0, rodada - 1);
        assert_eq!(p.round, rodada - 1);
        assert!(bt.tactic_offer().is_none(), "oferta fora da cadência (r{})", rodada - 1);
        // Fecha a rodada ímpar com padrões; a rodada par abre COM a oferta.
        let p = loop {
            let p = bt.prompt();
            if p.phase == PromptPhase::ChooseTactic {
                break p;
            }
            assert!(p.round <= rodada, "tática não chegou na rodada {rodada}");
            match p.phase {
                PromptPhase::ConfirmChase => {
                    bt.execute_action(ChosenAction::RejectChase).expect("recusa");
                }
                _ => {
                    bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
                        .expect("padrão");
                }
            }
        };
        assert_eq!(p.round, rodada, "cadência de tática quebrada");
        let oferta = bt.tactic_offer().expect("oferta Some na fase");
        assert_ne!(oferta[0], oferta[1], "oferta com cartas repetidas");
        assert!(oferta.iter().all(|i| *i < 4), "índice fora do pool de táticas");
        // Executar uma AÇÃO na fase de tática → WrongPhase.
        let erro = bt
            .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
            .unwrap_err();
        assert_eq!(erro, ActionError::WrongPhase);
        // Índice fora da OFERTA → IllegalAction.
        let erro = bt.execute_action(ChosenAction::PickTactic { index: 2 }).unwrap_err();
        assert_eq!(erro, ActionError::IllegalAction);
        // Escolher da oferta → evento do lado 0 + escolha silenciosa do inimigo.
        let evs = bt
            .execute_action(ChosenAction::PickTactic { index: 0 })
            .expect("tática válida");
        assert!(evs.iter().any(|e| matches!(e, BattleEvent::TacticPicked { side: 0, .. })));
        assert!(evs.iter().any(|e| matches!(e, BattleEvent::TacticPicked { side: 1, .. })));
        assert!(bt.enemy_tactic_pick().is_some(), "inimigo deveria ter adotado tática");
    }
}

/// Batalha completa com a IA simétrica converge dentro do limite de rodadas.
#[test]
fn batalha_completa_converge_sem_travamento() {
    let r = real_roster();
    for seed in [1u64, 2, 3] {
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], seed);
        let (outcome, passos, evs) = play_out(&mut bt, &recusa_chase);
        assert!(passos < 100_000, "seed {seed}: não converge");
        assert!(matches!(outcome, Outcome::TeamAWin | Outcome::TeamBWin | Outcome::Draw));
        assert!(bt.round() <= MAX_ROUNDS, "seed {seed}: passou do teto de rodadas");
        assert!(evs.iter().any(|e| matches!(e, BattleEvent::RoundStart { round: 1, .. })));
        assert!(evs.iter().any(|e| matches!(e, BattleEvent::BattleOver { .. })));
    }
}

/// Espelho simétrico: com o mesmo kit dos dois lados, os dois lados vencem
/// em um conjunto fixo de seeds (nenhum lado estruturalmente favorecido).
#[test]
fn espelho_nao_favorece_lado_nenhum() {
    let r = real_roster();
    let comp = [0, 1, 2, 3];
    let mut wins = [0usize; 2];
    for seed in 1..=24u64 {
        let mut bt = InteractiveBattle::new(&r, comp, comp, seed);
        let (outcome, _, _) = play_out(&mut bt, &recusa_chase);
        match outcome {
            Outcome::TeamAWin => wins[0] += 1,
            Outcome::TeamBWin => wins[1] += 1,
            Outcome::Draw => {}
        }
    }
    assert!(wins[0] >= 1 && wins[1] >= 1, "espelho com favoritismo estrutural: {wins:?}");
}

/// A geração de decisão incrementa uma vez por decisão consumida (âncora da UI).
#[test]
fn geracao_de_decisao_marca_cada_decisao() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let g0 = bt.generation();
    let _p = bt.prompt();
    bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão");
    assert_eq!(bt.generation(), g0 + 1);
    bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão 2");
    assert_eq!(bt.generation(), g0 + 2);
    // Recusa ilegal NÃO conta decisão (Peça 3 já provou; dupla checagem aqui).
    let _ = bt.prompt();
    let _ = bt.execute_action(ChosenAction::Act { kind: ActionKind::Skill, target: None });
    let g_err = bt.generation();
    let _ = bt.execute_action(ChosenAction::Act { kind: ActionKind::Skill, target: None });
    assert_eq!(bt.generation(), g_err, "erro consumiu geração");
}

/// Depois do fim, tudo recusa com BattleOver (a UI pode consultar sem medo).
#[test]
fn apos_o_fim_tudo_recusa_limpo() {
    let r = real_roster();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 3);
    let (outcome, _, _) = play_out(&mut bt, &recusa_chase);
    let _ = outcome;
    assert!(bt.is_over());
    let erro = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .unwrap_err();
    assert_eq!(erro, ActionError::BattleOver);
    assert!(bt.legal_actions(0, 0).is_empty());
    assert!(bt.outcome().is_some());
}

/// Conector: a Peça 8 (run) pode montar batalhas com vantagens de cartas
/// sabendo que a corrente de batalha responde direitinho.
#[test]
fn peca_07_encaixa_na_peca_08() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let p = bt.prompt();
    assert_eq!(p.phase, PromptPhase::ChooseAction);
}
