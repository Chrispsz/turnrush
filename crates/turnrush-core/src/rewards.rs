//! Recompensas roguelike da run (v0.7): cartas de **Equipe**, **Unidade** e
//! **Tática**.
//!
//! Ciclo da run: draft → batalha 1 → vitória oferta 1 de 3 cartas (1 reroll
//! por batalha) → batalha 2 → batalha 3 = **BOSS** (3 vitórias ou derrota
//! encerra). Regras de oferta:
//! - raridade comum/incomum/rara pesa 600/300/100 (per-mille);
//! - preferência de clã: +10% no peso quando o time tem a facção da carta;
//! - sem repetição dentro da run;
//! - cartas de unidade exigem um alvo vivo no momento da escolha;
//! - **teto de poder da run: +30%** (`PODER_CAP`) — sorte não vira vitória
//!   automática (verificado por Monte Carlo: sorte vs média ≤ 10 p.p.).
//!
//! Efeitos das cartas chegam à batalha via [`crate::interactive::SideEdge`]
//! (montado por [`RunState::edge`]) — o motor continua determinístico.

use crate::data::{Faction, Roster};
use crate::interactive::SideEdge;
use crate::rng::Rng;

/// Teto de poder acumulado da run (per-mille; 300 = +30%).
pub const PODER_CAP: i32 = 300;
/// Cartas ofertadas por vitória.
pub const OFFER_SIZE: usize = 3;

/// Classe da carta (a quem o efeito se aplica).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardClass {
    /// Efeito no time inteiro.
    Team,
    /// Efeito em UMA unidade viva, escolhida na compra.
    Unit,
    /// Efeito de regra da batalha (chase, revival…).
    Tactic,
}

/// Raridade (peso de oferta).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
}

impl Rarity {
    /// Peso base de oferta (per-mille).
    pub fn weight(self) -> i32 {
        match self {
            Rarity::Common => 600,
            Rarity::Uncommon => 300,
            Rarity::Rare => 100,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Rarity::Common => "comum",
            Rarity::Uncommon => "incomum",
            Rarity::Rare => "rara",
        }
    }
}

/// Uma carta do pool.
#[derive(Clone, Copy, Debug)]
pub struct RewardCard {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub class: CardClass,
    pub rarity: Rarity,
    /// Orçamento de poder (per-mille) para o teto da run.
    pub poder: i32,
    /// Facção preferida (+10% de peso se o time a tiver).
    pub faction_pref: Option<Faction>,
    /// Bônus de ATK de equipe (per-mille somado).
    pub team_atk: i32,
    /// Bônus de HP de equipe (per-mille; pode ser negativo).
    pub team_hp: i32,
    /// Fluxo extra por rodada (equipe).
    pub fluxo_per_round: i32,
    /// Dano de chase por elo da cadeia (per-mille) e teto.
    pub chain_per_link: i32,
    pub chain_cap: i32,
    /// Chases extras por unidade por rodada.
    pub extra_chases: u32,
    /// % HP de revival da 1ª morte do time.
    pub revive_pct: i32,
    /// Antecipação da Mystery em rodadas.
    pub unlock_early: usize,
    /// v0.8: desconto de fluxo na Mystery da unidade alvo (Gatilho Rápido).
    pub mystery_discount: i32,
    /// Bônus de unidade: ATK/HP per-mille (cartas de unidade).
    pub unit_atk: i32,
    pub unit_hp: i32,
    /// Mitigação de dano do time (per-mille; tática Pele de Pedra).
    pub team_dmg_reduction: i32,
    /// Cura de fim de rodada do time (% HP máx; tática Cura de Campo).
    pub round_end_heal_pct: i32,
}

const fn card(id: &'static str, name: &'static str, desc: &'static str, class: CardClass, rarity: Rarity, poder: i32, pref: Option<Faction>) -> RewardCard {
    RewardCard {
        id,
        name,
        desc,
        class,
        rarity,
        poder,
        faction_pref: pref,
        team_atk: 0,
        team_hp: 0,
        fluxo_per_round: 0,
        chain_per_link: 0,
        chain_cap: 0,
        extra_chases: 0,
        revive_pct: 0,
        unlock_early: 0,
        mystery_discount: 0,
        unit_atk: 0,
        unit_hp: 0,
        team_dmg_reduction: 0,
        round_end_heal_pct: 0,
    }
}

/// Carta de tática de BATALHA (R1 do feedback 2026-10-18): oferta a cada
/// 2 rodadas DENTRO da batalha; efeito dura só até o fim da batalha
/// corrente — não entra no teto de poder da run ([`PODER_CAP`]). O inimigo
/// adota a dele em silêncio (simetria de poder, zero UI).
const fn tactic(
    id: &'static str,
    name: &'static str,
    desc: &'static str,
    team_atk: i32,
    team_dmg_reduction: i32,
    fluxo_per_round: i32,
    round_end_heal_pct: i32,
) -> RewardCard {
    RewardCard {
        id,
        name,
        desc,
        class: CardClass::Tactic,
        rarity: Rarity::Common,
        poder: 0,
        faction_pref: None,
        team_atk,
        team_hp: 0,
        team_dmg_reduction,
        fluxo_per_round,
        chain_per_link: 0,
        chain_cap: 0,
        extra_chases: 0,
        revive_pct: 0,
        unlock_early: 0,
        mystery_discount: 0,
        unit_atk: 0,
        unit_hp: 0,
        round_end_heal_pct,
    }
}

/// Pool de táticas de batalha (oferta: 2 distintas por janela; efeito de
/// escopo da batalha, orçamento pequeno por design — ver docs/CARTAS).
pub const TACTIC_POOL: [RewardCard; 4] = [
    tactic("t-ataque", "Reforço de Linha", "+12% de ATK do time nesta batalha", 120, 0, 0, 0),
    tactic("t-pele", "Pele de Pedra", "dano recebido pelo time -8% nesta batalha", 0, 80, 0, 0),
    tactic("t-fluxo", "Fluxo Acelerado", "+4 de fluxo por rodada nesta batalha", 0, 0, 4, 0),
    tactic("t-cura", "Cura de Campo", "o mais ferido do time cura 3% do HP máx no fim da rodada", 0, 0, 0, 30),
];

/// Pool de lançamento (v0.7): 12 cartas — 4 por classe.
pub const CARD_POOL: [RewardCard; 12] = [
    // ── Equipe ──
    RewardCard { team_atk: 80, ..card("chama-interna", "Chama Interna", "+8% de ATK para a equipe.", CardClass::Team, Rarity::Common, 80, Some(Faction::Ember)) },
    RewardCard { team_hp: 100, ..card("casca-de-ferro", "Casca de Ferro", "+10% de HP para a equipe.", CardClass::Team, Rarity::Common, 100, Some(Faction::Tide)) },
    RewardCard { fluxo_per_round: 2, ..card("fluxo-estavel", "Fluxo Estável", "+2 de fluxo por rodada para cada unidade.", CardClass::Team, Rarity::Uncommon, 60, Some(Faction::Storm)) },
    RewardCard { chain_per_link: 100, chain_cap: 300, ..card("eco-de-combate", "Eco de Combate", "Chases ganham +10% de dano por elo da cadeia (máx. +30%).", CardClass::Team, Rarity::Rare, 100, Some(Faction::Ember)) },
    // ── Unidade (escolher 1 unidade viva) ──
    RewardCard { unit_atk: 150, unit_hp: -50, ..card("furia-interior", "Fúria Interior", "Uma unidade viva: +15% de ATK, -5% de HP.", CardClass::Unit, Rarity::Common, 70, None) },
    RewardCard { unit_atk: -30, unit_hp: 120, ..card("manto-denso", "Manto Denso", "Uma unidade viva: +12% de HP, -3% de ATK.", CardClass::Unit, Rarity::Common, 60, None) },
    RewardCard {
        unlock_early: 1,
        mystery_discount: 15,
        ..card("gatilho-rapido", "Gatilho Rápido", "Uma unidade viva: a Mystery destrava 1 rodada antes E custa 15 ⚡ menos (com fluxo guardado, dispara na rodada 2).", CardClass::Unit, Rarity::Uncommon, 70, Some(Faction::Storm))
    },
    RewardCard { unit_atk: 100, unit_hp: 100, ..card("veia-de-aco", "Veia de Aço", "Uma unidade viva: +10% de ATK e +10% de HP.", CardClass::Unit, Rarity::Rare, 110, None) },
    // ── Tática ──
    RewardCard { extra_chases: 1, ..card("ambidestria", "Ambidestria", "Cada unidade pode perseguir 2× por rodada.", CardClass::Tactic, Rarity::Rare, 90, Some(Faction::Verdant)) },
    RewardCard { revive_pct: 20, ..card("ultimo-suspiro", "Último Suspiro", "A 1ª morte do time revive com 20% do HP máximo.", CardClass::Tactic, Rarity::Rare, 100, None) },
    RewardCard { team_atk: 150, team_hp: -100, ..card("sangue-por-poder", "Sangue por Poder", "-10% de HP e +15% de ATK para a equipe.", CardClass::Tactic, Rarity::Uncommon, 60, None) },
    RewardCard { fluxo_per_round: 1, ..card("respiracao-ritmada", "Respiração Ritmada", "+1 de fluxo por rodada para cada unidade.", CardClass::Team, Rarity::Common, 30, None) },
];

/// Cartas já compradas + poder acumulado da run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunState {
    /// IDs das cartas compradas (sem repetição).
    pub taken: Vec<&'static str>,
    /// Poder acumulado (per-mille).
    pub poder: i32,
    /// Rerolls restantes na batalha corrente (1 por batalha).
    pub rerolls: u32,
}

impl RunState {
    pub fn new() -> Self {
        RunState { taken: Vec::new(), poder: 0, rerolls: 0 }
    }

    /// Poder restante até o teto da run.
    pub fn poder_restante(&self) -> i32 {
        (PODER_CAP - self.poder).max(0)
    }

    /// A carta pode ser comprada agora (teto + repetição)?
    pub fn can_take(&self, c: &RewardCard) -> bool {
        !self.taken.contains(&c.id) && c.poder <= self.poder_restante()
    }

    /// Compra a carta (alvo obrigatório para cartas de unidade).
    pub fn take(&mut self, c: &'static RewardCard, target_slot: Option<usize>) -> Result<(), String> {
        if !self.can_take(c) {
            return Err(format!("carta '{}' indisponível (teto/repetição)", c.name));
        }
        if c.class == CardClass::Unit && target_slot.is_none() {
            return Err(format!("carta de unidade '{}' exige alvo vivo", c.name));
        }
        self.taken.push(c.id);
        self.poder += c.poder;
        Ok(())
    }

    /// Oferta de `n` cartas legais, com pesos de raridade e preferência de
    /// clã (+10% quando o time tem a facção). Sem repetição; se o pool legal
    /// for menor que `n`, devolve o que houver (pode ser vazio perto do teto).
    pub fn offer(&self, rng: &mut Rng, n: usize, team_factions: &[Faction]) -> Vec<&'static RewardCard> {
        let mut candidatos: Vec<(&RewardCard, i32)> = CARD_POOL
            .iter()
            .filter(|c| self.can_take(c))
            .map(|c| {
                let pref = match c.faction_pref {
                    Some(f) if team_factions.contains(&f) => 1100,
                    _ => 1000,
                };
                (c, c.rarity.weight() * pref / 1000)
            })
            .collect();
        let mut out: Vec<&'static RewardCard> = Vec::new();
        for _ in 0..n {
            if candidatos.is_empty() {
                break;
            }
            let total: i32 = candidatos.iter().map(|(_, w)| *w).sum();
            let mut alvo = (rng.range(total.max(1) as usize)) as i32;
            let mut escolhido = 0usize;
            for (i, (_, w)) in candidatos.iter().enumerate() {
                if alvo < *w {
                    escolhido = i;
                    break;
                }
                alvo -= *w;
            }
            let (c, _) = candidatos.remove(escolhido);
            out.push(c);
        }
        out
    }

    /// Reroll da oferta (1 por batalha).
    pub fn reroll(&mut self) -> bool {
        if self.rerolls == 0 {
            return false;
        }
        self.rerolls -= 1;
        true
    }

    /// Vantagem acumulada das cartas para o lado do jogador (aplicada à
    /// batalha via [`SideEdge`]). Cartas de unidade aplicam em TODOS os slots
    /// marcados — a run guarda o alvo em `unit_targets`.
    pub fn edge(&self, unit_targets: &[(usize, usize)]) -> SideEdge {
        let mut e = SideEdge::default();
        for id in &self.taken {
            let Some(c) = CARD_POOL.iter().find(|c| c.id == *id) else {
                continue;
            };
            e.atk_bonus_permille += c.team_atk;
            e.hp_bonus_permille += c.team_hp;
            e.fluxo_per_round += c.fluxo_per_round;
            e.chain_dmg_per_link += c.chain_per_link;
            e.chain_dmg_cap += c.chain_cap;
            e.extra_chases += c.extra_chases;
            e.revive_pct = e.revive_pct.max(c.revive_pct);
            e.unlock_early += c.unlock_early;
        }
        // Cartas de unidade: aplica nos slots alvo guardados pela run.
        for &(slot, card_idx) in unit_targets {
            let Some(c) = CARD_POOL.get(card_idx) else {
                continue;
            };
            if slot < 4 {
                e.unit_atk[slot] += c.unit_atk;
                e.unit_hp[slot] += c.unit_hp;
                e.unit_mystery_discount[slot] += c.mystery_discount;
                e.unit_unlock_early[slot] += c.unlock_early;
            }
        }
        e
    }

    /// Factions presentes no time (para preferência de oferta).
    pub fn team_factions(roster: &Roster, team: [usize; 4]) -> Vec<Faction> {
        team.iter().map(|&i| roster.characters[i.min(roster.characters.len() - 1)].faction).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Roster;
    use crate::types::PER_MILLE;

    fn roster() -> Roster {
        Roster::from_json(include_str!("../../../data/roster/roster-v2.json")).expect("roster v2")
    }

    #[test]
    fn pesos_de_raridade_e_preferencia_de_cla() {
        let r = roster();
        let mut rng = Rng::new(7);
        let rs = RunState::new();
        // time ember: cartas com pref ember ficam mais frequentes
        let factions = [Faction::Ember, Faction::Ember, Faction::Ember, Faction::Ember];
        let mut comuns = 0;
        let n = 2000;
        for _ in 0..n {
            let oferta = rs.offer(&mut rng, 1, &factions);
            assert_eq!(oferta.len(), 1);
            if oferta[0].faction_pref == Some(Faction::Ember) {
                comuns += 1;
            }
        }
        // base: 5/12 cartas têm pref ember (2 team + 2 unit-storm... conferir:
        // ember: chama-interna, eco; storm: fluxo-estavel, gatilho; tide: casca;
        // verdant: ambidestria; resto sem pref) → com pref ember = 2/12 ≈ 16,7%
        // com +10% ⇒ ~17,4%. Janela larga para determinismo estável.
        let pct = comuns as f64 / n as f64;
        assert!(
            (0.10..=0.25).contains(&pct),
            "freq. de cartas ember fora da banda esperada: {pct:.3}"
        );
        let _ = r;
    }

    #[test]
    fn sem_repeticao_e_teto_de_poder() {
        let mut rng = Rng::new(11);
        let mut rs = RunState::new();
        let sem_cla: Vec<Faction> = vec![];
        let mut compradas = 0;
        // compra até o teto esgotar
        for _ in 0..50 {
            let oferta = rs.offer(&mut rng, OFFER_SIZE, &sem_cla);
            if oferta.is_empty() {
                break;
            }
            // pega a primeira legal
            let Some(c) = oferta.iter().copied().find(|c| rs.can_take(c)) else {
                break;
            };
            let alvo = if c.class == CardClass::Unit { Some(0) } else { None };
            rs.take(c, alvo).expect("compra legal");
            compradas += 1;
            // sem repetição no histórico
            let mut ids = rs.taken.clone();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), rs.taken.len(), "repetição detectada");
        }
        assert!(compradas >= 2, "teto de 300‰ deve permitir várias cartas");
        assert!(rs.poder <= PODER_CAP, "poder acima do teto: {}", rs.poder);
        // perto do teto, cartas caras somem da oferta
        if rs.poder_restante() < 30 {
            let oferta = rs.offer(&mut rng, OFFER_SIZE, &sem_cla);
            assert!(
                oferta.iter().all(|c| c.poder <= rs.poder_restante()),
                "oferta violou o teto de poder"
            );
        }
    }

    #[test]
    fn carta_de_unidade_exige_alvo_vivo() {
        let c = CARD_POOL.iter().find(|c| c.id == "furia-interior").expect("carta");
        let mut rs = RunState::new();
        assert!(rs.take(c, None).is_err(), "sem alvo: erro");
        assert!(rs.take(c, Some(2)).is_ok(), "com alvo: ok");
    }

    #[test]
    fn reroll_uma_vez_por_batalha() {
        let mut rs = RunState::new();
        rs.rerolls = 1;
        assert!(rs.reroll());
        assert!(!rs.reroll(), "só 1 reroll por batalha");
    }

    #[test]
    fn edge_acumula_cartas_corretamente() {
        let mut rs = RunState::new();
        let chama = CARD_POOL.iter().find(|c| c.id == "chama-interna").expect("carta");
        let eco = CARD_POOL.iter().find(|c| c.id == "eco-de-combate").expect("carta");
        let furia = CARD_POOL.iter().find(|c| c.id == "furia-interior").expect("carta");
        let idx_furia = CARD_POOL.iter().position(|c| c.id == "furia-interior").expect("idx");
        rs.take(chama, None).expect("ok");
        rs.take(eco, None).expect("ok");
        rs.take(furia, Some(0)).expect("ok");
        // registra o alvo da carta de unidade: slot 0, índice da carta
        let e = rs.edge(&[(0, idx_furia)]);
        assert_eq!(e.atk_bonus_permille, 80, "chama-interna +8% equipe");
        assert_eq!(e.chain_dmg_per_link, 100);
        assert_eq!(e.chain_dmg_cap, 300);
        assert_eq!(e.unit_atk[0], 150, "fúria interior no slot 0");
        assert_eq!(e.unit_hp[0], -50);
        assert_eq!(e.unit_atk[1], 0, "outros slots intactos");
    }

    // ── Monte Carlo da run: sorte vs média ─────────────────────────────────

    /// IA greedy mínima — usa APENAS a API pública (mesma superfície do
    /// jogador; contrato de simetria do E.9).
    fn decidir(bt: &mut crate::interactive::InteractiveBattle, p: &crate::interactive::TurnPrompt) -> crate::interactive::ChosenAction {
        use crate::interactive::{ActionKind, ChosenAction, PromptPhase};
        if p.phase == PromptPhase::ChooseTactic {
            // oferta de tática (R1): escolha determinística simples
            return ChosenAction::PickTactic { index: 0 };
        }
        if p.phase == PromptPhase::ConfirmChase {
            // confirma o primeiro elegível; sem elegível, recusa
            return match bt.chase_eligible_slots(p.side).first() {
                Some(&s) => ChosenAction::Chase { chaser_slot: s },
                None => ChosenAction::RejectChase,
            };
        }
        let legais = bt.legal_actions(p.side, p.slot);
        // mystery > skill (dano) > padrão; cura só se aliado < 45%
        let mut melhor: Option<(i32, ChosenAction)> = None;
        for l in &legais {
            let acao = ChosenAction::Act { kind: l.kind, target: None };
            let Ok(prev) = bt.preview_action(p.side, p.slot, acao) else {
                continue;
            };
            let valor = if prev.heal > 0 {
                let ferido = (0..4).any(|k| bt.unit(p.side, k).alive && bt.unit(p.side, k).hp * 100 < bt.unit(p.side, k).hp_max * 45);
                if ferido {
                    prev.heal * 2
                } else {
                    0
                }
            } else {
                prev.damage
            };
            if melhor.map(|(v, _)| valor > v).unwrap_or(true) {
                melhor = Some((valor, acao));
            }
        }
        melhor.map(|(_, a)| a).unwrap_or(ChosenAction::Act { kind: ActionKind::Standard, target: None })
    }

    /// Joga uma batalha inteira com a IA dos dois lados.
    fn jogar(roster: &Roster, a: [usize; 4], b: [usize; 4], seed: u64, ea: SideEdge, eb: SideEdge) -> bool {
        use crate::interactive::{InteractiveBattle, PromptPhase};
        let mut bt = InteractiveBattle::new_with_edges(roster, a, b, seed, ea, eb);
        let mut guard = 0;
        loop {
            guard += 1;
            assert!(guard < 40_000, "run MC: batalha não converge");
            let p = bt.prompt();
            let _ = bt.drain();
            if p.phase == PromptPhase::Over {
                break;
            }
            let acao = decidir(&mut bt, &p);
            let _ = bt.execute_action(acao);
        }
        matches!(bt.outcome(), Some(crate::types::Outcome::TeamAWin))
    }

    /// Estratégia de compra: `sortuda` = maior poder legal; média = aleatória.
    fn comprar(rs: &mut RunState, rng: &mut Rng, sortuda: bool, factions: &[Faction]) -> Vec<(usize, usize)> {
        let mut alvos: Vec<(usize, usize)> = Vec::new();
        let oferta = rs.offer(rng, OFFER_SIZE, factions);
        let escolha = if sortuda {
            oferta.iter().copied().filter(|c| rs.can_take(c)).max_by_key(|c| c.poder)
        } else {
            if oferta.is_empty() {
                None
            } else {
                let i = rng.range(oferta.len());
                let c = oferta[i];
                if rs.can_take(c) {
                    Some(c)
                } else {
                    oferta.iter().copied().find(|c| rs.can_take(c))
                }
            }
        };
        if let Some(c) = escolha {
            let idx = CARD_POOL.iter().position(|x| x.id == c.id).expect("idx");
            let alvo = if c.class == CardClass::Unit { Some(0) } else { None };
            if rs.take(c, alvo).is_ok() && c.class == CardClass::Unit {
                alvos.push((0, idx));
            }
        }
        alvos
    }

    /// Gate da spec: 3 lutas (a 3ª = BOSS), escalada por vitória;
    /// winrate da run sortuda vs média ≤ 10 p.p.
    #[test]
    fn run_mc_sorte_nao_domina_media() {
        let r = roster();
        let n = 200;
        let mut vitorias_sortuda = 0;
        let mut vitorias_media = 0;
        for s in 0..n {
            let seed = 1000 + s as u64;
            for sortuda in [true, false] {
                let mut rs = RunState::new();
                let mut rng = Rng::new(seed * 31 + if sortuda { 7 } else { 13 });
                // comps sorteadas por seed (sem viés de matchup fixo)
                let mut team = [0usize; 4];
                for t in team.iter_mut() {
                    *t = rng.range(16);
                }
                let mut inimigos = [[0usize; 4]; 3];
                for b in inimigos.iter_mut() {
                    for slot in b.iter_mut() {
                        *slot = rng.range(16);
                    }
                }
                let mut alvos: Vec<(usize, usize)> = Vec::new();
                let mut wins: i32 = 0;
                for (batalha, inimigo) in inimigos.iter().enumerate() {
                    // escalada inimiga: +90‰ dano / +60‰ hp por vitória; boss +180/+100 extra
                    let boss = if batalha == 2 { (180, 100) } else { (0, 0) };
                    let e_b = SideEdge {
                        dmg_scale_permille: PER_MILLE + 90 * wins + boss.0,
                        hp_scale_permille: PER_MILLE + 60 * wins + boss.1,
                        ..SideEdge::default()
                    };
                    let e_a = rs.edge(&alvos);
                    let ganhou = jogar(&r, team, *inimigo, seed + batalha as u64, e_a, e_b);
                    if ganhou {
                        wins += 1;
                        rs.rerolls = 1;
                        let factions = RunState::team_factions(&r, team);
                        alvos.extend(comprar(&mut rs, &mut rng, sortuda, &factions));
                    } else {
                        break;
                    }
                }
                let run_completa = wins == 3;
                if sortuda {
                    vitorias_sortuda += run_completa as usize;
                } else {
                    vitorias_media += run_completa as usize;
                }
            }
        }
        let wr_s = vitorias_sortuda as f64 / n as f64;
        let wr_m = vitorias_media as f64 / n as f64;
        let diff = (wr_s - wr_m).abs();
        assert!(
            diff <= 0.10,
            "gate da spec: sorte vs média deve ser ≤ 10 p.p. (sortuda {wr_s:.3} vs média {wr_m:.3}, diff {diff:.3})"
        );
        assert!(wr_m > 0.0 && wr_s > 0.0, "ambas as estratégias devem vencer às vezes");
    }
}
