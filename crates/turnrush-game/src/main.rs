//! TurnRush 2D — cliente visual jogável (Bevy).
//!
//! Modos:
//! - sem argumentos → abre a janela do jogo (menu, como-jogar, elenco,
//!   draft, batalha offline vs IA, recompensas, fim de run);
//! - `--check`     → verificação headless (sem janela): roster embutido +
//!   batalha real do engine + parser de eventos. Usado no CI/smoke test.
//! - `--shot <caminho_base>` → abre a janela, navega menu → como-jogar →
//!   elenco → draft → batalha, captura um PNG de cada tela
//!   (`<base>_menu.png`, `<base>_como-jogar.png`, `<base>_elenco.png`,
//!   `<base>_draft.png`, `<base>_battle.png`) e sai. Prova de renderização
//!   real sem interação — funciona com Xvfb + driver por software.

mod app;
mod battle_log;
mod controller;
mod scene;
mod sprites;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--check") {
        checar_headless();
        return;
    }
    let playback = args.iter().any(|a| a == "--playback");
    app::run(parse_shot(&args), playback);
}

/// Parseia `--shot <caminho_base>`; devolve `None` quando ausente.
fn parse_shot(args: &[String]) -> Option<app::ShotOpts> {
    let i = args.iter().position(|a| a == "--shot")?;
    let path = args
        .get(i + 1)
        .filter(|p| !p.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "/tmp/turnrush-shot".to_string());
    Some(app::ShotOpts { path })
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

    // v0.7: batalha interativa via controlador (eventos finos, IA simétrica)
    let mut ctl = controller::BattleController::new(
        &roster,
        [0, 1, 2, 3],
        [4, 5, 6, 7],
        11,
        turnrush_core::interactive::SideEdge::default(),
        turnrush_core::interactive::SideEdge::default(),
        true,
    );
    let mut passos = 0u64;
    let mut eventos_finos = 0u64;
    loop {
        passos += 1;
        assert!(passos < 100_000, "controlador não converge no --check");
        match ctl.pump() {
            controller::CtlPhase::PlayingEvents => {
                assert!(ctl.next_event().is_some(), "PlayingEvents sem evento");
                ctl.advance_event();
                eventos_finos += 1;
            }
            controller::CtlPhase::AwaitingPlayer { chase } => {
                let p = ctl.prompt().expect("prompt na vez");
                let acao = if chase {
                    match ctl.bt.chase_eligible_slots(p.side).first() {
                        Some(&s) => turnrush_core::interactive::ChosenAction::Chase { chaser_slot: s },
                        None => turnrush_core::interactive::ChosenAction::RejectChase,
                    }
                } else {
                    controller::greedy_decide(&ctl.bt, &p)
                };
                let _ = ctl.submit(acao);
            }
            controller::CtlPhase::Done(_) => break,
        }
    }
    println!(
        "batalha interativa: {:?} · {} eventos finos · {} rodadas · {} cadeias",
        ctl.outcome(),
        eventos_finos,
        ctl.bt.round(),
        ctl.bt.chain_links()
    );
    turnrush_core::logger::shutdown();
    println!("--check OK");
}
