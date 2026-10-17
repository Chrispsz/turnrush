//! **DWC — Dynamic Win-rate Calibration (v2)**.
//!
//! Duas camadas de calibração sobre o Monte Carlo de TODAS as composições:
//!
//! 1. **Por composição**: modificadores globais (dano/vida em per-mille)
//!    ajustados até o winrate médio de cada comp convergir para 50%.
//! 2. **Por confronto (matchup)**: mesmo com comps médias em 50%, podem
//!    existir *hard counters* (A ganha de B 100% das vezes). O sistema calcula
//!    um ajuste por PAR (i, j) aplicado no lado desfavorecido — o "sistema de
//!    inteligência" que garante que nenhuma dupla de composições tenha
//!    vantagem real, conforme a filosofia do jogo.
//!
//! Uma **passada de verificação** re-simula tudo com os ajustes aplicados e
//! produz as métricas finais usadas pelo gate de CI.

use crate::combat::{battle_teams, build_team, GreedyActor};
use crate::data::{ROSTER, TEAM_SIZE};
use crate::rng::Rng;
use crate::run::{DwcMods, DwcTable};
use crate::types::Outcome;

/// Limites da calibração por composição (per-mille).
pub const DMG_MIN: i32 = 780;
pub const DMG_MAX: i32 = 1220;
pub const HP_MIN: i32 = 850;
pub const HP_MAX: i32 = 1150;

/// Ganhos de ajuste por iteração (com amortecimento 0.6 contra oscilação).
const GAIN_DMG: i32 = 350;
const GAIN_HP: i32 = 175;
const COMP_DAMPING: f32 = 0.6;

/// Ganhos e limites do ajuste por confronto (reservados para o DWC v3 com
/// realimentação amortecida; a fase 2 atual usa os ganhos locais de
/// `compute_adj`). Mantidos para o teste de limites e para a v3.
#[allow(dead_code)]
const ADJ_DMG_GAIN: f32 = 200.0;
#[allow(dead_code)]
const ADJ_DMG_MAX: i32 = 200;
#[allow(dead_code)]
const ADJ_HP_GAIN: f32 = 100.0;
#[allow(dead_code)]
const ADJ_HP_MAX: i32 = 100;
/// Peso do ajuste recém-calculado (0.6 = resposta rápida, 0.4 = memória).
#[allow(dead_code)]
const ADJ_DAMPING: f32 = 0.6;

#[derive(Clone, Debug)]
pub struct CompReport {
    pub members: [usize; TEAM_SIZE],
    pub names: Vec<&'static str>,
    pub mods: DwcMods,
    /// Winrate médio contra todos os adversários (0..1), pós-verificação.
    pub avg_wr: f32,
    /// Pior matchup dominante (winrate e composição adversária).
    pub worst_wr: f32,
    pub worst_vs: usize,
}

#[derive(Clone, Debug)]
pub struct CalibResult {
    pub comps: Vec<CompReport>,
    pub sims_per_pair: usize,
    pub iters: usize,
    /// Maior desvio absoluto do winrate médio de 50% (pós-verificação).
    pub max_avg_dev: f32,
    /// Maior winrate de matchup isolado observado (pós-verificação).
    pub max_matchup_wr: f32,
    /// Diagnóstico: mesmas métricas no fim da fase 1 (antes do ajuste por confronto).
    pub phase1_max_avg_dev: f32,
    pub phase1_max_matchup_wr: f32,
    /// Ajuste por confronto: bônus de dano de i contra j (per-mille).
    pub adj_dmg: Vec<Vec<i32>>,
    /// Ajuste por confronto: bônus de vida de i contra j (per-mille).
    pub adj_hp: Vec<Vec<i32>>,
}

impl CalibResult {
    pub fn index_of(&self, comp: [usize; TEAM_SIZE]) -> Option<usize> {
        self.comps.iter().position(|c| c.members == comp)
    }

    /// Modificadores efetivos da comp `a` quando enfrenta a comp `b`.
    pub fn mods_for(&self, a: usize, b: usize) -> DwcMods {
        DwcMods {
            dmg_permille: (self.comps[a].mods.dmg_permille + self.adj_dmg[a][b])
                .clamp(700, 1300),
            hp_permille: (self.comps[a].mods.hp_permille + self.adj_hp[a][b]).clamp(750, 1250),
        }
    }
}

/// Adaptador que expõe o resultado calibrado como tabela DWC para partidas.
pub struct DwcCalibrated<'a>(pub &'a CalibResult);

impl DwcTable for DwcCalibrated<'_> {
    fn mods_for(&self, me: [usize; TEAM_SIZE], opp: [usize; TEAM_SIZE]) -> DwcMods {
        match (self.0.index_of(me), self.0.index_of(opp)) {
            (Some(i), Some(j)) => self.0.mods_for(i, j),
            _ => DwcMods::default(),
        }
    }
}

/// Todas as composições de tamanho TEAM_SIZE sobre o roster (C(n, k)).
pub fn all_comps() -> Vec<[usize; TEAM_SIZE]> {
    let n = ROSTER.len();
    let mut out = Vec::new();
    let mut chosen = [0usize; TEAM_SIZE];
    fn rec(
        start: usize,
        depth: usize,
        n: usize,
        k: usize,
        chosen: &mut [usize],
        out: &mut Vec<[usize; TEAM_SIZE]>,
    ) {
        if depth == k {
            out.push([chosen[0], chosen[1], chosen[2]]);
            return;
        }
        for i in start..n {
            chosen[depth] = i;
            rec(i + 1, depth + 1, n, k, chosen, out);
        }
    }
    rec(0, 0, n, TEAM_SIZE, &mut chosen, &mut out);
    out
}

type AdjMatrices = (Vec<Vec<i32>>, Vec<Vec<i32>>);

/// Simula todos os confrontos ordenados (i→j) com os modificadores e ajustes
/// informados. Retorna a matriz de vitórias de i contra j.
fn run_matrix(
    comps: &[[usize; TEAM_SIZE]],
    mods: &[DwcMods],
    adj: Option<&AdjMatrices>,
    sims: usize,
    iter: usize,
    threads: usize,
) -> Vec<Vec<u32>> {
    let n = comps.len();
    let mut pairs: Vec<(usize, usize)> = Vec::with_capacity(n * (n - 1));
    for i in 0..n {
        for j in 0..n {
            if i != j {
                pairs.push((i, j));
            }
        }
    }
    let chunk = (pairs.len() + threads - 1) / threads.max(1);

    // Cada thread calcula suas batalhas isoladamente (zero compartilhamento);
    // os resultados são mesclados sequencialmente ao final.
    let batches: Vec<Vec<((usize, usize), u32)>> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for slice in pairs.chunks(chunk.max(1)) {
            let slice = slice.to_vec();
            handles.push(scope.spawn(move || {
                let mut out = Vec::with_capacity(slice.len());
                for &(i, j) in &slice {
                    let mut w = 0u32;
                    for b in 0..sims {
                        let seed = (i as u64) * 1_000_003
                            + (j as u64) * 7_919
                            + b as u64 * 104_729
                            + (iter as u64) * 1_299_709;
                        // ajuste por confronto aplica-se aos DOIS lados:
                        // i recebe adj[i][j] e j recebe adj[j][i]
                        let (adj_a_d, adj_a_h, adj_b_d, adj_b_h) = match adj {
                            Some((ad, ah)) => (ad[i][j], ah[i][j], ad[j][i], ah[j][i]),
                            None => (0, 0, 0, 0),
                        };
                        let ta = build_team(
                            &comps[i],
                            0,
                            0,
                            mods[i].dmg_permille + adj_a_d,
                            mods[i].hp_permille + adj_a_h,
                            &[],
                        );
                        let tb = build_team(
                            &comps[j],
                            1,
                            0,
                            mods[j].dmg_permille + adj_b_d,
                            mods[j].hp_permille + adj_b_h,
                            &[],
                        );
                        let mut actor_a = GreedyActor { rng: Rng::new(seed ^ 0xA1) };
                        let mut actor_b = GreedyActor { rng: Rng::new(seed ^ 0xB2) };
                        if battle_teams(ta, tb, seed, &mut actor_a, &mut actor_b)
                            == Outcome::TeamAWin
                        {
                            w += 1;
                        }
                    }
                    out.push(((i, j), w));
                }
                out
            }));
        }
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut wins = vec![vec![0u32; n]; n];
    for batch in batches {
        for ((i, j), w) in batch {
            wins[i][j] = w;
        }
    }
    wins
}

/// Deriva o ajuste por confronto **direcionado aos extremos**: apenas pares
/// com wr > 0.85 ou < 0.15 (hard counters categóricos) recebem correção —
/// o lado dominante é penalizado e o lado fraco reforçado. Confrontos
/// moderados ficam intactos (correção linear neles causa overshoot).
fn compute_adj(wins: &[Vec<u32>], sims: usize) -> AdjMatrices {
    let n = wins.len();
    let mut adj_dmg = vec![vec![0i32; n]; n];
    let mut adj_hp = vec![vec![0i32; n]; n];
    const THRESH_HI: f32 = 0.85;
    const THRESH_LO: f32 = 0.15;
    const PEN_GAIN: f32 = 300.0;
    const PEN_MAX: i32 = 140;
    const BOOST_GAIN: f32 = 150.0;
    const BOOST_MAX: i32 = 70;
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            let wr = wins[i][j] as f32 / sims as f32;
            if wr > THRESH_HI {
                let dev = wr - 0.5; // 0.35..0.5
                adj_dmg[i][j] = -((dev * PEN_GAIN) as i32).clamp(0, PEN_MAX);
                adj_hp[i][j] = -((dev * PEN_GAIN / 2.0) as i32).clamp(0, PEN_MAX / 2);
                adj_dmg[j][i] = ((dev * BOOST_GAIN) as i32).clamp(0, BOOST_MAX);
                adj_hp[j][i] = ((dev * BOOST_GAIN / 2.0) as i32).clamp(0, BOOST_MAX / 2);
            } else if wr < THRESH_LO {
                let dev = 0.5 - wr;
                adj_dmg[i][j] = ((dev * BOOST_GAIN) as i32).clamp(0, BOOST_MAX);
                adj_hp[i][j] = ((dev * BOOST_GAIN / 2.0) as i32).clamp(0, BOOST_MAX / 2);
                adj_dmg[j][i] = -((dev * PEN_GAIN) as i32).clamp(0, PEN_MAX);
                adj_hp[j][i] = -((dev * PEN_GAIN / 2.0) as i32).clamp(0, PEN_MAX / 2);
            }
        }
    }
    (adj_dmg, adj_hp)
}

/// Executa a calibração completa em **duas fases** (sem realimentação, estável):
///
/// 1. **Fase de composição**: itera ajustando modificadores globais por
///    composição até o winrate médio convergir para 50%.
/// 2. **Fase de confronto**: calcula UMA vez o ajuste por par (i, j) a partir
///    da matriz final (mata hard counters residuais) e **verifica** em seeds
///    novos. Sem loop de realimentação → sem oscilação.
pub fn calibrate(sims_per_pair: usize, iters: usize, threads: usize) -> CalibResult {
    let comps = all_comps();
    let n = comps.len();
    let mut mods: Vec<DwcMods> = vec![DwcMods::default(); n];
    let mut wins = vec![vec![0u32; n]; n];

    // ── fase 1: calibração por composição ──
    for it in 0..iters.max(1) {
        wins = run_matrix(&comps, &mods, None, sims_per_pair, it, threads);
        if it + 1 >= iters.max(1) {
            break;
        }
        let mut avg = vec![0f32; n];
        for (i, row) in wins.iter().enumerate() {
            let sum: f32 = row
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, &w)| w as f32 / sims_per_pair as f32)
                .sum();
            avg[i] = sum / (n - 1) as f32;
        }
        for i in 0..n {
            let delta = 0.5 - avg[i];
            let step_d = (delta * GAIN_DMG as f32 * COMP_DAMPING) as i32;
            let step_h = (delta * GAIN_HP as f32 * COMP_DAMPING) as i32;
            mods[i].dmg_permille = (mods[i].dmg_permille + step_d).clamp(DMG_MIN, DMG_MAX);
            mods[i].hp_permille = (mods[i].hp_permille + step_h).clamp(HP_MIN, HP_MAX);
        }
    }

    // diagnóstico da fase 1
    let mut phase1_max_avg_dev = 0f32;
    let mut phase1_max_matchup_wr = 0f32;
    for (i, row) in wins.iter().enumerate() {
        let mut sum = 0f32;
        for (j, &w) in row.iter().enumerate() {
            if i != j {
                let wr = w as f32 / sims_per_pair as f32;
                sum += wr;
                if wr > phase1_max_matchup_wr {
                    phase1_max_matchup_wr = wr;
                }
            }
        }
        phase1_max_avg_dev = phase1_max_avg_dev.max((sum / (n - 1) as f32 - 0.5).abs());
    }

    // ── fase 2: ajuste por confronto (uma passada) + verificação ──
    let adj = compute_adj(&wins, sims_per_pair);
    let wins_verified = run_matrix(
        &comps,
        &mods,
        Some(&adj),
        sims_per_pair,
        iters.max(1) + 7,
        threads,
    );

    // ── relatório final (pós-verificação) ──
    let mut comps_report = Vec::with_capacity(n);
    let mut max_avg_dev = 0f32;
    let mut max_matchup_wr = 0f32;
    for (i, row) in wins_verified.iter().enumerate() {
        let mut sum = 0f32;
        let mut worst = 0f32;
        let mut worst_vs = 0usize;
        for (j, &w) in row.iter().enumerate() {
            if i == j {
                continue;
            }
            let wr = w as f32 / sims_per_pair as f32;
            sum += wr;
            if wr > worst {
                worst = wr;
                worst_vs = j;
            }
            if wr > max_matchup_wr {
                max_matchup_wr = wr;
            }
        }
        let avg_wr = sum / (n - 1) as f32;
        max_avg_dev = max_avg_dev.max((avg_wr - 0.5).abs());
        comps_report.push(CompReport {
            members: comps[i],
            names: comps[i].iter().map(|&c| ROSTER[c].name).collect(),
            mods: mods[i],
            avg_wr,
            worst_wr: worst,
            worst_vs,
        });
    }

    CalibResult {
        comps: comps_report,
        sims_per_pair,
        iters: iters.max(1),
        max_avg_dev,
        max_matchup_wr,
        phase1_max_avg_dev,
        phase1_max_matchup_wr,
        adj_dmg: adj.0,
        adj_hp: adj.1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gera_todas_as_composicoes() {
        // C(12,3) = 220
        assert_eq!(all_comps().len(), 220);
        let c = all_comps();
        assert_eq!(c[0], [0, 1, 2]);
        assert_eq!(c[219], [9, 10, 11]);
    }

    #[test]
    fn limites_de_calibracao() {
        const { assert!(DMG_MIN < 1000 && 1000 < DMG_MAX) };
        const { assert!(HP_MIN < 1000 && 1000 < HP_MAX) };
        const { assert!(ADJ_DMG_GAIN > 0.0 && ADJ_HP_GAIN > 0.0) };
    }
}
