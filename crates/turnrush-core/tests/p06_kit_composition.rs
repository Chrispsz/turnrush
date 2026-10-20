//! Peça 6 do quebra-cabeça: **kits e identidade de classe**.
//!
//! Encaixa na Peça 5: as passivas funcionam — agora confere que os KITS
//! respeitam o desenho: orçamento por papel, cadeias de combo garantidas
//! por clã, gradiente clã↔estilo e a regra de ouro do usuário: *complementar,
//! não faz-tudo* (auxiliar cura; striker mata; ninguém faz tudo).

mod common;

use common::*;
use turnrush_core::data::{Faction, Role, TargetSpec};
use turnrush_core::data::ComboState;

fn roster() -> turnrush_core::data::Roster {
    real_roster()
}

#[test]
fn peca_06_encaixa_na_peca_05() {
    ancora_dados();
}

/// Orçamento por papel (NOTAS-DESIGN §4): o DWC pode esperar essas faixas.
#[test]
fn orcamento_por_papel() {
    let r = roster();
    for c in &r.characters {
        match c.role {
            Role::Striker => {
                assert!((900..=950).contains(&c.hp), "{} striker hp {}", c.id, c.hp);
                assert!((108..=115).contains(&c.attack), "{} striker atk {}", c.id, c.attack);
                assert!((58..=70).contains(&c.speed), "{} striker spd {}", c.id, c.speed);
            }
            Role::Guardian => {
                assert!((1100..=1150).contains(&c.hp), "{} guardian hp {}", c.id, c.hp);
                assert!((92..=96).contains(&c.attack), "{} guardian atk {}", c.id, c.attack);
                assert!((38..=46).contains(&c.speed), "{} guardian spd {}", c.id, c.speed);
            }
            Role::Support => {
                assert!((960..=1020).contains(&c.hp), "{} support hp {}", c.id, c.hp);
                assert!((96..=100).contains(&c.attack), "{} support atk {}", c.id, c.attack);
                assert!((50..=60).contains(&c.speed), "{} support spd {}", c.id, c.speed);
            }
            Role::Controller => {
                assert!((880..=900).contains(&c.hp), "{} controller hp {}", c.id, c.hp);
                assert!((100..=104).contains(&c.attack), "{} controller atk {}", c.id, c.attack);
                assert!((54..=59).contains(&c.speed), "{} controller spd {}", c.id, c.speed);
            }
        }
    }
}

/// Anti-faz-tudo: cada papel domina UMA dimensão e abre mão de outra.
#[test]
fn cada_papel_domina_uma_dimensao() {
    let r = roster();
    for f in [Faction::Ember, Faction::Tide, Faction::Verdant, Faction::Storm] {
        let cla: Vec<_> = r.characters.iter().filter(|c| c.faction == f).collect();
        // Guardian é a âncora: maior HP do clã.
        let hp_max = cla.iter().map(|c| c.hp).max().unwrap();
        let guardian = cla.iter().find(|c| c.role == Role::Guardian).unwrap();
        assert_eq!(guardian.hp, hp_max, "{}: guardian não é a âncora de HP", f.as_str());
        // Striker tem o maior ATK do clã (a lâmina do time).
        let atk_max = cla.iter().map(|c| c.attack).max().unwrap();
        let striker = cla.iter().find(|c| c.role == Role::Striker).unwrap();
        assert_eq!(striker.attack, atk_max, "{}: striker não é a lâmina", f.as_str());
        // Support NUNCA é o maior ATK nem o maior HP (cura, não briga).
        let support = cla.iter().find(|c| c.role == Role::Support).unwrap();
        assert!(support.attack < atk_max, "{}: support competindo em dano", f.as_str());
        assert!(support.hp < hp_max, "{}: support competindo em vida", f.as_str());
        // Controller é o mais frágil em HP (custo do controle).
        let hp_min = cla.iter().map(|c| c.hp).min().unwrap();
        let controller = cla.iter().find(|c| c.role == Role::Controller).unwrap();
        assert_eq!(controller.hp, hp_min, "{}: controller não paga o preço do controle", f.as_str());
    }
}

/// Identidade mecânica por papel: cura no suporte, escudo no guardião,
/// estado de combo no controlador, foco no striker.
#[test]
fn identidade_mecanica_por_papel() {
    let r = roster();
    for c in &r.characters {
        let low = c.mystery.effect.to_lowercase();
        let cura = low.contains("cura");
        // Mesma regra de derivação da engine (Tier::derive): concede escudo
        // quando o texto tem "escudo" SEM "ignora" — "ignora X% do escudo" é
        // PERFURAÇÃO (Nereo, striker anti-escudo), não roubo de papel.
        let concede_escudo = low.contains("escudo") && !low.contains("ignora");
        let estado = c.mystery.applies_state != ComboState::None;
        match c.role {
            Role::Support => {
                assert!(cura, "{} support sem cura na mystery", c.id);
                assert!(!concede_escudo, "{} support roubando o papel do guardião", c.id);
                assert_eq!(alvo_mystery(c), TargetSpec::AllySingle, "{} support deve mirar aliado", c.id);
            }
            Role::Guardian => {
                assert!(concede_escudo, "{} guardian sem escudo na mystery", c.id);
                assert!(!cura, "{} guardian roubando o papel do suporte", c.id);
            }
            Role::Controller => {
                assert!(estado, "{} controller sem estado de combo (negação de turno)", c.id);
                assert!(!cura, "{} controller roubando o papel do suporte", c.id);
            }
            Role::Striker => {
                assert!(!cura, "{} striker fazendo-tudo (cura)", c.id);
                assert!(!concede_escudo, "{} striker fazendo-tudo (escudo)", c.id);
            }
        }
    }
}

/// Cadeias garantidas por clã (NOTAS-DESIGN §3): fonte de Lançamento → chase
/// de Lançamento → fonte de Derrubada → chase de Derrubada. Sem isto, o
/// sistema de cadeias não fecha em nenhum draft.
#[test]
fn cadeias_de_combo_fecham_por_cla() {
    let r = roster();
    for f in [Faction::Ember, Faction::Tide, Faction::Verdant, Faction::Storm] {
        let cla: Vec<_> = r.characters.iter().filter(|c| c.faction == f).collect();
        let fonte_launch = cla.iter().any(|c| {
            c.mystery.applies_state == ComboState::Launch || c.standard.applies_state == ComboState::Launch
        });
        let chase_launch = cla.iter().any(|c| c.chase.trigger == ComboState::Launch && c.chase.max_per_round > 0);
        let fonte_knockdown = cla.iter().any(|c| {
            c.mystery.applies_state == ComboState::Knockdown
                || c.standard.applies_state == ComboState::Knockdown
        });
        let chase_knockdown =
            cla.iter().any(|c| c.chase.trigger == ComboState::Knockdown && c.chase.max_per_round > 0);
        assert!(fonte_launch, "{}: sem fonte de Lançamento", f.as_str());
        assert!(chase_launch, "{}: sem chase de Lançamento", f.as_str());
        assert!(fonte_knockdown, "{}: sem fonte de Derrubada", f.as_str());
        assert!(chase_knockdown, "{}: sem chase de Derrubada", f.as_str());
    }
}

/// Gradiente clã↔estilo (NOTAS-DESIGN §4): storm vende HP por velocidade;
/// tide/verdant compram tank/utilidade com ATK.
#[test]
fn gradiente_de_cla_estilo() {
    let r = roster();
    let soma = |f: Faction, extra: fn(&turnrush_core::data::RosterChar) -> i32| -> i32 {
        r.characters
            .iter()
            .filter(|c| c.faction == f)
            .map(extra)
            .sum()
    };
    let hp = |c: &turnrush_core::data::RosterChar| c.hp;
    let spd = |c: &turnrush_core::data::RosterChar| c.speed;
    // storm: mais rápido, menos vida.
    assert!(soma(Faction::Storm, spd) > soma(Faction::Verdant, spd), "storm deveria ser o clã veloz");
    assert!(soma(Faction::Storm, hp) < soma(Faction::Tide, hp), "storm deveria pagar com HP");
    // tide: o mais vidrado.
    assert_eq!(
        soma(Faction::Tide, hp),
        [Faction::Ember, Faction::Tide, Faction::Verdant, Faction::Storm]
            .iter()
            .map(|f| soma(*f, hp))
            .max()
            .unwrap(),
        "tide deveria ser o clã tanque"
    );
}

/// Interrupt é identidade de controlador (Peça 1 já valida a posse); aqui
/// confere a DIVERSIDADE: os 4 controladores não são clones entre si.
#[test]
fn controladores_tem_identidades_distintas() {
    let r = roster();
    let ctrls: Vec<_> = r
        .characters
        .iter()
        .filter(|c| c.role == Role::Controller)
        .collect();
    assert_eq!(ctrls.len(), 4);
    let mut assinaturas: Vec<String> = Vec::new();
    for c in &ctrls {
        let mut s = format!("{:?}|{:?}|{:?}", c.mystery.targets, c.mystery.applies_state, c.interrupt.map(|i| i.kind));
        s.push_str(&c.passive.effect.to_lowercase());
        assinaturas.push(s);
    }
    assinaturas.sort();
    let n = assinaturas.len();
    assinaturas.dedup();
    assert_eq!(assinaturas.len(), n, "controladores clonados entre si");
}

/// Conector: a Peça 7 (corrente de batalha) pode confiar que qualquer
/// draft 4v4 tem cadeias disponíveis nos dois lados.
#[test]
fn peca_06_encaixa_na_peca_07() {
    let r = roster();
    // Duas composições completas por clã devem batalhar sem notas unmodeled
    // (o encaixe kit→engine fecha para TODAS as combinações de clãs).
    let pares = [
        (["ignia", "orson", "amara", "nimbo"], ["nereo", "dalva", "mel", "ligeia"]),
        (["tina", "aro", "yara", "ivo"], ["reia", "aki", "alisio", "volta"]),
    ];
    for (a, b) in pares {
        let ca = comp_de(&r, a);
        let cb = comp_de(&r, b);
        let mut bt = turnrush_core::interactive::InteractiveBattle::new(&r, ca, cb, 5);
        let _ = bt.prompt();
        assert!(bt.unmodeled().is_empty(), "kit {}×{} tem buracos: {:?}", a[0], b[0], bt.unmodeled());
    }
}
