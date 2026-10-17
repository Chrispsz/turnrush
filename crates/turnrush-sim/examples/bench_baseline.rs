//! TEMPORÁRIO (baseline do agente 8-a): mede ns/batalha 1v1. Apagar depois.
use turnrush_core::combat::{battle_teams, build_team, GreedyActor};
use turnrush_core::rng::Rng;
use turnrush_core::types::Outcome;

fn main() {
    let n: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let comp_a = [0usize, 1, 2];
    let comp_b = [3usize, 4, 5];
    let t0 = std::time::Instant::now();
    let mut wins = 0u64;
    for i in 0..n {
        let seed = 1000 + i;
        let ta = build_team(&comp_a, 0, 0, 1000, 1000, &[]);
        let tb = build_team(&comp_b, 1, 0, 1000, 1000, &[]);
        let mut aa = GreedyActor { rng: Rng::new(seed ^ 0xA1) };
        let mut ab = GreedyActor { rng: Rng::new(seed ^ 0xB2) };
        if battle_teams(ta, tb, seed, &mut aa, &mut ab) == Outcome::TeamAWin {
            wins += 1;
        }
    }
    let dt = t0.elapsed();
    println!(
        "baseline: {n} batalhas em {:.3}s → {:.0} ns/batalha (wr A = {:.3})",
        dt.as_secs_f64(),
        dt.as_nanos() as f64 / n as f64,
        wins as f64 / n as f64
    );
}
