//! Peça 2 do quebra-cabeça: **determinismo** (o relógio).
//!
//! Encaixa na Peça 1: dados limpos só valem se o motor der a MESMA partida
//! com a MESMA seed, em qualquer máquina. Também garante que OBSERVAR não
//! muda o mundo: `legal_actions`/`preview_action`/`prompt` são puros.

mod common;

use common::*;
use turnrush_core::combat::scale_for_round;
use turnrush_core::formation::Formation4v4;
use turnrush_core::interactive::{ActionKind, ChosenAction, InteractiveBattle};
use turnrush_core::rng::Rng;
use turnrush_core::run::{play_match, NoDwc};
use turnrush_core::types::Outcome;

#[test]
fn peca_02_encaixa_na_peca_01() {
    ancora_dados();
}

/// Mesma seed ⇒ sequência de eventos idêntica e mesmo resultado.
#[test]
fn mesma_seed_reproduz_a_partida_evento_a_evento() {
    let r = real_roster();
    let mut a = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 42);
    let mut b = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 42);
    let (oa, _, eva) = play_out(&mut a, &greedy);
    let (ob, _, evb) = play_out(&mut b, &greedy);
    assert_eq!(oa, ob);
    assert_eq!(eva, evb, "eventos divergem com a mesma seed");
    assert!(eva.len() > 50, "batalha real deve ter eventos suficientes");
}

/// Seeds diferentes produzem partidas diferentes (o relógio anda, não ecoa).
#[test]
fn seeds_diferentes_produzem_partidas_diferentes() {
    let r = real_roster();
    let mut vistos: Vec<(Outcome, usize)> = Vec::new();
    for seed in 1..=8u64 {
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], seed);
        let (o, _, evs) = play_out(&mut bt, &greedy);
        vistos.push((o, evs.len()));
    }
    let unicos: std::collections::HashSet<(String, usize)> = vistos
        .iter()
        .map(|(o, n)| (format!("{o:?}"), *n))
        .collect();
    assert!(unicos.len() > 1, "8 seeds geraram partidas idênticas — RNG morto?");
}

/// Observar é puro: prévia estável e sem efeito no estado.
#[test]
fn prever_nao_muda_o_mundo() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let p = bt.prompt(); // abre rodada 1

    let acao = ChosenAction::Act { kind: ActionKind::Standard, target: None };
    let prev1 = bt.preview_action(p.side, p.slot, acao).expect("prévia do padrão");
    let prev2 = bt.preview_action(p.side, p.slot, acao).expect("prévia 2×");
    let prev3 = bt.preview_action(p.side, p.slot, acao).expect("prévia 3×");
    assert_eq!(prev1, prev2, "prévia não é estável");
    assert_eq!(prev2, prev3, "prévia não é estável (3ª)");

    let antes = face(&bt);
    let gen = bt.generation();
    let legais1 = bt.legal_actions(p.side, p.slot);
    let legais2 = bt.legal_actions(p.side, p.slot);
    assert_eq!(legais1, legais2, "legal_actions não é determinístico");
    assert_eq!(face(&bt), antes, "observar mudou o estado");
    assert_eq!(bt.generation(), gen, "observar consumiu decisão");
}

/// Chamar prompt/legal_actions repetidamente não queima fluxo nem rodadas.
#[test]
fn observar_nao_queima_rodada() {
    let r = synth();
    let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 7);
    let _ = bt.prompt();
    for _ in 0..10 {
        let _ = bt.legal_actions(0, 0);
        let _ = bt.unit(0, 0);
        let _ = bt.chain_links();
    }
    assert_eq!(bt.round(), 1, "observação avançou rodada");
    assert_eq!(bt.unit(0, 0).fluxo, 10, "observação roubou fluxo");
}

/// A formação 4v4 (motor do Monte Carlo) também é um relógio.
#[test]
fn formacao_4v4_e_deterministica() {
    let r = real_roster();
    let mut a = Formation4v4::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 123);
    a.set_log(true);
    let oa = a.run();
    let eva = a.events().to_vec();
    let ca = a.chain_links();

    let mut b = Formation4v4::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 123);
    b.set_log(true);
    let ob = b.run();
    let evb = b.events().to_vec();
    let cb = b.chain_links();

    assert_eq!(oa, ob);
    assert_eq!(eva, evb, "log textual diverge com a mesma seed");
    assert_eq!(ca, cb);
    assert!(b.rounds() >= 1, "formação deve rodar ao menos 1 rodada");
    assert_ne!(oa, turnrush_core::types::Outcome::Draw, "4v4 deve ter vencedor");
}

/// A partida roguelike 1v1 (draft + 9 rodadas) também é determinística.
#[test]
fn partida_roguelike_1v1_e_deterministica() {
    let ma = play_match([0, 1, 2], [3, 4, 5], 77, &NoDwc);
    let mb = play_match([0, 1, 2], [3, 4, 5], 77, &NoDwc);
    assert_eq!(ma.winner, mb.winner);
    assert_eq!(ma.score, mb.score);
    assert_eq!(ma.rounds_played, mb.rounds_played);
    // A partida para CEDO quando alguém alcança 5 vitórias (WINS_TO_MATCH).
    assert!(ma.rounds_played >= 1 && ma.rounds_played <= turnrush_core::run::ROUNDS_MAX);
    if let Some(w) = ma.winner {
        assert_eq!(ma.score[w], turnrush_core::run::WINS_TO_MATCH, "vencedor precisa de 5 rodadas");
    }
}

/// A escala de rodada é determinística e monotônica (escalada da partida).
#[test]
fn escala_de_rodada_e_monotonica() {
    let s1 = scale_for_round(1);
    let s5 = scale_for_round(5);
    let s10 = scale_for_round(10);
    assert!(s1 <= s5 && s5 <= s10, "escalada deve ser não decrescente");
    assert_eq!(scale_for_round(1), s1, "escalada não é determinística");
}

/// O fluxo do RNG é reproduzível e `range` fica nos limites.
#[test]
fn fluxo_do_rng_e_reproduzivel() {
    let mut a = Rng::new(999);
    let mut b = Rng::new(999);
    for _ in 0..100 {
        assert_eq!(a.next_u64(), b.next_u64());
    }
    let mut c = Rng::new(1);
    for _ in 0..200 {
        let v = c.range(10);
        assert!(v < 10, "range(10) devolveu {v}");
    }
}

/// Conector: a Peça 3 (economia) pode confiar no relógio.
#[test]
fn peca_02_encaixa_na_peca_03() {
    ancora_determinismo();
}
