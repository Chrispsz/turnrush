//! TurnRush 2D — cliente visual jogável (Bevy).
//!
//! Modos:
//! - sem argumentos → abre a janela do jogo;
//! - `--check`     → verificação headless (sem janela): roster embutido +
//!   batalha real do engine + parser de eventos. Usado no CI/smoke test.

mod app;
mod battle_log;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--check") {
        checar_headless();
        return;
    }
    app::run();
}

/// Verificação sem janela: prova que roster + engine + parser funcionam.
/// Não inicializa winit/wgpu — roda em qualquer ambiente, inclusive CI.
fn checar_headless() {
    let _ = turnrush_core::logger::init(0);
    let roster = app::load_roster_embedded();
    println!("roster: {} personagens (v{})", roster.len(), roster.version);
    assert!(roster.len() >= 12, "roster embutido deve ter ≥12 personagens");

    let mut f = turnrush_core::formation::Formation4v4::new(&roster, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    f.set_log(true);
    let outcome = f.run();
    let evs = battle_log::parse_all(f.events());
    let ataques = evs.iter().filter(|e| e.kind == battle_log::EvKind::Attack).count();
    let rodadas = evs.iter().filter(|e| e.kind == battle_log::EvKind::Round).count();
    println!(
        "batalha headless: {:?} · {} eventos · {} ataques · {} rodadas · {} cadeias",
        outcome,
        evs.len(),
        ataques,
        rodadas,
        f.chain_links()
    );
    assert!(evs.len() > 10, "playback deve ter eventos suficientes");
    assert_eq!(rodadas, f.rounds(), "parser deve reconhecer todas as rodadas");
    assert!(ataques > 0, "deve haver ataques no log");
    turnrush_core::logger::shutdown();
    println!("--check OK");
}
