//! Peça 5 do quebra-cabeça: **passivas e assinaturas mecânicas**.
//!
//! Encaixa na Peça 4: bônus condicionais só são testáveis sabendo exatamente
//! quem mira quem. Cada família de token do contrato v2 tem aqui um
//! mini-roster dedicado e um assert comportamental (não textual): a passiva
//! EXISTE, é PARSEADA e MUDA o combate.

mod common;

use common::*;
use turnrush_core::interactive::{
    BattleEvent, ChosenAction, InteractiveBattle, PromptPhase,
};
use turnrush_core::interactive::ActionKind;

/// Coleta todos os eventos rodando a batalha com padrões+táticas até a
/// rodada pedida (ou o fim). O retorno de cada execute é coletado — é onde
/// vivem os eventos de fecho de rodada (curas EoR, RoundEnd).
fn eventos_de_corrida(
    r: &turnrush_core::data::Roster,
    a: [usize; 4],
    b: [usize; 4],
    seed: u64,
    rodadas_max: usize,
) -> Vec<BattleEvent> {
    let mut bt = InteractiveBattle::new(r, a, b, seed);
    let mut evs = Vec::new();
    for _ in 0..60 {
        if bt.is_over() || bt.round() > rodadas_max {
            break;
        }
        evs.extend(bt.drain());
        let p = bt.prompt();
        let passos = match p.phase {
            PromptPhase::ChooseTactic => {
                bt.execute_action(ChosenAction::PickTactic { index: 0 })
            }
            PromptPhase::ConfirmChase => bt.execute_action(ChosenAction::RejectChase),
            _ => bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None }),
        };
        evs.extend(passos.unwrap_or_default());
    }
    evs.extend(bt.drain());
    evs
}

#[test]
fn peca_05_encaixa_na_peca_04() {
    ancora_economia();
}

/// Reflexo: quem bate no reflexivo leva de volta X% do dano (número exato).
#[test]
fn passiva_reflete_porcentagem_exata() {
    let r = roster_of(&[
        CharSpec { id: "a0", name: "Batedor", faction: "ember", role: "striker", speed: 50, ..Default::default() },
        filler("a1"),
        filler("a2"),
        filler("a3"),
        CharSpec {
            id: "rf",
            name: "Espinhoso",
            faction: "tide",
            role: "guardian",
            speed: 30,
            hp: 1200,
            attack: 90,
            defense: 50,
            passive: "Reflete 15% do dano.",
            ..Default::default()
        },
        filler("t1"),
        filler("t2"),
        filler("t3"),
    ]);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let _p = vez_de(&mut bt, 0, 0, 1);
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão no espinhoso");
    let d = ultimo_dano(&evs, 1, 0);
    let volta = ultimo_dano(&evs, 0, 0);
    assert!(d > 0 && volta > 0, "reflexo não disparou (d={d}, volta={volta})");
    assert_eq!(volta, d * 150 / 1000, "reflexo ≠ 15% do dano");
}

/// Redução "na linha de frente" (v0.8): vale só nos slots 0–1.
#[test]
fn reducao_frontal_condicional_de_verdade() {
    let boneco = |id: &'static str| CharSpec { id, name: "Alvo Limpo", ..Default::default() };
    let r = roster_of(&[
        CharSpec { id: "a0", name: "Batedor 1", faction: "ember", role: "striker", speed: 50, ..Default::default() },
        filler("a1"),
        CharSpec { id: "a2", name: "Batedor 2", faction: "ember", role: "striker", speed: 50, ..Default::default() },
        filler("a3"),
        CharSpec {
            id: "fr",
            name: "Muralha Frontal",
            faction: "tide",
            role: "guardian",
            speed: 30,
            hp: 1200,
            attack: 90,
            defense: 50,
            passive: "-10% de dano recebido na linha de frente.",
            ..Default::default()
        },
        boneco("t1"),
        boneco("t2"),
        boneco("t3"),
    ]);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let _p = vez_de(&mut bt, 0, 0, 1); // a0 é o gêmeo que bate na MURALHA frontal
    let evs_a0 = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão no frontal");
    let _p = vez_de(&mut bt, 0, 2, 1); // a2 = gêmeo do a0, espelho limpo (1,2)
    let evs_a2 = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão no gêmeo");
    let d_frontal = ultimo_dano(&evs_a0, 1, 0);
    let d_tras = ultimo_dano(&evs_a2, 1, 2);
    assert!(d_frontal > 0 && d_tras > 0);
    assert!(d_frontal < d_tras, "redução frontal não reduziu ({d_frontal} vs {d_tras})");
}

/// Cura focada de fim de rodada: o aliado com menos HP recupera X% do HP máx.
#[test]
fn cura_fim_de_rodada_no_mais_ferido() {
    let r = roster_of(&[
        CharSpec { id: "a0", name: "Ponta", faction: "ember", role: "striker", speed: 50, ..Default::default() },
        filler("a1"),
        filler("a2"),
        CharSpec {
            id: "hm",
            name: "Madrinha",
            faction: "ember",
            role: "support",
            speed: 35,
            hp: 1000,
            m_targets: "ally_single",
            m_pct: 100,
            m_effect: "Cura 100% do ataque.",
            st_pct: 60,
            passive: "No fim da rodada, cura 5% do HP do aliado com menos HP.",
            ..Default::default()
        },
        filler("t1"),
        filler("t2"),
        filler("t3"),
        filler("t4"),
    ]);
    // Roda a r1 inteira (fecho incluído): b0 machuca a0, no fecho entra a cura.
    let evs = eventos_de_corrida(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7, 1);
    let heal_fim = evs.iter().find_map(|e| match e {
        BattleEvent::Heal { side: 0, slot, amount } if *amount == 50 => Some(*slot),
        _ => None,
    });
    assert!(heal_fim.is_some(), "cura 5% (50 de 1000) não apareceu no fecho da r1");
}

/// Cura em área de fim de rodada (Yara): TODOS os aliados curam 2%.
#[test]
fn cura_em_area_fim_de_rodada() {
    let r = roster_of(&[
        CharSpec { id: "a0", name: "Ponta", faction: "ember", role: "striker", speed: 50, ..Default::default() },
        CharSpec { id: "a1", name: "Ponta 2", faction: "ember", role: "striker", speed: 49, ..Default::default() },
        filler("a2"),
        CharSpec {
            id: "ya",
            name: "Guardiã da Seiva",
            faction: "ember",
            role: "support",
            speed: 35,
            hp: 1000,
            m_targets: "ally_single",
            m_pct: 100,
            m_effect: "Cura 100% do ataque.",
            st_pct: 60,
            passive: "No fim da rodada, todos os aliados curam 2% do HP máximo.",
            ..Default::default()
        },
        filler("t1"),
        filler("t2"),
        filler("t3"),
        filler("t4"),
    ]);
    let evs = eventos_de_corrida(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7, 1);
    let curas: Vec<(usize, i32)> = evs
        .iter()
        .filter_map(|e| match e {
            BattleEvent::Heal { side: 0, slot, amount } => Some((*slot, *amount)),
            _ => None,
        })
        .collect();
    assert!(curas.len() >= 2, "cura em área deve alcançar vários aliados: {curas:?}");
    let mut slots: Vec<usize> = curas.iter().map(|(s, _)| *s).collect();
    slots.sort_unstable();
    slots.dedup();
    assert!(slots.len() >= 2, "cura em área deve atingir slots distintos: {curas:?}");
    for (_, amount) in &curas {
        assert_eq!(*amount, 20, "cura em área deve ser 2% de 1000");
    }
}

/// Muro incandescente (Orson): quem bate com padrão no holder pega Queimadura 1 rodada.
#[test]
fn queimadura_de_retorno_ao_bater() {
    let r = roster_of(&[
        CharSpec { id: "a0", name: "Batedor", faction: "ember", role: "striker", speed: 50, ..Default::default() },
        filler("a1"),
        filler("a2"),
        filler("a3"),
        CharSpec {
            id: "mu",
            name: "Muro Incandescente",
            faction: "tide",
            role: "guardian",
            speed: 30,
            hp: 1200,
            attack: 90,
            defense: 50,
            passive: "Inimigos que o acertam com ataque padrão pegam Queimadura por 1 rodada.",
            ..Default::default()
        },
        filler("t1"),
        filler("t2"),
        filler("t3"),
    ]);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let _p = vez_de(&mut bt, 0, 0, 1);
    bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão no muro");
    assert_eq!(bt.unit(0, 0).dot_turns, 1, "atacante deveria estar queimando (1 rodada)");
    // r2: o tick da queimadura vem na abertura.
    let _p = vez_de(&mut bt, 0, 0, 2);
    assert!(bt.unit(0, 0).hp < bt.unit(0, 0).hp_max, "tick da queimadura não descontou HP");
}

/// Veneno nos ataques (Tina): o alvo fica 2 rodadas com dano contínuo.
#[test]
fn ataques_aplicam_veneno_por_dois_turnos() {
    let r = roster_of(&[
        CharSpec {
            id: "po",
            name: "FarpaVeneno",
            faction: "verdant",
            role: "striker",
            speed: 50,
            passive: "Os ataques aplicam Veneno (dano contínuo).",
            ..Default::default()
        },
        filler("a1"),
        filler("a2"),
        filler("a3"),
        CharSpec { id: "t1", name: "Alvo", faction: "tide", role: "guardian", speed: 30, hp: 1200, attack: 90, defense: 50, ..Default::default() },
        filler("t2"),
        filler("t3"),
        filler("t4"),
    ]);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let _p = vez_de(&mut bt, 0, 0, 1);
    bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
        .expect("padrão venenoso");
    assert_eq!(bt.unit(1, 0).dot_turns, 2, "veneno deve durar 2 rodadas");
}

/// Bônus contra alvos envenenados: compara batalhas gêmeas (com/sem DoT no alvo).
#[test]
fn bonus_contra_veneno_muda_o_dano() {
    let mk = |a0_passiva: &'static str| {
        roster_of(&[
            CharSpec {
                id: "a0",
                name: "Aplicador",
                faction: "verdant",
                role: "striker",
                speed: 50,
                passive: a0_passiva,
                ..Default::default()
            },
            CharSpec {
                id: "a1",
                name: "Punidor",
                faction: "verdant",
                role: "striker",
                speed: 49,
                passive: "+25% de dano contra alvos em Veneno.",
                ..Default::default()
            },
            filler("a2"),
            filler("a3"),
            CharSpec { id: "t1", name: "Cobaia", faction: "tide", role: "guardian", speed: 30, hp: 4000, attack: 90, defense: 50, ..Default::default() },
            filler("t2"),
            filler("t3"),
            filler("t4"),
        ])
    };
    let golpe_r2 = |r: &turnrush_core::data::Roster| -> i32 {
        let mut bt = InteractiveBattle::new(r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
        let _p = vez_de(&mut bt, 0, 1, 2); // r1: a0 envenena a cobaia; r2: vez do punidor
        let evs = bt
            .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: Some(0) })
            .expect("golpe do punidor");
        ultimo_dano(&evs, 1, 0)
    };
    let com_dot = golpe_r2(&mk("Os ataques aplicam Veneno (dano contínuo)."));
    let sem_dot = golpe_r2(&mk(""));
    assert!(com_dot > sem_dot, "bônus vs Veneno inerte ({com_dot} vs {sem_dot})");
}

/// Buff de dano incondicional "+X% de dano" aplica de fato.
#[test]
fn buff_de_dano_incondicional() {
    let mk = |passiva: &'static str| {
        roster_of(&[
            CharSpec { id: "a0", name: "Furioso", faction: "ember", role: "striker", speed: 50, passive: passiva, ..Default::default() },
            filler("a1"),
            filler("a2"),
            filler("a3"),
            CharSpec { id: "t1", name: "Alvo", faction: "tide", role: "guardian", speed: 30, hp: 4000, attack: 90, defense: 50, ..Default::default() },
            filler("t2"),
            filler("t3"),
            filler("t4"),
        ])
    };
    let golpe = |r: &turnrush_core::data::Roster| -> i32 {
        let mut bt = InteractiveBattle::new(r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
        let _p = vez_de(&mut bt, 0, 0, 1);
        let evs = bt
            .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
            .expect("padrão");
        ultimo_dano(&evs, 1, 0)
    };
    let com = golpe(&mk("+10% de dano."));
    let sem = golpe(&mk(""));
    assert!(com > sem, "buff de dano inerte ({com} vs {sem})");
}

/// Dissipação (Mel): mystery de cura limpa Veneno/Queimadura do alvo.
#[test]
fn dissipacao_limpa_dot() {
    let r = roster_of(&[
        CharSpec { id: "a0", name: "Ponta", faction: "ember", role: "striker", speed: 50, ..Default::default() },
        filler("a1"),
        filler("a2"),
        CharSpec {
            id: "me",
            name: "Maré Mansa",
            faction: "tide",
            role: "support",
            speed: 35,
            hp: 900,
            m_targets: "ally_single",
            m_pct: 100,
            m_effect: "Cura 100% do ataque e dissipa Veneno ou Queimadura do alvo.",
            st_pct: 60,
            ..Default::default()
        },
        CharSpec {
            id: "ve",
            name: "Venenosa",
            faction: "verdant",
            role: "striker",
            speed: 30,
            passive: "Os ataques aplicam Veneno (dano contínuo).",
            ..Default::default()
        },
        filler("t2"),
        filler("t3"),
        filler("t4"),
    ]);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    // r1: a venenosa bate em a0 (espelho) → a0 envenenado.
    // r1 e r2 com padrões; r3: mystery da curandeira (60 ⚡) dissipa.
    let _p = vez_de(&mut bt, 0, 3, 3);
    assert!(bt.unit(0, 0).dot_turns > 0, "a0 deveria estar envenenado antes da dissipação");
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: Some(0) })
        .expect("mystery com dissipação");
    assert_eq!(bt.unit(0, 0).dot_turns, 0, "dissipação não limpou o veneno");
    assert!(evs.iter().any(|e| matches!(e, BattleEvent::Heal { side: 0, slot: 0, amount } if *amount > 0)));
}

/// Regeneração (Yara): alvo curado recupera 6%/rodada durante 2 rodadas.
#[test]
fn regeneracao_cura_por_rodadas() {
    let r = roster_of(&[
        CharSpec { id: "a0", name: "Ponta", faction: "ember", role: "striker", speed: 50, ..Default::default() },
        filler("a1"),
        filler("a2"),
        CharSpec {
            id: "re",
            name: "Seiva",
            faction: "verdant",
            role: "support",
            speed: 35,
            hp: 900,
            m_targets: "ally_single",
            m_pct: 90,
            m_effect: "Cura 90% do ataque. O alvo regenera 6% do HP máximo durante 2 rodadas.",
            st_pct: 60,
            ..Default::default()
        },
        filler("t1"),
        filler("t2"),
        filler("t3"),
        filler("t4"),
    ]);
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let _p = vez_de(&mut bt, 0, 3, 3);
    let evs = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: Some(0) })
        .expect("mystery com regeneração");
    // O pulso de regeneração entra no FECHO da rodada (60 = 6% de 1000).
    let mut evs_fim = Vec::new();
    for _ in 0..8 {
        if bt.is_over() {
            break;
        }
        evs_fim.extend(bt.drain());
        let p = bt.prompt();
        let passos = match p.phase {
            PromptPhase::ChooseTactic => {
                bt.execute_action(ChosenAction::PickTactic { index: 0 })
            }
            PromptPhase::ConfirmChase => bt.execute_action(ChosenAction::RejectChase),
            _ => bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None }),
        };
        evs_fim.extend(passos.unwrap_or_default());
        if bt.round() > 3 {
            break;
        }
    }
    evs_fim.extend(bt.drain());
    let tudo: Vec<&BattleEvent> = evs.iter().chain(evs_fim.iter()).collect();
    assert!(
        tudo.iter().any(|e| matches!(e, BattleEvent::Heal { side: 0, slot: 0, amount } if *amount == 60)),
        "pulso de regeneração 6% (60) ausente"
    );
}

/// Escudo crescente (Aro): compara batalhas gêmeas com/sem o crescimento.
#[test]
fn escudo_crescente_cresce_de_verdade() {
    let mk = |efeito: &'static str| {
        roster_of(&[
            filler("a0"),
            filler("a1"),
            filler("a2"),
            filler("a3"),
            CharSpec {
                id: "ar",
                name: "Casca Viva",
                faction: "verdant",
                role: "guardian",
                speed: 30,
                hp: 1200,
                attack: 90,
                defense: 50,
                m_targets: "self",
                m_pct: 80,
                m_effect: efeito,
                st_pct: 65,
                ..Default::default()
            },
            filler("t2"),
            filler("t3"),
            filler("t4"),
        ])
    };
    let escudo_r4 = |efeito: &'static str| -> i32 {
        let r = mk(efeito);
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
        let _p = vez_de(&mut bt, 1, 0, 3); // banking até 60 ⚡
        bt.execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: None })
            .expect("mystery do escudo");
        assert!(bt.unit(1, 0).shield > 0);
        let _p = vez_de(&mut bt, 1, 0, 4); // r4: escudo cresceu no fecho da r3
        bt.unit(1, 0).shield
    };
    let cresce = escudo_r4("Escudo de 80% do ataque em si. Ao fim de cada rodada, o escudo ganha +25% do ataque.");
    let parado = escudo_r4("Escudo de 80% do ataque em si.");
    assert!(cresce > parado, "escudo crescente não cresceu ({cresce} vs {parado})");
}

/// Aceleração (Alísio): +25 de SPD faz o alvo agir antes dos aliados mais rápidos.
#[test]
fn aceleracao_muda_a_ordem_da_proxima_rodada() {
    let r = roster_of(&[
        CharSpec { id: "a0", name: "Ponta", faction: "storm", role: "striker", speed: 28, ..Default::default() },
        CharSpec { id: "a1", name: "Tanque 1", faction: "storm", role: "guardian", speed: 50, hp: 1200, attack: 40, defense: 50, m_pct: 80, m_targets: "self", m_effect: "Escudo de 80% do ataque em si.", ..Default::default() },
        CharSpec { id: "a2", name: "Tanque 2", faction: "storm", role: "guardian", speed: 49, hp: 1200, attack: 40, defense: 50, m_pct: 80, m_targets: "self", m_effect: "Escudo de 80% do ataque em si.", ..Default::default() },
        CharSpec {
            id: "al",
            name: "Vento Leve",
            faction: "storm",
            role: "support",
            speed: 35,
            hp: 900,
            m_targets: "ally_single",
            m_pct: 90,
            m_effect: "Cura 90% do ataque e concede aceleração de 25 de velocidade na próxima rodada.",
            st_pct: 60,
            ..Default::default()
        },
        CharSpec { id: "b0", name: "Gatilho", faction: "tide", role: "guardian", speed: 30, hp: 4000, attack: 90, defense: 50, ..Default::default() },
        filler("t2"),
        filler("t3"),
        filler("t4"),
    ]);
    let comp = ([0, 1, 2, 3], [4, 5, 6, 7]);
    let seed = seed_com_starter(comp.0, comp.1, &r, 4); // r4 começa pelo lado A
    let mut bt = InteractiveBattle::new(&r, comp.0, comp.1, seed);
    let _p = vez_de(&mut bt, 0, 3, 3);
    let evs_cast = bt
        .execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: Some(0) })
        .expect("mystery de aceleração");
    assert!(evs_cast.iter().any(|e| matches!(e, BattleEvent::Heal { side: 0, .. })));

    // Roda até o fim da r4 e coleta tudo; a fronteira da r4 é o RoundStart(4).
    let mut evs = evs_cast;
    for _ in 0..60 {
        if bt.is_over() || bt.round() > 4 {
            break;
        }
        evs.extend(bt.drain());
        let p = bt.prompt();
        let passos = match p.phase {
            PromptPhase::ChooseTactic => {
                bt.execute_action(ChosenAction::PickTactic { index: 0 })
            }
            PromptPhase::ConfirmChase => bt.execute_action(ChosenAction::RejectChase),
            _ => bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None }),
        };
        evs.extend(passos.unwrap_or_default());
    }
    evs.extend(bt.drain());
    let limite_r4 = evs
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, BattleEvent::RoundStart { round: 4, .. }))
        .map(|(i, _)| i)
        .next()
        .expect("r4 deveria começar");
    let primeira_decl_r4 = evs[limite_r4..].iter().find_map(|e| match e {
        BattleEvent::ActionDeclared { side, slot, .. } => Some((*side, *slot)),
        _ => None,
    });
    // a0 tem 28 de SPD (o último do lado A sem ajuda); acelerado com +25 vira 53,
    // à frente dos tanques de 50/49 — com a r4 abrindo pelo lado A, ele abre a rodada.
    assert_eq!(
        primeira_decl_r4,
        Some((0, 0)),
        "aceleração não colocou o alvo (28+25) na frente dos tanques (50)"
    );
}

#[test]
fn conectores_da_peca_05() {
    // Peça 6 (kits) pode confiar que cada família de passiva funciona.
    ancora_dados();
}
