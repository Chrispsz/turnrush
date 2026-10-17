//! Camada de "run" — partidas 1v1 estilo **roguelike acelerado**:
//! 9 rodadas (I–IX), draft simétrico por rodada com LCE, escalada +8%/rodada
//! e batalhas por turnos. Primeiro a 5 vitórias de rodada leva a partida.

use crate::cards::{self, CardKind, DraftState, Pick, CARD_POOL};
use crate::combat::{battle_teams, build_team, GreedyActor};
use crate::data::{ROSTER, TEAM_SIZE};
use crate::rng::Rng;
use crate::types::Outcome;

pub const ROUNDS_MAX: usize = 9; // Rodadas I..IX
pub const WINS_TO_MATCH: usize = 5;

/// Modificadores DWC de uma composição (per-mille).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DwcMods {
    pub dmg_permille: i32,
    pub hp_permille: i32,
}

impl Default for DwcMods {
    fn default() -> Self {
        DwcMods { dmg_permille: 1000, hp_permille: 1000 }
    }
}

/// Tabela de modificadores por composição/confronto (fornecida pelo calibrador).
pub trait DwcTable {
    /// Modificadores efetivos de `me` quando enfrenta `opp`.
    fn mods_for(&self, me: [usize; TEAM_SIZE], opp: [usize; TEAM_SIZE]) -> DwcMods;
}

/// Sem calibração (1000‰) — usado nos testes de simetria estrutural.
pub struct NoDwc;
impl DwcTable for NoDwc {
    fn mods_for(&self, _me: [usize; TEAM_SIZE], _opp: [usize; TEAM_SIZE]) -> DwcMods {
        DwcMods::default()
    }
}

#[derive(Clone, Debug, Default)]
pub struct PlayerRun {
    pub comp: [usize; TEAM_SIZE],
    pub draft: DraftState,
}

#[derive(Clone, Copy, Debug)]
pub struct MatchResult {
    /// Some(0) = jogador A venceu; Some(1) = B; None = empate.
    pub winner: Option<usize>,
    pub score: [usize; 2],
    pub rounds_played: usize,
}

/// Política de escolha de carta do draft (IA do simulador; no jogo real é o
/// humano que escolhe — a interface é a mesma).
fn pick_for(me: &PlayerRun, offers: &[usize]) -> Pick {
    let score = |c: &cards::Card| -> i32 {
        c.value * 10
            + match c.kind {
                CardKind::UnitStats { atk, .. } if atk > 0 => 5,
                CardKind::TeamPct { dmg_permille, .. } if dmg_permille > 0 => 8,
                _ => 0,
            }
    };
    let mut best = offers[0];
    for &o in offers.iter().skip(1) {
        if score(&CARD_POOL[o]) > score(&CARD_POOL[best]) {
            best = o;
        }
    }
    let target_unit = match CARD_POOL[best].kind {
        CardKind::UnitStats { atk, spd, .. } => {
            if atk > 0 || spd > 0 {
                (0..TEAM_SIZE)
                    .max_by_key(|&i| ROSTER[me.comp[i]].stats.atk)
                    .unwrap_or(0)
            } else {
                (0..TEAM_SIZE)
                    .max_by_key(|&i| ROSTER[me.comp[i]].stats.hp)
                    .unwrap_or(0)
            }
        }
        _ => 0,
    };
    Pick { card: best, target_unit }
}

/// Roda uma partida completa 1v1 (estilo roguelike acelerado).
pub fn play_match(
    comp_a: [usize; TEAM_SIZE],
    comp_b: [usize; TEAM_SIZE],
    seed: u64,
    dwc: &dyn DwcTable,
) -> MatchResult {
    let mut rng = Rng::new(seed);
    let mut pa = PlayerRun { comp: comp_a, draft: DraftState::default() };
    let mut pb = PlayerRun { comp: comp_b, draft: DraftState::default() };
    let mut score = [0usize; 2];
    let mut rounds_played = 0usize;

    for round in 0..ROUNDS_MAX {
        rounds_played += 1;

        // 1) Draft simétrico: UMA oferta para ambos + LCE ao resolver.
        let offers = cards::offer(&mut rng, 3);
        let pick_a = pick_for(&pa, &offers);
        let pick_b = pick_for(&pb, &offers);
        let (_, _) = cards::resolve_picks(pick_a.card, pick_b.card, &mut pa.draft, &mut pb.draft);

        // registra alvos corretos das cartas efetivas
        let last_a = pa.draft.picks.len() - 1;
        let last_b = pb.draft.picks.len() - 1;
        pa.draft.picks[last_a].target_unit = pick_a.target_unit;
        pb.draft.picks[last_b].target_unit = pick_b.target_unit;

        // 2) Batalha escalada da rodada com modificadores DWC por confronto.
        let ma = dwc.mods_for(pa.comp, pb.comp);
        let mb = dwc.mods_for(pb.comp, pa.comp);
        let ta = build_team(&pa.comp, 0, round, ma.dmg_permille, ma.hp_permille, &pa.draft.picks);
        let tb = build_team(&pb.comp, 1, round, mb.dmg_permille, mb.hp_permille, &pb.draft.picks);

        let mut actor_a = GreedyActor { rng: Rng::new(rng.next_u64()) };
        let mut actor_b = GreedyActor { rng: Rng::new(rng.next_u64()) };
        let outcome = battle_teams(ta, tb, rng.next_u64(), &mut actor_a, &mut actor_b);

        match outcome {
            Outcome::TeamAWin => score[0] += 1,
            Outcome::TeamBWin => score[1] += 1,
            Outcome::Draw => {}
        }

        if score[0] >= WINS_TO_MATCH || score[1] >= WINS_TO_MATCH {
            break;
        }
    }

    let winner = if score[0] > score[1] {
        Some(0)
    } else if score[1] > score[0] {
        Some(1)
    } else {
        None
    };
    MatchResult { winner, score, rounds_played }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partida_termina_com_placar_valido() {
        let r = play_match([0, 1, 2], [3, 4, 5], 42, &NoDwc);
        assert!(r.rounds_played >= 1 && r.rounds_played <= ROUNDS_MAX);
        assert!(r.score[0] <= WINS_TO_MATCH && r.score[1] <= WINS_TO_MATCH);
        assert!(r.score[0] + r.score[1] <= ROUNDS_MAX);
        // partida decidida → alguém chegou a 5 (ou placar 4-4/desempate indefinido)
        if let Some(w) = r.winner {
            assert!(r.score[w] >= WINS_TO_MATCH || r.rounds_played == ROUNDS_MAX);
        }
    }

    #[test]
    fn partida_deterministica() {
        let a = play_match([0, 5, 9], [2, 4, 11], 2024, &NoDwc);
        let b = play_match([0, 5, 9], [2, 4, 11], 2024, &NoDwc);
        assert_eq!(a.winner, b.winner);
        assert_eq!(a.score, b.score);
    }

    #[test]
    fn pick_prefere_carta_de_maior_valor() {
        let me = PlayerRun { comp: [0, 1, 2], draft: DraftState::default() };
        let p = pick_for(&me, &[0, 10, 2]); // 10 = tier 4 épica
        assert_eq!(p.card, 10);
    }
}
