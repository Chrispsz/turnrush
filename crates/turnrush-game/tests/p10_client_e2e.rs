//! Peça 10 do quebra-cabeça: **o cliente de verdade, ponta a ponta**.
//!
//! Encaixa nas peças 1–9: com dados, engine, economia, mira, passivas,
//! kits, corrente e run auditados, o binário `turnrush-game --check` prova
//! o CIRCUITO COMPLETO do produto: roster embutido → batalha headless do
//! motor → parser de eventos do cliente → controlador interativo →
//! convergência. É o mesmo gate que o CI roda em cada push.

use std::process::Command;
use std::time::{Duration, Instant};

fn binario() -> &'static str {
    env!("CARGO_BIN_EXE_turnrush-game")
}

fn rodar_check() -> (bool, String, String) {
    let mut cmd = Command::new(binario());
    cmd.arg("--check");
    let inicio = Instant::now();
    let ok = cmd.output().expect("spawn do binário do jogo");
    let duracao = inicio.elapsed();
    assert!(
        duracao < Duration::from_secs(120),
        "--check demorou demais: {duracao:?}"
    );
    (
        ok.status.success(),
        String::from_utf8_lossy(&ok.stdout).into_owned(),
        String::from_utf8_lossy(&ok.stderr).into_owned(),
    )
}

/// O binário do jogo fecha o circuito completo headless.
#[test]
fn cliente_headless_fecha_o_circuito() {
    let (ok, stdout, stderr) = rodar_check();
    assert!(ok, "--check falhou\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(stdout.contains("--check OK"), "carimbo final ausente:\n{stdout}");
}

/// O circuito inclui os dois motores: formação compacta (parser do cliente)
/// e batalha interativa (controlador + eventos finos).
#[test]
fn check_cobre_formacao_e_interativo() {
    let (ok, stdout, stderr) = rodar_check();
    assert!(ok, "--check falhou\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        stdout.contains("batalha headless"),
        "motor de formação não rodou:\n{stdout}"
    );
    assert!(
        stdout.contains("batalha interativa"),
        "motor interativo não rodou:\n{stdout}"
    );
    assert!(
        stdout.contains("roster: 16 personagens"),
        "roster embutido não é o elenco de 16:\n{stdout}"
    );
    // As contagens do parser devem reconhecer rodadas e ataques reais.
    let linha_headless = stdout
        .lines()
        .find(|l| l.contains("batalha headless"))
        .expect("linha da batalha headless");
    assert!(!linha_headless.contains("0 ataques"), "parser não viu ataques: {linha_headless}");
    let linha_interativa = stdout
        .lines()
        .find(|l| l.contains("batalha interativa"))
        .expect("linha da batalha interativa");
    assert!(
        !linha_interativa.contains("0 eventos"),
        "controlador sem eventos finos: {linha_interativa}"
    );
}

/// Reprodutibilidade do binário: duas execuções produzem a mesma saída.
#[test]
fn check_e_reprodutivel() {
    let (ok1, out1, _) = rodar_check();
    let (ok2, out2, _) = rodar_check();
    assert!(ok1 && ok2);
    assert_eq!(out1, out2, "--check não é determinístico entre execuções");
}
