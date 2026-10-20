//! Peça 3 do quebra-cabeça: **economia do fluxo** (a moeda do encaixe).
//!
//! Encaixa na Peça 2 (determinismo): só audita a economia porque ela é
//! reproduzível. Cobre: ganhos (+10/rodada, +15 no padrão, passiva de
//! chakra com teto), custos (Skill 30 / Mystery 60 / Interrupt 35),
//! cooldowns, teto 100, desbloqueio por rodada e erros que NÃO consomem
//! a vez.

mod common;

use common::*;
use turnrush_core::interactive::{
    ActionError, ActionKind, BattleEvent, ChosenAction, FLUXO_CAP, FLUXO_PASSIVE_CAP,
    FLUXO_PER_ROUND, FLUXO_STD_GAIN, INTERRUPT_COST, MYSTERY_COST, PromptPhase, SKILL_COST,
    SideEdge,
};
use turnrush_core::interactive::InteractiveBattle;

#[test]
fn peca_03_encaixa_na_peca_02() {
    ancora_determinismo();
    ancora_economia();
}

/// Abertura de rodada: +10 para cada vivo (e nada para os mortos).
#[test]
fn ganho_de_rodada_e_dez_por_vivo() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let evs = {
        let _ = bt.prompt();
        bt.drain()
    };
    let ganhos: Vec<(usize, usize, i32)> = evs
        .iter()
        .filter_map(|e| match e {
            BattleEvent::FluxoGain { side, slot, amount } => Some((*side, *slot, *amount)),
            _ => None,
        })
        .collect();
    assert_eq!(ganhos.len(), 8, "todos os 8 vivos ganham na abertura");
    for (side, slot, amount) in &ganhos {
        // c3 (lado 0, slot 2) tem passiva de chakra +8; os demais ganham +10.
        let esperado = if (*side, *slot) == (0, 2) { FLUXO_PER_ROUND + 8 } else { FLUXO_PER_ROUND };
        assert_eq!(*amount, esperado, "ganho de rodada errado em {side}/{slot}");
    }
    for side in 0..2 {
        for slot in 0..4 {
            let esperado = if (side, slot) == (0, 2) { FLUXO_PER_ROUND + 8 } else { FLUXO_PER_ROUND };
            assert_eq!(bt.unit(side, slot).fluxo, esperado);
        }
    }
}

/// O padrão é grátis e GERA +15 (a moeda circula) — para quem quer que seja a vez.
#[test]
fn padrao_gera_fluxo_quinze() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let p = bt.prompt();
    let (sd, sl) = (p.side, p.slot);
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão sempre legal");
    assert!(evs.iter().any(|e| matches!(
        e,
        BattleEvent::FluxoGain { side, slot, amount }
            if *side == sd && *slot == sl && *amount == FLUXO_STD_GAIN
    )));
    assert_eq!(bt.unit(sd, sl).fluxo, FLUXO_PER_ROUND + FLUXO_STD_GAIN);
}

/// Skill custa 30: sem fluxo o erro vem claro e NÃO consome a vez.
#[test]
fn skill_sem_fluxo_recusa_sem_consumir_vez() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let p = bt.prompt();
    assert_eq!(bt.unit(p.side, p.slot).fluxo, 10, "rodada 1: só o ganho base");
    assert!(bt.legal_actions(p.side, p.slot).iter().all(|l| l.kind != ActionKind::Skill));

    let antes = face(&bt);
    let gen = bt.generation();
    let erro = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Skill, target: None })
        .unwrap_err();
    assert_eq!(erro, ActionError::NotEnoughFluxo);
    assert_eq!(face(&bt), antes, "recusa mutou o estado");
    assert_eq!(bt.generation(), gen, "recusa consumiu decisão");
    let p2 = bt.prompt();
    assert_eq!((p2.side, p2.slot), (p.side, p.slot), "recusa roubou a vez");
}

/// Banking: guardando padrões, a Skill de s1 sai na rodada 2 — e custa 30.
#[test]
fn skill_custa_trinta_e_entra_no_cd() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    // Roda a r1 inteira (s1 faz padrão → 25 ⚡) e chega à vez de s1 na r2 (35 ⚡).
    let p = vez_de(&mut bt, 0, 0, 2);
    assert_eq!(p.round, 2);
    let antes = bt.unit(0, 0).fluxo;
    assert_eq!(antes, FLUXO_PER_ROUND * 2 + FLUXO_STD_GAIN, "banking de 1 rodada = 35");
    assert!(bt
        .legal_actions(0, 0)
        .iter()
        .any(|l| l.kind == ActionKind::Skill && l.fluxo_cost == SKILL_COST));
    bt.execute_action(ChosenAction::Act { kind: ActionKind::Skill, target: None })
        .expect("skill no banking");
    // v0.9: o padrão do time repete elemento no comp sintético → a ação do
    // executor fecha Ressonância (+5 de fluxo) antes de pagar a Skill.
    assert_eq!(bt.unit(0, 0).fluxo, antes + turnrush_core::data::RESONANCE_FLUXO - SKILL_COST);
    assert_eq!(bt.unit(0, 0).skill_cd, turnrush_core::interactive::SKILL_CD);

    // r3: CD decrementou 2→1 na abertura → ainda bloqueado, erro claro.
    let _p = vez_de(&mut bt, 0, 0, 3);
    let erro = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Skill, target: None })
        .unwrap_err();
    assert_eq!(erro, ActionError::OnCooldown);
    assert_eq!(bt.unit(0, 0).skill_cd, 1, "CD deve liberar na rodada seguinte");
}

/// Mystery tem portão de rodada (`IllegalAction` antes) e custa exatamente 60.
#[test]
fn mystery_tem_portao_de_rodada_e_custo_sessenta() {
    // Portão: striker com unlock R4 é barrado na R2 mesmo com a phrase certa.
    let r = roster_of(&[
        CharSpec {
            id: "m4",
            name: "Porta Tardia",
            faction: "ember",
            role: "striker",
            speed: 50,
            m_targets: "enemy_single",
            m_pct: 120,
            unlock: Some(4),
            skill_targets: Some("enemy_single"),
            ..Default::default()
        },
        filler("f1"),
        filler("f2"),
        filler("f3"),
        filler("b1"),
        filler("b2"),
        filler("b3"),
        filler("b4"),
    ]);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let _p = vez_de(&mut bt, 0, 0, 2);
    let erro = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: None })
        .unwrap_err();
    assert_eq!(erro, ActionError::IllegalAction, "portão de rodada não aplicado");

    // Custo: roster do laboratório, s1 (unlock R2) guarda até a R3 = 60 ⚡ exatos.
    let rs = synth();
    let mut b2 = InteractiveBattle::new(&rs, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let p = vez_de(&mut b2, 0, 0, 3);
    assert_eq!(p.round, 3);
    let antes = b2.unit(0, 0).fluxo;
    assert!(antes >= MYSTERY_COST, "banking até R3 deve dar 60+ (tem {antes})");
    let evs = b2
        .execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: Some(0) })
        .expect("mystery no banking");
    // v0.9: Ressonância (+5) entra na execução (mesmo elemento repetido).
    assert_eq!(b2.unit(0, 0).fluxo, antes + turnrush_core::data::RESONANCE_FLUXO - MYSTERY_COST);
    assert_eq!(b2.unit(0, 0).mystery_cd, turnrush_core::interactive::MYSTERY_CD);
    assert!(
        evs.iter()
            .any(|e| matches!(e, BattleEvent::StateApplied { side: 1, slot: 0, .. })),
        "mystery single deve aplicar estado no alvo escolhido"
    );
}

/// Teto 100: fluxo nunca passa de 100, nem com bônus gigante de cartas.
#[test]
fn teto_de_fluxo_e_cem() {
    let r = synth();
    let edge = SideEdge {
        fluxo_per_round: 500, // carta do absurdolândia
        ..SideEdge::default()
    };
    let mut bt =
        InteractiveBattle::new_with_edges(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7, edge, SideEdge::default());
    let _ = bt.prompt();
    assert_eq!(bt.unit(0, 0).fluxo, FLUXO_CAP, "teto não aplicado na abertura");
    let _ = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão");
    assert_eq!(bt.unit(0, 0).fluxo, FLUXO_CAP, "+15 do padrão furou o teto");
    for _ in 0..20 {
        if bt.is_over() {
            break;
        }
        let p = bt.prompt();
        match p.phase {
            PromptPhase::ChooseTactic => {
                let _ = bt.execute_action(ChosenAction::PickTactic { index: 0 }).expect("tática");
            }
            PromptPhase::ConfirmChase => {
                let _ = bt.execute_action(ChosenAction::RejectChase).expect("recusa");
            }
            _ => {
                let _ = bt
                    .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
                    .expect("padrão");
            }
        }
        for side in 0..2 {
            for slot in 0..4 {
                assert!(bt.unit(side, slot).fluxo <= FLUXO_CAP);
            }
        }
    }
}

/// Passiva de chakra entra no ganho da abertura (+8 → 18) com teto próprio (+20).
#[test]
fn passiva_de_chakra_soma_na_abertura_com_teto() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let _ = bt.prompt();
    assert_eq!(
        bt.unit(0, 2).fluxo,
        FLUXO_PER_ROUND + 8,
        "passiva de chakra do c3 não entrou"
    );

    // Teto da passiva (20): gerador absurdamente forte clampa em +20.
    let r2 = roster_of(&[
        CharSpec {
            id: "ch",
            name: "Usina",
            faction: "ember",
            role: "controller",
            speed: 50,
            m_targets: "enemies_front",
            m_pct: 85,
            interrupt: Some("stun"),
            passive: "No início da rodada, gera +999 de chakra para a equipe.",
            ..Default::default()
        },
        filler("f1"),
        filler("f2"),
        filler("f3"),
        filler("b1"),
        filler("b2"),
        filler("b3"),
        filler("b4"),
    ]);
    let mut b2 = InteractiveBattle::new(&r2, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let evs = {
        let _ = b2.prompt();
        b2.drain()
    };
    let ganho_usina = evs.iter().find_map(|e| match e {
        BattleEvent::FluxoGain { side: 0, slot: 0, amount } => Some(*amount),
        _ => None,
    });
    assert_eq!(ganho_usina, Some(FLUXO_PER_ROUND + FLUXO_PASSIVE_CAP));
}

/// Interrupt custa 35, tem janela (contra quem ainda vai agir) e CD 3.
#[test]
fn interrupt_custo_janela_e_cd() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    // r1: c3 tem 18 ⚡ (10+8 da passiva) → sem fluxo pro interrupt (35).
    let _p = vez_de(&mut bt, 0, 2, 1);
    assert!(!bt.interrupt_targets(0).is_empty(), "janela de interrupt fechada cedo demais");
    let alvo_r1 = bt.interrupt_targets(0)[0];
    let erro = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Interrupt, target: Some(alvo_r1) })
        .unwrap_err();
    assert_eq!(erro, ActionError::NotEnoughFluxo);
    // banking: padrão no lugar do interrupt.
    bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão");

    // r2: c3 com 51 ⚡ (18+15+10+8) → interrupt entra e custa exatamente 35.
    let p = vez_de(&mut bt, 0, 2, 2);
    assert_eq!(p.round, 2);
    let fluxo_antes = bt.unit(0, 2).fluxo;
    assert!(fluxo_antes >= INTERRUPT_COST, "banking deve dar 35+ (tem {fluxo_antes})");
    let alvo = bt.interrupt_targets(0)[0];
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Interrupt, target: Some(alvo) })
        .expect("interrupt no banking");
    assert_eq!(bt.unit(0, 2).fluxo, fluxo_antes - INTERRUPT_COST);
    assert_eq!(bt.unit(0, 2).interrupt_cd, turnrush_core::interactive::INTERRUPT_CD);
    assert!(evs.iter().any(|e| matches!(
        e,
        BattleEvent::Debuffed { side: 1, slot, kind: _ } if *slot == alvo
    )));
    assert_eq!(bt.unit(1, alvo).stun_turns, 1, "stun do interrupt não aplicado");
}

/// Mystery paga desconto de carta (Gatilho Rápido): custo na ponta da engine.
#[test]
fn desconto_de_mystery_aparece_no_custo() {
    let r = synth();
    let edge = SideEdge {
        unit_mystery_discount: [15, 0, 0, 0],
        ..SideEdge::default()
    };
    let bt =
        InteractiveBattle::new_with_edges(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7, edge, SideEdge::default());
    assert_eq!(bt.mystery_cost(0, 0), MYSTERY_COST - 15);
    assert_eq!(bt.mystery_cost(0, 1), MYSTERY_COST, "desconto deve ser por slot");
}

/// Conector: a Peça 4 (mira) pode confiar que os custos estão certos.
#[test]
fn peca_03_encaixa_na_peca_04() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let p = bt.prompt();
    let legais = bt.legal_actions(p.side, p.slot);
    assert!(legais.iter().any(|l| l.kind == ActionKind::Standard));
    assert!(legais.iter().all(|l| l.fluxo_cost >= 0));
}
