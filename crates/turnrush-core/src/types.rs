//! Tipos centrais: stats, habilidades, status, unidades, times e resultados.
//!
//! Multiplicadores usam **per-mille** (1000 = 100%) para evitar float e
//! garantir determinismo entre plataformas.

/// 1000 = 100%.
pub const PER_MILLE: i32 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Stats {
    pub hp: i32,
    pub atk: i32,
    pub def: i32,
    pub spd: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Targeting {
    /// Um inimigo.
    Enemy,
    /// Todos os inimigos.
    EnemyAll,
    /// Um aliado.
    Ally,
    /// Todos os aliados.
    AllyAll,
    /// O próprio usuário.
    SelfTarget,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effect {
    /// % do ATK efetivo (100 = dano igual ao ATK).
    pub dmg_pct: i32,
    /// Cura fixa.
    pub heal: i32,
    /// Escudo concedido (absorve dano).
    pub shield: i32,
    /// Rodadas de atordoamento (o alvo perde a ação).
    pub stun: u8,
    /// Dano de veneno por rodada.
    pub poison_dmg: i32,
    /// Rodadas de veneno.
    pub poison_turns: u8,
    /// Modificação de ATK em % (positivo = buff, negativo = debuff).
    pub atk_pct: i32,
    /// Duração do buff/debuff em rodadas.
    pub buff_turns: u8,
    /// Inicia combo: permite perseguições (chase) dos aliados nesta ação.
    pub is_launch: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Ability {
    pub id: &'static str,
    pub name: &'static str,
    pub cost: i32,
    pub cooldown: u8,
    pub targeting: Targeting,
    pub effect: Effect,
    /// Pode ser disparada como perseguição (chase) de um lançamento, sem custo.
    pub is_chase: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Status {
    pub stun: u8,
    pub poison_dmg: i32,
    pub poison_turns: u8,
    pub atk_pct: i32,
    pub buff_turns: u8,
    pub shield: i32,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub cid: usize,
    pub name: &'static str,
    pub stats: Stats,
    pub hp_now: i32,
    pub energy: i32,
    pub abilities: Vec<Ability>,
    pub cds: Vec<u8>,
    pub status: Status,
    pub team: usize,
    /// Já perseguiu nesta rodada (chase limitado a 1 por rodada).
    pub chased_this_round: bool,
}

#[derive(Clone, Debug)]
pub struct Team {
    pub units: Vec<Unit>,
    /// Multiplicador de dano da composição (per-mille, calibrado pelo DWC).
    pub dmg_permille: i32,
    /// Multiplicador de vida da composição (per-mille).
    pub hp_permille: i32,
    /// Fator de escalada da rodada da partida (per-mille) — usado para
    /// escalar curas/escudos/venenos junto com o dano (anti-obsolescência).
    pub round_scale: i32,
}

impl Default for Team {
    fn default() -> Self {
        Team {
            units: Vec::new(),
            dmg_permille: PER_MILLE,
            hp_permille: PER_MILLE,
            round_scale: PER_MILLE,
        }
    }
}

impl Team {
    pub fn living(&self) -> impl Iterator<Item = &Unit> {
        self.units.iter().filter(|u| u.hp_now > 0)
    }

    pub fn is_defeated(&self) -> bool {
        self.units.iter().all(|u| u.hp_now <= 0)
    }

    /// % de vida restante (0..=100) — critério de desempate por tempo.
    pub fn total_hp_pct(&self) -> i32 {
        let max: i64 = self.units.iter().map(|u| u.stats.hp as i64).sum();
        let now: i64 = self.units.iter().map(|u| u.hp_now.max(0) as i64).sum();
        if max == 0 {
            0
        } else {
            ((now * 100) / max) as i32
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    TeamAWin,
    TeamBWin,
    Draw,
}
