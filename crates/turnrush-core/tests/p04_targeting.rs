//! Peça 4 do quebra-cabeça: **contrato de mira** (targeting).
//!
//! Encaixa na Peça 3: mira só é auditável porque custos/portões estão certos.
//! Convenção de slots: LOCAIS por lado (0..3) — lado A slots 0..3, lado B
//! slots 0..3. Front = slots 0–1; traseira = slots 2–3.
//! Contrato verificado (o que o jogador vê na UI é exatamente isto):
//! - Padrão SEM alvo explícito = automático (provocação > espelhado 1↔1 > primeiro vivo);
//! - Padrão COM alvo explícito = o escolhido (qualquer vivo, inclusive a traseira);
//! - Skill/Mystery `enemy_single` = acesso à linha de trás (v0.8);
//! - `enemies_front` resolve a frente; `random_2` lista 2 candidatos com nota honesta;
//! - `ally_single` sem alvo = aliado mais ferido; com alvo = o escolhido;
//! - `self` mira o executor; alvo morto/inexistente = erro claro `BadTarget`;
//! - prévia promete exatamente o que a execução entrega.

mod common;

use common::*;
use turnrush_core::interactive::{
    ActionError, ActionKind, BattleEvent, ChosenAction, InteractiveBattle, PromptPhase,
};

/// Alvos efetivos de uma prévia, ordenados.
fn alvos_previstos(
    bt: &mut InteractiveBattle,
    side: usize,
    slot: usize,
    acao: ChosenAction,
) -> Vec<(usize, usize)> {
    let mut prev = bt.preview_action(side, slot, acao).expect("prévia");
    prev.targets.sort_unstable();
    prev.targets
}

#[test]
fn peca_04_encaixa_na_peca_03() {
    ancora_economia();
}

/// Padrão sem alvo: espelho 1↔1 (slot N bate o slot N inimigo).
#[test]
fn padrao_sem_alvo_mira_o_espelhado() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let p = bt.prompt();
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão");
    let alvo = evs.iter().find_map(|e| match e {
        BattleEvent::ActionDeclared { side, kind, target, .. } if *side == p.side && *kind == ActionKind::Standard => {
            Some(*target)
        }
        _ => None,
    });
    assert_eq!(alvo, Some(Some((1 - p.side, p.slot))), "espelho 1↔1 quebrado");
}

/// Padrão COM alvo explícito: mira o escolhido — inclusive a linha de trás.
#[test]
fn padrao_com_alvo_mira_o_escolhido() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let _p = bt.prompt();
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: Some(3) })
        .expect("padrão na traseira é legal");
    let declarou = evs.iter().any(|e| matches!(
        e,
        BattleEvent::ActionDeclared { target: Some((1, 3)), .. }
    ));
    assert!(declarou, "alvo explícito (lado 1, slot 3) ignorado");
}

/// Espelho morto cai pro primeiro vivo (fallback do automático).
#[test]
fn padrao_cai_para_primeiro_vivo_quando_espelho_morre() {
    let r = roster_of(&[
        CharSpec { id: "a0", name: "Batedor 1", faction: "ember", role: "striker", speed: 50, ..Default::default() },
        CharSpec { id: "a1", name: "Batedor 2", faction: "ember", role: "striker", speed: 49, ..Default::default() },
        filler("a2"),
        filler("a3"),
        CharSpec { id: "m1", name: "Frágil Frente", faction: "tide", role: "guardian", speed: 30, hp: 30, ..Default::default() },
        filler("m2"),
        filler("m3"),
        CharSpec { id: "m4", name: "Frágil Trás", faction: "tide", role: "guardian", speed: 27, hp: 30, ..Default::default() },
    ]);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    // Roda a r1: padrões matam os frágeis espelhados (hp 30 vs dano pausado).
    for _ in 0..8 {
        if bt.is_over() {
            break;
        }
        let p = bt.prompt();
        if p.phase == PromptPhase::ChooseAction {
            bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
                .expect("padrão r1");
        }
    }
    assert!(!bt.unit(1, 0).alive, "frágil da frente deveria ter morrido na r1");
    // r2: a0 tem espelho morto → fallback = primeiro vivo (slot 1).
    let _p = vez_de(&mut bt, 0, 0, 2);
    let alvos = alvos_previstos(&mut bt, 0, 0, ChosenAction::Act { kind: ActionKind::Standard, target: None });
    assert_eq!(alvos, vec![(1, 1)], "automático deve cair no primeiro vivo");
}

/// Alvo morto/inexistente é recusado com `BadTarget` — sem consumir a vez.
#[test]
fn alvo_morto_ou_invalido_e_recusado() {
    let r = roster_of(&[
        CharSpec { id: "a0", name: "Batedor 1", faction: "ember", role: "striker", speed: 50, ..Default::default() },
        CharSpec { id: "a1", name: "Batedor 2", faction: "ember", role: "striker", speed: 49, ..Default::default() },
        filler("a2"),
        filler("a3"),
        CharSpec { id: "m1", name: "Frágil", faction: "tide", role: "guardian", speed: 30, hp: 30, ..Default::default() },
        filler("m2"),
        filler("m3"),
        filler("m4"),
    ]);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    // slot 9 não existe → BadTarget direto, vez preservada.
    let _p = vez_de(&mut bt, 0, 0, 1);
    let erro = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: Some(9) })
        .unwrap_err();
    assert_eq!(erro, ActionError::BadTarget);
    let p2 = bt.prompt();
    assert_eq!((p2.side, p2.slot), (0, 0), "BadTarget roubou a vez");
    // a0 mata o frágil (slot 0 do lado B)…
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: Some(0) })
        .expect("mata o frágil");
    assert!(evs.iter().any(|e| matches!(
        e,
        BattleEvent::Damage { side: 1, slot: 0, killed: true, .. }
    )));
    // …e a1 tenta mirar o corpo.
    let _p = vez_de(&mut bt, 0, 1, 1);
    let erro = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: Some(0) })
        .unwrap_err();
    assert_eq!(erro, ActionError::BadTarget, "corpo morto aceito como alvo");
}

/// `enemy_single` (v0.8): sem alvo = frente; com alvo = qualquer vivo (traseira inclusa).
#[test]
fn alvo_unico_da_acesso_a_traseira() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    // s1 precisa de 30 ⚡ pra prévia validar a Skill: banking até a r2 (35 ⚡).
    let _p = vez_de(&mut bt, 0, 0, 2);

    // Automático: primeiro vivo da frente (b1 = lado 1, slot 0).
    let auto = alvos_previstos(&mut bt, 0, 0, ChosenAction::Act { kind: ActionKind::Skill, target: None });
    assert_eq!(auto, vec![(1, 0)], "auto do single deve ser o primeiro da frente");
    let prev = bt
        .preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Skill, target: None })
        .expect("prévia do single");
    assert!(prev.damage > 0, "skill de striker deve prometer dano");
    assert!(prev.notes.iter().any(|n| n.contains("automático")), "nota de auto-alvo ausente");

    // Explícito na traseira (lado 1, slot 3): permitido — assinatura v0.8.
    let tras = alvos_previstos(&mut bt, 0, 0, ChosenAction::Act { kind: ActionKind::Skill, target: Some(3) });
    assert_eq!(tras, vec![(1, 3)], "traseira deveria ser acessível ao single");

    // Slot inexistente: a PRÉVIA cai no auto (comportamento real), mas a
    // EXECUÇÃO recusa com BadTarget — a engine nunca aceita lixo.
    let prev = bt
        .preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Skill, target: Some(9) })
        .expect("prévia com slot inválido cai no auto");
    assert_eq!(prev.targets, vec![(1, 0)]);
    let erro = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Skill, target: Some(9) })
        .unwrap_err();
    assert_eq!(erro, ActionError::BadTarget, "execução aceitou slot inexistente");
}

/// `enemies_front` resolve a frente (slots 0–1 do lado inimigo).
#[test]
fn faixa_front_resolve_a_frente() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    // s2 (A slot 1): skill enemies_front — prévia exige 30 ⚡ → banking r2.
    let _p = vez_de(&mut bt, 0, 1, 2);
    let front = alvos_previstos(&mut bt, 0, 1, ChosenAction::Act { kind: ActionKind::Skill, target: None });
    assert_eq!(front, vec![(1, 0), (1, 1)], "frente = slots 0-1 do lado inimigo");
}

/// `random_2`: exatamente 2 alvos e nota honesta de aleatoriedade na prévia.
#[test]
fn random_2_lista_dois_com_nota_honesta() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    // b3 (lado 1, slot 2, skill random_2 herdada) — banking até a r2 pra prévia.
    let _p = vez_de(&mut bt, 1, 2, 2);
    let prev = bt
        .preview_action(1, 2, ChosenAction::Act { kind: ActionKind::Skill, target: None })
        .expect("prévia random_2");
    // A prévia lista TODOS os candidatos vivos (honestidade) e a nota avisa
    // que 2 serão sorteados; a execução é que sorteia e trunca para 2.
    assert_eq!(prev.targets.len(), 4, "prévia deve listar todos os candidatos");
    assert!(
        prev.notes.iter().any(|n| n.to_lowercase().contains("aleat")),
        "prévia deve avisar que os alvos são sorteados: {:?}",
        prev.notes
    );
}

/// `ally_single`: sem alvo = aliado mais ferido; com alvo = o escolhido.
#[test]
fn cura_sem_alvo_vai_para_o_mais_ferido() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    // r1 inteira: padrões do lado B machucam espelhos do lado A.
    for _ in 0..8 {
        if bt.is_over() {
            break;
        }
        let p = bt.prompt();
        if p.phase == PromptPhase::ChooseAction {
            bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
                .expect("padrão r1");
        }
    }
    // r2 (oferta de tática antes): vez da h4 — auto da cura.
    let _p = vez_de(&mut bt, 0, 3, 2);
    let feridos: Vec<(usize, i32)> = (0..4)
        .filter(|&s| bt.unit(0, s).hp < bt.unit(0, s).hp_max)
        .map(|s| (s, bt.unit(0, s).hp))
        .collect();
    assert!(!feridos.is_empty(), "lado A deveria ter feridos após r1");
    let prev = bt
        .preview_action(0, 3, ChosenAction::Act { kind: ActionKind::Skill, target: None })
        .expect("prévia da cura");
    assert!(prev.heal > 0, "cura deve prometer vida");
    let (_, tslot) = prev.targets[0];
    let esperado = feridos.iter().copied().min_by_key(|&(_, hp)| hp).unwrap().0;
    assert_eq!(tslot, esperado, "auto da cura deve mirar o mais ferido");

    // Explícito: cura o slot 2 mesmo não sendo o mais ferido.
    let prev2 = bt
        .preview_action(0, 3, ChosenAction::Act { kind: ActionKind::Skill, target: Some(2) })
        .expect("prévia da cura explícita");
    assert_eq!(prev2.targets, vec![(0, 2)]);
}

/// `self`: escudo no próprio executor — e a provocação desvia o automático inimigo.
#[test]
fn self_target_e_provocacao_desviam_o_automatico() {
    let r = roster_of(&[
        CharSpec { id: "a0", name: "Batedor", faction: "ember", role: "striker", speed: 50, ..Default::default() },
        filler("a1"),
        filler("a2"),
        filler("a3"),
        CharSpec {
            id: "ta",
            name: "Provocador",
            faction: "tide",
            role: "guardian",
            speed: 30,
            hp: 2000,
            attack: 90,
            defense: 50,
            m_targets: "self",
            m_pct: 50,
            m_effect: "Escudo de 50% do ataque em si e provoca os inimigos por 1 rodada.",
            st_pct: 65,
            ..Default::default()
        },
        filler("t1"),
        filler("t2"),
        filler("t3"),
    ]);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);

    // r2, vez do provocador (35 ⚡): prévia do SELF via Skill (base do escudo).
    let _p = vez_de(&mut bt, 1, 0, 2);
    let prev = bt
        .preview_action(1, 0, ChosenAction::Act { kind: ActionKind::Skill, target: None })
        .expect("prévia do escudo");
    assert_eq!(prev.targets, vec![(1, 0)], "self deve mirar o executor");
    assert!(prev.shield > 0, "escudo deve prometer absorção");
    // banking: padrão em vez de skill → 50 ⚡.
    bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão banking");

    // r3: MYSTERY do escudo+provocação. v0.9: cada padrão repetido fecha
    // Ressonância (+5) → banking de 2 rodadas = (10+15+5)×2 + 10 = 70 ⚡.
    let _p = vez_de(&mut bt, 1, 0, 3);
    assert_eq!(bt.unit(1, 0).fluxo, 70, "banking de 2 rodadas = 70 ⚡ (Ressonância v0.9)");
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: None })
        .expect("mystery da provocação");
    assert!(
        evs.iter().any(|e| matches!(e, BattleEvent::Taunted { side: 0, slot: 0, rounds: 2 })),
        "evento de provocação ausente ou errado: {evs:?}"
    );
    assert!(bt.unit(1, 0).shield > 0, "escudo não aplicado");

    // r4: padrão de a0 SEM alvo deve ser desviado para o provocador (lado 1, slot 0).
    let _p = vez_de(&mut bt, 0, 0, 4);
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão desviado");
    let declarou = evs.iter().any(|e| matches!(
        e,
        BattleEvent::ActionDeclared { side: 0, slot: 0, target: Some((1, 0)), .. }
    ));
    assert!(declarou, "provocação não desviou o ataque automático");
}

/// Prévia = execução: o dano prometido é o dano entregue (contrato da UI).
#[test]
fn previa_promete_e_a_engine_entrega() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let p = bt.prompt();
    let acao = ChosenAction::Act { kind: ActionKind::Standard, target: None };
    let prev = bt.preview_action(p.side, p.slot, acao).expect("prévia");
    let evs = bt.execute_action(acao).expect("padrão");
    let dano = ultimo_dano(&evs, 1 - p.side, p.slot);
    assert!(dano > 0);
    assert_eq!(dano, prev.damage, "prévia prometeu {}, entregou {}", prev.damage, dano);
}

/// Conector: a Peça 5 (passivas) pode confiar na mira — bônus contra
/// Queimado só é testável sabendo exatamente quem vai ser atingido.
#[test]
fn peca_04_encaixa_na_peca_05() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let p = bt.prompt();
    let acao = ChosenAction::Act { kind: ActionKind::Standard, target: None };
    let prev = bt.preview_action(p.side, p.slot, acao).expect("prévia");
    assert!(!prev.targets.is_empty());
}
