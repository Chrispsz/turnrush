//! Peça 8 do quebra-cabeça: **ciclo de run roguelike**.
//!
//! Encaixa na Peça 7: a batalha responde — agora o loop de recompensa fecha:
//! carta comprada → SideEdge → batalha seguinte muda de fato. Cobre teto de
//! poder, reroll, revive, chases extras, Gatilho Rápido (portão + desconto)
//! e o draft simétrico 1v1 com LCE.

mod common;

use common::*;
use turnrush_core::interactive::{BattleEvent, ChosenAction, InteractiveBattle, SideEdge};
use turnrush_core::interactive::ActionKind;
use turnrush_core::rewards::{CardClass, RunState, CARD_POOL};
use turnrush_core::rng::Rng;

fn pos_carta(id: &str) -> usize {
    CARD_POOL.iter().position(|c| c.id == id).unwrap_or_else(|| panic!("carta '{id}' ausente do pool"))
}

#[test]
fn peca_08_encaixa_na_peca_07() {
    let rs = RunState::new();
    let mut rng = Rng::new(1);
    let oferta = rs.offer(&mut rng, 1, &[turnrush_core::data::Faction::Ember]);
    assert!(!oferta.is_empty(), "run deve ofertar cartas");
}

/// Compra de carta de unidade exige alvo; teto e repetição recusam.
#[test]
fn compras_respeitam_regras_da_run() {
    let mut rs = RunState::new();
    let furia = &CARD_POOL[pos_carta("furia-interior")];
    assert_eq!(furia.class, CardClass::Unit);
    // Sem alvo → recusa clara.
    assert!(rs.take(furia, None).is_err(), "carta de unidade sem alvo foi aceita");
    // Com alvo → entra e soma poder.
    rs.take(furia, Some(0)).expect("compra com alvo");
    assert_eq!(rs.poder, furia.poder);
    // Repetição → recusa.
    assert!(rs.take(furia, Some(0)).is_err(), "carta repetida foi aceita");
    // Reroll: 1 por batalha — o contador é abastecido pela camada de run
    // a cada batalha (RunState::new() nasce com 0).
    rs.rerolls = 1;
    assert!(rs.reroll(), "primeiro reroll deveria funcionar");
    assert!(!rs.reroll(), "segundo reroll deveria falhar");
}

/// Cartas viram vantagem: Fluxo Estável aparece na abertura de rodada.
#[test]
fn carta_de_fluxo_muda_a_batalha_de_verdade() {
    let mut rs = RunState::new();
    rs.take(&CARD_POOL[pos_carta("fluxo-estavel")], None).expect("compra");
    let edge = rs.edge(&[]);
    assert_eq!(edge.fluxo_per_round, 2);

    let r = synth();
    let mut bt = InteractiveBattle::new_with_edges(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7, edge, SideEdge::default());
    let evs = {
        let _ = bt.prompt();
        bt.drain()
    };
    let ganho = evs.iter().find_map(|e| match e {
        BattleEvent::FluxoGain { side: 0, slot: 0, amount } => Some(*amount),
        _ => None,
    });
    assert_eq!(ganho, Some(12), "Fluxo Estável não somou na abertura (10+2)");
}

/// Gatilho Rápido: uma carta que muda PORTÃO (R3→R2) E CUSTO (60→45).
#[test]
fn gatilho_rapido_antecipa_e_barateia() {
    let mut rs = RunState::new();
    rs.take(&CARD_POOL[pos_carta("gatilho-rapido")], Some(0)).expect("compra");
    let edge = rs.edge(&[(0, pos_carta("gatilho-rapido"))]);
    assert_eq!(edge.unit_unlock_early[0], 1);
    assert_eq!(edge.unit_mystery_discount[0], 15);

    let r = synth(); // s1 tem unlock R2 e mystery single — com a carta vira R1
    // A promessa "dispara na rodada 2" pede 45⚡ na r2: banking puro dá 35,
    // então somamos uma fonte de fluxo por rodada (ex.: passiva de chakra de
    // suporte / tática Fluxo Acelerado — o combo real da run).
    let mut edge_fl = edge;
    edge_fl.fluxo_per_round = 10;
    let mut bt = InteractiveBattle::new_with_edges(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7, edge_fl, SideEdge::default());
    assert_eq!(bt.mystery_cost(0, 0), 45, "desconto não chegou na engine");

    // r2: 10+15+10 (r1) + 10+10 (r2) = 45 ⚡ exatos → Mystery legal e pagável.
    let p = vez_de(&mut bt, 0, 0, 2);
    assert_eq!(p.round, 2);
    let legais = bt.legal_actions(0, 0);
    let mystery = legais.iter().find(|l| l.kind == ActionKind::Mystery);
    assert!(mystery.is_some(), "Gatilho Rápido não abriu a Mystery na r2");
    let antes = bt.unit(0, 0).fluxo;
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: Some(0) })
        .expect("mystery antecipada");
    assert_eq!(bt.unit(0, 0).fluxo, antes - 45);
    assert!(evs.iter().any(|e| matches!(e, BattleEvent::StateApplied { side: 1, slot: 0, .. })));
}

/// Último Suspiro: a 1ª morte do time revive com 20% do HP máximo — uma vez.
#[test]
fn ultimo_suspiro_revive_uma_vez() {
    let mut rs = RunState::new();
    rs.take(&CARD_POOL[pos_carta("ultimo-suspiro")], None).expect("compra");
    let edge = rs.edge(&[]);
    assert_eq!(edge.revive_pct, 20);

    let r = roster_of(&[
        CharSpec { id: "a0", name: "Batedor 1", faction: "ember", role: "striker", speed: 50, ..Default::default() },
        CharSpec { id: "a1", name: "Batedor 2", faction: "ember", role: "striker", speed: 49, ..Default::default() },
        filler("a2"),
        filler("a3"),
        CharSpec { id: "m1", name: "Frágil 1", faction: "tide", role: "guardian", speed: 30, hp: 30, ..Default::default() },
        CharSpec { id: "m2", name: "Frágil 2", faction: "tide", role: "guardian", speed: 29, hp: 30, ..Default::default() },
        filler("t3"),
        filler("t4"),
    ]);
    let mut bt =
        InteractiveBattle::new_with_edges(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7, SideEdge::default(), edge);
    // O lado B TEM a carta: as mortes de B revivem. a0 e a1 matam os frágeis.
    let mut revives = 0;
    for _ in 0..8 {
        if bt.is_over() {
            break;
        }
        let p = bt.prompt();
        if p.phase == turnrush_core::interactive::PromptPhase::ChooseAction {
            let evs = bt
                .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
                .expect("padrão");
            revives += evs.iter().filter(|e| matches!(e, BattleEvent::Revive { side: 1, .. })).count();
        }
    }
    assert_eq!(revives, 1, "revive deveria disparar exatamente 1 vez (1ª morte)");
    // Após o revive, o time B segue vivo com HP parcial.
    assert!(bt.unit(1, 0).alive || bt.unit(1, 1).alive);
}

/// Ambidestria: com MAIS estados pendentes na fila, a mesma unidade
/// persegue 2× por rodada (max_per_round + extra).
#[test]
fn ambidestria_abre_segunda_perseguicao() {
    let mut rs = RunState::new();
    rs.take(&CARD_POOL[pos_carta("ambidestria")], None).expect("compra");
    let edge = rs.edge(&[]);
    assert_eq!(edge.extra_chases, 1);

    // s1 lança a FRENTE inteira (2 estados na fila); s2 persegue.
    let r = roster_of(&[
        CharSpec {
            id: "s1",
            name: "Arremessador",
            faction: "ember",
            role: "striker",
            speed: 50,
            m_targets: "enemies_front",
            m_pct: 100,
            m_state: "launch",
            m_effect: "Arremessa os inimigos da frente.",
            unlock: Some(2),
            ..Default::default()
        },
        CharSpec {
            id: "s2",
            name: "Perseguidor",
            faction: "ember",
            role: "striker",
            speed: 45,
            chase_trigger: "launch",
            chase_pct: 25,
            chase_max: 1,
            ..Default::default()
        },
        filler("a2"),
        filler("a3"),
        filler("b1"),
        filler("b2"),
        filler("b3"),
        filler("b4"),
    ]);
    let mut bt =
        InteractiveBattle::new_with_edges(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7, edge, SideEdge::default());
    let _p = vez_de(&mut bt, 0, 0, 3);
    bt.execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: None })
        .expect("mystery de lançamento em área");
    // 1ª perseguição
    let p = bt.prompt();
    assert_eq!(p.phase, turnrush_core::interactive::PromptPhase::ConfirmChase);
    bt.execute_action(ChosenAction::Chase { chaser_slot: 1 }).expect("1ª chase");
    // 2ª perseguição: a fila ainda tem um estado e Ambidestria permite reusar s2.
    let p2 = bt.prompt();
    assert_eq!(
        p2.phase,
        turnrush_core::interactive::PromptPhase::ConfirmChase,
        "Ambidestria não reabriu a janela com estado pendente"
    );
    let evs = bt
        .execute_action(ChosenAction::Chase { chaser_slot: 1 })
        .expect("2ª chase com Ambidestria");
    assert_eq!(evs.iter().filter(|e| matches!(e, BattleEvent::ChaseResolved { .. })).count(), 1);
    // Fila vazia → recusa fecha o turno.
    let p3 = bt.prompt();
    if p3.phase == turnrush_core::interactive::PromptPhase::ConfirmChase {
        bt.execute_action(ChosenAction::RejectChase).expect("recusa final");
    }
    let p4 = bt.prompt();
    assert_eq!(p4.phase, turnrush_core::interactive::PromptPhase::ChooseAction);
}

/// Eco de Combate: o bônus cresce POR elo já executado na rodada — o 1º elo
/// não ganha nada (não há cadeia ainda); o 2º ganha +10%; teto +30%.
#[test]
fn eco_de_combate_potencializa_cadeias() {
    let mut rs = RunState::new();
    rs.take(&CARD_POOL[pos_carta("eco-de-combate")], None).expect("compra");
    let edge = rs.edge(&[]);
    assert_eq!(edge.chain_dmg_per_link, 100);
    assert_eq!(edge.chain_dmg_cap, 300);

    // s1 lança a frente (2 estados); s2 e s3 perseguem → cadeia de 2 elos.
    let r = roster_of(&[
        CharSpec {
            id: "s1",
            name: "Arremessador",
            faction: "ember",
            role: "striker",
            speed: 50,
            m_targets: "enemies_front",
            m_pct: 100,
            m_state: "launch",
            m_effect: "Arremessa os inimigos da frente.",
            unlock: Some(2),
            ..Default::default()
        },
        CharSpec {
            id: "s2",
            name: "Perseguidor 1",
            faction: "ember",
            role: "striker",
            speed: 46,
            chase_trigger: "launch",
            chase_pct: 25,
            chase_max: 1,
            ..Default::default()
        },
        CharSpec {
            id: "s3",
            name: "Perseguidor 2",
            faction: "ember",
            role: "striker",
            speed: 44,
            chase_trigger: "launch",
            chase_pct: 25,
            chase_max: 1,
            ..Default::default()
        },
        filler("a3"),
        filler("b1"),
        filler("b2"),
        filler("b3"),
        filler("b4"),
    ]);
    let dano_total = |edge: SideEdge| -> i32 {
        let mut bt = InteractiveBattle::new_with_edges(
            &r, [0, 1, 2, 3], [4, 5, 6, 7], 7, edge, SideEdge::default(),
        );
        let _p = vez_de(&mut bt, 0, 0, 3);
        bt.execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: None })
            .expect("mystery");
        let mut total = 0;
        for chaser in [1usize, 2] {
            let p = bt.prompt();
            if p.phase != turnrush_core::interactive::PromptPhase::ConfirmChase {
                break;
            }
            let Ok(evs) = bt.execute_action(ChosenAction::Chase { chaser_slot: chaser }) else {
                break;
            };
            total += evs
                .iter()
                .find_map(|e| match e {
                    BattleEvent::ChaseResolved { amount, .. } => Some(*amount),
                    _ => None,
                })
                .unwrap_or(0);
        }
        total
    };
    let com_eco = dano_total(edge);
    let sem_eco = dano_total(SideEdge::default());
    assert!(com_eco > sem_eco, "Eco de Combate não potenciou a cadeia ({com_eco} vs {sem_eco})");
}

/// Draft 1v1: oferta única e simétrica; LCE promove quem está atrás.
#[test]
fn draft_simetrico_com_lce() {
    use turnrush_core::cards;
    let mut rng = Rng::new(42);
    let oferta = cards::offer(&mut rng, 3);
    assert_eq!(oferta.len(), 3);
    let mut vistos = oferta.to_vec();
    vistos.sort_unstable();
    vistos.dedup();
    assert_eq!(vistos.len(), 3, "oferta com repetição");

    let mut a = cards::DraftState::default();
    let mut b = cards::DraftState::default();
    let (ea, eb) = cards::resolve_picks(oferta[0], oferta[1], &mut a, &mut b);
    assert_eq!(a.value, cards::CARD_POOL[ea].value);
    assert_eq!(b.value, cards::CARD_POOL[eb].value);
    assert_eq!(a.picks.len(), 1);
    assert_eq!(b.picks.len(), 1);

    // LCE: atrás por ≥ 20 → a carta efetiva nunca fica abaixo do tier escolhido.
    let promovida = cards::effective_card(oferta[0], 0, cards::CARD_POOL[oferta[0]].value + 40);
    assert!(cards::CARD_POOL[promovida].tier >= cards::CARD_POOL[oferta[0]].tier);
}

/// A run inteira (compra → edge → batalha) fica sob o teto de poder (+30%).
#[test]
fn teto_da_run_e_trinta_por_cento() {
    assert_eq!(turnrush_core::rewards::PODER_CAP, 300);
    let mut rs = RunState::new();
    let mut rng = Rng::new(9);
    let faccoes = [turnrush_core::data::Faction::Ember];
    let mut rodadas_de_compra = 0;
    while rodadas_de_compra < 60 {
        let oferta = rs.offer(&mut rng, 2, &faccoes);
        if oferta.is_empty() {
            break;
        }
        let carta = oferta[0];
        let alvo = if carta.class == CardClass::Unit { Some(0) } else { None };
        if rs.take(carta, alvo).is_err() {
            break;
        }
        rodadas_de_compra += 1;
        assert!(rs.poder <= turnrush_core::rewards::PODER_CAP, "teto furado: {}", rs.poder);
    }
    assert!(rs.poder_restante() == 0 || rodadas_de_compra < 60, "teto não travou compras");
}

/// Conector final do core: a Peça 9 (cliente) roda sobre tudo isto.
#[test]
fn peca_08_encaixa_na_peca_09() {
    ancora_dados();
    ancora_economia();
}
