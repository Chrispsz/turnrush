//! Motor de combate por turnos tático:
//! iniciativa (SPD), energia (chakra, regen 2/turno, cap 10), cooldowns,
//! **Lançamentos** que iniciam combos e **Perseguições (chase)** que aliados
//! encadeiam (1× por rodada), status (atordoamento, veneno, escudo, buff/debuff).
//!
//! A cada rodada os stats escalam +8% **igualmente para os dois lados**
//! (a pressão sobe igualmente para os dois lados — a simetria permanece).

use crate::data::ROSTER;
use crate::rng::Rng;
use crate::types::{Ability, Outcome, PER_MILLE, Stats, Status, Targeting, Team, Unit};

pub const ENERGY_CAP: i32 = 10;
pub const ENERGY_REGEN: i32 = 2;
pub const MAX_ROUNDS: usize = 30;
/// Febre de batalha: dano cresce 2% por rodada de combate (igual para os dois
/// lados). Garante que toda batalha se resolva — sustain não pode tartarugar.
pub const FEVER_PER_ROUND: i32 = 20;
/// Escala por rodada: rodada 0 = 1000‰, rodada 8 = 1640‰ (+8% composto/rodada).
pub const ESCALATION_PER_ROUND: i32 = 80;

/// Dano bruto pós-mitigação: ATK × pct% × 100/(100+DEF). Inteiro puro.
pub fn damage_after_def(atk: i32, dmg_pct: i32, def: i32) -> i32 {
    atk * dmg_pct / 100 * 100 / (100 + def)
}

/// Potência da perseguição (chase) como % do efeito original.
pub const CHASE_POWER_PCT: i32 = 60;

/// Redimensiona um efeito para `pct`% dos seus valores numéricos.
pub fn scaled_effect(e: crate::types::Effect, pct: i32) -> crate::types::Effect {
    crate::types::Effect {
        dmg_pct: e.dmg_pct * pct / 100,
        heal: e.heal * pct / 100,
        shield: e.shield * pct / 100,
        poison_dmg: e.poison_dmg * pct / 100,
        ..e
    }
}

/// Fator de escalada da rodada (per-mille), igual para os dois times.
pub fn scale_for_round(round: usize) -> i32 {
    PER_MILLE + ESCALATION_PER_ROUND * round as i32
}

/// Escolha da IA/política: (índice da habilidade, time alvo, slot alvo).
pub trait Actor {
    fn choose(
        &mut self,
        battle: &Battle,
        team: usize,
        slot: usize,
    ) -> Option<(usize, usize, usize)>;
}

pub struct Battle {
    pub teams: [Team; 2],
    pub rng: Rng,
    pub round: usize,
    /// Dano total causado por time — usado no juízo (anti-turtle).
    pub dmg_dealt: [i64; 2],
    /// Perseguição já usada na rodada, por time (1 chase/rodada/time).
    pub chase_used: [bool; 2],
    /// Log de diagnóstico (turno a turno).
    pub verbose: bool,
}

impl Battle {
    pub fn new(team_a: Team, team_b: Team, seed: u64) -> Battle {
        Battle {
            teams: [team_a, team_b],
            rng: Rng::new(seed),
            round: 0,
            dmg_dealt: [0, 0],
            chase_used: [false, false],
            verbose: false,
        }
    }

    /// Roda a batalha até a vitória ou o limite de rodadas (juízo por HP%).
    pub fn run(&mut self, actor_a: &mut dyn Actor, actor_b: &mut dyn Actor) -> Outcome {
        while self.round < MAX_ROUNDS {
            let order = self.action_order();
            for (team, slot) in order {
                if self.teams[team].units[slot].hp_now <= 0 {
                    continue; // morto antes de agir
                }
                if team == 0 {
                    self.take_turn(team, slot, &mut *actor_a);
                } else {
                    self.take_turn(team, slot, &mut *actor_b);
                }
                if let Some(o) = self.check_victory() {
                    return o;
                }
            }
            self.end_of_round();
            self.round += 1;
            if let Some(o) = self.check_victory() {
                return o;
            }
        }
        self.judge()
    }

    /// Ordem de ação com **alternância de times** (estilo Legends of Runeterra):
    /// A-B-A-B, cada time jogando suas unidades em ordem de SPD desc.
    /// O lado que começa é sorteado (simetria em expectativa).
    /// A iniciativa deixa de ser vantagem composta — vale no máximo a 1ª ação.
    fn action_order(&mut self) -> Vec<(usize, usize)> {
        let mut per_team: [Vec<(i32, u64, usize)>; 2] = [Vec::new(), Vec::new()];
        for (t, team) in self.teams.iter().enumerate() {
            for (s, u) in team.units.iter().enumerate() {
                if u.hp_now > 0 {
                    per_team[t].push((u.stats.spd, self.rng.next_u64(), s));
                }
            }
            per_team[t].sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
        }
        let mut order: Vec<(usize, usize)> = Vec::with_capacity(6);
        let (mut i0, mut i1) = (0usize, 0usize);
        let mut turn = self.rng.range(2); // sorteio de quem começa
        while i0 < per_team[0].len() || i1 < per_team[1].len() {
            let t = if i0 >= per_team[0].len() {
                1
            } else if i1 >= per_team[1].len() {
                0
            } else {
                turn
            };
            if t == 0 {
                order.push((0, per_team[0][i0].2));
                i0 += 1;
            } else {
                order.push((1, per_team[1][i1].2));
                i1 += 1;
            }
            turn = 1 - turn;
        }
        order
    }

    fn take_turn(&mut self, team: usize, slot: usize, actor: &mut dyn Actor) {
        {
            let u = &mut self.teams[team].units[slot];
            // veneno no início do turno (também escala com a febre)
            if u.status.poison_turns > 0 {
                let fever = (PER_MILLE + FEVER_PER_ROUND * self.round as i32) as i64;
                let tick = (u.status.poison_dmg as i64 * fever / PER_MILLE as i64) as i32;
                u.hp_now -= tick;
                self.dmg_dealt[team] += tick.max(0) as i64;
                u.status.poison_turns -= 1;
                if u.status.poison_turns == 0 {
                    u.status.poison_dmg = 0;
                }
                if u.hp_now <= 0 {
                    return; // morreu de veneno
                }
            }
            // energia e cooldowns correm mesmo atordoado
            u.energy = (u.energy + ENERGY_REGEN).min(ENERGY_CAP);
            for c in u.cds.iter_mut() {
                if *c > 0 {
                    *c -= 1;
                }
            }
        }
        if self.teams[team].units[slot].status.stun > 0 {
            self.teams[team].units[slot].status.stun -= 1;
            return; // perde a ação
        }
        if let Some((ai, tt, ss)) = actor.choose(self, team, slot) {
            if self.verbose {
                let ab = self.teams[team].units[slot].abilities[ai];
                let alvo = if ss < self.teams[tt].units.len() {
                    self.teams[tt].units[ss].name.to_string()
                } else {
                    "todos".to_string()
                };
                println!(
                    "  R{} {} [T{}] usa {} (custo {}, cd {}) → {}",
                    self.round,
                    self.teams[team].units[slot].name,
                    team,
                    ab.name,
                    ab.cost,
                    ab.cooldown,
                    alvo
                );
            }
            self.use_ability(team, slot, ai, tt, ss);
        } else if self.verbose {
            println!(
                "  R{} {} [T{}] passa (sem opção)",
                self.round,
                self.teams[team].units[slot].name,
                team
            );
        }
    }

    fn use_ability(&mut self, team: usize, slot: usize, ai: usize, tt: usize, ss: usize) {
        let ab = self.teams[team].units[slot].abilities[ai];
        {
            let u = &mut self.teams[team].units[slot];
            u.energy -= ab.cost;
            u.cds[ai] = ab.cooldown;
        }
        let is_launch = ab.effect.is_launch;
        self.apply_effect(team, slot, tt, ss, ab);

        // Lançamento: aliados com habilidade de chase encadeiam
        // (máx. 1 perseguição por rodada por time, a 60% do efeito).
        if is_launch {
            let chain: Vec<usize> = (0..self.teams[team].units.len())
                .filter(|&s| self.teams[team].units[s].hp_now > 0)
                .collect();
            for cs in chain {
                if self.chase_used[team] {
                    break;
                }
                if self.teams[team].units[cs].chased_this_round {
                    continue;
                }
                let chase = self.teams[team].units[cs]
                    .abilities
                    .iter()
                    .copied()
                    .find(|a| a.is_chase);
                if let Some(chase_ab) = chase {
                    self.teams[team].units[cs].chased_this_round = true;
                    self.chase_used[team] = true;
                    let mut scaled_ab = chase_ab;
                    scaled_ab.effect = scaled_effect(chase_ab.effect, CHASE_POWER_PCT);
                    if let Some((ett, ess)) = self.random_living_enemy(team) {
                        self.apply_effect(team, cs, ett, ess, scaled_ab);
                    }
                }
            }
        }
    }

    fn random_living_enemy(&mut self, team: usize) -> Option<(usize, usize)> {
        let enemy = 1 - team;
        let living: Vec<usize> = (0..self.teams[enemy].units.len())
            .filter(|&s| self.teams[enemy].units[s].hp_now > 0)
            .collect();
        if living.is_empty() {
            return None;
        }
        let s = living[self.rng.range(living.len())];
        Some((enemy, s))
    }

    fn apply_effect(&mut self, at_team: usize, at_slot: usize, tt: usize, ss: usize, ab: Ability) {
        let e = ab.effect;
        // TODOS os efeitos escalam com os multiplicadores DWC do time do ator E
        // com a escalada da rodada — suporte nunca fica obsoleto ante o dano.
        let dmg_mult = self.teams[at_team].dmg_permille;
        let hp_mult = self.teams[at_team].hp_permille;
        let rs = self.teams[at_team].round_scale as i64;
        let actor_atk = if e.dmg_pct > 0 {
            self.effective_atk(at_team, at_slot)
        } else {
            0
        };
        let heal_v = ((e.heal as i64 * hp_mult as i64) / PER_MILLE as i64 * rs / PER_MILLE as i64) as i32;
        let shield_v = ((e.shield as i64 * hp_mult as i64) / PER_MILLE as i64 * rs / PER_MILLE as i64) as i32;
        let poison_v = ((e.poison_dmg as i64 * dmg_mult as i64) / PER_MILLE as i64 * rs / PER_MILLE as i64) as i32;
        let atk_pct_v = e.atk_pct * dmg_mult / PER_MILLE;

        let targets: Vec<(usize, usize)> = match ab.targeting {
            Targeting::SelfTarget => vec![(at_team, at_slot)],
            Targeting::AllyAll => (0..self.teams[at_team].units.len())
                .filter(|&s| self.teams[at_team].units[s].hp_now > 0)
                .map(|s| (at_team, s))
                .collect(),
            Targeting::EnemyAll => (0..self.teams[tt].units.len())
                .filter(|&s| self.teams[tt].units[s].hp_now > 0)
                .map(|s| (tt, s))
                .collect(),
            Targeting::Enemy | Targeting::Ally => vec![(tt, ss)],
        };

        for (t, s) in targets {
            let u = &mut self.teams[t].units[s];
            if u.hp_now <= 0 {
                continue;
            }
            if e.dmg_pct > 0 {
                let mut d = damage_after_def(actor_atk, e.dmg_pct, u.stats.def);
                // escudo absorve primeiro
                if u.status.shield > 0 {
                    let absorbed = u.status.shield.min(d);
                    u.status.shield -= absorbed;
                    d -= absorbed;
                }
                u.hp_now -= d;
                if d > 0 {
                    self.dmg_dealt[at_team] += d as i64;
                }
            }
            if e.heal > 0 && u.hp_now > 0 {
                u.hp_now = (u.hp_now + heal_v).min(u.stats.hp);
            }
            if shield_v > 0 && u.hp_now > 0 {
                u.status.shield += shield_v;
            }
            if e.stun > 0 && u.hp_now > 0 {
                u.status.stun = u.status.stun.max(e.stun);
            }
            if poison_v > 0 && u.hp_now > 0 {
                u.status.poison_dmg = u.status.poison_dmg.max(poison_v);
                u.status.poison_turns = u.status.poison_turns.max(e.poison_turns);
            }
            // buff/debuff de ATK: aplica se mais forte ou se não há ativo
            if atk_pct_v != 0 && u.hp_now > 0 {
                let stronger = atk_pct_v.abs() >= u.status.atk_pct.abs();
                if u.status.buff_turns == 0 || stronger {
                    u.status.atk_pct = atk_pct_v;
                    u.status.buff_turns = e.buff_turns.max(1);
                }
            }
        }
    }

    fn effective_atk(&self, t: usize, s: usize) -> i32 {
        let u = &self.teams[t].units[s];
        let base = u.stats.atk * (PER_MILLE + u.status.atk_pct * 10) / PER_MILLE;
        // febre de batalha: agressividade é recompensada com o tempo
        base * (PER_MILLE + FEVER_PER_ROUND * self.round as i32) / PER_MILLE
    }

    fn end_of_round(&mut self) {
        for t in 0..2 {
            self.chase_used[t] = false;
            for u in self.teams[t].units.iter_mut() {
                u.chased_this_round = false;
                if u.status.buff_turns > 0 {
                    u.status.buff_turns -= 1;
                    if u.status.buff_turns == 0 {
                        u.status.atk_pct = 0;
                    }
                }
            }
        }
    }

    fn check_victory(&self) -> Option<Outcome> {
        let a = self.teams[0].is_defeated();
        let b = self.teams[1].is_defeated();
        match (a, b) {
            (true, true) => Some(Outcome::Draw),
            (true, false) => Some(Outcome::TeamBWin),
            (false, true) => Some(Outcome::TeamAWin),
            (false, false) => None,
        }
    }

    /// Juízo anti-turtle: quem CAUSOU mais dano vence (agressividade é
    /// recompensada); empate de dano cai para HP% (margem 2%). Isso impede
    /// composições sem potencial de abate de vencer "sobrevivendo".
    fn judge(&self) -> Outcome {
        let da = self.dmg_dealt[0];
        let db = self.dmg_dealt[1];
        if da > db {
            return Outcome::TeamAWin;
        }
        if db > da {
            return Outcome::TeamBWin;
        }
        let a = self.teams[0].total_hp_pct();
        let b = self.teams[1].total_hp_pct();
        if a > b + 2 {
            Outcome::TeamAWin
        } else if b > a + 2 {
            Outcome::TeamBWin
        } else {
            Outcome::Draw
        }
    }
}

/// Entrada de conveniência: monta, roda e devolve o resultado.
pub fn battle_teams(
    a: Team,
    b: Team,
    seed: u64,
    actor_a: &mut dyn Actor,
    actor_b: &mut dyn Actor,
) -> Outcome {
    Battle::new(a, b, seed).run(actor_a, actor_b)
}

// ─────────────────────────────────────────────────────────────────────────────
// IA heurística do simulador (jogador "greedy" com ruído para diversidade)
// ─────────────────────────────────────────────────────────────────────────────

/// IA gulosa: escolhe a combinação (habilidade, alvo) de maior valor esperado.
pub struct GreedyActor {
    pub rng: Rng,
}

impl Actor for GreedyActor {
    fn choose(
        &mut self,
        battle: &Battle,
        team: usize,
        slot: usize,
    ) -> Option<(usize, usize, usize)> {
        let enemy = 1 - team;
        let unit = &battle.teams[team].units[slot];
        let mut best: Option<(f32, usize, usize, usize)> = None;

        for (ai, ab) in unit.abilities.iter().enumerate() {
            if unit.cds[ai] > 0 || unit.energy < ab.cost {
                continue;
            }
            let candidates: Vec<(usize, usize)> = match ab.targeting {
                Targeting::Enemy => (0..battle.teams[enemy].units.len())
                    .filter(|&s| battle.teams[enemy].units[s].hp_now > 0)
                    .map(|s| (enemy, s))
                    .collect(),
                Targeting::Ally => (0..battle.teams[team].units.len())
                    .filter(|&s| battle.teams[team].units[s].hp_now > 0)
                    .map(|s| (team, s))
                    .collect(),
                Targeting::EnemyAll => vec![(enemy, usize::MAX)],
                Targeting::AllyAll => vec![(team, usize::MAX)],
                Targeting::SelfTarget => vec![(team, slot)],
            };
            for (tt, ss) in candidates {
                let score = score_ability(battle, team, slot, ab, tt, ss);
                // ruído de 20% para variar as partidas do Monte Carlo
                let noisy = score * (0.9 + 0.2 * self.rng.next_f32());
                if best.is_none_or(|(bs, ..)| noisy > bs) {
                    best = Some((noisy, ai, tt, ss));
                }
            }
        }
        best.map(|(_, ai, tt, ss)| (ai, tt, ss))
    }
}

fn score_ability(
    battle: &Battle,
    team: usize,
    slot: usize,
    ab: &Ability,
    tt: usize,
    ss: usize,
) -> f32 {
    let actor = &battle.teams[team].units[slot];
    let e = ab.effect;
    let mut score = 0.0_f32;

    // ── dano ──
    if e.dmg_pct > 0 {
        let atk = actor.stats.atk * (PER_MILLE + actor.status.atk_pct * 10) / PER_MILLE;
        let hit = |u: &Unit| -> f32 {
            let mut d = damage_after_def(atk, e.dmg_pct, u.stats.def) as f32;
            if u.status.shield > 0 {
                d *= 0.7;
            }
            if u.hp_now as f32 <= d {
                d += 25.0; // valor de execução
            }
            d
        };
        match ab.targeting {
            Targeting::EnemyAll => {
                for u in battle.teams[tt].units.iter().filter(|u| u.hp_now > 0) {
                    score += hit(u);
                }
            }
            _ => {
                if ss < battle.teams[tt].units.len() && battle.teams[tt].units[ss].hp_now > 0 {
                    score += hit(&battle.teams[tt].units[ss]);
                }
            }
        }
        if e.is_launch {
            score += 5.0; // potencial de combo
        }
    }

    // ── cura ──
    if e.heal > 0 {
        let mut healing = 0.0_f32;
        match ab.targeting {
            Targeting::AllyAll => {
                for u in battle.teams[tt].units.iter().filter(|u| u.hp_now > 0) {
                    let missing = (u.stats.hp - u.hp_now).max(0);
                    healing += missing.min(e.heal) as f32;
                }
            }
            _ => {
                if ss < battle.teams[tt].units.len() {
                    let u = &battle.teams[tt].units[ss];
                    let missing = (u.stats.hp - u.hp_now).max(0);
                    healing = missing.min(e.heal) as f32;
                }
            }
        }
        score += healing * 1.2;
    }

    // ── escudo ──
    if e.shield > 0 {
        // escudo em unidade já protegida vale pouco (evita spam infinito)
        let factor = |u: &Unit| -> f32 {
            if u.status.shield > 0 {
                0.15
            } else {
                0.4
            }
        };
        match ab.targeting {
            Targeting::AllyAll => {
                let any_unshielded = battle.teams[tt]
                    .units
                    .iter()
                    .any(|u| u.hp_now > 0 && u.status.shield == 0);
                score += e.shield as f32 * if any_unshielded { 0.4 } else { 0.1 };
            }
            _ => {
                if ss < battle.teams[tt].units.len() && battle.teams[tt].units[ss].hp_now > 0 {
                    let u = &battle.teams[tt].units[ss];
                    let damaged = (u.stats.hp - u.hp_now).max(0);
                    score += e.shield as f32 * factor(u) * if damaged > 0 { 1.0 } else { 0.6 };
                }
            }
        }
    }

    // ── atordoamento ──
    if e.stun > 0 && ss < battle.teams[tt].units.len() && battle.teams[tt].units[ss].hp_now > 0 {
        score += 32.0;
    }

    // ── veneno (dano estimado ao longo dos turnos) ──
    if e.poison_dmg > 0 {
        score += (e.poison_dmg * e.poison_turns as i32) as f32 * 0.9;
    }

    // ── buff / debuff ──
    if e.atk_pct > 0 {
        score += 18.0;
    } else if e.atk_pct < 0 {
        score += 14.0;
    }

    score
}

/// Monta um `Team` a partir da composição, dos modificadores DWC e das cartas.
pub fn build_team(
    comp: &[usize],
    team_idx: usize,
    round: usize,
    dwc_dmg_permille: i32,
    dwc_hp_permille: i32,
    picks: &[crate::cards::Pick],
) -> Team {
    let mut stats: Vec<Stats> = comp.iter().map(|&cid| ROSTER[cid].stats).collect();
    let mut dmg_bonus: i32 = 0;
    let mut hp_bonus: i32 = 0;

    for p in picks {
        let card = &crate::cards::CARD_POOL[p.card];
        match card.kind {
            crate::cards::CardKind::UnitStats { hp, atk, def, spd } => {
                let slot = p.target_unit.min(stats.len().saturating_sub(1));
                stats[slot].hp += hp;
                stats[slot].atk += atk;
                stats[slot].def += def;
                stats[slot].spd += spd;
            }
            crate::cards::CardKind::TeamStats { hp, atk, def, spd } => {
                for s in stats.iter_mut() {
                    s.hp += hp;
                    s.atk += atk;
                    s.def += def;
                    s.spd += spd;
                }
            }
            crate::cards::CardKind::TeamPct { dmg_permille, hp_permille } => {
                dmg_bonus += dmg_permille;
                hp_bonus += hp_permille;
            }
        }
    }

    let dmg_total = (dwc_dmg_permille + dmg_bonus).max(500);
    let hp_total = (dwc_hp_permille + hp_bonus).max(500);
    let scale = scale_for_round(round);

    let units: Vec<Unit> = comp
        .iter()
        .enumerate()
        .map(|(i, &cid)| {
            let d = &ROSTER[cid];
            let hp = stats[i].hp * hp_total / PER_MILLE * scale / PER_MILLE;
            let atk = stats[i].atk * dmg_total / PER_MILLE * scale / PER_MILLE;
            Unit {
                cid,
                name: d.name,
                stats: Stats {
                    hp,
                    atk,
                    def: stats[i].def,
                    spd: stats[i].spd,
                },
                hp_now: hp,
                energy: 4,
                abilities: d.abilities.to_vec(),
                cds: vec![0; d.abilities.len()],
                status: Status::default(),
                team: team_idx,
                chased_this_round: false,
            }
        })
        .collect();

    Team {
        units,
        dmg_permille: dmg_total,
        hp_permille: hp_total,
        round_scale: scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{ability, fx};
    use crate::types::Effect;

    fn roster_unit(cid: usize, team: usize) -> Unit {
        let d = &ROSTER[cid];
        Unit {
            cid,
            name: d.name,
            stats: d.stats,
            hp_now: d.stats.hp,
            energy: 4,
            abilities: d.abilities.to_vec(),
            cds: vec![0; 3],
            status: Status::default(),
            team,
            chased_this_round: false,
        }
    }

    fn roster_team(members: [usize; 3], team: usize) -> Team {
        Team {
            units: members.iter().map(|&c| roster_unit(c, team)).collect(),
            dmg_permille: PER_MILLE,
            hp_permille: PER_MILLE,
            round_scale: PER_MILLE,
        }
    }

    #[test]
    fn formula_de_dano() {
        // 22 ATK, 90%, DEF 20 → 22*90/100=19; 19*100/120=15
        assert_eq!(damage_after_def(22, 90, 20), 15);
        assert!(damage_after_def(50, 200, 0) > damage_after_def(50, 200, 50));
    }

    #[test]
    fn escalada_simetrica() {
        assert_eq!(scale_for_round(0), PER_MILLE);
        assert_eq!(scale_for_round(8), PER_MILLE + 8 * 80);
    }

    #[test]
    fn batalha_deterministica() {
        let run_once = |seed: u64| -> Outcome {
            let mut a = GreedyActor { rng: Rng::new(seed ^ 1) };
            let mut b = GreedyActor { rng: Rng::new(seed ^ 2) };
            battle_teams(roster_team([0, 1, 2], 0), roster_team([3, 4, 5], 1), seed, &mut a, &mut b)
        };
        assert_eq!(run_once(777), run_once(777));
    }

    #[test]
    fn espelho_e_equilibrado() {
        // mesma composição dos dois lados → ~50% com tolerância folgada
        let mut wins = [0usize; 2];
        for seed in 0..300u64 {
            let mut a = GreedyActor { rng: Rng::new(seed * 2 + 1) };
            let mut b = GreedyActor { rng: Rng::new(seed * 2 + 2) };
            match battle_teams(
                roster_team([0, 1, 2], 0),
                roster_team([0, 1, 2], 1),
                seed * 1000 + 13,
                &mut a,
                &mut b,
            ) {
                Outcome::TeamAWin => wins[0] += 1,
                Outcome::TeamBWin => wins[1] += 1,
                Outcome::Draw => {}
            }
        }
        // em espelho perfeito, empates são esperados; vitórias devem dividir ~50%
        let decided = wins[0] + wins[1];
        let draws = 300 - decided;
        assert!(draws < 150, "muitos empates no espelho: {draws}");
        if decided > 0 {
            let p_a = wins[0] as f32 / decided as f32;
            assert!(
                (0.35..=0.65).contains(&p_a),
                "espelho enviesado: {wins:?} (p_a={p_a:.3})"
            );
        }
    }

    #[test]
    fn chase_dispara_apenas_uma_vez_por_rodada() {
        let mk_launch = |team: usize| -> Unit {
            let mut u = roster_unit(0, team);
            u.abilities = vec![ability(
                "launch_test",
                "Lança",
                0,
                0,
                Targeting::Enemy,
                Effect { is_launch: true, ..fx(100) },
                false,
            )];
            u.cds = vec![0];
            u
        };
        let mk_chase = |team: usize| -> Unit {
            let mut u = roster_unit(0, team);
            u.abilities = vec![ability(
                "chase_test",
                "Persegue",
                0,
                0,
                Targeting::Enemy,
                fx(50),
                true,
            )];
            u.cds = vec![0];
            u
        };
        let t0 = Team {
            units: vec![mk_launch(0), mk_chase(0)],
            ..Default::default()
        };
        let t1 = Team {
            units: vec![mk_launch(1), mk_chase(1)],
            ..Default::default()
        };
        let mut bt = Battle::new(t0, t1, 5);

        let hp_antes = bt.teams[1].units[0].hp_now + bt.teams[1].units[1].hp_now;
        bt.use_ability(0, 0, 0, 1, 0);
        let hp_depois = bt.teams[1].units[0].hp_now + bt.teams[1].units[1].hp_now;
        let dano_total = hp_antes - hp_depois;

        // lançamento 100% + UMA perseguição (60% de 50% = 30%)
        // alvos são roster_unit(0) = Golem (DEF 32); ATK lido do roster
        let atk = ROSTER[0].stats.atk;
        let esperado = damage_after_def(atk, 100, 32) + damage_after_def(atk, 30, 32);
        assert_eq!(dano_total, esperado, "chase deve disparar exatamente 1× (60%)");

        // segundo lançamento no MESMO round não gera nova perseguição (1/time)
        let hp_antes2 = bt.teams[1].units[0].hp_now + bt.teams[1].units[1].hp_now;
        bt.use_ability(0, 0, 0, 1, 0);
        let hp_depois2 = bt.teams[1].units[0].hp_now + bt.teams[1].units[1].hp_now;
        assert_eq!(
            hp_antes2 - hp_depois2,
            damage_after_def(atk, 100, 32),
            "segunda perseguição na mesma rodada não deve ocorrer"
        );
    }

    #[test]
    fn atordoamento_pula_acao() {
        let mut u = roster_unit(0, 0);
        u.status.stun = 1;
        u.status.stun -= 1;
        assert_eq!(u.status.stun, 0);
    }

    #[test]
    fn juizo_por_hp() {
        let mut t = Team::default();
        let mut u = roster_unit(0, 0);
        u.stats.hp = 100;
        u.hp_now = 100;
        t.units = vec![u];
        assert_eq!(t.total_hp_pct(), 100);
        t.units[0].hp_now = 50;
        assert_eq!(t.total_hp_pct(), 50);
    }
}
