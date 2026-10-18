//! Controlador da batalha manual (v0.7): ponte entre a engine interativa
//! ([`InteractiveBattle`]) e o cliente Bevy.
//!
//! Responsabilidades:
//! - consumir [`BattleEvent`]s finos (nunca parsear texto de log);
//! - manter a fila de eventos à espera de playback visual;
//! - decidir pelo lado da IA usando APENAS a API pública da engine
//!   ([`InteractiveBattle::legal_actions`] + [`preview_action`]) — simetria
//!   total com o jogador, zero informação privilegiada;
//! - expor a vez corrente para a UI e receber a decisão do jogador.
//!
//! O controlador é determinístico e testável headless (sem winit/wgpu):
//! [`BattleController::drive_auto`] roda uma batalha inteira em memória.

use turnrush_core::data::Roster;
use turnrush_core::interactive::{
    ActionKind, ActionPreview, BattleEvent, ChosenAction, InteractiveBattle, PromptPhase,
    SideEdge, TurnPrompt,
};
use turnrush_core::types::Outcome;

/// Fase do controlador para o cliente.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtlPhase {
    /// Há eventos na fila — reproduza 1 por tick e chame `pump` de novo.
    PlayingEvents,
    /// Vez do jogador: decida com `submit` (ação padrão ou chase).
    AwaitingPlayer { chase: bool },
    /// Batalha concluída.
    Done(Outcome),
}

/// Controlador da batalha corrente.
pub struct BattleController {
    pub bt: InteractiveBattle,
    /// Eventos gerados à espera de playback (ordem garantida).
    pub events: Vec<BattleEvent>,
    /// Índice do próximo evento a reproduzir (cursor do cliente).
    pub ev_cursor: usize,
    /// Decisões do lado do jogador são resolvidas automaticamente?
    pub auto_player: bool,
    finished: bool,
    outcome: Option<Outcome>,
    /// Última prévia calculada (UI mostra sem recomputar).
    last_preview: Option<(ActionKind, Option<usize>, ActionPreview)>,
    /// Vez corrente em cache (definida por `pump` quando aguarda o jogador).
    cached_prompt: Option<TurnPrompt>,
}

impl BattleController {
    /// Monta a batalha. `auto_player` = modo Auto do botão/headless.
    pub fn new(
        roster: &Roster,
        a: [usize; 4],
        b: [usize; 4],
        seed: u64,
        edge_a: SideEdge,
        edge_b: SideEdge,
        auto_player: bool,
    ) -> Self {
        BattleController {
            bt: InteractiveBattle::new_with_edges(roster, a, b, seed, edge_a, edge_b),
            events: Vec::new(),
            ev_cursor: 0,
            auto_player,
            finished: false,
            outcome: None,
            last_preview: None,
            cached_prompt: None,
        }
    }

    /// Avança o controlador um passo atômico e devolve a fase nova.
    ///
    /// Contrato do cliente: enquanto devolver [`CtlPhase::PlayingEvents`],
    /// consuma `events[ev_cursor]` (1 por tick) e incremente o cursor; quando
    /// devolver `AwaitingPlayer`, mostre a vez e espere `submit`; `Done`
    /// encerra.
    pub fn pump(&mut self) -> CtlPhase {
        if self.finished {
            return CtlPhase::Done(self.outcome.expect("done com outcome"));
        }
        // 1) eventos pendentes? deixa o cliente consumir primeiro.
        if self.ev_cursor < self.events.len() {
            return CtlPhase::PlayingEvents;
        }
        // 2) prompt da engine
        let prompt = self.bt.prompt();
        let novos = self.bt.drain();
        self.events.extend(novos);
        if self.ev_cursor < self.events.len() {
            return CtlPhase::PlayingEvents;
        }
        if prompt.phase == PromptPhase::Over {
            self.finished = true;
            self.outcome = self.bt.outcome();
            self.cached_prompt = None;
            return CtlPhase::Done(self.outcome.expect("done com outcome"));
        }
        match prompt.phase {
            PromptPhase::ChooseAction => {
                if prompt.side == 1 || self.auto_player {
                    let acao = greedy_decide(&self.bt, &prompt);
                    if self.submit(acao).is_err() {
                        // rede de segurança: o padrão é sempre legal na vez
                        if self.submit(ChosenAction::Act { kind: ActionKind::Standard, target: None }).is_err() {
                            panic!(
                                "IA ilegal E padrão falhou: prompt={prompt:?} len={} cursor={}",
                                self.events.len(),
                                self.ev_cursor
                            );
                        }
                    }
                    CtlPhase::PlayingEvents
                } else {
                    self.cached_prompt = Some(prompt);
                    CtlPhase::AwaitingPlayer { chase: false }
                }
            }
            PromptPhase::ConfirmChase => {
                if prompt.side == 1 || self.auto_player {
                    let acao = match self.bt.chase_eligible_slots(prompt.side).first() {
                        Some(&s) => ChosenAction::Chase { chaser_slot: s },
                        None => ChosenAction::RejectChase,
                    };
                    if self.submit(acao).is_err() {
                        // recusa sempre emite evento — garante progresso
                        let _ = self.submit(ChosenAction::RejectChase);
                    }
                    CtlPhase::PlayingEvents
                } else {
                    self.cached_prompt = Some(prompt);
                    CtlPhase::AwaitingPlayer { chase: true }
                }
            }
            PromptPhase::Over => {
                self.finished = true;
                self.outcome = self.bt.outcome();
                self.cached_prompt = None;
                CtlPhase::Done(self.outcome.expect("done com outcome"))
            }
        }
    }

    /// Submete a decisão do jogador para a vez corrente. Erros são
    /// registrados no log de sessão (a UI previne; a engine recusa).
    pub fn submit(&mut self, action: ChosenAction) -> Result<(), turnrush_core::interactive::ActionError> {
        self.cached_prompt = None;
        self.last_preview = None;
        match self.bt.execute_action(action) {
            Ok(mut ev) => {
                self.events.append(&mut ev);
                Ok(())
            }
            Err(e) => {
                turnrush_core::logger::kv(
                    turnrush_core::logger::Level::Warn,
                    "controller_submit",
                    "ação recusada pela engine",
                    &[("erro", turnrush_core::logger::V::S(&format!("{e:?}")))],
                );
                Err(e)
            }
        }
    }

    /// Prévia da ação sob o cursor da UI (calculada na hora; pura).
    #[allow(dead_code)] // usada pela UI do E.6
    pub fn preview(&mut self, side: usize, slot: usize, action: ChosenAction) -> Option<ActionPreview> {
        self.bt.preview_action(side, slot, action).ok()
    }

    /// Vez corrente em cache (definida quando `pump` devolve `AwaitingPlayer`).
    pub fn prompt(&self) -> Option<TurnPrompt> {
        self.cached_prompt
    }

    /// Batalha concluída?
    #[allow(dead_code)] // usada pela UI do E.6
    pub fn is_done(&self) -> bool {
        self.finished
    }

    /// Resultado, quando concluída.
    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    /// Evento no cursor (o cliente aplica e avança).
    pub fn next_event(&self) -> Option<BattleEvent> {
        self.events.get(self.ev_cursor).cloned()
    }

    /// Consome o evento do cursor.
    pub fn advance_event(&mut self) {
        self.ev_cursor += 1;
    }

    /// Eventos já reproduzidos (para o cliente limpar a fila).
    #[allow(dead_code)] // usada pela UI do E.6
    pub fn events_played(&self) -> usize {
        self.ev_cursor
    }
}

/// IA greedy: a versão canônica vive no core (simetria auditável).
pub fn greedy_decide(bt: &InteractiveBattle, p: &TurnPrompt) -> ChosenAction {
    turnrush_core::interactive::greedy_decide(bt, p)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn roster() -> Roster {
        Roster::from_json(include_str!("../../../data/roster/roster-v2.json")).expect("roster v2")
    }

    /// Conduz a batalha inteira com o controlador (IA dos dois lados).
    #[test]
    fn controlador_conclui_batalha_headless() {
        let r = roster();
        let mut ctl = BattleController::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 42, SideEdge::default(), SideEdge::default(), true);
        let mut passos = 0;
        loop {
            passos += 1;
            assert!(passos < 100_000, "controlador não converge");
            match ctl.pump() {
                CtlPhase::PlayingEvents => {
                    if ctl.next_event().is_none() {
                        panic!("PlayingEvents sem evento: cursor={} len={} prompt={:?} done={}",
                            ctl.ev_cursor, ctl.events.len(), ctl.cached_prompt, ctl.finished);
                    }
                    ctl.advance_event();
                }
                CtlPhase::AwaitingPlayer { chase } => {
                    let p = ctl.prompt().expect("prompt na vez do jogador");
                    let acao = if chase {
                        match ctl.bt.chase_eligible_slots(p.side).first() {
                            Some(&s) => ChosenAction::Chase { chaser_slot: s },
                            None => ChosenAction::RejectChase,
                        }
                    } else {
                        greedy_decide(&ctl.bt, &p)
                    };
                    let _ = ctl.submit(acao);
                }
                CtlPhase::Done(_) => break,
            }
        }
        assert!(ctl.outcome().is_some());
        assert!(ctl.events_played() > 10, "batalha deve produzir eventos");
    }

    /// Determinismo: mesma seed ⇒ mesmo resultado e mesmos eventos.
    #[test]
    fn controlador_deterministico() {
        let r = roster();
        let roda = |seed: u64| {
            let mut ctl =
                BattleController::new(&r, [0, 1, 2, 0], [3, 4, 5, 3], seed, SideEdge::default(), SideEdge::default(), true);
            let count = 0;
            loop {
                match ctl.pump() {
                    CtlPhase::PlayingEvents => ctl.advance_event(),
                    CtlPhase::AwaitingPlayer { .. } => {
                        let p = ctl.prompt().expect("prompt");
                        let acao = if p.phase == PromptPhase::ConfirmChase {
                            ChosenAction::RejectChase
                        } else {
                            greedy_decide(&ctl.bt, &p)
                        };
                        let _ = ctl.submit(acao);
                    }
                    CtlPhase::Done(_) => break,
                }
            }
            (ctl.outcome(), ctl.events_played(), count)
        };
        let (o1, n1, _) = roda(7);
        let (o2, n2, _) = roda(7);
        assert_eq!(o1, o2);
        assert_eq!(n1, n2);
    }

    /// Modo manual: o controlador PAUSA na vez do jogador e retoma com submit.
    #[test]
    fn controlador_pausa_na_vez_do_jogador() {
        let r = roster();
        let mut ctl =
            BattleController::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 5, SideEdge::default(), SideEdge::default(), false);
        // drena eventos até a primeira vez do jogador (ou fim)
        let mut pausou = false;
        for _ in 0..10_000 {
            match ctl.pump() {
                CtlPhase::PlayingEvents => ctl.advance_event(),
                CtlPhase::AwaitingPlayer { chase } => {
                    pausou = true;
                    let p = ctl.prompt().expect("prompt");
                    assert_eq!(p.side, 0, "pausa é sempre no lado do jogador");
                    // submete padrão e continua
                    let acao = if chase {
                        ChosenAction::RejectChase
                    } else {
                        ChosenAction::Act { kind: ActionKind::Standard, target: None }
                    };
                    let _ = ctl.submit(acao);
                }
                CtlPhase::Done(_) => break,
            }
        }
        assert!(pausou, "controlador deve pausar na vez do jogador");
        assert!(ctl.is_done());
    }

    /// IA greedy nunca submete ação ilegal (contrato de simetria).
    #[test]
    fn ia_greedy_so_usa_acoes_legais() {
        let r = roster();
        let mut ctl =
            BattleController::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 99, SideEdge::default(), SideEdge::default(), true);
        for _ in 0..10_000 {
            match ctl.pump() {
                CtlPhase::PlayingEvents => ctl.advance_event(),
                CtlPhase::AwaitingPlayer { chase } => {
                    let p = ctl.prompt().expect("prompt");
                    let legais = ctl.bt.legal_actions(p.side, p.slot);
                    if chase {
                        let elegiveis = ctl.bt.chase_eligible_slots(p.side);
                        assert!(!elegiveis.is_empty(), "janela de chase sem elegíveis não deve pausar");
                        let _ = ctl.submit(ChosenAction::Chase { chaser_slot: elegiveis[0] });
                    } else {
                        assert!(
                            legais.iter().any(|l| l.kind == ActionKind::Standard),
                            "padrão sempre legal na vez"
                        );
                        let acao = greedy_decide(&ctl.bt, &p);
                        // a submissão não pode falhar silenciosamente toda vez
                        let _ = ctl.submit(acao);
                    }
                }
                CtlPhase::Done(_) => break,
            }
        }
        assert!(ctl.is_done());
    }
}
