//! Suíte Quebra-Cabeça — mesa compartilhada de todas as peças.
//!
//! Cada `pNN_*` é uma peça que encaixa na anterior: dados → determinismo →
//! economia → mira → passivas → kits → corrente de batalha → run → cliente.
//! Helpers aqui são a base comum: rosters sintéticos controlados e drivers
//! de batalha que usam APENAS a API pública da engine.

#![allow(dead_code)]

use turnrush_core::data::{Faction, Roster, Role, TargetSpec};
use turnrush_core::interactive::{
    greedy_decide, ActionKind, ChosenAction, InteractiveBattle, LegalAction, PromptPhase,
    SideEdge, TurnPrompt,
};
use turnrush_core::types::Outcome;

/// Roster real de lançamento (16 personagens) — fonte da verdade do jogo.
pub fn real_roster() -> Roster {
    Roster::from_json(include_str!("../../../../data/roster/roster-v2.json")).expect("roster v2")
}

/// Âncora da Peça 1 (dados): usada como conector pelas peças seguintes.
pub fn ancora_dados() -> usize {
    let r = real_roster();
    assert_eq!(r.version, 2);
    assert_eq!(r.characters.len(), 16);
    r.characters.len()
}

// ─────────────────────────────────────────────────────────────────────────────
// Roster sintético parametrizável
// ─────────────────────────────────────────────────────────────────────────────

/// Especificação de um personagem sintético (contrato v2).
#[derive(Clone, Debug)]
pub struct CharSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub faction: &'static str,
    pub role: &'static str,
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub m_targets: &'static str,
    pub m_chakra: i32,
    pub m_cd: u32,
    pub m_pct: i32,
    pub m_state: &'static str,
    pub m_effect: &'static str,
    pub st_pct: i32,
    pub st_state: &'static str,
    pub chase_trigger: &'static str,
    pub chase_pct: i32,
    pub chase_state: &'static str,
    pub chase_max: u32,
    pub passive: &'static str,
    pub unlock: Option<usize>,
    pub interrupt: Option<&'static str>,
    pub skill_targets: Option<&'static str>,
}

impl Default for CharSpec {
    fn default() -> Self {
        CharSpec {
            id: "x",
            name: "Boneco",
            faction: "tide",
            role: "guardian",
            hp: 1000,
            attack: 100,
            defense: 40,
            speed: 50,
            m_targets: "enemies_front",
            m_chakra: 20,
            m_cd: 3,
            m_pct: 90,
            m_state: "none",
            m_effect: "",
            st_pct: 70,
            st_state: "none",
            chase_trigger: "none",
            chase_pct: 0,
            chase_state: "none",
            chase_max: 0,
            passive: "",
            unlock: None,
            interrupt: None,
            skill_targets: None,
        }
    }
}

impl CharSpec {
    pub fn to_json(&self) -> String {
        let unlock = match self.unlock {
            Some(n) => format!(r#","unlock_schedule":{{"mystery_round":{n}}}"#),
            None => String::new(),
        };
        let interrupt = match self.interrupt {
            Some(k) => format!(r#","interrupt":{{"type":"{k}","window":1}}"#),
            None => String::new(),
        };
        let skill_targets = match self.skill_targets {
            Some(t) => format!(r#","skill_targets":"{t}""#),
            None => String::new(),
        };
        format!(
            r#"{{"id":"{id}","name":"{name}","faction":"{faction}","role":"{role}",
"hp":{hp},"attack":{attack},"defense":{defense},"speed":{speed},
"mystery":{{"name":"M-{id}","chakra":{m_chakra},"cooldown":{m_cd},"targets":"{m_targets}","damage_pct":{m_pct},"applies_state":"{m_state}","effect":"{m_effect}"}},
"standard":{{"name":"P-{id}","damage_pct":{st_pct},"applies_state":"{st_state}"}},
"chase":{{"name":"C-{id}","trigger":"{chase_trigger}","damage_pct":{chase_pct},"applies_state":"{chase_state}","max_per_round":{chase_max}}},
"passive":{{"name":"Pass-{id}","effect":"{passive}"}}{unlock}{interrupt}{skill_targets}}}"#,
            id = self.id,
            name = self.name,
            faction = self.faction,
            role = self.role,
            hp = self.hp,
            attack = self.attack,
            defense = self.defense,
            speed = self.speed,
            m_chakra = self.m_chakra,
            m_cd = self.m_cd,
            m_targets = self.m_targets,
            m_pct = self.m_pct,
            m_state = self.m_state,
            m_effect = self.m_effect,
            st_pct = self.st_pct,
            st_state = self.st_state,
            chase_trigger = self.chase_trigger,
            chase_pct = self.chase_pct,
            chase_state = self.chase_state,
            chase_max = self.chase_max,
            passive = self.passive,
        )
    }
}

/// Monta um roster a partir de specs (o índice no roster = ordem do array).
pub fn roster_of(chars: &[CharSpec]) -> Roster {
    let json = format!(
        r#"{{"version":2,"characters":[{}]}}"#,
        chars.iter().map(|c| c.to_json()).collect::<Vec<_>>().join(",")
    );
    Roster::from_json(&json).unwrap_or_else(|e| panic!("roster sintético inválido: {e}"))
}

/// Striker genérico (para preencher slots sem interesse do teste).
pub fn filler(id: &'static str) -> CharSpec {
    CharSpec { id, name: "Preenchedor", ..Default::default() }
}

/// Roster sintético principal (8 personagens) — laboratório da cadeia.
/// Slots de batalha são LOCAIS por lado (0..3).
/// Lado A: [0]s1 striker mystery single+launch (unlock R2), [1]s2 perseguidor
/// (chase launch), [2]c3 controlador (interrupt stun + passiva chakra +8),
/// [3]h4 suporte (cura + passiva fim de rodada).
/// Lado B: [0]b1 muralha reflexo + escudo, [1]b2 redução frontal, [2]b3 veneno
/// (dot_on_hit + mystery random_2), [3]b4 suporte cura em área EoR.
pub fn synth() -> Roster {
    roster_of(&[
        CharSpec {
            id: "s1",
            name: "Solta-Meteoro",
            faction: "ember",
            role: "striker",
            speed: 50,
            m_targets: "enemy_single",
            m_pct: 120,
            m_state: "launch",
            m_effect: "Golpe devastador que arremessa o alvo.",
            unlock: Some(2),
            skill_targets: Some("enemy_single"),
            ..Default::default()
        },
        CharSpec {
            id: "s2",
            name: "Perseguidor",
            faction: "ember",
            role: "striker",
            speed: 45,
            m_targets: "enemies_front",
            m_pct: 90,
            chase_trigger: "launch",
            chase_pct: 25,
            chase_max: 1,
            skill_targets: Some("enemies_front"),
            ..Default::default()
        },
        CharSpec {
            id: "c3",
            name: "Elo da Corrente",
            faction: "ember",
            role: "controller",
            speed: 40,
            hp: 900,
            m_targets: "enemies_front",
            m_pct: 85,
            m_state: "knockdown",
            m_effect: "Dano e Derrubada nos inimigos da frente.",
            st_pct: 60,
            interrupt: Some("stun"),
            passive: "No início da rodada, gera +8 de chakra para a equipe.",
            ..Default::default()
        },
        CharSpec {
            id: "h4",
            name: "Cuidadora",
            faction: "ember",
            role: "support",
            speed: 35,
            hp: 900,
            m_targets: "ally_single",
            m_pct: 100,
            m_effect: "Cura 100% do ataque.",
            st_pct: 60,
            passive: "No fim da rodada, cura 5% do HP do aliado com menos HP.",
            ..Default::default()
        },
        CharSpec {
            id: "b1",
            name: "Muralha",
            faction: "tide",
            role: "guardian",
            speed: 30,
            hp: 1200,
            attack: 90,
            defense: 50,
            m_targets: "self",
            m_pct: 80,
            m_effect: "Escudo de 80% do ataque em si.",
            st_pct: 65,
            passive: "Reflete 15% do dano.",
            ..Default::default()
        },
        CharSpec {
            id: "b2",
            name: "Sentinela",
            faction: "tide",
            role: "guardian",
            speed: 29,
            hp: 1200,
            attack: 90,
            defense: 50,
            m_targets: "self",
            m_pct: 70,
            m_effect: "Escudo de 70% do ataque em si.",
            st_pct: 65,
            passive: "-10% de dano recebido na linha de frente.",
            ..Default::default()
        },
        CharSpec {
            id: "b3",
            name: "Farpa",
            faction: "tide",
            role: "striker",
            speed: 28,
            hp: 1000,
            attack: 100,
            defense: 40,
            m_targets: "random_2",
            m_pct: 90,
            m_effect: "Aplica Veneno (dano contínuo) em dois inimigos aleatórios.",
            st_pct: 65,
            passive: "Os ataques aplicam Veneno (dano contínuo).",
            ..Default::default()
        },
        CharSpec {
            id: "b4",
            name: "Guardiã Calma",
            faction: "tide",
            role: "support",
            speed: 27,
            hp: 950,
            attack: 95,
            defense: 45,
            m_targets: "ally_single",
            m_pct: 90,
            m_effect: "Cura 90% do ataque.",
            st_pct: 60,
            passive: "No fim da rodada, todos os aliados curam 2% do HP máximo.",
            ..Default::default()
        },
    ])
}

// ─────────────────────────────────────────────────────────────────────────────
// Drivers de batalha (só API pública)
// ─────────────────────────────────────────────────────────────────────────────

/// Política de decisão: recebe a engine e a vez corrente, devolve a ação.
pub type Decide<'a> = &'a dyn Fn(&InteractiveBattle, &TurnPrompt) -> ChosenAction;

/// IA greedy simétrica da engine (chase confirm / tática índice 0 / melhor ação).
pub fn greedy(bt: &InteractiveBattle, p: &TurnPrompt) -> ChosenAction {
    greedy_decide(bt, p)
}

/// Sempre recusa janela de perseguição (isolamento de cadeias).
pub fn recusa_chase(bt: &InteractiveBattle, p: &TurnPrompt) -> ChosenAction {
    match p.phase {
        PromptPhase::ConfirmChase => ChosenAction::RejectChase,
        PromptPhase::ChooseTactic => ChosenAction::PickTactic { index: 0 },
        _ => greedy_decide(bt, p),
    }
}

/// Executa uma decisão com a política dada (resolve as fases de janela antes).
pub fn advance_with(
    bt: &mut InteractiveBattle,
    decide: Decide<'_>,
) -> Vec<turnrush_core::interactive::BattleEvent> {
    let p = bt.prompt();
    let acao = decide(bt, &p);
    match bt.execute_action(acao) {
        Ok(evs) => evs,
        Err(turnrush_core::interactive::ActionError::BattleOver) => bt.drain(),
        Err(e) => panic!("ação da política recusada pela engine: {e:?}"),
    }
}

/// Roda a batalha até o fim. Retorna (resultado, passos, eventos acumulados).
pub fn play_out(
    bt: &mut InteractiveBattle,
    decide: Decide<'_>,
) -> (Outcome, u64, Vec<turnrush_core::interactive::BattleEvent>) {
    let mut passos = 0u64;
    let mut evs = Vec::new();
    loop {
        if bt.is_over() {
            break;
        }
        evs.extend(bt.drain());
        passos += 1;
        assert!(passos < 100_000, "batalha não converge (< 100k passos)");
        let evs_passo = advance_with(bt, decide);
        evs.extend(evs_passo);
    }
    evs.extend(bt.drain());
    (bt.outcome().expect("outcome pós-fim"), passos, evs)
}

/// Primeira ação legal do tipo pedido (para scripts cirúrgicos).
pub fn legal_de(
    bt: &InteractiveBattle,
    p: &TurnPrompt,
    kind: ActionKind,
) -> Option<LegalAction> {
    bt.legal_actions(p.side, p.slot)
        .into_iter()
        .find(|l| l.kind == kind)
}

/// Roda com padrões (resolvendo táticas/perseguições) até a vez de
/// `(side, slot)` numa rodada ≥ `rodada_min`; devolve o prompt da vez.
/// A unidade alvo também joga padrões nos turnos intermediários (banking).
pub fn vez_de(
    bt: &mut InteractiveBattle,
    side: usize,
    slot: usize,
    rodada_min: usize,
) -> TurnPrompt {
    loop {
        if bt.is_over() {
            panic!("batalha acabou antes da vez de {side}/{slot} (r≥{rodada_min})");
        }
        let p = bt.prompt();
        if p.phase == PromptPhase::ChooseAction && p.side == side && p.slot == slot && p.round >= rodada_min {
            return p;
        }
        match p.phase {
            PromptPhase::ChooseTactic => {
                bt.execute_action(ChosenAction::PickTactic { index: 0 })
                    .expect("tática automática");
            }
            PromptPhase::ConfirmChase => {
                bt.execute_action(ChosenAction::RejectChase).expect("recusa automática");
            }
            PromptPhase::ChooseAction => {
                bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None })
                    .expect("padrão no caminho até a vez");
            }
            PromptPhase::Over => panic!("Over antes da vez de {side}/{slot}"),
        }
    }
}

/// Snapshot textual do estado visível (para igualdade profunda).
pub fn face(b: &InteractiveBattle) -> Vec<String> {
    let mut out = Vec::new();
    for side in 0..2 {
        for slot in 0..4 {
            let u = b.unit(side, slot);
            out.push(format!(
                "{side}/{slot} {} hp={} shield={} fluxo={} alive={} stun={} sil={} dot={} scd={} mcd={} icd={} acted={} chased={}",
                u.name, u.hp, u.shield, u.fluxo, u.alive, u.stun_turns, u.silence_turns,
                u.dot_turns, u.skill_cd, u.mystery_cd, u.interrupt_cd, u.acted, u.chased
            ));
        }
    }
    out
}

/// Procura seed (a partir de 1) em que a rodada pedida começa com o lado 0 —
/// permite asserts de ordem sem acoplar ao sorteio de iniciativa.
pub fn seed_com_starter(
    comp_a: [usize; 4],
    comp_b: [usize; 4],
    roster: &Roster,
    rodada: usize,
) -> u64 {
    for seed in 1..=64u64 {
        let mut bt = InteractiveBattle::new(roster, comp_a, comp_b, seed);
        loop {
            if bt.is_over() || bt.round() >= rodada {
                break;
            }
            advance_with(&mut bt, &greedy);
        }
        if bt.starter() == 0 {
            return seed;
        }
    }
    panic!("nenhuma seed com starter 0 na rodada {rodada} (improvável)");
}

/// Índice no roster real de um id (ex.: "tina" → slot no elenco).
pub fn idx_de(roster: &Roster, id: &str) -> usize {
    roster
        .characters
        .iter()
        .position(|c| c.id == id)
        .unwrap_or_else(|| panic!("personagem '{id}' não está no roster"))
}

/// Comp 4v4 com os ids pedidos (ordem = slots 0..3).
pub fn comp_de(roster: &Roster, ids: [&str; 4]) -> [usize; 4] {
    [
        idx_de(roster, ids[0]),
        idx_de(roster, ids[1]),
        idx_de(roster, ids[2]),
        idx_de(roster, ids[3]),
    ]
}

/// Facções de uma comp (para oferta de cartas com preferência de clã).
pub fn factions_de(roster: &Roster, comp: [usize; 4]) -> Vec<Faction> {
    turnrush_core::rewards::RunState::team_factions(roster, comp)
}

/// Confere que um TargetSpec textual é válido (sanidade de contrato).
pub fn alvo_valido(t: &str) -> bool {
    TargetSpec::parse(t).is_ok()
}

/// Confere que um ComboState textual é válido (sanidade de contrato).
pub fn estado_valido(t: &str) -> bool {
    turnrush_core::data::ComboState::parse(t).is_ok()
}

/// Reassert barato do determinismo (âncora da Peça 2) para conectores.
pub fn ancora_determinismo() {
    let r = synth();
    let mut a = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 42);
    let mut b = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 42);
    let (oa, _, _) = play_out(&mut a, &greedy);
    let (ob, _, _) = play_out(&mut b, &greedy);
    assert_eq!(oa, ob, "mesma seed deve dar o mesmo resultado");
}

/// Reassert barato da economia (âncora da Peça 3) para conectores.
pub fn ancora_economia() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let p = bt.prompt(); // abre a rodada 1
    assert_eq!(p.round, 1);
    assert_eq!(bt.unit(0, 0).fluxo, turnrush_core::interactive::FLUXO_PER_ROUND);
}

/// Aliados vivos de um lado (slots 0..4).
pub fn vivos(b: &InteractiveBattle, side: usize) -> Vec<usize> {
    (0..4).filter(|&s| b.unit(side, s).alive).collect()
}

/// Soma de HP vivo de um lado.
pub fn hp_total(b: &InteractiveBattle, side: usize) -> i32 {
    vivos(b, side).iter().map(|&s| b.unit(side, s).hp).sum()
}

/// Último evento de dano de um lado/slot (0 se não houver).
pub fn ultimo_dano(
    evs: &[turnrush_core::interactive::BattleEvent],
    side: usize,
    slot: usize,
) -> i32 {
    evs.iter()
        .rev()
        .find_map(|e| match e {
            turnrush_core::interactive::BattleEvent::Damage { side: s, slot: k, amount, .. }
                if *s == side && *k == slot =>
            {
                Some(*amount)
            }
            _ => None,
        })
        .unwrap_or(0)
}

/// SideEdge com bônus de ATK/HP por slot (conector cartas → batalha).
pub fn edge_unit(slot: usize, atk: i32, hp: i32) -> SideEdge {
    let mut e = SideEdge::default();
    e.unit_atk[slot] = atk;
    e.unit_hp[slot] = hp;
    e
}

/// Papel lido do roster real (para testes de kit).
pub fn papel(c: &turnrush_core::data::RosterChar) -> Role {
    c.role
}

/// Alvo da mystery lido do roster real.
pub fn alvo_mystery(c: &turnrush_core::data::RosterChar) -> TargetSpec {
    c.mystery.targets
}
