//! Peça 1 do quebra-cabeça: **fundação de dados**.
//!
//! Nada acima dela pode encaixar se o contrato de dados estiver torto:
//! roster v2 (16 personagens), bandas do orçamento de balanceamento,
//! cartas da run com efeito real, e o compromisso v0.8 de honestidade —
//! `unmodeled` VAZIO (toda passiva/assinatura tem efeito modelado).

mod common;

use common::*;
use turnrush_core::data::{Faction, InterruptKind, Role, TargetSpec};
use turnrush_core::interactive::InteractiveBattle;
use turnrush_core::rewards::{CardClass as RunCardClass, RunState, CARD_POOL, TACTIC_POOL};
use turnrush_core::rng::Rng;

#[test]
fn peca_01_base_do_quebra_cabeca() {
    let r = real_roster();
    assert_eq!(r.version, 2, "contrato v2 é a fundação");
    assert_eq!(r.characters.len(), 16, "4 clãs × 4 classes");
}

#[test]
fn ids_e_nomes_sao_unicos() {
    let r = real_roster();
    let mut ids: Vec<&str> = r.characters.iter().map(|c| c.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 16, "ids duplicados no roster");
    let mut nomes: Vec<&str> = r.characters.iter().map(|c| c.name.as_str()).collect();
    nomes.sort_unstable();
    nomes.dedup();
    assert_eq!(nomes.len(), 16, "nomes duplicados no roster");
}

#[test]
fn quatro_clas_vez_quatro_papeis() {
    let r = real_roster();
    let mut por_cla: Vec<(Faction, Vec<Role>)> = Vec::new();
    for c in &r.characters {
        match por_cla.iter_mut().find(|(f, _)| *f == c.faction) {
            Some((_, v)) => v.push(c.role),
            None => por_cla.push((c.faction, vec![c.role])),
        }
    }
    assert_eq!(por_cla.len(), 4, "exatamente 4 clãs");
    for (f, papeis) in &por_cla {
        assert_eq!(papeis.len(), 4, "clã {f:?} sem os 4 papéis");
        for papel in [Role::Striker, Role::Guardian, Role::Support, Role::Controller] {
            assert!(papeis.contains(&papel), "clã {f:?} sem papel {papel:?}");
        }
    }
}

/// Bandas obrigatórias do contrato (NOTAS-DESIGN §4).
#[test]
fn orcamento_de_balanceamento_respeitado() {
    let r = real_roster();
    for c in &r.characters {
        assert!((850..=1150).contains(&c.hp), "{} hp {} fora de 850–1150", c.id, c.hp);
        assert!(
            (60..=80).contains(&c.standard.damage_pct),
            "{} standard {} fora de 60–80",
            c.id,
            c.standard.damage_pct
        );
        // v0.8: banda da mystery depende da forma de alvo — alvo único (striker)
        // = 200%; multi/escudo/cura = 80–100.
        if c.mystery.targets == TargetSpec::EnemySingle {
            assert_eq!(c.mystery.damage_pct, 200, "{} mystery single ≠ 200%", c.id);
        } else {
            assert!(
                (80..=100).contains(&c.mystery.damage_pct),
                "{} mystery {} fora de 80–100",
                c.id,
                c.mystery.damage_pct
            );
        }
        assert!(
            (15..=30).contains(&c.mystery.chakra),
            "{} chakra {} fora de 15–30",
            c.id,
            c.mystery.chakra
        );
        assert!(
            (2..=4).contains(&c.mystery.cooldown),
            "{} cooldown {} fora de 2–4",
            c.id,
            c.mystery.cooldown
        );
        assert!(
            (20..=35).contains(&c.chase.damage_pct),
            "{} chase {} fora de 20–35",
            c.id,
            c.chase.damage_pct
        );
        assert_eq!(c.chase.max_per_round, 1, "{}: 1 perseguição por rodada", c.id);
        assert!((36..=70).contains(&c.speed), "{} speed {} fora de 36–70", c.id, c.speed);
        assert!(
            (36..=62).contains(&c.defense),
            "{} defense {} fora de 36–62",
            c.id,
            c.defense
        );
        // Estados e alvos são enum válidos (o loader já recusa lixo, mas o
        // encaixe exige que TODOS os textos de origem parseiem limpo).
        assert!(alvo_valido(c.mystery.targets.as_str()));
        assert!(estado_valido(c.standard.applies_state.as_str()));
        assert!(estado_valido(c.chase.trigger.as_str()));
        assert!(estado_valido(c.chase.applies_state.as_str()));
        assert!(estado_valido(c.mystery.applies_state.as_str()));
    }
}

/// v0.8: unlock da Mystery é R3 uniforme (60⚡ não dá antes com +10/rodada).
#[test]
fn unlock_da_mystery_e_uniforme_r3() {
    let r = real_roster();
    for c in &r.characters {
        assert_eq!(c.mystery_unlock_round, 3, "{} deve destravar a Mystery na R3", c.id);
    }
}

/// Interrupt: identidade dos 4 controladores (nenhum outro papel tem).
#[test]
fn interrupt_e_identidade_dos_controladores() {
    let r = real_roster();
    for c in &r.characters {
        if c.role == Role::Controller {
            let def = c.interrupt.expect("controller sem interrupt");
            assert!(
                matches!(def.kind, InterruptKind::Stun | InterruptKind::Silence),
                "{} interrupt inválido",
                c.id
            );
            assert!(def.window >= 1, "{} janela de interrupt < 1", c.id);
        } else {
            assert!(c.interrupt.is_none(), "{} não-controller tem interrupt", c.id);
        }
    }
}

/// v0.8: strikers miram alvo único (Tina espalha veneno com random_2);
/// ninguém além dos strikers define skill_targets (herdam a mystery).
#[test]
fn formas_de_alvo_por_papel() {
    let r = real_roster();
    for c in &r.characters {
        match c.role {
            Role::Striker if c.id != "tina" => {
                assert_eq!(
                    c.skill_targets,
                    Some(TargetSpec::EnemySingle),
                    "{} striker deve mirar 1 alvo",
                    c.id
                );
            }
            // Tina não declara skill_targets: a Skill HERDA a random_2 da
            // mystery — espalhar Veneno é a assinatura dela nas duas formas.
            Role::Striker => {
                assert!(c.skill_targets.is_none(), "tina deve herdar a mystery");
                assert_eq!(
                    alvo_mystery(c),
                    TargetSpec::Random2,
                    "mystery da tina deve ser random_2"
                );
            }
            _ => assert!(c.skill_targets.is_none(), "{} herda a mystery", c.id),
        }
    }
}

/// Toda passiva existe e é única (v0.8: variedade > uniformidade).
#[test]
fn passivas_presentes_e_unicas() {
    let r = real_roster();
    let mut nomes: Vec<&str> = Vec::new();
    for c in &r.characters {
        assert!(!c.passive.name.is_empty(), "{} sem passiva", c.id);
        assert!(!c.passive.effect.is_empty(), "{} passiva sem efeito", c.id);
        nomes.push(c.passive.name.as_str());
    }
    nomes.sort_unstable();
    let n = nomes.len();
    nomes.dedup();
    assert_eq!(nomes.len(), n, "passivas duplicadas — kit deve ser único");
}

/// O pool da run tem efeito real: ids únicos, poder dentro do teto e
/// pelo menos um campo mecânico preenchido.
#[test]
fn cartas_da_run_tem_efeito_real() {
    assert!(CARD_POOL.len() >= 12, "pool da run pequeno demais");
    assert_eq!(TACTIC_POOL.len(), 4, "oferta de tática R1 = 2 de um pool de 4");
    let mut ids: Vec<&str> = Vec::new();
    for c in CARD_POOL.iter().chain(TACTIC_POOL.iter()) {
        ids.push(c.id);
        // Táticas têm escopo DE BATALHA (v0.7.1): poder 0, fora do teto da run.
        if c.id.starts_with("t-") {
            assert_eq!(c.poder, 0, "tática '{}' deve ser fora do teto", c.id);
        } else {
            assert!(c.poder > 0, "carta '{}' sem orçamento de poder", c.id);
            assert!(c.poder <= turnrush_core::rewards::PODER_CAP);
        }
        let tem_efeito = c.team_atk != 0
            || c.team_hp != 0
            || c.fluxo_per_round != 0
            || c.chain_per_link != 0
            || c.extra_chases != 0
            || c.revive_pct != 0
            || c.unlock_early != 0
            || c.mystery_discount != 0
            || c.unit_atk != 0
            || c.unit_hp != 0
            || c.team_dmg_reduction != 0
            || c.round_end_heal_pct != 0;
        assert!(tem_efeito, "carta '{}' é placebo", c.id);
        if c.class == RunCardClass::Unit {
            assert!(
                c.unit_atk != 0 || c.unit_hp != 0 || c.mystery_discount != 0 || c.unlock_early != 0,
                "carta de unidade '{}' sem efeito de unidade",
                c.id
            );
        }
    }
    ids.sort_unstable();
    let n = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), n, "ids de cartas duplicados");
}

/// A economia da oferta respeita o teto: perto do fim, sobra vazio — nunca erro.
#[test]
fn oferta_respeita_teto_de_poder() {
    let mut rs = RunState::new();
    let mut rng = Rng::new(11);
    let faccoes = [Faction::Ember];
    let mut compras = 0;
    while compras < 40 {
        let oferta = rs.offer(&mut rng, 3, &faccoes);
        if oferta.is_empty() {
            break;
        }
        let carta = oferta[0];
        let alvo = if carta.class == turnrush_core::rewards::CardClass::Unit { Some(0) } else { None };
        if rs.take(carta, alvo).is_ok() {
            compras += 1;
        } else {
            break;
        }
    }
    assert!(rs.poder <= turnrush_core::rewards::PODER_CAP, "teto de poder estourado");
    assert!(
        rs.poder_restante() == 0 || rs.offer(&mut rng, 1, &faccoes).is_empty() || compras < 40,
        "oferta deveria esvaziar no teto"
    );
}

/// COMPROMISSO v0.8 (o encaixe mais importante desta peça): construir
/// batalhas com o roster REAL não produz nenhuma nota `unmodeled` —
/// assinaturas mecânicas 100% modeladas, honestidade contratual.
#[test]
fn unmodeled_fica_vazio_no_roster_real() {
    let r = real_roster();
    let comps = [
        ([0, 1, 2, 3], [4, 5, 6, 7]),
        ([4, 5, 6, 7], [8, 9, 10, 11]),
        ([8, 9, 10, 11], [12, 13, 14, 15]),
        ([12, 13, 14, 15], [0, 1, 2, 3]),
    ];
    for (a, b) in comps {
        let mut bt = InteractiveBattle::new(&r, a, b, 99);
        let _ = bt.prompt();
        let notas = bt.unmodeled();
        assert!(
            notas.is_empty(),
            "comp {a:?} vs {b:?} tem mecânicas não modeladas: {notas:?}"
        );
    }
}

/// Conector: a fundação carrega as 16 peças do elenco para as peças seguintes.
#[test]
fn peca_01_encaixa_nas_proximas() {
    assert_eq!(ancora_dados(), 16);
}
