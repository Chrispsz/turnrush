//! Peça 9 do quebra-cabeça: **elementos e cadeias de combo** (v0.9).
//!
//! Encaixa na Peça 8: com a run fechada, o sistema elemental atravessa os
//! DOIS motores (formação/Monte Carlo e interativo) sem quebrar nada que
//! já existia — economia (peça 3), mira (peça 4) e corrente (peça 7)
//! continuam valendo, agora com a camada elemental por cima:
//!
//! - Ciclo de fraquezas: Água>Fogo, Elétrico>Água, Pedra>Elétrico,
//!   Fogo>Pedra (cada elemento vence exatamente 1 e perde para exatamente 1).
//! - **Ressonância**: mesmo elemento em sequência → +15% de dano e +5⚡.
//! - **Cadeia**: progressão do ciclo com elemento DIFERENTE → +10% (passiva
//!   de sintonia soma aqui — combinar elementos é recompensado).
//! - Cura/escudo/interrupt não tocam a cadeia; DoT não é ação.

mod common;

use common::*;
use turnrush_core::data::{Element, RESONANCE_FLUXO};
use turnrush_core::formation::Formation4v4;
use turnrush_core::interactive::{
    greedy_decide, ActionKind, BattleEvent, ChosenAction, ElementalKind, FLUXO_PER_ROUND,
    FLUXO_STD_GAIN, InteractiveBattle, TurnPrompt,
};
use turnrush_core::types::Outcome;

// ── Laboratório ──────────────────────────────────────────────────────────────

/// Striker neutro de teste: dano padrão 70%, sem estado, sem chase.
fn el_striker(id: &'static str, elemento: &'static str, spd: i32) -> CharSpec {
    CharSpec {
        id,
        name: id,
        element: Some(elemento),
        role: "striker",
        attack: 100,
        defense: 40,
        speed: spd,
        ..Default::default()
    }
}

fn drive_to(bt: &mut InteractiveBattle, side: usize, slot: usize, rodada_min: usize) -> TurnPrompt {
    vez_de(bt, side, slot, rodada_min)
}

/// Roda uma batalha 100% automática (IA simétrica) e devolve o resultado
/// + quantos elos elementais fecharam.
fn auto_battle(roster: &turnrush_core::data::Roster, a: [usize; 4], b: [usize; 4], seed: u64) -> (Outcome, usize) {
    let mut bt = InteractiveBattle::new(roster, a, b, seed);
    let mut elos = 0usize;
    let mut guard = 0usize;
    while !bt.is_over() {
        guard += 1;
        assert!(guard < 20_000, "batalha automática não converge");
        let p = bt.prompt();
        let action = greedy_decide(&bt, &p);
        let evs = bt.execute_action(action).expect("decisão da IA sempre legal");
        elos += evs.iter().filter(|e| matches!(e, BattleEvent::ElementalCombo { .. })).count();
    }
    (bt.outcome().expect("batalha terminada"), elos)
}

// ── Âncora ───────────────────────────────────────────────────────────────────

#[test]
fn peca_09_encaixa_na_peca_08() {
    // Dados (peça 1) + economia (peça 3) continuam de pé sob os elementos.
    assert_eq!(ancora_dados(), 16);
    assert_eq!(FLUXO_PER_ROUND, 10);
    assert_eq!(FLUXO_STD_GAIN, 15);
}

// ── O ciclo de fraquezas ─────────────────────────────────────────────────────

#[test]
fn ciclo_fechado_cada_elemento_vence_um_e_perde_para_um() {
    let todos = [Element::Fogo, Element::Agua, Element::Eletrico, Element::Pedra];
    for &e in &todos {
        // Ciclo fecha: 4 passos voltam ao início.
        let ciclo = e.cycle_next().cycle_next().cycle_next().cycle_next();
        assert_eq!(ciclo, e, "ciclo de {e:?} não fecha");
        // Nunca vence nem perde para si mesmo.
        assert_eq!(Element::advantage_permille(e, e), 0, "{e:?} não pode ter auto-relação");
        // Vence exatamente um (o próximo do ciclo) com +200‰.
        let presa = e.cycle_next();
        assert_eq!(Element::advantage_permille(e, presa), 200);
        // Perde para exatamente um (o anterior do ciclo) com -200‰.
        let predador = todos
            .iter()
            .copied()
            .find(|&x| x.cycle_next() == e)
            .expect("ciclo garante predador");
        assert_eq!(Element::advantage_permille(e, predador), -200);
        // Diagonal neutra: todo o resto é 0.
        for &outro in &todos {
            if outro != e && outro != presa && outro != predador {
                assert_eq!(Element::advantage_permille(e, outro), 0, "{e:?} vs {outro:?} devia ser neutro");
            }
        }
    }
    // As 4 relações do ciclo, escritas como o design manda:
    assert_eq!(Element::advantage_permille(Element::Agua, Element::Fogo), 200, "Água apaga Fogo");
    assert_eq!(Element::advantage_permille(Element::Eletrico, Element::Agua), 200, "Elétrico conduz pela Água");
    assert_eq!(Element::advantage_permille(Element::Pedra, Element::Eletrico), 200, "Pedra isola Elétrico");
    assert_eq!(Element::advantage_permille(Element::Fogo, Element::Pedra), 200, "Fogo vitrifica Pedra");
}

// ── Contrato de dados ────────────────────────────────────────────────────────

#[test]
fn json_elemento_explicito_derivado_do_cla_e_erro_claro() {
    // Explícito vence o clã: personagem ember com element "agua".
    let r = roster_of(&[CharSpec { id: "a", faction: "ember", element: Some("agua"), ..Default::default() }]);
    assert_eq!(r.characters[0].element, Element::Agua);
    // Ausente → deriva do clã (4 clãs → 4 elementos).
    let r = roster_of(&[
        CharSpec { id: "a", faction: "ember", ..Default::default() },
        CharSpec { id: "b", faction: "tide", ..Default::default() },
        CharSpec { id: "c", faction: "storm", ..Default::default() },
        CharSpec { id: "d", faction: "verdant", ..Default::default() },
    ]);
    assert_eq!(r.characters[0].element, Element::Fogo);
    assert_eq!(r.characters[1].element, Element::Agua);
    assert_eq!(r.characters[2].element, Element::Eletrico);
    assert_eq!(r.characters[3].element, Element::Pedra);
    // Inválido → erro com o caminho do campo.
    let json = CharSpec { id: "a", element: Some("raio"), ..Default::default() }.to_json();
    let err = turnrush_core::data::Roster::from_json(&format!(r#"{{"version":2,"characters":[{json}]}}"#))
        .unwrap_err();
    assert!(err.contains("element"), "erro deve citar o campo: {err}");
}

#[test]
fn roster_real_cobre_os_quatro_elementos_com_um_papel_de_cada() {
    let r = real_roster();
    assert_eq!(r.characters.len(), 16);
    for el in [Element::Fogo, Element::Agua, Element::Eletrico, Element::Pedra] {
        let do_el: Vec<_> = r.characters.iter().filter(|c| c.element == el).collect();
        assert_eq!(do_el.len(), 4, "elemento {el:?} deve ter 4 personagens");
        let mut papeis: Vec<&str> = do_el.iter().map(|c| c.role.as_str()).collect();
        papeis.sort_unstable();
        papeis.dedup();
        assert_eq!(papeis.len(), 4, "elemento {el:?} deve cobrir striker/guardian/support/controller");
    }
    // Alísio carrega a passiva de sintonia e é elétrico (storm).
    let alisio = r.characters.iter().find(|c| c.id == "alisio").expect("alisio no roster");
    assert_eq!(alisio.element, Element::Eletrico);
    assert!(alisio.passive.effect.to_lowercase().contains("cadeia elemental"));
}

// ── Vantagem elemental nos DOIS motores ──────────────────────────────────────

#[test]
fn vantagem_muda_o_dano_no_interativo_com_matematica_exata() {
    // Água ×4 vs Fogo ×4 (forte) e Água ×4 vs Pedra ×4 (neutro, diagonal).
    let r_forte = roster_of(&[
        el_striker("a0", "agua", 60),
        el_striker("a1", "agua", 50),
        el_striker("a2", "agua", 40),
        el_striker("a3", "agua", 30),
        el_striker("f0", "fogo", 60),
        el_striker("f1", "fogo", 50),
        el_striker("f2", "fogo", 40),
        el_striker("f3", "fogo", 30),
    ]);
    let r_neutro = roster_of(&[
        el_striker("a0", "agua", 60),
        el_striker("a1", "agua", 50),
        el_striker("a2", "agua", 40),
        el_striker("a3", "agua", 30),
        el_striker("p0", "pedra", 60),
        el_striker("p1", "pedra", 50),
        el_striker("p2", "pedra", 40),
        el_striker("p3", "pedra", 30),
    ]);
    // Mesma seed e mesmas SPDs → mesma ordem; o padrão é o 1º golpe (sem cadeia).
    let mut b_forte = InteractiveBattle::new(&r_forte, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let mut b_neutro = InteractiveBattle::new(&r_neutro, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let _ = drive_to(&mut b_forte, 0, 0, 1);
    let _ = drive_to(&mut b_neutro, 0, 0, 1);
    let forte = b_forte
        .preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("prévia forte")
        .damage;
    let neutro = b_neutro
        .preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("prévia neutra")
        .damage;
    assert_eq!(forte, neutro * 1200 / 1000, "vantagem = ×1200‰ exato");

    // Desvantagem: no mesmo embate, o Fogo bate na Água com ×800‰.
    let fraco = b_forte
        .preview_action(1, 0, ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("prévia fraca")
        .damage;
    assert_eq!(fraco, neutro * 800 / 1000, "desvantagem = ×800‰ exato");
}

#[test]
fn formacao_emite_o_log_elemental_dos_dois_lados() {
    let r = roster_of(&[
        el_striker("a0", "agua", 60),
        el_striker("a1", "agua", 50),
        el_striker("a2", "agua", 40),
        el_striker("a3", "agua", 30),
        el_striker("f0", "fogo", 60),
        el_striker("f1", "fogo", 50),
        el_striker("f2", "fogo", 40),
        el_striker("f3", "fogo", 30),
    ]);
    let mut b = Formation4v4::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 42);
    b.set_log(true);
    let _ = b.run();
    let evs = b.events();
    assert!(
        evs.iter().any(|l| l.contains("[elemental] Água vs Fogo: +20%")),
        "log de vantagem ofensiva ausente: {evs:?}"
    );
    assert!(
        evs.iter().any(|l| l.contains("[elemental] Fogo vs Água: -20%")),
        "log de desvantagem ausente"
    );
}

// ── Ressonância (mesmo elemento) ─────────────────────────────────────────────

#[test]
fn ressonancia_da_dano_e_fluxo_e_fica_visivel_na_previa() {
    // Dois águas idênticos: o 2º golpe em sequência fecha Ressonância.
    let r = roster_of(&[
        el_striker("a0", "agua", 60),
        el_striker("a1", "agua", 50),
        el_striker("a2", "agua", 40),
        el_striker("a3", "agua", 30),
        el_striker("p0", "pedra", 60),
        el_striker("p1", "pedra", 50),
        el_striker("p2", "pedra", 40),
        el_striker("p3", "pedra", 30),
    ]);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    // 1º golpe do time A: sem cadeia, sem bônus.
    let _ = drive_to(&mut bt, 0, 0, 1);
    let base = bt
        .preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("prévia do 1º golpe")
        .damage;
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("1º golpe");
    assert!(
        !evs.iter().any(|e| matches!(e, BattleEvent::ElementalCombo { side: 0, .. })),
        "1º golpe do time não pode fechar elo"
    );
    assert_eq!(bt.chain_state(0), Some(Element::Agua));
    assert_eq!(bt.unit(0, 0).fluxo, FLUXO_PER_ROUND + FLUXO_STD_GAIN, "sem Ressonância no 1º golpe");

    // 2º golpe (mesmo elemento): a PRÉVIA promete o bônus e o +5⚡.
    let _p = drive_to(&mut bt, 0, 1, 1);
    let prev = bt
        .preview_action(0, 1, ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("prévia ressonante");
    assert_eq!(prev.damage, base * 1150 / 1000, "Ressonância = ×1150‰ exato");
    assert_eq!(prev.fluxo_delta, FLUXO_STD_GAIN + RESONANCE_FLUXO);
    assert!(prev.notes.iter().any(|n| n.contains("Ressonância")), "nota da prévia: {:?}", prev.notes);
    let antes = bt.unit(0, 1).fluxo;
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("2º golpe");
    // Prévia == execução: o dano no alvo (side 1 recebe) é o prometido e o
    // fluxo do executor também.
    let dano_exec = evs
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Damage { side: 1, amount, .. } => Some(*amount),
            _ => None,
        })
        .sum::<i32>();
    assert_eq!(dano_exec, prev.damage, "prévia != execução");
    assert_eq!(bt.unit(0, 1).fluxo, antes + prev.fluxo_delta, "fluxo da prévia != executado");
    assert!(evs.iter().any(|e| matches!(
        e,
        BattleEvent::ElementalCombo { side: 0, slot: 1, element: Element::Agua, kind: ElementalKind::Ressonancia, bonus_permille: 150 }
    )));
}

// ── Cadeia (elementos DIFERENTES em progressão) + passiva de sintonia ────────

#[test]
fn cadeia_progressao_do_ciclo_com_passiva_de_sintonia_e_wraparound() {
    let sintonia = "Quando age em cadeia elemental (elemento diferente do golpe anterior do time, na progressão do ciclo), o golpe dela causa 20% a mais de dano nessa ação.";
    // Ordem do ciclo: fogo → pedra → eletrico → agua (→ fogo…).
    let mut specs = vec![
        el_striker("e0", "fogo", 60),
        el_striker("e1", "pedra", 50),
        el_striker("e2", "eletrico", 40),
        el_striker("e3", "agua", 30),
    ];
    specs[1].passive = sintonia;
    for i in 0..4 {
        specs.push(el_striker("b", "pedra", 25 - i));
    }
    // ids únicos obrigatórios (contrato de dados)
    let ids = ["b0", "b1", "b2", "b3"];
    for (i, id) in ids.iter().enumerate() {
        specs[4 + i].id = id;
    }
    let r = roster_of(&specs);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);

    // r1: fogo → pedra (CADEIA +20% da sintonia) → eletrico → agua.
    let mut elos: Vec<(usize, ElementalKind, i32)> = Vec::new();
    for slot in 0..4 {
        let _p = drive_to(&mut bt, 0, slot, 1);
        let evs = bt
            .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
            .expect("golpe do ciclo");
        for e in &evs {
            if let BattleEvent::ElementalCombo { slot: s, kind, bonus_permille, .. } = e {
                elos.push((*s, *kind, *bonus_permille));
            }
        }
    }
    assert!(
        !elos.iter().any(|&(s, _, _)| s == 0),
        "1º golpe não fecha elo"
    );
    assert_eq!(elos[0], (1, ElementalKind::Cadeia, 300), "sintonia soma na CADEIA (100+200)");
    assert_eq!(elos[1], (2, ElementalKind::Cadeia, 100));
    assert_eq!(elos[2], (3, ElementalKind::Cadeia, 100));
    assert_eq!(bt.chain_state(0), Some(Element::Agua));

    // r2: o fogo segue a água — agua.cycle_next() == Fogo → a progressão
    // FECHA O CICLO atravessando a virada de rodada (wraparound).
    let _p = drive_to(&mut bt, 0, 0, 2);
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("golpe r2");
    assert!(
        evs.iter().any(|e| matches!(
            e,
            BattleEvent::ElementalCombo { side: 0, slot: 0, element: Element::Fogo, kind: ElementalKind::Cadeia, bonus_permille: 100 }
        )),
        "wraparound agua→fogo deve fechar CADEIA: {evs:?}"
    );
    assert_eq!(bt.chain_state(0), Some(Element::Fogo));
}

#[test]
fn sintonia_nao_dispara_na_ressonancia() {
    // Dois pedras: o 2º tem a passiva de sintonia — mas Ressonância é bônus
    // de mesmo elemento e a passiva é de CADEIA (outro elemento).
    let sintonia = "Quando age em cadeia elemental (elemento diferente do golpe anterior do time, na progressão do ciclo), o golpe dela causa 20% a mais de dano nessa ação.";
    let mut specs = vec![
        el_striker("q0", "pedra", 60),
        el_striker("q1", "pedra", 50),
        el_striker("q2", "pedra", 40),
        el_striker("q3", "pedra", 30),
    ];
    specs[1].passive = sintonia;
    for i in 0..4 {
        specs.push(el_striker("n", "fogo", 25 - i));
    }
    let ids = ["n0", "n1", "n2", "n3"];
    for (i, id) in ids.iter().enumerate() {
        specs[4 + i].id = id;
    }
    let r = roster_of(&specs);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let _ = drive_to(&mut bt, 0, 0, 1);
    bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("1º golpe");
    let _p = drive_to(&mut bt, 0, 1, 1);
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("2º golpe (ressonante)");
    assert!(
        evs.iter().any(|e| matches!(
            e,
            BattleEvent::ElementalCombo { side: 0, slot: 1, kind: ElementalKind::Ressonancia, bonus_permille: 150, .. }
        )),
        "Ressonância vale 150‰ SEM a sintonia: {evs:?}"
    );
}

// ── Cura não toca a cadeia ───────────────────────────────────────────────────

#[test]
fn cura_nao_estende_nem_quebra_a_cadeia() {
    let mut specs = vec![
        el_striker("g0", "fogo", 60),
        CharSpec {
            id: "g1",
            name: "Sacerdotisa",
            element: Some("agua"),
            role: "support",
            speed: 50,
            m_targets: "ally_single",
            m_effect: "Cura 100% do ataque.",
            // +20 de chakra no início da rodada (teto passivo) → Skill pagável na r1.
            passive: "No início da rodada, gera +20 de chakra.",
            ..Default::default()
        },
        el_striker("g2", "fogo", 40),
        el_striker("g3", "fogo", 30),
    ];
    for i in 0..4 {
        specs.push(el_striker("x", "pedra", 25 - i));
    }
    let ids = ["x0", "x1", "x2", "x3"];
    for (i, id) in ids.iter().enumerate() {
        specs[4 + i].id = id;
    }
    let r = roster_of(&specs);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);

    // r1: fogo bate (cadeia = Fogo).
    let _ = drive_to(&mut bt, 0, 0, 1);
    bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("golpe fogo");
    assert_eq!(bt.chain_state(0), Some(Element::Fogo));

    // A suporte paga a Skill de cura (30⚡ = 10 da rodada + 20 da passiva).
    let _p = drive_to(&mut bt, 0, 1, 1);
    assert_eq!(bt.unit(0, 1).fluxo, 30, "passiva de chakra entra antes da Skill");
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Skill, target: None })
        .expect("cura da suporte");
    assert!(evs.iter().any(|e| matches!(e, BattleEvent::Heal { side: 0, .. })), "cura emitida");
    assert!(
        !evs.iter().any(|e| matches!(e, BattleEvent::ElementalCombo { side: 0, .. })),
        "cura NÃO fecha elo elemental"
    );
    assert_eq!(bt.chain_state(0), Some(Element::Fogo), "cura não reseta a cadeia");

    // O 2º fogo em sequência REAL (ignorando a cura) fecha Ressonância.
    let _p = drive_to(&mut bt, 0, 2, 1);
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("golpe do 2º fogo");
    assert!(evs.iter().any(|e| matches!(
        e,
        BattleEvent::ElementalCombo { side: 0, slot: 2, kind: ElementalKind::Ressonancia, .. }
    )));
}

// ── Simetria e determinismo com elementos ────────────────────────────────────

#[test]
fn batalha_automatica_com_elementos_e_deterministica_e_simetrica() {
    // Espelho água: os dois lados têm a MESMA composição elemental.
    let espelho: Vec<CharSpec> = (0..4)
        .map(|i| el_striker("m", "agua", 60 - i))
        .chain((0..4).map(|i| el_striker("m", "agua", 60 - i)))
        .enumerate()
        .map(|(j, mut c)| {
            c.id = match j {
                0..=3 => ["m0", "m1", "m2", "m3"][j],
                _ => ["m4", "m5", "m6", "m7"][j - 4],
            };
            c
        })
        .collect();
    let r = roster_of(&espelho);
    let (o1, elos1) = auto_battle(&r, [0, 1, 2, 3], [4, 5, 6, 7], 9);
    let (o2, elos2) = auto_battle(&r, [0, 1, 2, 3], [4, 5, 6, 7], 9);
    assert_eq!(o1, o2, "mesma seed → mesmo resultado");
    assert_eq!(elos1, elos2, "mesma seed → mesmos elos elementais");
    assert!(elos1 > 0, "batalha água×4 deve fechar Ressonâncias (elas acontecem)");
    // E a batalha termina dentro do teto: vitória de alguém ou empate.
    assert!(matches!(o1, Outcome::TeamAWin | Outcome::TeamBWin | Outcome::Draw));
}

#[test]
fn snapshot_expoe_o_elemento_do_roster() {
    let r = real_roster();
    let bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    for side in 0..2 {
        for slot in 0..4 {
            let ci = if side == 0 { slot } else { 4 + slot };
            assert_eq!(
                bt.unit(side, slot).element,
                r.characters[ci].element,
                "snapshot deve expor o elemento do contrato"
            );
        }
    }
}
