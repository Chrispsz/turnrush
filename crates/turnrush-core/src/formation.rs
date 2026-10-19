//! Modo **Formação 4v4** — combate por turnos tático com cadeias de chase.
//!
//! - 4 unidades por lado em formação 2×2 (frente/trás × esquerda/direita).
//! - Ordem de movimento por slot (`order: [usize; 4]` — 1º..4º do time).
//! - Ações intercaladas entre os times a cada passo (iniciativa alternada:
//!   quem abre a rodada é decidido por moeda na 1ª e alterna — simetria justa).
//! - **Cadeias de perseguição (chase)**: estados `launch` (arremesso),
//!   `knockback` (empurrão) e `knockdown` (derrubada) aplicados por um ataque
//!   permitem que aliados com o gatilho correspondente persigam, no máximo
//!   `max_per_round` por unidade por rodada.
//! - **Mystery** com custo de chakra + cooldown; chakra regenera por rodada.
//! - Determinismo total: RNG xoshiro256**, matemática inteira (per-mille),
//!   zero float no caminho quente, sem `unsafe`.
//!
//! Decisões v0.3 (documentadas em docs/ARQUITETURA.md):
//! - `ally_single`/`self` com `damage_pct` = **cura/escudo** em % do ATK.
//! - Efeitos em texto do roster interpretados por palavras-chave (escudo,
//!   cura, chakra, perfuração de escudo/defesa, Queimadura/Veneno = 10% do
//!   ATK por rodada por 2 rodadas, sem stack). O que não é modelado fica
//!   visível nos docs (nunca silencioso).

use crate::data::{ComboState, Roster, TargetSpec};
use crate::rng::Rng;
use crate::types::PER_MILLE;

/// Resultado da batalha (re-exportado para o cliente de jogo).
pub use crate::types::Outcome;

/// Tamanho da formação (4v4).
pub const FORMATION_SIZE: usize = 4;
/// Máximo de rodadas antes do desempate por % de vida.
///
/// v0.6: reduzido de 24 para 12 — partidas mais curtas e dinâmicas; o
/// desempate por % de vida total resolve confrontos que se arrastam.
pub const MAX_ROUNDS: usize = 12;
/// Chakra inicial de cada unidade.
///
/// v0.6: 30 (era 20) — mysteries entram mais cedo, rodadas rendem mais ação.
const CHAKRA_START: i32 = 30;
/// Chakra regenerado por rodada.
///
/// v0.6: 14 (era 12) — cadência de mysteries mais rápida, batalhas mais curtas.
const CHAKRA_GAIN: i32 = 14;
/// Teto de chakra.
const CHAKRA_CAP: i32 = 100;
/// Rodadas de dano contínuo (Queimadura/Veneno).
const DOT_TURNS: u8 = 2;
/// Dano contínuo em % do ATK do aplicador.
const DOT_ATK_PCT: i32 = 10;
/// Limite de elos por cadeia de perseguição (prova contra loops).
const CHAIN_CAP: usize = 12;

/// Ritmo de dano do 4v4 (per-mille).
///
/// v0.6: 2500 (×2.5) — com o teto de 12 rodadas, o dano nominal do roster
/// levava TODAS as batalhas ao desempate por HP% (guerra de atrito). Escalar
/// o dano de AMBOS os lados igualmente preserva o balanceamento relativo
/// (o DWC recalibra por composição) e faz as partidas terminarem por
/// derrota real, mais rápido e mais dinâmico.
const DAMAGE_PACE: i32 = 2500;

/// Escala o dano direto pelo ritmo global (mantém log ↔ dano fiéis).
fn pace(dmg: i32) -> i32 {
    dmg * DAMAGE_PACE / PER_MILLE
}

/// Escolha manual do jogador para a ação da unidade no turno dela.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionChoice {
    /// Conjure a mystery (o alvo segue a especialização da habilidade).
    Mystery,
    /// Ataque padrão; `Some(slot)` = alvo escolhido no lado inimigo,
    /// `None` = primeiro inimigo vivo (regra da IA).
    Standard { target: Option<usize> },
}

/// Decisão aguardando o jogador (lado interativo).
#[derive(Clone, Debug)]
pub struct PendingDecision {
    /// Lado que age (0 = jogador, 1 = adversário).
    pub side: usize,
    /// Slot da unidade que age (índice na formação).
    pub slot: usize,
    /// Nome da unidade.
    pub name: String,
    /// Chakra atual da unidade.
    pub chakra: i32,
    /// Mystery pronta (cooldown zerado e chakra suficiente)?
    pub mystery_ready: bool,
    /// Custo de chakra da mystery.
    pub mystery_cost: i32,
}

/// Resumo de uma unidade para a interface (botões de alvo, barras).
#[derive(Clone, Debug)]
pub struct UnitBrief {
    pub name: String,
    pub hp: i32,
    pub hp_max: i32,
    pub alive: bool,
}

/// Fase corrente da máquina de estados da batalha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Abertura da rodada: chakra/cooldowns/DoT.
    RoundStart,
    /// Ações intercaladas: `step` = 1º..4º da ordem, `sub` = 0/1 (lado).
    Action { step: usize, sub: usize },
    /// Fechamento da rodada: curas de passiva.
    RoundEnd,
    /// Batalha concluída (outcome preenchido).
    Done,
}

/// Modificadores de calibração por composição (DWC do 4v4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormationMods {
    /// Multiplicador de dano (per-mille).
    pub dmg_permille: i32,
    /// Multiplicador de vida (per-mille).
    pub hp_permille: i32,
}

impl Default for FormationMods {
    fn default() -> Self {
        FormationMods { dmg_permille: PER_MILLE, hp_permille: PER_MILLE }
    }
}

/// Unidade do modo formação (contrato v2 copiado do roster).
struct FU {
    name: String,
    hp_max: i32,
    hp: i32,
    atk: i32,
    def: i32,
    chakra: i32,
    mcd: u32,
    shield: i32,
    atk_buff_permille: i32,
    atk_buff_turns: u8,
    dot_dmg: i32,
    dot_turns: u8,
    dmg_reduction_permille: i32,
    reflect_permille: i32,
    dot_on_hit: bool,
    /// Bônus de dano próprio: `(ComboState::None, p)` = alvo sob DoT.
    bonus_vs: Vec<(ComboState, i32)>,
    /// Aura de equipe vs estado recém-aplicado.
    team_bonus_vs: Vec<(ComboState, i32)>,
    round_end_heal_pct: i32,
    round_start_chakra: i32,
    chased: bool,
    alive: bool,
    mystery: crate::data::MysteryDef,
    standard: crate::data::StandardDef,
    chase: crate::data::ChaseDef,
}

impl FU {
    fn atk_eff(&self) -> i32 {
        self.atk * (PER_MILLE + self.atk_buff_permille) / PER_MILLE
    }

    fn hp_pct(&self) -> i32 {
        if self.hp_max == 0 {
            return 0;
        }
        self.hp.max(0) * 100 / self.hp_max
    }
}

struct Side {
    units: Vec<FU>,
    /// Sequência de slots que age 1º..4º.
    order: [usize; FORMATION_SIZE],
}

impl Side {
    fn defeated(&self) -> bool {
        self.units.iter().all(|u| !u.alive)
    }

    fn total_hp_pct(&self) -> i32 {
        let max: i64 = self.units.iter().map(|u| u.hp_max as i64).sum();
        let now: i64 = self.units.iter().map(|u| u.hp.max(0) as i64).sum();
        if max == 0 {
            0
        } else {
            (now * 100 / max) as i32
        }
    }

    fn living_slots(&self) -> Vec<usize> {
        self.order
            .iter()
            .copied()
            .filter(|&s| self.units[s].alive)
            .collect()
    }
}

/// Batalha de formação 4v4 determinística com log opcional.
pub struct Formation4v4 {
    sides: [Side; 2],
    rng: Rng,
    round: usize,
    log: bool,
    events: Vec<String>,
    /// Estados frescos à espera de perseguição.
    pending_chase: Vec<(usize, usize, ComboState)>,
    chain_links: u64,
    mysteries_used: u64,
    unmodeled: Vec<String>,
    outcome: Option<Outcome>,
    // ── máquina de fases / modo interativo ──
    phase: Phase,
    started: bool,
    first_starter: usize,
    starter: usize,
    /// Lado que decide manualmente (None = totalmente automático).
    manual_side: Option<usize>,
    submitted: Option<ActionChoice>,
    pending: Option<PendingDecision>,
}

/// Primeiro `N%` após a palavra-chave (compartilhado com `interactive.rs`).
pub(crate) fn pct_after(text: &str, keyword: &str) -> Option<i32> {
    let idx = text.find(keyword)?;
    let rest = &text[idx + keyword.len()..];
    let b = rest.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i < b.len() && b[i] == b'%' {
                return rest[start..i].parse().ok();
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Primeiro `+N` após a palavra-chave (compartilhado com `interactive.rs`).
pub(crate) fn plus_after(text: &str, keyword: &str) -> Option<i32> {
    let idx = text.find(keyword)?;
    let rest = &text[idx..];
    let b = rest.as_bytes();
    let mut i = keyword.len();
    while i + 1 < b.len() {
        if b[i] == b'+' && b[i + 1].is_ascii_digit() {
            let start = i + 1;
            let mut j = start;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            return rest[start..j].parse().ok();
        }
        i += 1;
    }
    None
}

/// Primeiro inteiro após a palavra-chave, com ou sem `+` (v0.8 — ex.:
/// "perdem 5 de velocidade", "+25 de velocidade").
pub(crate) fn num_after(text: &str, keyword: &str) -> Option<i32> {
    plus_after(text, keyword).or_else(|| {
        let idx = text.find(keyword)?;
        let rest = &text[idx + keyword.len()..];
        let b = rest.as_bytes();
        let mut i = 0;
        while i < b.len() && !b[i].is_ascii_digit() {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if start == i {
            return None;
        }
        rest[start..i].parse().ok()
    })
}

impl Formation4v4 {
    /// Monta a batalha: `comp_a`/`comp_b` são 4 índices no roster.
    /// Ordem padrão [0,1,2,3] (frente-esq, frente-dir, trás-esq, trás-dir).
    pub fn new(roster: &Roster, comp_a: [usize; 4], comp_b: [usize; 4], seed: u64) -> Self {
        Self::new_with_mods(roster, comp_a, comp_b, seed, FormationMods::default(), FormationMods::default())
    }

    /// Igual a [`Self::new`] com modificadores de calibração por lado.
    pub fn new_with_mods(
        roster: &Roster,
        comp_a: [usize; 4],
        comp_b: [usize; 4],
        seed: u64,
        mods_a: FormationMods,
        mods_b: FormationMods,
    ) -> Self {
        let mk = |comp: [usize; 4], mods: FormationMods| -> Side {
            let units = comp
                .iter()
                .map(|&ci| {
                    let last = roster.characters.len().saturating_sub(1);
                    let c = &roster.characters[ci.min(last)];
                    let hp = (c.hp.max(1) * mods.hp_permille / PER_MILLE).max(1);
                    let atk = c.attack * mods.dmg_permille / PER_MILLE;
                    let mut u = FU {
                        name: c.name.clone(),
                        hp_max: hp,
                        hp,
                        atk,
                        def: c.defense,
                        chakra: CHAKRA_START,
                        mcd: 0,
                        shield: 0,
                        atk_buff_permille: 0,
                        atk_buff_turns: 0,
                        dot_dmg: 0,
                        dot_turns: 0,
                        dmg_reduction_permille: 0,
                        reflect_permille: 0,
                        dot_on_hit: false,
                        bonus_vs: Vec::new(),
                        team_bonus_vs: Vec::new(),
                        round_end_heal_pct: 0,
                        round_start_chakra: 0,
                        chased: false,
                        alive: true,
                        mystery: c.mystery.clone(),
                        standard: c.standard.clone(),
                        chase: c.chase.clone(),
                    };
                    Self::apply_passive(&mut u, &c.passive.effect);
                    u
                })
                .collect();
            Side { units, order: [0, 1, 2, 3] }
        };
        let _ = seed; // seed documentada nos eventos do simulador (sessão única)
        Formation4v4 {
            sides: [mk(comp_a, mods_a), mk(comp_b, mods_b)],
            rng: Rng::new(seed),
            round: 0,
            log: false,
            events: Vec::new(),
            pending_chase: Vec::new(),
            chain_links: 0,
            mysteries_used: 0,
            unmodeled: Vec::new(),
            outcome: None,
            phase: Phase::RoundStart,
            started: false,
            first_starter: 0,
            starter: 0,
            manual_side: None,
            submitted: None,
            pending: None,
        }
    }

    /// Interpreta a passiva (texto do contrato) em campos estruturados.
    /// O não modelado fica documentado — a simulação nunca assume efeito
    /// que não existiu no design.
    fn apply_passive(u: &mut FU, text: &str) {
        let low = text.to_lowercase();
        let condicional =
            low.contains("contra") || low.contains("ao ser") || low.contains("ao atingir");
        // buff de dano simples "+X% de dano" (não condicional)
        if !condicional && low.contains("dano") {
            if let Some(p) = pct_after(&low, "+") {
                u.atk_buff_permille += p * 10;
            }
        }
        // "+X% de dano contra alvos em Queimadura/Veneno"
        if low.contains("queimadura") || low.contains("veneno") {
            if let Some(p) = pct_after(&low, "+") {
                u.bonus_vs.push((ComboState::None, p * 10));
            }
        }
        // aura de equipe vs estado: "Inimigos Arremessados recebem +15% de dano"
        let aura = [
            ("arremessados", ComboState::Launch),
            ("derrubados", ComboState::Knockdown),
            ("empurrados", ComboState::Knockback),
        ];
        for (kw, st) in aura {
            if low.contains(kw) && low.contains("recebem") {
                if let Some(p) = pct_after(&low, "recebem") {
                    u.team_bonus_vs.push((st, p * 10));
                }
            }
        }
        // "-X% de dano recebido"
        if let Some(p) = pct_after(&low, "dano recebido") {
            u.dmg_reduction_permille = p * 10;
        }
        // "reflete X% do dano" / "contra-ataca com X% do ataque"
        if let Some(p) = pct_after(&low, "reflete") {
            u.reflect_permille = p * 10;
        }
        if let Some(p) = pct_after(&low, "contra-ataca") {
            u.reflect_permille = u.reflect_permille.max(p * 10);
        }
        // "No fim da rodada, cura 5% do HP do aliado com menos HP"
        if low.contains("fim da rodada") && low.contains("cura") {
            if let Some(p) = pct_after(&low, "cura") {
                u.round_end_heal_pct = p;
            }
        }
        // "No início da rodada, gera +8 de chakra para a equipe"
        if low.contains("início da rodada") && low.contains("chakra") {
            if let Some(n) = plus_after(&low, "chakra") {
                u.round_start_chakra = n;
            }
        }
        // "Os ataques de X aplicam Veneno (dano contínuo)"
        if low.contains("aplicam veneno") || low.contains("aplica veneno") || low.contains("queimadura") && low.contains("aplica") {
            u.dot_on_hit = true;
        }
    }

    pub fn set_log(&mut self, on: bool) {
        self.log = on;
    }

    /// Define a **ordem de movimento** (1º..4º) dos dois lados.
    ///
    /// A ordem é uma escolha tática do design (docs/GDD.md): quem age antes
    /// decide foco de fogo e timing de mystery. O simulador Monte Carlo amostra
    /// ordens por batalha (distribuição de escolhas dos jogadores) para que o
    /// winrate meça a média sobre táticas — sem isso, batalhas determinísticas
    /// quantizam o winrate em 0/0.5/1 e a calibração persegue ruído.
    ///
    /// Ordem inválida (não é permutação de 0..4) é ignorada silenciosamente
    /// (fallback: ordem padrão dos slots).
    pub fn set_orders(&mut self, a: [usize; FORMATION_SIZE], b: [usize; FORMATION_SIZE]) {
        let valida = |o: &[usize; FORMATION_SIZE]| -> bool {
            let mut v = *o;
            v.sort_unstable();
            v == [0, 1, 2, 3]
        };
        if valida(&a) {
            self.sides[0].order = a;
        }
        if valida(&b) {
            self.sides[1].order = b;
        }
    }

    /// Log de eventos legível da batalha (preenchido se `set_log(true)`).
    pub fn events(&self) -> &[String] {
        &self.events
    }

    /// Elos de perseguição executados.
    pub fn chain_links(&self) -> u64 {
        self.chain_links
    }

    /// Mysteries utilizadas.
    pub fn mysteries_used(&self) -> u64 {
        self.mysteries_used
    }

    /// Rodadas decorridas.
    pub fn rounds(&self) -> usize {
        self.round
    }

    /// Passivas/efeitos não modelados (diagnóstico honesto).
    pub fn unmodeled(&self) -> &[String] {
        &self.unmodeled
    }

    fn ev(&mut self, line: String) {
        if self.log {
            self.events.push(line);
        }
    }

    /// Executa a batalha completa. Idempotente: se já rodou, devolve o resultado.
    ///
    /// Com lado manual configurado ([`Self::set_manual_side`]) as decisões
    /// pendentes são resolvidas pela regra da IA (compatibilidade com o
    /// simulador) — para jogo manual use [`Self::advance_one`] + [`Self::submit`].
    pub fn run(&mut self) -> Outcome {
        if let Some(o) = self.outcome {
            return o;
        }
        while self.phase != Phase::Done {
            if self.pending.is_some() {
                self.auto_resolve_pending();
            }
            self.advance_one();
        }
        self.outcome.expect("fase Done sem outcome")
    }

    // ── API interativa (v0.6): batalha por passos com decisão manual ──

    /// Define o lado que decide manualmente (0 = jogador). `None` volta ao
    /// modo totalmente automático (comportamento do simulador).
    pub fn set_manual_side(&mut self, side: Option<usize>) {
        self.manual_side = side;
    }

    /// Decisão aguardando o jogador, se houver.
    pub fn pending_decision(&self) -> Option<&PendingDecision> {
        self.pending.as_ref()
    }

    /// Submete a escolha do jogador para a decisão pendente.
    ///
    /// Escolha inválida (mystery sem chakra/em cooldown, alvo morto) cai
    /// defensivamente para o ataque padrão automático — a UI deve prevenir,
    /// mas a engine nunca trava.
    pub fn submit(&mut self, choice: ActionChoice) {
        if self.pending.is_none() {
            return;
        }
        self.pending = None;
        self.submitted = Some(choice);
    }

    /// Resolve a decisão pendente com a regra da IA (mystery pronta com
    /// jitter de timing, senão padrão). Usado pelo `run()` e pelo botão
    /// "Auto" do cliente.
    pub fn auto_resolve_pending(&mut self) {
        if self.pending.is_none() {
            return;
        }
        let p = self.pending.as_ref().expect("pending verificado");
        let (side, slot) = (p.side, p.slot);
        let ready = self.mystery_castable(side, slot);
        let segura = ready && self.rng.range(4) == 0;
        self.pending = None;
        self.submitted = Some(if ready && !segura {
            ActionChoice::Mystery
        } else {
            ActionChoice::Standard { target: None }
        });
    }

    /// Batalha concluída?
    pub fn is_done(&self) -> bool {
        self.phase == Phase::Done
    }

    /// Resultado, quando concluída.
    pub fn outcome_opt(&self) -> Option<Outcome> {
        self.outcome
    }

    /// Resumo das unidades de um lado (nomes/HP para a interface).
    pub fn side_brief(&self, side: usize) -> [UnitBrief; FORMATION_SIZE] {
        std::array::from_fn(|i| {
            let u = &self.sides[side].units[i];
            UnitBrief { name: u.name.clone(), hp: u.hp.max(0), hp_max: u.hp_max, alive: u.alive }
        })
    }

    /// Avança EXATAMENTE um passo atômico da batalha:
    /// abertura de rodada, OU a ação de uma unidade (+ cadeias), OU o
    /// fechamento da rodada. Pausa quando uma decisão manual é necessária —
    /// nesse caso [`Self::pending_decision`] retorna `Some` e o cliente
    /// deve chamar [`Self::submit`] (ou [`Self::auto_resolve_pending`])
    /// antes do próximo `advance_one`.
    pub fn advance_one(&mut self) {
        if self.is_done() {
            return;
        }
        if !self.started {
            self.started = true;
            self.ev(format!(
                "── batalha 4v4 ── A: [{}] vs B: [{}]",
                self.sides[0].units.iter().map(|u| u.name.as_str()).collect::<Vec<_>>().join(", "),
                self.sides[1].units.iter().map(|u| u.name.as_str()).collect::<Vec<_>>().join(", ")
            ));
            // moeda: quem abre a 1ª rodada (simetria; depois alterna)
            self.first_starter = self.rng.range(2);
        }
        match self.phase {
            Phase::RoundStart => {
                if self.sides[0].defeated() || self.sides[1].defeated() {
                    self.finish();
                    return;
                }
                self.round += 1;
                self.starter = (self.first_starter + self.round - 1) % 2;
                self.ev(format!(
                    "── rodada {} (abre: {}) ──",
                    self.round,
                    if self.starter == 0 { "A" } else { "B" }
                ));
                self.round_start();
                self.phase = Phase::Action { step: 0, sub: 0 };
            }
            Phase::Action { step, sub } => {
                if step >= FORMATION_SIZE || self.sides[0].defeated() || self.sides[1].defeated() {
                    self.phase = Phase::RoundEnd;
                    return;
                }
                let side = if sub == 0 { self.starter } else { 1 - self.starter };
                let slot = self.sides[side].order[step];
                if !self.sides[side].units[slot].alive {
                    self.next_sub(step, sub);
                    return;
                }
                if self.manual_side == Some(side) && self.submitted.is_none() {
                    let u = &self.sides[side].units[slot];
                    self.pending = Some(PendingDecision {
                        side,
                        slot,
                        name: u.name.clone(),
                        chakra: u.chakra,
                        mystery_ready: self.mystery_castable(side, slot),
                        mystery_cost: u.mystery.chakra,
                    });
                    return; // aguarda submit/auto
                }
                let choice = self.submitted.take();
                self.pending = None;
                match choice {
                    Some(ActionChoice::Mystery) if self.mystery_castable(side, slot) => {
                        self.cast_mystery(side, slot);
                    }
                    Some(ActionChoice::Standard { target }) => {
                        self.standard_attack(side, slot, target);
                    }
                    // Lado automático (ou escolha manual inválida): regra da IA
                    // — mystery quando pronta, com jitter de timing.
                    _ => self.act(side, slot),
                }
                self.resolve_chases(side);
                self.next_sub(step, sub);
            }
            Phase::RoundEnd => {
                self.round_end();
                if self.round >= MAX_ROUNDS || self.sides[0].defeated() || self.sides[1].defeated() {
                    self.finish();
                } else {
                    self.phase = Phase::RoundStart;
                }
            }
            Phase::Done => {}
        }
    }

    fn next_sub(&mut self, step: usize, sub: usize) {
        self.phase = if sub == 0 {
            Phase::Action { step, sub: 1 }
        } else {
            Phase::Action { step: step + 1, sub: 0 }
        };
    }

    fn mystery_castable(&self, side: usize, slot: usize) -> bool {
        let u = &self.sides[side].units[slot];
        u.mcd == 0 && u.chakra >= u.mystery.chakra && u.mystery.chakra > 0
    }

    /// Calcula e registra o resultado final (idempotente).
    fn finish(&mut self) {
        if self.outcome.is_some() {
            self.phase = Phase::Done;
            return;
        }
        let outcome = if self.sides[1].defeated() {
            Outcome::TeamAWin
        } else if self.sides[0].defeated() {
            Outcome::TeamBWin
        } else {
            let ha = self.sides[0].total_hp_pct();
            let hb = self.sides[1].total_hp_pct();
            match ha.cmp(&hb) {
                std::cmp::Ordering::Greater => Outcome::TeamAWin,
                std::cmp::Ordering::Less => Outcome::TeamBWin,
                std::cmp::Ordering::Equal => Outcome::Draw,
            }
        };
        self.ev(format!(
            "── fim: {outcome:?} em {} rodadas · A={}% HP · B={}% HP · cadeias={} · mysteries={} ──",
            self.round,
            self.sides[0].total_hp_pct(),
            self.sides[1].total_hp_pct(),
            self.chain_links,
            self.mysteries_used
        ));
        self.outcome = Some(outcome);
        self.phase = Phase::Done;
    }

    /// Início de rodada: chakra, cooldowns, buffs e dano contínuo.
    fn round_start(&mut self) {
        let log = self.log;
        let mut logs: Vec<String> = Vec::new();
        for side in self.sides.iter_mut() {
            for u in side.units.iter_mut() {
                if !u.alive {
                    continue;
                }
                u.chased = false;
                u.mcd = u.mcd.saturating_sub(1);
                u.chakra = (u.chakra + CHAKRA_GAIN + u.round_start_chakra).min(CHAKRA_CAP);
                if u.atk_buff_turns > 0 {
                    u.atk_buff_turns -= 1;
                    if u.atk_buff_turns == 0 {
                        u.atk_buff_permille = 0;
                    }
                }
                if u.dot_turns > 0 {
                    u.dot_turns -= 1;
                    let d = u.dot_dmg;
                    if d > 0 {
                        u.hp -= d;
                        if u.hp <= 0 {
                            u.hp = 0;
                            u.alive = false;
                        }
                        if log {
                            logs.push(format!("    [DoT] {} sofre {d} de dano contínuo", u.name));
                        }
                    }
                }
            }
        }
        if log {
            self.events.extend(logs);
        }
    }

    /// Fim de rodada: curas de passiva (o aliado com menos HP%).
    fn round_end(&mut self) {
        let log = self.log;
        let mut logs: Vec<String> = Vec::new();
        for si in 0..2 {
            let pcts: Vec<i32> = self.sides[si]
                .units
                .iter()
                .map(|u| if u.alive { u.round_end_heal_pct } else { 0 })
                .collect();
            for &p in &pcts {
                if p == 0 {
                    continue;
                }
                let mut best = usize::MAX;
                let mut best_pct = 101;
                for (idx, t) in self.sides[si].units.iter().enumerate() {
                    if t.alive {
                        let hp = t.hp_pct();
                        if hp < best_pct {
                            best_pct = hp;
                            best = idx;
                        }
                    }
                }
                if best != usize::MAX {
                    let t = &mut self.sides[si].units[best];
                    let heal = t.hp_max * p / 100;
                    t.hp = (t.hp + heal).min(t.hp_max);
                    if log {
                        let tname = t.name.clone();
                        logs.push(format!("    [passiva] cura {heal} em {tname}"));
                    }
                }
            }
        }
        if log {
            self.events.extend(logs);
        }
    }

    /// Ação da unidade: mystery se pronta, senão ataque padrão.
    ///
    /// **Jitter de timing**: 1/4 de chance de segurar a mystery por um turno
    /// (semeado, simétrico para os dois lados). Modela a imperfeição humana de
    /// timing tático e dá variância real às batalhas — sem isso,
    /// táticas determinísticas tornam confrontos categóricos (wr 0.000/1.000)
    /// e o Monte Carlo não tem o que medir.
    fn act(&mut self, side_idx: usize, slot: usize) {
        let ready = {
            let u = &self.sides[side_idx].units[slot];
            u.mcd == 0 && u.chakra >= u.mystery.chakra && u.mystery.chakra > 0
        };
        let segura = ready && self.rng.range(4) == 0;
        if ready && !segura {
            self.cast_mystery(side_idx, slot);
        } else {
            self.standard_attack(side_idx, slot, None);
        }
    }

    fn cast_mystery(&mut self, side_idx: usize, slot: usize) {
        let (mname, chakra, cd, targets, dmg_pct, applies, effect) = {
            let u = &self.sides[side_idx].units[slot];
            (
                u.mystery.name.clone(),
                u.mystery.chakra,
                u.mystery.cooldown,
                u.mystery.targets,
                u.mystery.damage_pct,
                u.mystery.applies_state,
                u.mystery.effect.clone(),
            )
        };
        let low = effect.to_lowercase();
        let atk = self.sides[side_idx].units[slot].atk_eff();
        let pierce_def_pct = if low.contains("defesa") { pct_after(&low, "defesa").unwrap_or(0) } else { 0 };
        let pierce_shield_pct =
            if low.contains("ignora") { pct_after(&low, "escudo").unwrap_or(0) } else { 0 };
        let mut queue: Vec<(usize, usize, ComboState)> = Vec::new();

        let uname = self.sides[side_idx].units[slot].name.clone();
        self.sides[side_idx].units[slot].chakra -= chakra;
        self.sides[side_idx].units[slot].mcd = cd.max(1);
        self.mysteries_used += 1;
        self.ev(format!("  [mystery] {uname} usa «{mname}» (chakra -{chakra})"));

        // suporte: cura / escudo / chakra de equipe
        let heal_pct = if low.contains("cura") { pct_after(&low, "cura").unwrap_or(0) } else { 0 };
        let shield_pct =
            if low.contains("escudo") && !low.contains("ignora") { pct_after(&low, "escudo").unwrap_or(0) } else { 0 };
        let team_chakra = if low.contains("chakra") {
            plus_after(&low, "chakra").unwrap_or(plus_after(&low, "+").unwrap_or(0))
        } else {
            0
        };
        if team_chakra > 0 {
            for u in self.sides[side_idx].units.iter_mut() {
                if u.alive {
                    u.chakra = (u.chakra + team_chakra).min(CHAKRA_CAP);
                }
            }
            self.ev(format!("    equipe ganha +{team_chakra} de chakra"));
        }

        match targets {
            TargetSpec::AllySingle | TargetSpec::SelfTarget => {
                let tslot = if targets == TargetSpec::SelfTarget {
                    slot
                } else {
                    self.most_wounded_ally(side_idx)
                };
                let heal = atk * heal_pct / 100;
                let shield = if heal > 0 {
                    atk * shield_pct / 100
                } else {
                    atk * shield_pct.max(dmg_pct) / 100
                };
                let tname = self.sides[side_idx].units[tslot].name.clone();
                if heal > 0 {
                    let t = &mut self.sides[side_idx].units[tslot];
                    t.hp = (t.hp + heal).min(t.hp_max);
                    self.ev(format!("    cura {heal} em {tname}"));
                }
                if shield > 0 {
                    self.sides[side_idx].units[tslot].shield += shield;
                    self.ev(format!("    escudo {shield} em {tname}"));
                }
                if low.contains("reflete") {
                    if let Some(p) = pct_after(&low, "reflete") {
                        self.sides[side_idx].units[tslot].reflect_permille = p * 10;
                    }
                }
            }
            TargetSpec::EnemiesFront | TargetSpec::EnemiesAll | TargetSpec::EnemySingle | TargetSpec::Random2 => {
                let enemy = 1 - side_idx;
                let mut targets_slots: Vec<usize> = match targets {
                    TargetSpec::EnemiesFront => {
                        let living = self.sides[enemy].living_slots();
                        let front: Vec<usize> = living.iter().copied().take(2).collect();
                        if front.is_empty() { living } else { front }
                    }
                    TargetSpec::EnemiesAll => self.sides[enemy].living_slots(),
                    // v0.8: modo compacto não tem mira manual — single cai no
                    // primeiro vivo da frente (determinístico, igual ao auto).
                    TargetSpec::EnemySingle => self.sides[enemy].living_slots().into_iter().take(1).collect(),
                    TargetSpec::Random2 => {
                        let mut living = self.sides[enemy].living_slots();
                        let n = living.len().min(2);
                        self.rng.shuffle(&mut living);
                        living.truncate(n);
                        living
                    }
                    _ => Vec::new(),
                };
                targets_slots.retain(|&s| self.sides[enemy].units[s].alive);
                let dot_text = low.contains("queimadura") || low.contains("veneno");
                for tslot in targets_slots {
                    let (tname, tdef, t_red) = {
                        let t = &self.sides[enemy].units[tslot];
                        (t.name.clone(), t.def, t.dmg_reduction_permille)
                    };
                    let eff_def = tdef * (100 - pierce_def_pct) / 100;
                    let mut dmg = pace(crate::combat::damage_after_def(atk, dmg_pct, eff_def));
                    // aura de equipe vs estado recém-aplicado
                    if applies != ComboState::None {
                        let bonus = self.sides[side_idx].units[slot]
                            .team_bonus_vs
                            .iter()
                            .find(|(st, _)| *st == applies)
                            .map(|(_, p)| *p)
                            .unwrap_or(0);
                        if bonus > 0 {
                            dmg = dmg * (PER_MILLE + bonus) / PER_MILLE;
                        }
                    }
                    if t_red > 0 {
                        dmg = dmg * (PER_MILLE - t_red) / PER_MILLE;
                    }
                    self.deal_damage(enemy, tslot, dmg, pierce_shield_pct);
                    if self.sides[enemy].units[tslot].alive {
                        let t = &mut self.sides[enemy].units[tslot];
                        if dot_text && t.dot_turns == 0 {
                            t.dot_dmg = atk * DOT_ATK_PCT / 100;
                            t.dot_turns = DOT_TURNS;
                        }
                    }
                    if applies != ComboState::None && self.sides[enemy].units[tslot].alive {
                        queue.push((enemy, tslot, applies));
                        let st = applies.label();
                        self.ev(format!("    {tname} sofre {dmg} e recebe {st}"));
                    } else {
                        self.ev(format!("    {tname} sofre {dmg}"));
                    }
                }
            }
        }
        // estados aguardando perseguição (substitui o que restou da ação anterior)
        self.pending_chase = queue;
    }

    /// Ataque padrão; `chosen` = alvo manual no lado inimigo (validado;
    /// inválido/morto → primeiro vivo, regra da IA).
    fn standard_attack(&mut self, side_idx: usize, slot: usize, chosen: Option<usize>) {
        let (sname, dmg_pct, applies) = {
            let u = &self.sides[side_idx].units[slot];
            (u.standard.name.clone(), u.standard.damage_pct, u.standard.applies_state)
        };
        let enemy = 1 - side_idx;
        let targets = self.sides[enemy].living_slots();
        if targets.is_empty() {
            return;
        }
        let tslot = match chosen {
            Some(c) if c < FORMATION_SIZE && self.sides[enemy].units[c].alive => c,
            _ => targets[0],
        };
        let atk = self.sides[side_idx].units[slot].atk_eff();
        let dot_on_hit = self.sides[side_idx].units[slot].dot_on_hit;
        let self_bonus = self.sides[side_idx].units[slot].bonus_vs.clone();
        let (tname, tdef, t_red, t_dot) = {
            let t = &self.sides[enemy].units[tslot];
            (t.name.clone(), t.def, t.dmg_reduction_permille, t.dot_turns > 0)
        };
        let mut dmg = pace(crate::combat::damage_after_def(atk, dmg_pct, tdef));
        if t_dot {
            if let Some((_, p)) = self_bonus.iter().find(|(st, _)| *st == ComboState::None) {
                dmg = dmg * (PER_MILLE + p) / PER_MILLE;
            }
        }
        if applies != ComboState::None {
            if let Some((_, p)) = self.sides[side_idx].units[slot]
                .team_bonus_vs
                .iter()
                .find(|(st, _)| *st == applies)
            {
                dmg = dmg * (PER_MILLE + p) / PER_MILLE;
            }
        }
        if t_red > 0 {
            dmg = dmg * (PER_MILLE - t_red) / PER_MILLE;
        }
        if dot_on_hit {
            let t = &mut self.sides[enemy].units[tslot];
            if t.alive && t.dot_turns == 0 {
                t.dot_dmg = atk * DOT_ATK_PCT / 100;
                t.dot_turns = DOT_TURNS;
            }
        }
        self.deal_damage(enemy, tslot, dmg, 0);
        self.ev(format!(
            "  [padrão] {} usa «{sname}» → {tname} sofre {dmg}",
            self.sides[side_idx].units[slot].name
        ));
        if applies != ComboState::None && self.sides[enemy].units[tslot].alive {
            self.pending_chase.push((enemy, tslot, applies));
            let st = applies.label();
            self.ev(format!("    {tname} recebe {st}"));
        }
        // reflexo do alvo (só em ataque padrão)
        let refl = self.sides[enemy].units[tslot].reflect_permille;
        if refl > 0 && self.sides[side_idx].units[slot].alive {
            let back = dmg * refl / PER_MILLE;
            let aname = self.sides[side_idx].units[slot].name.clone();
            self.deal_damage(side_idx, slot, back, 0);
            self.ev(format!("    reflexo: {aname} sofre {back}"));
        }
    }

    fn most_wounded_ally(&self, side_idx: usize) -> usize {
        let side = &self.sides[side_idx];
        side.living_slots()
            .iter()
            .copied()
            .min_by_key(|&s| side.units[s].hp_pct())
            .unwrap_or(side.order[0])
    }

    fn deal_damage(&mut self, side_idx: usize, slot: usize, dmg: i32, pierce_shield_pct: i32) {
        let u = &mut self.sides[side_idx].units[slot];
        if !u.alive || dmg <= 0 {
            return;
        }
        let mut d = dmg;
        if u.shield > 0 {
            let hit_shield = u.shield.min(d * (100 - pierce_shield_pct) / 100);
            u.shield -= hit_shield;
            d -= hit_shield;
        }
        u.hp -= d;
        if u.hp <= 0 {
            u.hp = 0;
            u.alive = false;
            let name = u.name.clone();
            self.ev(format!("    ✖ {name} caiu"));
        }
    }

    /// Resolve cadeias de perseguição do time `side_idx` sobre estados frescos.
    fn resolve_chases(&mut self, side_idx: usize) {
        let mut guard = 0usize;
        while !self.pending_chase.is_empty() {
            guard += 1;
            if guard > CHAIN_CAP {
                self.pending_chase.clear();
                break;
            }
            let (enemy, tslot, st) = self.pending_chase.remove(0);
            if !self.sides[enemy].units[tslot].alive || st == ComboState::None {
                continue;
            }
            // procura perseguidor vivo do time `side_idx` com o gatilho
            let mut chosen: Option<usize> = None;
            for &s in &self.sides[side_idx].order {
                let u = &self.sides[side_idx].units[s];
                if u.alive && !u.chased && u.chase.trigger == st && u.chase.max_per_round > 0 {
                    chosen = Some(s);
                    break;
                }
            }
            let Some(cslot) = chosen else { continue };
            let (cname, cdmg_pct, capplies) = {
                let c = &self.sides[side_idx].units[cslot];
                (c.name.clone(), c.chase.damage_pct, c.chase.applies_state)
            };
            self.sides[side_idx].units[cslot].chased = true;
            let atk = self.sides[side_idx].units[cslot].atk_eff();
            let tdef = self.sides[enemy].units[tslot].def;
            let dmg = pace(crate::combat::damage_after_def(atk, cdmg_pct, tdef));
            let tname = self.sides[enemy].units[tslot].name.clone();
            self.deal_damage(enemy, tslot, dmg, 0);
            self.chain_links += 1;
            self.ev(format!(
                "  [chase] {cname} persegue {} de {tname} → {dmg} de dano",
                st.label()
            ));
            if capplies != ComboState::None && self.sides[enemy].units[tslot].alive {
                self.pending_chase.push((enemy, tslot, capplies));
                let st2 = capplies.label();
                self.ev(format!("    {tname} recebe {st2}"));
            }
        }
    }
}

/// Batalha rápida com modificadores de calibração (usada pelo Monte Carlo).
pub fn quick_battle_mods(
    roster: &Roster,
    a: [usize; 4],
    b: [usize; 4],
    seed: u64,
    mods_a: FormationMods,
    mods_b: FormationMods,
) -> Outcome {
    let mut bt = Formation4v4::new_with_mods(roster, a, b, seed, mods_a, mods_b);
    bt.run()
}

/// Igual a [`quick_battle_mods`] com **ordens de movimento** explícitas por
/// lado (escolha tática; ver [`Formation4v4::set_orders`]).
#[allow(clippy::too_many_arguments)] // construtor de batalha do Monte Carlo
pub fn quick_battle_mods_orders(
    roster: &Roster,
    a: [usize; 4],
    b: [usize; 4],
    seed: u64,
    mods_a: FormationMods,
    mods_b: FormationMods,
    order_a: [usize; 4],
    order_b: [usize; 4],
) -> Outcome {
    let mut bt = Formation4v4::new_with_mods(roster, a, b, seed, mods_a, mods_b);
    bt.set_orders(order_a, order_b);
    bt.run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Roster;

    fn roster() -> Roster {
        Roster::builtin()
    }

    #[test]
    fn determinismo_total() {
        let r = roster();
        let mut b1 = Formation4v4::new(&r, [0, 1, 2, 0], [3, 4, 5, 3], 42);
        let mut b2 = Formation4v4::new(&r, [0, 1, 2, 0], [3, 4, 5, 3], 42);
        let o1 = b1.run();
        let o2 = b2.run();
        assert_eq!(o1, o2);
        assert_eq!(b1.events(), b2.events());
        assert_eq!(b1.chain_links(), b2.chain_links());
    }

    #[test]
    fn espelho_converge_para_metade() {
        let r = roster();
        let mut wins_a = 0;
        let n = 101;
        for s in 0..n {
            let mut b = Formation4v4::new(&r, [0, 1, 2, 0], [0, 1, 2, 0], s as u64);
            match b.run() {
                Outcome::TeamAWin => wins_a += 1,
                Outcome::Draw => {}
                Outcome::TeamBWin => {}
            }
        }
        let wr = wins_a as f32 / n as f32;
        assert!(
            (0.35..=0.65).contains(&wr),
            "espelho deve ~50% (A={wr:.2} em {n} batalhas)"
        );
    }

    /// Roster mínimo de teste: lançadores aplicam arremesso no padrão;
    /// perseguidores perseguem arremesso 1×/rodada; tanques não aplicam nada.
    const CHASE_TEST_ROSTER: &str = r#"{"version":2,"characters":[
        {"id":"a1","name":"Lançador","faction":"ember","role":"striker","hp":1000,"attack":100,"defense":40,"speed":50,
         "mystery":{"name":"M","chakra":40,"cooldown":4,"targets":"enemies_front","damage_pct":90,"applies_state":"none","effect":""},
         "standard":{"name":"P","damage_pct":70,"applies_state":"launch"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}},
        {"id":"b1","name":"Perseguidor","faction":"ember","role":"striker","hp":1000,"attack":100,"defense":40,"speed":50,
         "mystery":{"name":"M","chakra":40,"cooldown":4,"targets":"enemies_front","damage_pct":90,"applies_state":"none","effect":""},
         "standard":{"name":"P","damage_pct":70,"applies_state":"none"},
         "chase":{"name":"C","trigger":"launch","damage_pct":25,"applies_state":"none","max_per_round":1},
         "passive":{"name":"","effect":""}},
        {"id":"a2","name":"Lançador2","faction":"ember","role":"striker","hp":1000,"attack":100,"defense":40,"speed":50,
         "mystery":{"name":"M","chakra":40,"cooldown":4,"targets":"enemies_front","damage_pct":90,"applies_state":"none","effect":""},
         "standard":{"name":"P","damage_pct":70,"applies_state":"launch"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}},
        {"id":"b2","name":"Perseguidor2","faction":"ember","role":"striker","hp":1000,"attack":100,"defense":40,"speed":50,
         "mystery":{"name":"M","chakra":40,"cooldown":4,"targets":"enemies_front","damage_pct":90,"applies_state":"none","effect":""},
         "standard":{"name":"P","damage_pct":70,"applies_state":"none"},
         "chase":{"name":"C","trigger":"launch","damage_pct":25,"applies_state":"none","max_per_round":1},
         "passive":{"name":"","effect":""}},
        {"id":"c1","name":"Tanque","faction":"tide","role":"guardian","hp":1150,"attack":100,"defense":40,"speed":50,
         "mystery":{"name":"M","chakra":40,"cooldown":4,"targets":"self","damage_pct":80,"applies_state":"none","effect":""},
         "standard":{"name":"P","damage_pct":70,"applies_state":"none"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}},
        {"id":"c2","name":"Tanque2","faction":"tide","role":"guardian","hp":1150,"attack":100,"defense":40,"speed":50,
         "mystery":{"name":"M","chakra":40,"cooldown":4,"targets":"self","damage_pct":80,"applies_state":"none","effect":""},
         "standard":{"name":"P","damage_pct":70,"applies_state":"none"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}},
        {"id":"c3","name":"Tanque3","faction":"tide","role":"guardian","hp":1150,"attack":100,"defense":40,"speed":50,
         "mystery":{"name":"M","chakra":40,"cooldown":4,"targets":"self","damage_pct":80,"applies_state":"none","effect":""},
         "standard":{"name":"P","damage_pct":70,"applies_state":"none"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}},
        {"id":"c4","name":"Tanque4","faction":"tide","role":"guardian","hp":1150,"attack":100,"defense":40,"speed":50,
         "mystery":{"name":"M","chakra":40,"cooldown":4,"targets":"self","damage_pct":80,"applies_state":"none","effect":""},
         "standard":{"name":"P","damage_pct":70,"applies_state":"none"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}}
    ]}"#;

    #[test]
    fn cadeia_respeita_max_por_rodada() {
        let r = Roster::from_json(CHASE_TEST_ROSTER).expect("roster de teste");
        let mut b = Formation4v4::new(&r, [0, 2, 1, 3], [4, 5, 6, 7], 7);
        b.set_log(true);
        let _ = b.run();
        assert!(b.chain_links() > 0, "esperado ≥1 elo de chase");
        assert!(
            b.chain_links() as usize <= b.rounds() * 2,
            "max_per_round violado: {} elos em {} rodadas",
            b.chain_links(),
            b.rounds()
        );
    }

    #[test]
    fn chakra_e_cooldown_respeitados() {
        let r = roster();
        let mut b = Formation4v4::new(&r, [0, 1, 2, 0], [3, 4, 5, 3], 99);
        b.set_log(true);
        let _ = b.run();
        assert!(b.mysteries_used() > 0);
        assert!(b.rounds() <= MAX_ROUNDS);
    }

    /// v0.6: a máquina de fases (`advance_one`) deve produzir EXATAMENTE a
    /// mesma sequência de eventos e o mesmo resultado do caminho compacto.
    #[test]
    fn avanco_por_passos_igual_a_run() {
        let r = roster();
        let mut b1 = Formation4v4::new(&r, [0, 1, 2, 0], [3, 4, 5, 3], 77);
        let o1 = b1.run();

        let mut b2 = Formation4v4::new(&r, [0, 1, 2, 0], [3, 4, 5, 3], 77);
        while !b2.is_done() {
            b2.advance_one();
        }
        assert_eq!(o1, b2.outcome_opt().expect("b2 concluída"));
        assert_eq!(b1.events(), b2.events());
        assert_eq!(b1.chain_links(), b2.chain_links());
    }

    /// v0.6: modo manual — o jogador submete escolhas e a batalha conclui;
    /// decisões aparecem somente para o lado manual.
    #[test]
    fn modo_manual_aceita_escolhas() {
        let r = roster();
        let mut b = Formation4v4::new(&r, [0, 1, 2, 0], [3, 4, 5, 3], 55);
        b.set_log(true);
        b.set_manual_side(Some(0));
        let mut decisoes = 0usize;
        let mut guard = 0usize;
        while !b.is_done() {
            guard += 1;
            assert!(guard < 5000, "batalha manual não converge");
            if b.pending_decision().is_some() {
                decisoes += 1;
                let p = b.pending_decision().expect("pending");
                assert_eq!(p.side, 0, "decisão deve ser sempre do lado manual");
                // alterna: mystery quando pronta, senão padrão no alvo 0
                if p.mystery_ready {
                    b.submit(ActionChoice::Mystery);
                } else {
                    b.submit(ActionChoice::Standard { target: Some(0) });
                }
                assert!(b.pending_decision().is_none(), "submit deve consumir a pendência");
            }
            b.advance_one();
        }
        assert!(decisoes > 0, "lado manual deve gerar decisões");
        assert!(b.outcome_opt().is_some());
        assert!(b.rounds() <= MAX_ROUNDS);
    }

    /// v0.6: escolha inválida (alvo inexistente) cai para o padrão sem travar.
    #[test]
    fn escolha_invalida_cai_para_padrao() {
        let r = roster();
        let mut b = Formation4v4::new(&r, [0, 1, 2, 0], [3, 4, 5, 3], 66);
        b.set_manual_side(Some(0));
        let mut guard = 0usize;
        while !b.is_done() {
            guard += 1;
            assert!(guard < 5000);
            if b.pending_decision().is_some() {
                // submit sempre inválido: alvo 9 não existe; mystery sem checar chakra
                b.submit(ActionChoice::Standard { target: Some(9) });
            }
            b.advance_one();
        }
        assert!(b.outcome_opt().is_some());
    }

    /// v0.6: `run()` com lado manual resolve pendências como IA (compat).
    #[test]
    fn run_com_lado_manual_ainda_conclui() {
        let r = roster();
        let mut b = Formation4v4::new(&r, [0, 1, 2, 0], [3, 4, 5, 3], 44);
        b.set_manual_side(Some(0));
        let o = b.run();
        assert!(b.outcome_opt() == Some(o));
    }

    #[test]
    fn batalha_rapida_no_roster_embutido() {
        let r = roster();
        let t0 = std::time::Instant::now();
        let n = 200;
        for s in 0..n {
            let mut b = Formation4v4::new(
                &r,
                [s % 12, (s + 1) % 12, (s + 2) % 12, (s + 3) % 12],
                [(s + 4) % 12, (s + 5) % 12, (s + 6) % 12, (s + 7) % 12],
                s as u64,
            );
            let _ = b.run();
        }
        let dt = t0.elapsed();
        assert!(dt.as_millis() < 2000, "200 batalhas 4v4 em <2s (levou {dt:?})");
    }
}
