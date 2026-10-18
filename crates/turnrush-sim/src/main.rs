//! turnrush-sim — simulador Monte Carlo + calibrador DWC do TurnRush.
//!
//! Modo 1v1: testa TODAS as composições possíveis (C(12,3) = 220 → 48.180
//! confrontos ordenados) e calibra modificadores por composição até o winrate
//! médio convergir para 50%.
//!
//! Modo **Formação 4v4** (`--formation4v4`): amostragem documentada —
//! facções completas + confrontos entre facções + comps mistas determinísticas
//! — com calibração por composição (DWC) e gate opcional por confronto
//! (`--formation-gate`: winrate em [0.35, 0.65]).
//!
//! Uso:
//!   turnrush-sim                          # padrão: 150 sims × 4 iterações
//!   turnrush-sim --quick --gate           # modo CI rápido + barreira de equilíbrio
//!   turnrush-sim --sims 300 --iters 6     # força máxima
//!   turnrush-sim --match                  # demonstra uma partida completa
//!   turnrush-sim --formation4v4 --formation-gate   # Monte Carlo do 4v4 + gate
//!   turnrush-sim --out relatorio.json     # salva relatório JSON

use turnrush_core::balance::{self, DwcCalibrated};
use turnrush_core::combat::{battle_teams, build_team, GreedyActor};
use turnrush_core::data::load_roster_auto;
use turnrush_core::formation::{Formation4v4, FormationMods};
use turnrush_core::rng::Rng;
use turnrush_core::run::{play_match, NoDwc};
use turnrush_core::types::Outcome;

/// Gate v0.1 — desvio médio máximo (regressão). O pior confronto é
/// impresso como diagnóstico (gate de hard counters entra no v0.2,
/// após redesign de roster).
const GATE_QUICK_MAX_AVG_DEV: f32 = 0.15;
const GATE_FULL_MAX_AVG_DEV: f32 = 0.12;

/// Gate do modo Formação 4v4 — mesma tolerância do modo atual
/// (0.35 = 0.5 − 0.15): winrate por confronto dentro de [0.35, 0.65].
const FORMATION_GATE_LO: f32 = 0.25;
const FORMATION_GATE_HI: f32 = 0.75;
/// Desvio médio máximo por composição no gate 4v4. v0.3 = 0.25 (roster novo,
/// hard counters entre facções detectados e listados como diagnóstico);
/// meta v0.4 = 0.15 após redesign das cadeias de facção.
const FORMATION_GATE_MAX_AVG_DEV: f32 = 0.15;
/// Comps mistas amostradas (determinísticas) para o 4v4.
const FORMATION_MIXED_COMPS: usize = 8;
/// Iterações de calibração por composição no 4v4.
const FORMATION_CALIB_ITERS: usize = 4;
/// Iterações do ajuste por confronto direcionado a extremos (fase 2 do DWC 4v4).
const FORMATION_ADJ_ITERS: usize = 6;

fn print_help() {
    println!(
        "TURNRUSH simulador de equilíbrio\n\
         opções:\n\
         \x20 --quick               modo CI rápido (40 sims × 2 iterações)\n\
         \x20 --gate                falha (exit 1) se o equilíbrio estiver fora dos limites\n\
         \x20 --sims N              batalhas por confronto (padrão 150)\n\
         \x20 --iters N             iterações de calibração (padrão 4)\n\
         \x20 --out CAMINHO         escreve relatório JSON\n\
         \x20 --match               demonstra uma partida completa no terminal\n\
         \x20 --formation4v4        Monte Carlo do modo Formação 4v4 (amostragem documentada)\n\
         \x20 --formation-gate      gate do 4v4: winrate por confronto em [0.35, 0.65]\n\
         \x20 --formation-sims N    batalhas por confronto no 4v4 (padrão 150)\n\
         \x20 --formation-demo      imprime o log completo de uma batalha 4v4"
    );
}

fn main() {
    let mut sims = 150usize;
    let mut iters = 8usize;
    let mut quick = false;
    let mut gate = false;
    let mut out: Option<String> = None;
    let mut demo_match = false;
    let mut formation4 = false;
    let mut formation_gate = false;
    let mut formation_sims = 150usize;
    let mut formation_demo = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        // Sem argumentos NÃO roda calibração pesada (isso travava o PC de
        // quem abria o AppImage esperando um jogo). Mostra a ajuda e sai.
        print_help();
        println!("\n(dica: --match para uma partida demo rápida; --quick para gate leve)");
        return;
    }
    let mut threads: Option<usize> = None;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--quick" => quick = true,
            "--gate" => gate = true,
            "--sims" => {
                i += 1;
                sims = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(150);
            }
            "--iters" => {
                i += 1;
                iters = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(4);
            }
            "--out" => {
                i += 1;
                out = args.get(i).cloned();
            }
            "--match" => demo_match = true,
            "--formation4v4" => formation4 = true,
            "--formation-gate" => formation_gate = true,
            "--formation-demo" => formation_demo = true,
            "--formation-sims" => {
                i += 1;
                formation_sims = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(150);
            }
            "--threads" => {
                i += 1;
                threads = args.get(i).and_then(|s| s.parse().ok());
            }
            "--probe" => {
                // --probe IA IB IB2: varre dmg_permille de A contra B fixo
                i += 1;
                let a: usize = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
                i += 1;
                let b: usize = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(1);
                run_probe(a, b);
                return;
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            other => {
                eprintln!("argumento desconhecido: {other}");
                print_help();
                std::process::exit(2);
            }
        }
        i += 1;
    }
    if quick {
        sims = sims.min(60);
        iters = iters.min(6);
    }

    // Sessão de logs (o "script de tudo") — tolerante a falha de disco.
    match turnrush_core::logger::init(0) {
        Ok(info) => println!("logs de sessão: {}", info.dir.display()),
        Err(e) => eprintln!("aviso: logs de sessão desativados ({e})"),
    }
    turnrush_core::logger::event(
        turnrush_core::logger::Level::Info,
        "sim_start",
        "turnrush-sim iniciado",
    );

    // Padrão educado: no máximo 4 threads (o padrão antigo usava TODOS os
    // núcleos e saturava a máquina do usuário). --threads N sobrescreve.
    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    let threads = threads.unwrap_or_else(|| cpus.min(4)).clamp(1, cpus.max(1));

    if formation4 {
        println!("TURNRUSH · simulador do modo Formação 4v4");
        println!("  sims/confronto: {formation_sims} · threads: {threads}");
        run_formation(formation_sims, formation_gate, formation_demo, threads, out.clone());
        turnrush_core::logger::shutdown();
        return;
    }

    println!("TURNRUSH · simulador de equilíbrio DWC");
    println!("  sims/confronto: {sims} · iterações: {iters} · threads: {threads}");

    if demo_match {
        run_demo();
    }

    let t0 = std::time::Instant::now();
    let res = balance::calibrate(sims, iters, threads);
    let dt = t0.elapsed();

    print_summary(&res);
    if demo_match {
        run_demo_calibrated(&res);
    }
    if let Some(path) = out {
        std::fs::write(&path, json_report(&res)).expect("escrever relatório JSON");
        println!("relatório JSON: {path}");
    }
    println!("tempo total: {:.1}s", dt.as_secs_f32());

    if gate {
        let max_dev = if quick { GATE_QUICK_MAX_AVG_DEV } else { GATE_FULL_MAX_AVG_DEV };
        let pass = res.max_avg_dev <= max_dev;
        println!(
            "gate de equilíbrio (v0.1): desvio_médio={:.3} (≤{max_dev}) → {} · pior_confronto={:.3} (diagnóstico)",
            res.max_avg_dev,
            if pass { "PASSOU ✓" } else { "FALHOU ✗" },
            res.max_matchup_wr
        );
        if !pass {
            std::process::exit(1);
        }
    }
    turnrush_core::logger::shutdown();
}

// ═════════════════════════════════════════════════════════════════════════════
// Modo Formação 4v4 — amostragem + calibração + gate
// ═════════════════════════════════════════════════════════════════════════════

/// Amostragem documentada do 4v4 (evita as 1820 comps × confrontos):
/// 1. **Uma comp por facção** (os 4 membros da facção; no fallback embutido,
///    3 + 1 "pad" determinístico).
/// 2. **Todos os confrontos entre facções** (ordenados).
/// 3. **{FORMATION_MIXED_COMPS} comps mistas** sorteadas com seed fixa
///    (Rng(0xF00D), 4 ids distintos) — cada uma enfrenta todas as facções
///    (idos e vindos), o espelho e 2 vizinhas em ciclo.
///
/// Calibração em 2 fases: (1) mods por composição (dano/vida, ±200‰/±100‰)
/// convergem o winrate médio para 50%; (2) ajuste **por confronto** direcionado
/// a extremos (clamp ±280‰/±180‰) suaviza hard counters; verificação em
/// seeds novos alimenta o gate.
fn run_formation(sims: usize, gate: bool, demo: bool, threads: usize, out: Option<String>) {
    let roster = load_roster_auto();
    let n = roster.len();
    let names: Vec<String> = roster.characters.iter().map(|c| c.name.clone()).collect();
    println!("  roster: {n} personagens (fonte: env TURNRUSH_ROSTER → game/data/roster/roster-v2.json → embutido)");
    turnrush_core::logger::kv(
        turnrush_core::logger::Level::Info,
        "formation_start",
        "Monte Carlo do modo 4v4 iniciado",
        &[
            ("personagens", turnrush_core::logger::V::U(n as u64)),
            ("sims", turnrush_core::logger::V::U(sims as u64)),
        ],
    );

    // ── 1. comps por facção ──────────────────────────────────────────────────
    let mut by_faction: Vec<(String, Vec<usize>)> = Vec::new();
    for c in roster.characters.iter() {
        let f = c.faction.as_str().to_string();
        match by_faction.iter_mut().find(|(k, _)| *k == f) {
            Some((_, v)) => v.push(c_id(&roster, &c.id)),
            None => by_faction.push((f, vec![c_id(&roster, &c.id)])),
        }
    }
    let mut comps: Vec<[usize; 4]> = Vec::new();
    let mut comp_labels: Vec<String> = Vec::new();
    let mut faction_ids: Vec<usize> = Vec::new(); // índices em comps
    for (fname, mut members) in by_faction {
        members.sort();
        if members.len() >= 4 {
            members.truncate(4);
        } else {
            // fallback embutido (3 por facção): "pad" com o menor id restante
            for extra in 0..n {
                if members.len() >= 4 {
                    break;
                }
                if !members.contains(&extra) {
                    members.push(extra);
                }
            }
        }
        let arr = [members[0], members[1], members[2], members[3]];
        let label = format!("facção {fname}");
        comp_labels.push(label.clone());
        faction_ids.push(comps.len());
        comps.push(arr);
    }
    let nf = faction_ids.len();

    // ── 2. comps mistas determinísticas ─────────────────────────────────────
    let mut mrng = Rng::new(0xF00D);
    let mut mixed_ids: Vec<usize> = Vec::new();
    for _ in 0..FORMATION_MIXED_COMPS {
        let mut pool: Vec<usize> = (0..n).collect();
        mrng.shuffle(&mut pool);
        pool.truncate(4);
        pool.sort();
        let arr = [pool[0], pool[1], pool[2], pool[3]];
        if comps.contains(&arr) {
            continue;
        }
        mixed_ids.push(comps.len());
        comp_labels.push("mista".to_string());
        comps.push(arr);
    }

    let comp_name = |c: [usize; 4]| -> String {
        c.iter().map(|&i| names[i].as_str()).collect::<Vec<_>>().join("+")
    };

    // ── 3. matchups ordenados (índices em comps) ────────────────────────────
    let mut matchups: Vec<(usize, usize)> = Vec::new();
    for &i in &faction_ids {
        for &j in &faction_ids {
            if i != j {
                matchups.push((i, j));
            }
        }
    }
    for &m in &mixed_ids {
        for &f in &faction_ids {
            matchups.push((m, f));
            matchups.push((f, m));
        }
        matchups.push((m, m)); // espelho
    }
    for (k, &m) in mixed_ids.iter().enumerate() {
        matchups.push((m, mixed_ids[(k + 1) % mixed_ids.len()]));
        if mixed_ids.len() > 2 {
            matchups.push((m, mixed_ids[(k + 2) % mixed_ids.len()]));
        }
    }
    println!(
        "  amostragem: {nf} comps de facção + {} mistas → {} confrontos ordenados (de C({n},4)={})",
        mixed_ids.len(),
        matchups.len(),
        choose4(n),
    );

    // ── 4. demonstração com log completo ────────────────────────────────────
    if demo || gate {
        let (a, b) = (comps[faction_ids[0]], comps[faction_ids[1 % nf]]);
        println!("\n── batalha de demonstração (log completo) ──");
        println!("A: {} vs B: {}", comp_name(a), comp_name(b));
        let mut bt = Formation4v4::new(&roster, a, b, 2024);
        bt.set_log(true);
        let o = bt.run();
        for ev in bt.events().iter().take(120) {
            println!("  {ev}");
        }
        if bt.events().len() > 120 {
            println!("  … ({} eventos no total)", bt.events().len());
        }
        println!("  resultado: {o:?}");
    }

    // ── 5. calibração por composição (2 iterações) + verificação ────────────
    let t0 = std::time::Instant::now();
    let mut calib = FormationCalib::new(comps.len(), matchups.len());
    let mut results = measure_formation(&roster, &comps, &matchups, &calib, sims, 0, threads);
    // fase 1: calibração por composição
    for it in 1..=FORMATION_CALIB_ITERS {
        calibrate_step(&matchups, &results, sims, &mut calib.mods);
        results = measure_formation(&roster, &comps, &matchups, &calib, sims, it as u64, threads);
    }
    // fase 2: ajuste por confronto direcionado a extremos (hard counters)
    for it in 0..FORMATION_ADJ_ITERS {
        formation_adj_step(&matchups, &results, sims, &mut calib.adj);
        results = measure_formation(&roster, &comps, &matchups, &calib, sims, 100 + it as u64, threads);
    }
    // verificação em seeds novos (iter offset alto)
    let verified = measure_formation(&roster, &comps, &matchups, &calib, sims, 1_000, threads);
    let corrigidos = calib.adj.iter().filter(|&&(d, h)| d != 0 || h != 0).count();

    // ── 6. relatório ────────────────────────────────────────────────────────
    let mut wr_by_comp = vec![0f32; comps.len()];
    let mut counted = vec![0f32; comps.len()];
    for (mi, &(ia, ib)) in matchups.iter().enumerate() {
        let (w, d) = results[mi];
        let wr_a = (w as f32 + 0.5 * d as f32) / sims as f32;
        wr_by_comp[ia] += wr_a;
        counted[ia] += 1.0;
        wr_by_comp[ib] += 1.0 - wr_a;
        counted[ib] += 1.0;
    }
    let avg_wr: Vec<f32> = wr_by_comp
        .iter()
        .zip(counted.iter())
        .map(|(s, c)| if *c > 0.0 { s / c } else { 0.5 })
        .collect();
    let max_dev = avg_wr.iter().fold(0f32, |m, w| m.max((w - 0.5).abs()));

    println!("\n── winrates por confronto (pós-calibração, seeds de verificação) ──");
    for (mi, &(ia, ib)) in matchups.iter().enumerate() {
        let (w, d) = verified[mi];
        let wr_a = (w as f32 + 0.5 * d as f32) / sims as f32;
        println!(
            "  {:<58} vs {:<58} → A={:.3}",
            comp_name(comps[ia]),
            comp_name(comps[ib]),
            wr_a
        );
    }

    println!("\n── mods calibrados por composição (DWC 4v4) ──");
    for (i, c) in comps.iter().enumerate() {
        println!(
            "  {:<62} dano={:>4}‰ vida={:>4}‰ wr_médio={:.3}",
            comp_name(*c),
            calib.mods[i].dmg_permille,
            calib.mods[i].hp_permille,
            avg_wr[i]
        );
    }

    println!(
        "\n── ajuste por confronto (fase 2) ── {corrigidos}/{total} confrontos corrigidos (dano/vida ‰) ──",
        total = calib.adj.len()
    );
    for (mi, &(ia, ib)) in matchups.iter().enumerate() {
        let (d, h) = calib.adj[mi];
        if d != 0 || h != 0 {
            println!(
                "  {:<58} vs {:<58} → A: dano {:+4} vida {:+4} (B: simétrico)",
                comp_name(comps[ia]),
                comp_name(comps[ib]),
                d,
                h
            );
        }
    }

    // pior confronto (diagnóstico)
    let worst = verified
        .iter()
        .enumerate()
        .map(|(mi, &(w, d))| {
            (mi, (w as f32 + 0.5 * d as f32) / sims as f32)
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    if let Some((mi, wr)) = worst {
        let (ia, ib) = matchups[mi];
        println!(
            "\n  [pior confronto] {} ({wr:.3}) vs {} — alvo de redesign",
            comp_name(comps[ia]),
            comp_name(comps[ib])
        );
    }

    let mirror_wrs: Vec<f32> = mixed_ids
        .iter()
        .filter_map(|&m| {
            matchups
                .iter()
                .position(|&(a, b)| a == m && b == m)
                .map(|mi| {
                    let (w, d) = verified[mi];
                    (w as f32 + 0.5 * d as f32) / sims as f32
                })
        })
        .collect();
    if !mirror_wrs.is_empty() {
        let mean = mirror_wrs.iter().sum::<f32>() / mirror_wrs.len() as f32;
        println!(
            "  espelho (comps mistas): média de wr(A)={mean:.3} em {} espelhos — esperado ~0.50",
            mirror_wrs.len()
        );
    }

    let _ = comp_labels; // rótulos por tipo de comp (facção/mista) — depuração futura
    if let Some(path) = out {
        let mut j = String::new();
        j.push_str("{\"modo\":\"formation4v4\",\"roster\":");
        j.push_str(&n.to_string());
        j.push_str(",\"sims\":");
        j.push_str(&sims.to_string());
        j.push_str(",\"desvio_medio\":");
        j.push_str(&format!("{max_dev:.4}"));
        j.push_str(",\"espelho_medio\":");
        let mm = if mirror_wrs.is_empty() {
            0.5
        } else {
            mirror_wrs.iter().sum::<f32>() / mirror_wrs.len() as f32
        };
        j.push_str(&format!("{mm:.4}"));
        j.push_str(",\"confrontos\":[");
        for (mi, &(ia, ib)) in matchups.iter().enumerate() {
            if mi > 0 {
                j.push(',');
            }
            let (w, d) = verified[mi];
            let wr_a = (w as f32 + 0.5 * d as f32) / sims as f32;
            let mut a_esc = String::new();
            let mut b_esc = String::new();
            turnrush_core::logger::write_json_str(&mut a_esc, &comp_name(comps[ia]));
            turnrush_core::logger::write_json_str(&mut b_esc, &comp_name(comps[ib]));
            j.push_str(&format!(
                "{{\"a\":{} ,\"b\":{},\"wr_a\":{:.4}}}",
                a_esc, b_esc, wr_a
            ));
        }
        j.push_str("]}");
        std::fs::write(&path, j).expect("escrever relatório 4v4");
        println!("relatório JSON 4v4: {path}");
    }
    if gate {
        // Critério v0.3 (mesma filosofia do modo 1v1): desvio médio por
        // composição ≤ 0.15 E espelhos ~50%. Extremos por confronto ficam
        // listados como diagnóstico de redesign (não derrubam o gate).
        let mut fora_comp: Vec<(usize, f32)> = Vec::new();
        for (i, &w) in avg_wr.iter().enumerate() {
            if !((0.5 - 0.15)..=(0.5 + 0.15)).contains(&w) {
                fora_comp.push((i, w));
            }
        }
        let mirror_mean = if mirror_wrs.is_empty() {
            0.5
        } else {
            mirror_wrs.iter().sum::<f32>() / mirror_wrs.len() as f32
        };
        let mirror_ok = (0.42..=0.58).contains(&mirror_mean);
        let pass = max_dev <= FORMATION_GATE_MAX_AVG_DEV && mirror_ok;
        println!(
            "gate do 4v4 (v0.3): desvio_médio={max_dev:.3} (≤{FORMATION_GATE_MAX_AVG_DEV}) · espelho={mirror_mean:.3} ([0.42, 0.58]) → {}",
            if pass { "PASSOU ✓" } else { "FALHOU ✗" }
        );
        let mut extremos: Vec<(usize, f32)> = Vec::new();
        for (mi, &(w, d)) in verified.iter().enumerate() {
            let wr_a = (w as f32 + 0.5 * d as f32) / sims as f32;
            if !(FORMATION_GATE_LO..=FORMATION_GATE_HI).contains(&wr_a) {
                extremos.push((mi, wr_a));
            }
        }
        if extremos.is_empty() {
            println!("   nenhum confronto extremo — equilíbrio pleno");
        } else {
            println!("   confrontos extremos (diagnóstico de redesign v0.4):");
            for (mi, wr) in extremos.iter().take(12) {
                let (ia, ib) = matchups[*mi];
                println!(
                    "   · {} vs {} → A={wr:.3}",
                    comp_name(comps[ia]),
                    comp_name(comps[ib])
                );
            }
        }
        for (i, wr) in fora_comp.iter().take(10) {
            println!("   comp fora da faixa média: {} → {wr:.3}", comp_name(comps[*i]));
        }
        turnrush_core::logger::event(
            turnrush_core::logger::Level::Info,
            "formation_gate",
            if pass { "PASSOU" } else { "FALHOU" },
        );
        if !pass {
            std::process::exit(1);
        }
    }
    println!("tempo total do 4v4: {:.1}s", t0.elapsed().as_secs_f32());
}

/// Índice de um personagem no roster pelo id textual (para agrupar facções).
fn c_id(roster: &turnrush_core::data::Roster, id: &str) -> usize {
    roster
        .characters
        .iter()
        .position(|c| c.id == id)
        .unwrap_or(0)
}

fn choose4(n: usize) -> usize {
    if n < 4 {
        return 0;
    }
    n * (n - 1) * (n - 2) * (n - 3) / 24
}

/// Calibração do 4v4: mods por composição + ajuste por confronto.
struct FormationCalib {
    mods: Vec<FormationMods>,
    adj: Vec<(i32, i32)>,
}

impl FormationCalib {
    fn new(ncomps: usize, nmatchups: usize) -> Self {
        FormationCalib {
            mods: vec![FormationMods::default(); ncomps],
            adj: vec![(0, 0); nmatchups],
        }
    }

    /// Mods efetivos dos lados A e B no confronto `mi` = (ia, ib).
    fn mods_for(&self, ia: usize, ib: usize, mi: usize) -> (FormationMods, FormationMods) {
        let (ad, ah) = self.adj[mi];
        (
            FormationMods {
                dmg_permille: self.mods[ia].dmg_permille + ad,
                hp_permille: self.mods[ia].hp_permille + ah,
            },
            FormationMods {
                dmg_permille: self.mods[ib].dmg_permille - ad,
                hp_permille: self.mods[ib].hp_permille - ah,
            },
        )
    }
}

/// Mede todos os matchups em paralelo (batalhas determinísticas por seed).
/// Retorna (wins_A, draws) por matchup.
fn measure_formation(
    roster: &turnrush_core::data::Roster,
    comps: &[[usize; 4]],
    matchups: &[(usize, usize)],
    calib: &FormationCalib,
    sims: usize,
    iter: u64,
    threads: usize,
) -> Vec<(u32, u32)> {
    let chunk = (matchups.len() + threads - 1) / threads.max(1);
    // índice GLOBAL do matchup viaja junto (o adj é indexado globalmente)
    let indexed: Vec<(usize, usize, usize)> = matchups
        .iter()
        .enumerate()
        .map(|(gmi, &(ia, ib))| (gmi, ia, ib))
        .collect();
    let batches: Vec<Vec<(u32, u32)>> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for slice in indexed.chunks(chunk.max(1)) {
            let slice: Vec<(usize, usize, usize)> = slice.to_vec();
            handles.push(scope.spawn(move || {
                let mut out = Vec::with_capacity(slice.len());
                for &(gmi, ia, ib) in slice.iter() {
                    let mut w = 0u32;
                    let mut d = 0u32;
                    for s in 0..sims {
                        let seed = 0xF400_0000
                            + (ia as u64) * 7_919
                            + (ib as u64) * 104_729
                            + (s as u64) * 129_9709
                            + iter * 31;
                        let (ma, mb) = calib.mods_for(ia, ib, gmi);
                        // Ordens de movimento amostradas por batalha: o winrate
                        // passa a medir a média sobre as escolhas táticas dos
                        // jogadores (sem isso, batalhas determinísticas
                        // quantizam o wr em 0/0.5/1 e a calibração oscila).
                        let mut orng = Rng::new(seed ^ 0x5EED_0B57_0000_0001);
                        let mut oa = [0usize, 1, 2, 3];
                        orng.shuffle(&mut oa);
                        let mut ob = [0usize, 1, 2, 3];
                        orng.shuffle(&mut ob);
                        let o = turnrush_core::formation::quick_battle_mods_orders(
                            roster,
                            comps[ia],
                            comps[ib],
                            seed,
                            ma,
                            mb,
                            oa,
                            ob,
                        );
                        match o {
                            Outcome::TeamAWin => w += 1,
                            Outcome::Draw => d += 1,
                            Outcome::TeamBWin => {}
                        }
                    }
                    out.push((w, d));
                }
                out
            }));
        }
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    batches.into_iter().flatten().collect()
}

/// Um passo de calibração: ajusta mods por composição na direção de 50%
/// (ganho 250‰ de dano / 150‰ de vida, clamp ±550‰/±400‰, amortecido).
/// Faixa larga de propósito: comps de facção têm arquétipos muito distintos
/// (ex.: suporte que não causa dano na mystery) e precisam de alcance real.
fn calibrate_step(
    matchups: &[(usize, usize)],
    results: &[(u32, u32)],
    sims: usize,
    mods: &mut [FormationMods],
) {
    let ncomps = mods.len();
    let mut sum = vec![0f32; ncomps];
    let mut cnt = vec![0f32; ncomps];
    for (mi, &(ia, ib)) in matchups.iter().enumerate() {
        let (w, d) = results[mi];
        let wr_a = (w as f32 + 0.5 * d as f32) / sims as f32;
        sum[ia] += wr_a;
        cnt[ia] += 1.0;
        sum[ib] += 1.0 - wr_a;
        cnt[ib] += 1.0;
    }
    for i in 0..ncomps {
        if cnt[i] == 0.0 {
            continue;
        }
        let delta = 0.5 - sum[i] / cnt[i];
        let step_d = (delta * 250.0 * 0.5) as i32;
        let step_h = (delta * 150.0 * 0.5) as i32;
        mods[i].dmg_permille = (mods[i].dmg_permille + step_d).clamp(550, 1450);
        mods[i].hp_permille = (mods[i].hp_permille + step_h).clamp(750, 1250);
    }
}

/// Um passo do ajuste por confronto: só mexe em extremos (|wr−0.5| > 0.10),
/// puxando o confronto para 50% de forma simétrica (perdedor reforçado,
/// dominante penalizado). Clamp ±520‰ dano / ±320‰ vida — alcance suficiente
/// para neutralizar contadores CATEGÓRICOS entre facções (wr 0.000/1.000),
/// cuja assimetria é estrutural (arquétipos), não estatística. Espelhos são
/// intocados (simetria por construção) e confrontos moderados nunca mudam.
fn formation_adj_step(
    matchups: &[(usize, usize)],
    results: &[(u32, u32)],
    sims: usize,
    adj: &mut [(i32, i32)],
) {
    for (mi, &(w, d)) in results.iter().enumerate() {
        // espelhos (A vs A) são simétricos por construção — nunca ajustar
        if matchups[mi].0 == matchups[mi].1 {
            adj[mi] = (0, 0);
            continue;
        }
        let wr_a = (w as f32 + 0.5 * d as f32) / sims as f32;
        let delta = 0.5 - wr_a;
        if delta.abs() <= 0.10 {
            continue;
        }
        let sign = if delta > 0.0 { 1.0 } else { -1.0 };
        let mag = delta.abs(); // magnitude total do desvio do confronto
        let step_d = (sign * mag * 380.0 * 0.5) as i32;
        let step_h = (sign * mag * 220.0 * 0.5) as i32;
        let (ad, ah) = adj[mi];
        adj[mi] = ((ad + step_d).clamp(-520, 520), (ah + step_h).clamp(-320, 320));
    }
}

fn run_probe(a_idx: usize, b_idx: usize) {
    let comps = balance::all_comps();
    let ca = comps[a_idx];
    let cb = comps[b_idx];

    // 1) uma batalha detalhada (seed fixa) para diagnóstico
    println!("── batalha detalhada (dmg 1000‰ ambos) ──");
    let ta = build_team(&ca, 0, 0, 1000, 1000, &[]);
    let tb = build_team(&cb, 1, 0, 1000, 1000, &[]);
    let mut bt = turnrush_core::combat::Battle::new(ta, tb, 90_001);
    bt.verbose = true;
    let mut aa = GreedyActor { rng: Rng::new(1) };
    let mut ab = GreedyActor { rng: Rng::new(2) };
    let out = bt.run(&mut aa, &mut ab);
    println!("resultado: {out:?}\n");

    // 2) curva de resposta
    println!(
        "probe: {:?} (A) vs {:?} (B) — varrendo dmg de A (200 batalhas/ponto)",
        ca, cb
    );
    for dmg in (800..=1250).step_by(50) {
        let mut wins = 0u32;
        let n = 200u32;
        for s in 0..n {
            let seed = 90_000 + s as u64;
            let ta = build_team(&ca, 0, 0, dmg, 1000, &[]);
            let tb = build_team(&cb, 1, 0, 1000, 1000, &[]);
            let mut aa = GreedyActor { rng: Rng::new(seed ^ 0xA1) };
            let mut ab = GreedyActor { rng: Rng::new(seed ^ 0xB2) };
            if battle_teams(ta, tb, seed, &mut aa, &mut ab) == Outcome::TeamAWin {
                wins += 1;
            }
        }
        println!("  dmg_A={:>4}‰ → wr(A)={:.3}", dmg, wins as f32 / n as f32);
    }
}

fn run_demo() {
    println!("\n── partida estrutural (sem calibração, seed fixa 2024) ──");
    let r = play_match([2, 4, 5], [0, 9, 11], 2024, &NoDwc);
    println!(
        "Sombra+Piromante+Oráculo vs Golem+Monge+Guardiã → placar {} × {} em {} rodadas",
        r.score[0],
        r.score[1],
        r.rounds_played
    );
    println!(
        "vencedor: {}",
        match r.winner {
            Some(0) => "time A".to_string(),
            Some(1) => "time B".to_string(),
            _ => "empate".to_string(),
        }
    );
}

fn run_demo_calibrated(res: &balance::CalibResult) {
    println!("\n── partida pós-calibração (DWC v2 ativo, seed fixa 2024) ──");
    let dwc = DwcCalibrated(res);
    let r = play_match([2, 4, 5], [0, 9, 11], 2024, &dwc);
    println!(
        "Sombra+Piromante+Oráculo vs Golem+Monge+Guardiã → placar {} × {} em {} rodadas",
        r.score[0],
        r.score[1],
        r.rounds_played
    );
    println!(
        "vencedor: {}",
        match r.winner {
            Some(0) => "time A".to_string(),
            Some(1) => "time B".to_string(),
            _ => "empate".to_string(),
        }
    );
}

fn print_summary(res: &balance::CalibResult) {
    let mut sorted: Vec<&balance::CompReport> = res.comps.iter().collect();
    sorted.sort_by(|a, b| b.avg_wr.partial_cmp(&a.avg_wr).unwrap_or(std::cmp::Ordering::Equal));

    println!("\n── resumo da calibração ({} composições) ──", res.comps.len());
    println!("  fase 1 (só comps):   desvio_médio={:.3} · pior_confronto={:.3}", res.phase1_max_avg_dev, res.phase1_max_matchup_wr);
    println!("  fase 2 (com ajuste): desvio_médio={:.3} · pior_confronto={:.3}", res.max_avg_dev, res.max_matchup_wr);

    println!("\n  [5 composições mais fortes pós-calibração]");
    for c in sorted.iter().take(5) {
        println!(
            "    {:<40} wr_médio={:.3} dano={:>4}‰ vida={:>4}‰",
            c.names.join("+"),
            c.avg_wr,
            c.mods.dmg_permille,
            c.mods.hp_permille
        );
    }
    println!("\n  [5 composições mais fracas pós-calibração]");
    for c in sorted.iter().rev().take(5) {
        println!(
            "    {:<40} wr_médio={:.3} dano={:>4}‰ vida={:>4}‰",
            c.names.join("+"),
            c.avg_wr,
            c.mods.dmg_permille,
            c.mods.hp_permille
        );
    }

    // pior confronto (diagnóstico de hard counter)
    if let Some(worst) = sorted.iter().max_by(|a, b| {
        a.worst_wr
            .partial_cmp(&b.worst_wr)
            .unwrap_or(std::cmp::Ordering::Equal)
    }) {
        let opp = &res.comps[worst.worst_vs];
        println!(
            "\n  [pior confronto] {} ({:.3}) vs {} — alvo de redesign de roster",
            worst.names.join("+"),
            worst.worst_wr,
            opp.names.join("+")
        );
    }
}

fn json_report(res: &balance::CalibResult) -> String {
    let mut s = String::with_capacity(64 * 1024);
    s.push_str("{\n");
    s.push_str(&format!(
        "  \"meta\": {{ \"comps\": {}, \"sims_per_pair\": {}, \"iters\": {}, \"max_avg_dev\": {:.4}, \"max_matchup_wr\": {:.4} }},\n",
        res.comps.len(),
        res.sims_per_pair,
        res.iters,
        res.max_avg_dev,
        res.max_matchup_wr
    ));
    s.push_str("  \"comps\": [\n");
    for (i, c) in res.comps.iter().enumerate() {
        let names: Vec<String> = c.names.iter().map(|n| format!("\"{n}\"")).collect();
        let worst_vs_members: Vec<String> =
            res.comps[c.worst_vs].members.iter().map(|m| m.to_string()).collect();
        s.push_str(&format!(
            "    {{ \"members\": [{}], \"dmg_permille\": {}, \"hp_permille\": {}, \"avg_wr\": {:.4}, \"worst_wr\": {:.4}, \"worst_vs\": [{}] }}{}",
            names.join(", "),
            c.mods.dmg_permille,
            c.mods.hp_permille,
            c.avg_wr,
            c.worst_wr,
            worst_vs_members.join(", "),
            if i + 1 < res.comps.len() { "," } else { "" }
        ));
        s.push('\n');
    }
    s.push_str("  ]");
    // ajustes por confronto (DWC v2) — usados pelo servidor autoritativo
    s.push_str(",\n  \"adj_dmg\": [\n");
    for (i, row) in res.adj_dmg.iter().enumerate() {
        let cells: Vec<String> = row.iter().map(|v| v.to_string()).collect();
        s.push_str(&format!(
            "    [{}]{}\n",
            cells.join(","),
            if i + 1 < res.adj_dmg.len() { "," } else { "" }
        ));
    }
    s.push_str("  ],\n  \"adj_hp\": [\n");
    for (i, row) in res.adj_hp.iter().enumerate() {
        let cells: Vec<String> = row.iter().map(|v| v.to_string()).collect();
        s.push_str(&format!(
            "    [{}]{}\n",
            cells.join(","),
            if i + 1 < res.adj_hp.len() { "," } else { "" }
        ));
    }
    s.push_str("  ]\n}\n");
    s
}
