//! Modo **interativo** da batalha 4v4 — o motor do jogo manual (v0.7).
//!
//! API determinística para clientes manuais: o cliente pergunta de quem é a
//! vez ([`InteractiveBattle::prompt`]), lista as ações legais
//! ([`InteractiveBattle::legal_actions`]), executa UMA ação por vez
//! ([`InteractiveBattle::execute_action`]) e consome eventos finos
//! ([`BattleEvent`]) — sem nunca parsear texto de log. A IA inimiga usa
//! EXATAMENTE a mesma superfície (simetria por construção; ver E.9).
//!
//! Regras v0.7 (especificação "manual-first"):
//! - **Timeline visível**: ordem A1→B1→A2→B2… por SPD decrescente dentro do
//!   lado; quem abre a 1ª rodada é moeda semeada, depois alterna.
//! - **Teto de 10 rodadas** — desempate por % de HP total do time.
//! - **Fluxo** (energia): teto 100, começa em 0, +10 fixo por rodada; o
//!   ataque padrão é grátis e GERA +15; Skill custa 30 (CD 2), Mystery 60
//!   (CD 3, destrava R2/R3 por papel ou `unlock_schedule` do JSON), interrupt
//!   35 (CD 3). Geração passiva por rodada tem teto +20.
//! - **Tiers de habilidade** derivados do roster: Standard = contrato;
//!   Skill = a mystery em forma base (dano 120% alvo único/frente, 100% AoE,
//!   cura/escudo do contrato, sem estado/DoT); Mystery = forma final
//!   (200% alvo único, 100% multi-alvo, cura/escudo ×165‰ — bandas da spec).
//! - **Interrupt**: reação usada na SUA vez, contra inimigo que AINDA vai
//!   agir nesta rodada — Stun (perde a ação) ou Silence (sem Skill/Mystery).
//!   Zero dica na UI: quem quiser interromper lê a timeline e o fluxo alheio.
//! - **Chase manual**: estados aplicados abrem janela de perseguição para o
//!   time atacante; confirmar (elencar perseguidor) ou recusar (também é
//!   tática). Teto de 12 elos, 1 chase por unidade por rodada.
//! - Stun/Silence duram 1 ação; Queimadura/Veneno: 30% do ATK do aplicador
//!   por rodada, 2 rodadas, sem acumular (refresh).
//!
//! Toda a matemática é inteira (per-mille) e o RNG é o xoshiro256** semeado
//! herdado de `rng.rs` — mesma seed, mesma batalha, qualquer plataforma.
//! O modo compacto antigo (`formation.rs::Formation4v4::run`) permanece
//! intocado: alimentação do Monte Carlo e playback legado.

use crate::data::{
    ComboState, InterruptDef, MysteryDef, Role, Roster, RosterChar, StandardDef, TargetSpec,
};
use crate::formation::{pct_after, plus_after};
use crate::rng::Rng;
use crate::types::Outcome;

// ─────────────────────────────────────────────────────────────────────────────
// Constantes de balanceamento (spec PARTE 1 — números fechados)
// ─────────────────────────────────────────────────────────────────────────────

/// Teto de rodadas: desempate por % de HP total.
pub const MAX_ROUNDS: usize = 10;

/// R1 (feedback 2026-10-18): oferta de tática a cada N rodadas dentro da
/// batalha (rodadas 2, 4, 6, 8 — a rodada final não oferta).
pub const TACTIC_INTERVAL: usize = 2;
/// Teto de fluxo por unidade.
pub const FLUXO_CAP: i32 = 100;
/// Fluxo ganho no início de cada rodada.
pub const FLUXO_PER_ROUND: i32 = 10;
/// Fluxo gerado por ataque padrão (custo 0).
pub const FLUXO_STD_GAIN: i32 = 15;
/// Custo da Skill (forma base).
pub const SKILL_COST: i32 = 30;
/// Custo da Mystery (forma final).
pub const MYSTERY_COST: i32 = 60;
/// Custo do interrupt.
pub const INTERRUPT_COST: i32 = 35;
/// Cooldown da Skill (rodadas).
pub const SKILL_CD: u32 = 2;
/// Cooldown da Mystery (rodadas).
pub const MYSTERY_CD: u32 = 3;
/// Cooldown do interrupt (rodadas).
pub const INTERRUPT_CD: u32 = 3;
/// Teto de geração passiva de fluxo por rodada (por unidade).
pub const FLUXO_PASSIVE_CAP: i32 = 20;
/// Dano contínuo (Queimadura/Veneno): % do ATK do aplicador por tick.
pub const DOT_ATK_PCT: i32 = 30;
/// Rodadas de dano contínuo.
pub const DOT_TURNS: u8 = 2;
/// Escala global de dano (mantém o TTK da spec com o roster real).
const DAMAGE_PACE: i32 = 2500;
/// Teto de elos por cadeia de perseguição (guarda contra loops).
const CHAIN_CAP: usize = 12;
/// Banda de dano da Skill: alvo único/frente.
const SKILL_DMG_SINGLE: i32 = 120;
/// Banda de dano da Skill: multi-alvo (idem Mystery AoE).
const SKILL_DMG_AOE: i32 = 100;
/// Banda de dano da Mystery: alvo único.
const MYSTERY_DMG_SINGLE: i32 = 200;
/// Banda de dano da Mystery: multi-alvo.
const MYSTERY_DMG_AOE: i32 = 100;
/// Amplificação de cura/escudo da Mystery (per-mille).
const MYSTERY_SUPPORT_SCALE: i32 = 1650;

/// 1000 = 100%.
const PER_MILLE: i32 = 1000;

fn pace(dmg: i32) -> i32 {
    dmg * DAMAGE_PACE / PER_MILLE
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipos públicos da API
// ─────────────────────────────────────────────────────────────────────────────

/// Fase corrente do turno — o cliente espelha isto na UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptPhase {
    /// Unidade escolhendo ação (e alvo, se houver).
    ChooseAction,
    /// Janela de perseguição aberta: confirmar perseguidor ou recusar.
    ConfirmChase,
    /// R1 (feedback): oferta de tática de batalha a cada 2 rodadas —
    /// escolher 1 das 2 cartas ofertadas ([`ChosenAction::PickTactic`]).
    ChooseTactic,
    /// Batalha concluída ([`InteractiveBattle::outcome`] preenchido).
    Over,
}

/// Vez atual: de quem é a decisão e em que fase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TurnPrompt {
    /// Lado que decide (0 = jogador, 1 = adversário).
    pub side: usize,
    /// Slot da unidade que decide.
    pub slot: usize,
    /// Rodada corrente (1..=`MAX_ROUNDS`).
    pub round: usize,
    pub phase: PromptPhase,
}

/// Categoria de ação.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    /// Grátis, gera +15 de fluxo.
    Standard,
    /// 30 de fluxo, CD 2, sempre disponível.
    Skill,
    /// 60 de fluxo, CD 3, destrava por rodada de desbloqueio.
    Mystery,
    /// 35 de fluxo, CD 3: interrompe inimigo que ainda vai agir.
    Interrupt,
}

impl ActionKind {
    /// Rótulo pt-BR para UI/log.
    pub fn label(self) -> &'static str {
        match self {
            ActionKind::Standard => "Padrão",
            ActionKind::Skill => "Skill",
            ActionKind::Mystery => "Mystery",
            ActionKind::Interrupt => "Interrupt",
        }
    }
}

/// Para quem a ação aponta (guia de UI; a validação é da engine).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    /// Um inimigo vivo (`Some` = escolhido, `None` = primeiro vivo).
    Enemy,
    /// Um aliado vivo (`None` = o mais ferido, regra da IA).
    Ally,
    /// Sem alvo explícito (frente/AoE/aleatório/si mesmo).
    Auto,
}

/// Ação legal, pronta para pré-visualização e execução.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegalAction {
    pub kind: ActionKind,
    /// Custo em fluxo (0 para Padrão, que na verdade gera fluxo).
    pub fluxo_cost: i32,
    pub targeting: TargetKind,
    /// Nome de exibição da habilidade.
    pub name: String,
}

/// Decisão do controlador (jogador ou IA) para a vez atual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChosenAction {
    /// Ação de combate; `target` = slot no lado inimigo (0..4) ou aliado
    /// (para Skill/Mystery de alvo único aliado). `None` = regra automática.
    Act { kind: ActionKind, target: Option<usize> },
    /// Confirma a perseguição com o perseguidor do slot dado.
    Chase { chaser_slot: usize },
    /// Recusa a janela de perseguição (tática válida).
    RejectChase,
    /// Adota 1 das 2 táticas ofertadas (fase [`PromptPhase::ChooseTactic`];
    /// índice 0/1 DENTRO da oferta, não no pool).
    PickTactic { index: usize },
}

/// Erro de execução — a UI deve prevenir, a engine nunca trava em silêncio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionError {
    NotYourTurn,
    WrongPhase,
    IllegalAction,
    BadTarget,
    NotEnoughFluxo,
    OnCooldown,
    LockedOut,
    BattleOver,
}

/// Pré-visualização EXATA de uma ação (o cliente mostra isto antes de
/// confirmar). Números vêm dos MESMOS helpers da execução — o que a prévia
/// promete é o que acontece. Random2 lista candidatos com nota honesta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionPreview {
    /// Dano total estimado (soma sobre os alvos efetivos).
    pub damage: i32,
    /// Cura total estimada.
    pub heal: i32,
    /// Escudo total estimado.
    pub shield: i32,
    /// Estados que a ação vai aplicar: (lado_alvo, slot, estado).
    pub states: Vec<(usize, usize, ComboState)>,
    /// Delta de fluxo do executor (negativo = gasta; +15 no padrão).
    pub fluxo_delta: i32,
    /// Alvos efetivos resolvidos: (lado, slot).
    pub targets: Vec<(usize, usize)>,
    /// Notas honestas (ex.: "alvos aleatórios: 2 sorteados entre N vivos").
    pub notes: Vec<String>,
}

/// Motivo de ação pulada (transparência total para o cliente).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// Unidade morta.
    Dead,
    /// Atordoada (perdeu a ação — ex.: interrupt Stun).
    Stunned,
}

/// Evento fino da batalha: o cliente toca isto, não parseia texto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BattleEvent {
    /// Abertura de rodada (`starter` = lado que age primeiro).
    RoundStart { round: usize, starter: usize },
    /// Ganho de fluxo (rodada, padrão, passiva).
    FluxoGain { side: usize, slot: usize, amount: i32 },
    /// Declaração de ação (a UI mostra o cartão da habilidade).
    ActionDeclared {
        side: usize,
        slot: usize,
        kind: ActionKind,
        name: String,
        target: Option<(usize, usize)>,
    },
    /// Dano direto/contínuo/reflexo; `killed` = alvo caiu.
    Damage { side: usize, slot: usize, amount: i32, killed: bool },
    /// Cura (habilidade ou passiva de fim de rodada).
    Heal { side: usize, slot: usize, amount: i32 },
    /// Unidade reviveu (Último Suspiro) com `amount` de HP.
    Revive { side: usize, slot: usize, amount: i32 },
    /// Escudo concedido.
    Shield { side: usize, slot: usize, amount: i32 },
    /// Estado de combo aplicado (abre janela de chase).
    StateApplied { side: usize, slot: usize, state: ComboState },
    /// Atordoado/Silenciado por interrupt.
    Debuffed { side: usize, slot: usize, kind: crate::data::InterruptKind },
    /// Ação pulada (morta/atordoada) — visível, nunca silenciosa.
    Skip { side: usize, slot: usize, reason: SkipReason },
    /// Janela de perseguição aberta para o time `side`.
    ChaseOffered { side: usize, target: (usize, usize), state: ComboState, eligible: Vec<usize> },
    /// Elo de perseguição executado.
    ChaseResolved { chaser: (usize, usize), target: (usize, usize), amount: i32, killed: bool },
    /// Janela recusada.
    ChaseRejected { side: usize },
    /// R1 (feedback): tática de batalha adotada. O log do cliente exibe só
    /// a do jogador (lado 0) — a do inimigo é silenciosa por design.
    TacticPicked { side: usize, tactic: String },
    /// Fechamento de rodada (curas de passiva já emitidas como [`BattleEvent::Heal`]).
    RoundEnd { round: usize },
    /// Batalha concluída.
    BattleOver { outcome: Outcome },
    /// Passiva/efeito não modelado — honestidade acima de silêncio.
    Unmodeled { text: String },
}

/// Retrato imutável de uma unidade para UI/IA (sem acesso ao interior).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitSnapshot {
    pub name: String,
    pub role: Role,
    pub hp: i32,
    pub hp_max: i32,
    pub shield: i32,
    pub fluxo: i32,
    pub alive: bool,
    pub stun_turns: u8,
    pub silence_turns: u8,
    pub dot_turns: u8,
    pub skill_cd: u32,
    pub mystery_cd: u32,
    pub interrupt_cd: u32,
    /// Já agiu nesta rodada (janela de interrupt).
    pub acted: bool,
    pub standard_name: String,
    pub skill_name: String,
    pub mystery_name: String,
    pub mystery_unlock_round: usize,
    pub interrupt: Option<InterruptDef>,
    /// Já perseguiu nesta rodada.
    pub chased: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Internos
// ─────────────────────────────────────────────────────────────────────────────

/// Habilidade resolvida em um tier (Skill ou Mystery) a partir do contrato.
#[derive(Clone, Debug)]
struct Tier {
    name: String,
    targets: TargetSpec,
    dmg_pct: i32,
    heal_pct: i32,
    shield_pct: i32,
    team_fluxo: i32,
    applies: ComboState,
    dot: bool,
    pierce_def_pct: i32,
    pierce_shield_pct: i32,
    reflect_pct: i32,
}

impl Tier {
    /// Deriva os dois tiers a partir da mystery do contrato.
    ///
    /// Skill = forma base (dano 120 único / 100 multi, utilidade do contrato,
    /// SEM estado); Mystery = forma final (200 único / 100 multi, utilidade
    /// ×1650‰, COM estado/DoT/perfuração).
    ///
    /// v0.8: `skill_targets` permite forma de alvo própria da Skill (ex.:
    /// striker com Skill single 120% e Mystery single 200%). `None` = herda
    /// a mystery (comportamento v0.7). `EnemySingle` NÃO é multi — cai nas
    /// bandas de alvo único.
    fn derive(m: &MysteryDef, skill_targets: Option<TargetSpec>) -> (Tier, Tier) {
        let low = m.effect.to_lowercase();
        let heal_json = if low.contains("cura") { pct_after(&low, "cura").unwrap_or(0) } else { 0 };
        let shield_json = if low.contains("escudo") && !low.contains("ignora") {
            pct_after(&low, "escudo").unwrap_or(0).max(m.damage_pct)
        } else {
            0
        };
        let team_fluxo = if low.contains("chakra") {
            plus_after(&low, "gera").unwrap_or(plus_after(&low, "chakra").unwrap_or(plus_after(&low, "+").unwrap_or(0)))
        } else {
            0
        };
        let reflect = if low.contains("reflete") { pct_after(&low, "reflete").unwrap_or(0) } else { 0 };
        let pierce_def =
            if low.contains("defesa") && low.contains("ignora") { pct_after(&low, "defesa").unwrap_or(0) } else { 0 };
        let pierce_shield =
            if low.contains("ignora") && low.contains("escudo") { pct_after(&low, "escudo").unwrap_or(0) } else { 0 };
        let dot = low.contains("queimadura") || low.contains("veneno");
        let multi = matches!(m.targets, TargetSpec::EnemiesFront | TargetSpec::EnemiesAll | TargetSpec::Random2);
        let skill_t = skill_targets.unwrap_or(m.targets);
        let skill_multi = matches!(skill_t, TargetSpec::EnemiesFront | TargetSpec::EnemiesAll | TargetSpec::Random2);

        let skill = Tier {
            name: m.name.clone(),
            targets: skill_t,
            dmg_pct: if heal_json > 0 || shield_json > 0 { 0 } else if skill_multi { SKILL_DMG_AOE } else { SKILL_DMG_SINGLE },
            heal_pct: heal_json,
            shield_pct: shield_json,
            team_fluxo,
            applies: ComboState::None,
            dot: false,
            pierce_def_pct: 0,
            pierce_shield_pct: 0,
            reflect_pct: reflect,
        };
        let mystery = Tier {
            name: m.name.clone(),
            targets: m.targets,
            dmg_pct: if heal_json > 0 || shield_json > 0 { 0 } else if multi { MYSTERY_DMG_AOE } else { MYSTERY_DMG_SINGLE },
            heal_pct: heal_json * MYSTERY_SUPPORT_SCALE / PER_MILLE,
            shield_pct: shield_json * MYSTERY_SUPPORT_SCALE / PER_MILLE,
            team_fluxo,
            applies: m.applies_state,
            dot,
            pierce_def_pct: pierce_def,
            pierce_shield_pct: pierce_shield,
            reflect_pct: reflect,
        };
        (skill, mystery)
    }
}

/// Vantagem de um lado na batalha: escala (escalada inimiga/boss) + bônus
/// acumulados de cartas da run. Tudo inteiro/per-mille — default = neutro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideEdge {
    /// Escala de dano base (1000 = 100%). Usada pela escalada de inimigos.
    pub dmg_scale_permille: i32,
    /// Escala de vida base (1000 = 100%).
    pub hp_scale_permille: i32,
    /// Bônus de ATK de cartas (per-mille somado).
    pub atk_bonus_permille: i32,
    /// Bônus de HP de cartas (per-mille somado; pode ser negativo).
    pub hp_bonus_permille: i32,
    /// Fluxo extra por rodada (carta Fluxo Estável etc.).
    pub fluxo_per_round: i32,
    /// Dano de chase por elo de cadeia na rodada (per-mille; Eco de Combate).
    pub chain_dmg_per_link: i32,
    /// Teto do bônus de cadeia (per-mille).
    pub chain_dmg_cap: i32,
    /// Chases extras por unidade por rodada (Ambidestria = 1).
    pub extra_chases: u32,
    /// % de HP de revival da 1ª morte do time (Último Suspiro; 0 = sem).
    pub revive_pct: i32,
    /// Rodadas de antecipação da Mystery (Gatilho Rápido).
    pub unlock_early: usize,
    /// Bônus de ATK por slot (cartas de unidade).
    pub unit_atk: [i32; 4],
    /// Bônus de HP por slot (cartas de unidade).
    pub unit_hp: [i32; 4],
}

impl Default for SideEdge {
    fn default() -> Self {
        SideEdge {
            dmg_scale_permille: PER_MILLE,
            hp_scale_permille: PER_MILLE,
            atk_bonus_permille: 0,
            hp_bonus_permille: 0,
            fluxo_per_round: 0,
            chain_dmg_per_link: 0,
            chain_dmg_cap: 0,
            extra_chases: 0,
            revive_pct: 0,
            unlock_early: 0,
            unit_atk: [0; 4],
            unit_hp: [0; 4],
        }
    }
}

/// Unidade do modo interativo.
struct IU {
    name: String,
    role: Role,
    hp_max: i32,
    hp: i32,
    atk: i32,
    def: i32,
    spd: i32,
    fluxo: i32,
    skill_cd: u32,
    mystery_cd: u32,
    interrupt_cd: u32,
    shield: i32,
    stun_turns: u8,
    silence_turns: u8,
    dot_dmg: i32,
    dot_turns: u8,
    dmg_reduction_permille: i32,
    reflect_permille: i32,
    bonus_vs_dot: i32,
    team_bonus_vs: Vec<(ComboState, i32)>,
    round_end_heal_pct: i32,
    round_start_fluxo: i32,
    dot_on_hit: bool,
    chase_dmg_bonus_permille: i32,
    chase_fluxo_bonus: i32,
    chased: bool,
    chase_used: u32,
    acted: bool,
    alive: bool,
    standard: StandardDef,
    skill: Tier,
    mystery: Tier,
    chase: crate::data::ChaseDef,
    mystery_unlock_round: usize,
    interrupt: Option<InterruptDef>,
}

impl IU {
    fn atk_eff(&self) -> i32 {
        self.atk
    }

    fn hp_pct(&self) -> i32 {
        if self.hp_max == 0 {
            return 0;
        }
        self.hp.max(0) * 100 / self.hp_max
    }
}

struct ISide {
    units: Vec<IU>,
    /// Ordem de ação da rodada corrente (slots por SPD desc).
    order: Vec<usize>,
    acted: [bool; 4],
    /// Bônus da primeira perseguição do time já usado nesta rodada.
    first_chase_used: bool,
}

impl ISide {
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
        (0..self.units.len()).filter(|&s| self.units[s].alive).collect()
    }
}

/// Batalha 4v4 interativa determinística.
pub struct InteractiveBattle {
    sides: [ISide; 2],
    rng: Rng,
    round: usize,
    first_starter: usize,
    starter: usize,
    /// Sequência de (lado, slot) da rodada corrente.
    timeline: Vec<(usize, usize)>,
    cursor: usize,
    round_active: bool,
    over: bool,
    outcome: Option<Outcome>,
    /// Eventos à espera de consumo (`drain` ou retorno de `execute_action`).
    outbox: Vec<BattleEvent>,
    /// Estados frescos à espera de perseguição: (lado_alvo, slot, estado).
    chase_queue: Vec<(usize, usize, ComboState)>,
    /// Lado dono da janela de chase corrente.
    chase_side: usize,
    /// R1: oferta corrente de táticas (2 índices no pool) — `Some` abre a
    /// fase [`PromptPhase::ChooseTactic`] para o jogador (lado 0).
    tactic_offer: Option<[usize; 2]>,
    /// Tática que o inimigo adotou em silêncio (índice no pool).
    enemy_tactic: Option<usize>,
    chain_links: u64,
    /// Elos de cadeia na rodada corrente (bônus do Eco de Combate).
    chain_links_round: u64,
    /// Revival da 1ª morte já usado por lado (Último Suspiro).
    revive_used: [bool; 2],
    /// Vantagens por lado (escala + cartas).
    edges: [SideEdge; 2],
    unmodeled: Vec<String>,
    /// Geração de decisão: incrementa a cada decisão consumida (a UI usa
    /// para NÃO resetar a escolha em andamento — feedback 2026-10-18 B1).
    decision_gen: u64,
}

impl InteractiveBattle {
    /// Monta a batalha interativa (`comp_a`/`comp_b` = 4 índices no roster).
    /// Alias de [`Self::new`] com o nome da especificação.
    pub fn start_interactive(roster: &Roster, comp_a: [usize; 4], comp_b: [usize; 4], seed: u64) -> Self {
        Self::new(roster, comp_a, comp_b, seed)
    }

    /// Monta a batalha interativa sem vantagens (rodadas neutras).
    /// Não sorteia nada ainda: a 1ª rodada abre de forma preguiçosa no
    /// primeiro `prompt`/`execute_action`.
    pub fn new(roster: &Roster, comp_a: [usize; 4], comp_b: [usize; 4], seed: u64) -> Self {
        Self::new_with_edges(roster, comp_a, comp_b, seed, SideEdge::default(), SideEdge::default())
    }

    /// Monta a batalha com vantagens por lado ([`SideEdge`]): escala de
    /// inimigos/boss + bônus de cartas da run. Determinismo preservado.
    pub fn new_with_edges(
        roster: &Roster,
        comp_a: [usize; 4],
        comp_b: [usize; 4],
        seed: u64,
        edge_a: SideEdge,
        edge_b: SideEdge,
    ) -> Self {
        let mut unmodeled: Vec<String> = Vec::new();
        let mk = |comp: [usize; 4], edge: &SideEdge, unmodeled: &mut Vec<String>| -> ISide {
            let units = comp
                .iter()
                .enumerate()
                .map(|(slot, &ci)| {
                    let last = roster.characters.len().saturating_sub(1);
                    let c: &RosterChar = &roster.characters[ci.min(last)];
                    let (u, notas) = Self::build_unit(c, edge, slot);
                    unmodeled.extend(notas);
                    u
                })
                .collect();
            ISide { units, order: Vec::new(), acted: [false; 4], first_chase_used: false }
        };
        let sides = [mk(comp_a, &edge_a, &mut unmodeled), mk(comp_b, &edge_b, &mut unmodeled)];
        InteractiveBattle {
            sides,
            edges: [edge_a, edge_b],
            rng: Rng::new(seed),
            round: 0,
            first_starter: 0,
            starter: 0,
            timeline: Vec::new(),
            cursor: 0,
            round_active: false,
            over: false,
            outcome: None,
            outbox: Vec::new(),
            chase_queue: Vec::new(),
            chase_side: 0,
            chain_links: 0,
            chain_links_round: 0,
            revive_used: [false; 2],
            tactic_offer: None,
            enemy_tactic: None,
            unmodeled,
            decision_gen: 0,
        }
    }

    /// Constrói uma unidade a partir do contrato + vantagem do lado.
    fn build_unit(c: &RosterChar, edge: &SideEdge, slot: usize) -> (IU, Vec<String>) {
        let mut unmodeled: Vec<String> = Vec::new();
        let low = c.passive.effect.to_lowercase();
        let (skill, mystery) = Tier::derive(&c.mystery, c.skill_targets);
        let condicional = low.contains("contra") || low.contains("ao ser") || low.contains("ao atingir");

        // +X% de dano incondicional (buff passivo permanente)
        let mut atk_buff_permille = 0;
        if !condicional && low.contains("dano") {
            if let Some(p) = pct_after(&low, "+") {
                atk_buff_permille += p * 10;
            }
        }
        // +X% de dano contra alvos sob DoT
        let mut bonus_vs_dot = 0;
        if low.contains("queimadura") || low.contains("veneno") {
            if let Some(p) = pct_after(&low, "+") {
                bonus_vs_dot = p * 10;
            }
        }
        // Aura de equipe vs estado recém-aplicado
        let mut team_bonus_vs: Vec<(ComboState, i32)> = Vec::new();
        for (kw, st) in [
            ("arremessados", ComboState::Launch),
            ("derrubados", ComboState::Knockdown),
            ("empurrados", ComboState::Knockback),
        ] {
            if low.contains(kw) && low.contains("recebem") {
                if let Some(p) = pct_after(&low, "recebem") {
                    team_bonus_vs.push((st, p * 10));
                }
            }
        }
        let mut dmg_reduction_permille = 0;
        if let Some(p) = pct_after(&low, "dano recebido") {
            dmg_reduction_permille = p * 10;
        }
        let mut reflect_permille = 0;
        if let Some(p) = pct_after(&low, "reflete") {
            reflect_permille = p * 10;
        }
        if let Some(p) = pct_after(&low, "contra-ataca") {
            reflect_permille = reflect_permille.max(p * 10);
        }
        let mut round_end_heal_pct = 0;
        if low.contains("fim da rodada") && low.contains("cura") {
            if let Some(p) = pct_after(&low, "cura") {
                round_end_heal_pct = p;
            }
        }
        let mut round_start_fluxo = 0;
        if low.contains("início da rodada") && low.contains("chakra") {
            // "+N de chakra" pode vir antes ou depois da palavra — aceitar ambos
            if let Some(n) = plus_after(&low, "gera").or_else(|| plus_after(&low, "chakra")) {
                round_start_fluxo = n;
            }
        }
        let dot_on_hit =
            low.contains("aplicam veneno") || low.contains("aplica veneno") || low.contains("queimadura") && low.contains("aplica");
        // Bônus da 1ª perseguição do time por rodada
        let mut chase_dmg_bonus_permille = 0;
        let mut chase_fluxo_bonus = 0;
        if low.contains("primeira perseguição") {
            if let Some(p) = pct_after(&low, "+") {
                chase_dmg_bonus_permille = p * 10;
            }
            if let Some(n) = plus_after(&low, "chakra") {
                chase_fluxo_bonus = n;
            }
        }
        // SPD com bônus passivo (+10% de velocidade)
        let spd = if low.contains("velocidade") {
            if let Some(p) = pct_after(&low, "+") {
                c.speed + c.speed * p / 100
            } else {
                c.speed
            }
        } else {
            c.speed
        };
        // Honestidade: o que ficou de fora, dito em voz alta
        for (kw, nota) in [
            ("linha de frente", "condição 'linha de frente' tratada como sempre ativa"),
            ("velocidade até o fim", "debuff de velocidade temporário não modelado"),
            ("causam -", "debuff condicional de dano inimigo não modelado"),
            ("ao fim de cada rodada enquanto existir", "escudo crescente por rodada não modelado"),
            ("age primeiro na rodada de abertura", "prioridade de abertura não modelada (SPD já cobre)"),
        ] {
            if low.contains(kw) {
                unmodeled.push(format!("{}: {}", c.name, nota));
            }
        }

        // Escala (inimigo/boss) e bônus de cartas, tudo inteiro:
        // base = round(atk × dmg_scale); final = base × (1000 + bônus)/1000.
        let hp_base = (c.hp.max(1) * edge.hp_scale_permille / PER_MILLE).max(1);
        let hp_final = (hp_base * (PER_MILLE + edge.hp_bonus_permille + edge.unit_hp[slot]) / PER_MILLE).max(1);
        let atk_base = (c.attack * edge.dmg_scale_permille / PER_MILLE).max(1);
        let atk_final = (atk_base * (PER_MILLE + edge.atk_bonus_permille + edge.unit_atk[slot]) / PER_MILLE)
            * (PER_MILLE + atk_buff_permille)
            / PER_MILLE;

        (
            IU {
                name: c.name.clone(),
                role: c.role,
                hp_max: hp_final,
                hp: hp_final,
                atk: atk_final,
                def: c.defense,
                spd,
                fluxo: 0,
                skill_cd: 0,
                mystery_cd: 0,
                interrupt_cd: 0,
                shield: 0,
                stun_turns: 0,
                silence_turns: 0,
                dot_dmg: 0,
                dot_turns: 0,
                dmg_reduction_permille,
                reflect_permille,
                bonus_vs_dot,
                team_bonus_vs,
                round_end_heal_pct,
                round_start_fluxo,
                dot_on_hit,
                chase_dmg_bonus_permille,
                chase_fluxo_bonus,
                chased: false,
                chase_used: 0,
                acted: false,
                alive: true,
                standard: c.standard.clone(),
                skill,
                mystery,
                chase: c.chase.clone(),
                mystery_unlock_round: c.mystery_unlock_round.saturating_sub(edge.unlock_early).max(1),
                interrupt: c.interrupt,
            },
            unmodeled,
        )
    }

    // ── Consultas ────────────────────────────────────────────────────────────

    /// Vez atual. Avança a máquina (pulando mortas/atordoadas, abrindo rodada)
    /// de forma preguiçosa; eventos gerados ficam em `outbox` — use [`Self::drain`].
    pub fn prompt(&mut self) -> TurnPrompt {
        if self.over {
            return TurnPrompt { side: 0, slot: 0, round: self.round, phase: PromptPhase::Over };
        }
        loop {
            if !self.round_active {
                self.open_round();
                if self.over {
                    return TurnPrompt { side: 0, slot: 0, round: self.round, phase: PromptPhase::Over };
                }
            }
            if let Some(_oferta) = self.tactic_offer {
                // R1: escolha de tática pendente — pausa o jogador ANTES da
                // rodada correr (o inimigo já adotou a dele em silêncio).
                let slot = self.current_entry().map(|(_, k)| k).unwrap_or(0);
                return TurnPrompt { side: 0, slot, round: self.round, phase: PromptPhase::ChooseTactic };
            }
            if !self.chase_queue.is_empty() {
                if self.chase_eligible().is_empty() {
                    // Ninguém pode perseguir: a janela fecha sozinha (transparente).
                    self.decision_gen += 1;
                    self.reject_chase(self.chase_side);
                    continue;
                }
                let slot = self.current_entry().map(|(_, k)| k).unwrap_or(0);
                return TurnPrompt { side: self.chase_side, slot, round: self.round, phase: PromptPhase::ConfirmChase };
            }
            self.advance_cursor();
            if self.over {
                return TurnPrompt { side: 0, slot: 0, round: self.round, phase: PromptPhase::Over };
            }
            if let Some((side, slot)) = self.current_entry() {
                return TurnPrompt { side, slot, round: self.round, phase: PromptPhase::ChooseAction };
            }
            // cursor no fim da rodada: `advance_cursor` fechou; o loop abre a próxima
        }
    }

    /// Consumo dos eventos pendentes (abertura de rodada, pulos etc.).
    pub fn drain(&mut self) -> Vec<BattleEvent> {
        std::mem::take(&mut self.outbox)
    }

    /// Sequência completa de ações da rodada corrente (lado, slot).
    pub fn timeline(&self) -> Vec<(usize, usize)> {
        self.timeline.clone()
    }

    /// Posição corrente na timeline (índice em [`Self::timeline`]).
    pub fn timeline_cursor(&self) -> usize {
        self.cursor
    }

    /// Rodada corrente.
    pub fn round(&self) -> usize {
        self.round
    }

    /// Elos de perseguição executados.
    pub fn chain_links(&self) -> u64 {
        self.chain_links
    }

    /// Passivas/efeitos não modelados (diagnóstico honesto).
    pub fn unmodeled(&self) -> &[String] {
        &self.unmodeled
    }

    /// Batalha concluída?
    pub fn is_over(&self) -> bool {
        self.over
    }

    /// Resultado final, quando concluída.
    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    /// Lado que abre a rodada corrente (0 = A, 1 = B).
    pub fn starter(&self) -> usize {
        self.starter
    }

    /// Geração de decisão: incrementa a cada decisão consumida (ação,
    /// chase, recusa, skip). Estável enquanto a MESMA vez continua — a UI
    /// compara antes de reencher o painel para não resetar a escolha
    /// em andamento do jogador (feedback 2026-10-18 B1).
    pub fn generation(&self) -> u64 {
        self.decision_gen
    }

    /// Retrato imutável de uma unidade.
    pub fn unit(&self, side: usize, slot: usize) -> UnitSnapshot {
        let u = &self.sides[side].units[slot];
        UnitSnapshot {
            name: u.name.clone(),
            role: u.role,
            hp: u.hp.max(0),
            hp_max: u.hp_max,
            shield: u.shield,
            fluxo: u.fluxo,
            alive: u.alive,
            stun_turns: u.stun_turns,
            silence_turns: u.silence_turns,
            dot_turns: u.dot_turns,
            skill_cd: u.skill_cd,
            mystery_cd: u.mystery_cd,
            interrupt_cd: u.interrupt_cd,
            acted: self.sides[side].acted[slot],
            standard_name: u.standard.name.clone(),
            skill_name: u.skill.name.clone(),
            mystery_name: u.mystery.name.clone(),
            mystery_unlock_round: u.mystery_unlock_round,
            interrupt: u.interrupt,
            chased: u.chased,
        }
    }

    /// Ações legais da unidade dada (independe de ser a vez — a UI usa isto
    /// para pré-carregar botões; a execução valida a vez à parte).
    pub fn legal_actions(&self, side: usize, slot: usize) -> Vec<LegalAction> {
        if self.over || side > 1 || slot >= 4 || !self.sides[side].units[slot].alive {
            return Vec::new();
        }
        if !self.chase_queue.is_empty() {
            // Janela de chase: confirmar com um perseguidor elegível…
            let mut out: Vec<LegalAction> = self
                .chase_eligible()
                .into_iter()
                .map(|s| LegalAction {
                    kind: ActionKind::Skill, // marcador de categoria chase p/ UI
                    fluxo_cost: 0,
                    targeting: TargetKind::Auto,
                    name: format!("Perseguir com {}", self.sides[side].units[s].chase.name),
                })
                .collect();
            // …ou recusar (tática válida).
            out.push(LegalAction {
                kind: ActionKind::Standard,
                fluxo_cost: 0,
                targeting: TargetKind::Auto,
                name: "Recusar perseguição".to_string(),
            });
            return out;
        }
        let u = &self.sides[side].units[slot];
        let mut out = Vec::new();
        out.push(LegalAction {
            kind: ActionKind::Standard,
            fluxo_cost: 0,
            // U1 (feedback 2026-10-18): padrão é de alvo automático — a UI
            // executa direto, sem fila de alvos (básico mira o espelhado).
            targeting: TargetKind::Auto,
            name: u.standard.name.clone(),
        });
        if u.silence_turns == 0 && u.skill_cd == 0 && u.fluxo >= SKILL_COST {
            let targeting = match u.skill.targets {
                TargetSpec::AllySingle => TargetKind::Ally,
                TargetSpec::EnemySingle => TargetKind::Enemy,
                TargetSpec::SelfTarget | TargetSpec::EnemiesFront | TargetSpec::EnemiesAll | TargetSpec::Random2 => {
                    TargetKind::Auto
                }
            };
            out.push(LegalAction {
                kind: ActionKind::Skill,
                fluxo_cost: SKILL_COST,
                targeting,
                name: u.skill.name.clone(),
            });
        }
        if u.silence_turns == 0
            && u.mystery_cd == 0
            && u.fluxo >= MYSTERY_COST
            && self.round >= u.mystery_unlock_round
        {
            let targeting = match u.mystery.targets {
                TargetSpec::AllySingle => TargetKind::Ally,
                TargetSpec::EnemySingle => TargetKind::Enemy,
                TargetSpec::SelfTarget | TargetSpec::EnemiesFront | TargetSpec::EnemiesAll | TargetSpec::Random2 => {
                    TargetKind::Auto
                }
            };
            out.push(LegalAction {
                kind: ActionKind::Mystery,
                fluxo_cost: MYSTERY_COST,
                targeting,
                name: u.mystery.name.clone(),
            });
        }
        if let Some(def) = u.interrupt {
            if u.interrupt_cd == 0 && u.fluxo >= INTERRUPT_COST {
                let enemy = 1 - side;
                let janela_aberta =
                    (0..4).any(|s| self.sides[enemy].units[s].alive && !self.sides[enemy].acted[s]);
                if janela_aberta {
                    let nome = if def.kind == crate::data::InterruptKind::Stun {
                        "Interromper (Atordoar)"
                    } else {
                        "Interromper (Silenciar)"
                    };
                    out.push(LegalAction {
                        kind: ActionKind::Interrupt,
                        fluxo_cost: INTERRUPT_COST,
                        targeting: TargetKind::Enemy,
                        name: nome.to_string(),
                    });
                }
            }
        }
        out
    }

    /// Slots (do lado `side`) que podem perseguir agora — para o highlight
    /// dourado da UI e para IAs.
    pub fn chase_eligible_slots(&self, side: usize) -> Vec<usize> {
        if self.chase_queue.is_empty() || side != self.chase_side {
            return Vec::new();
        }
        self.chase_eligible()
    }

    /// Oferta corrente de táticas (2 índices no [`crate::rewards::TACTIC_POOL`])
    /// — `None` fora da janela de escolha (fase [`PromptPhase::ChooseTactic`]).
    pub fn tactic_offer(&self) -> Option<[usize; 2]> {
        self.tactic_offer
    }

    /// Índice no pool da tática que o inimigo adotou em silêncio (`None`
    /// até a primeira oferta). Diagnóstico/testes — a UI NÃO exibe
    /// (feedback R1 pediu decisão oculta do adversário).
    pub fn enemy_tactic_pick(&self) -> Option<usize> {
        self.enemy_tactic
    }

    /// Adota uma tática de batalha (R1 do feedback): efeito de escopo da
    /// batalha aplicado na hora; emite [`BattleEvent::TacticPicked`].
    fn apply_tactic(&mut self, side: usize, idx_pool: usize) {
        let pool = &crate::rewards::TACTIC_POOL;
        let c = &pool[idx_pool.min(pool.len() - 1)];
        match c.id {
            "t-ataque" => {
                for u in &mut self.sides[side].units {
                    if u.alive {
                        u.atk = u.atk * (PER_MILLE + c.team_atk) / PER_MILLE;
                    }
                }
            }
            "t-pele" => {
                for u in &mut self.sides[side].units {
                    if u.alive {
                        u.dmg_reduction_permille = (u.dmg_reduction_permille + c.team_dmg_reduction).min(300);
                    }
                }
            }
            "t-fluxo" => {
                self.edges[side].fluxo_per_round += c.fluxo_per_round;
            }
            "t-cura" => {
                for u in &mut self.sides[side].units {
                    if u.alive {
                        u.round_end_heal_pct += c.round_end_heal_pct;
                    }
                }
            }
            _ => {}
        }
        self.outbox.push(BattleEvent::TacticPicked { side, tactic: c.name.to_string() });
    }

    /// Slots inimigos que ainda vão agiar nesta rodada (janela de interrupt).
    pub fn interrupt_targets(&self, side: usize) -> Vec<usize> {
        let enemy = 1 - side;
        (0..4).filter(|&s| self.sides[enemy].units[s].alive && !self.sides[enemy].acted[s]).collect()
    }

    // ── Execução ─────────────────────────────────────────────────────────────

    /// Pré-visualização exata da ação dada (não consome RNG, não muta
    /// estado). Os números usam os MESMOS helpers da execução — e os erros
    /// são os mesmos que `execute_action` devolveria.
    pub fn preview_action(
        &self,
        side: usize,
        slot: usize,
        action: ChosenAction,
    ) -> Result<ActionPreview, ActionError> {
        if self.over {
            return Err(ActionError::BattleOver);
        }
        let ChosenAction::Act { kind, target } = action else {
            // Fases de chase: prévia do elo de perseguição (recusa = zero tudo).
            if self.chase_queue.is_empty() {
                return Err(ActionError::WrongPhase);
            }
            let ChosenAction::Chase { chaser_slot } = action else {
                return Ok(ActionPreview {
                    damage: 0,
                    heal: 0,
                    shield: 0,
                    states: Vec::new(),
                    fluxo_delta: 0,
                    targets: Vec::new(),
                    notes: vec!["recusa: nenhum elo executado".to_string()],
                });
            };
            let Some(&(enemy, tslot, st)) = self.chase_queue.first() else {
                return Err(ActionError::WrongPhase);
            };
            let elegivel = self
                .chase_eligible()
                .into_iter()
                .any(|s| s == chaser_slot && self.sides[self.chase_side].units[chaser_slot].chase.trigger == st);
            if !elegivel {
                return Err(ActionError::BadTarget);
            }
            let states = {
                let ap = self.sides[self.chase_side].units[chaser_slot].chase.applies_state;
                if ap != ComboState::None && self.sides[enemy].units[tslot].alive {
                    vec![(enemy, tslot, ap)]
                } else {
                    Vec::new()
                }
            };
            return Ok(ActionPreview {
                damage: self.chase_damage(self.chase_side, chaser_slot, enemy, tslot),
                heal: 0,
                shield: 0,
                states,
                fluxo_delta: 0,
                targets: vec![(enemy, tslot)],
                notes: Vec::new(),
            });
        };
        if !self.chase_queue.is_empty() {
            return Err(ActionError::WrongPhase);
        }
        let u = &self.sides[side].units[slot];
        let mut notes: Vec<String> = Vec::new();
        match kind {
            ActionKind::Standard => {
                let Some(tslot) = self.standard_target(side, slot, target) else {
                    return Err(ActionError::BadTarget);
                };
                if let Some(t) = target {
                    if t != tslot {
                        notes.push("alvo escolhido morto: caiu no auto-alvo (primeiro vivo)".to_string());
                    }
                }
                let states = {
                    let ap = u.standard.applies_state;
                    if ap != ComboState::None {
                        vec![(1 - side, tslot, ap)]
                    } else {
                        Vec::new()
                    }
                };
                Ok(ActionPreview {
                    damage: self.standard_damage(side, slot, tslot),
                    heal: 0,
                    shield: 0,
                    states,
                    fluxo_delta: FLUXO_STD_GAIN,
                    targets: vec![(1 - side, tslot)],
                    notes,
                })
            }
            ActionKind::Skill | ActionKind::Mystery => {
                let (tier, custo, cd) = if kind == ActionKind::Skill {
                    (&u.skill, SKILL_COST, u.skill_cd)
                } else {
                    (&u.mystery, MYSTERY_COST, u.mystery_cd)
                };
                if u.silence_turns > 0 {
                    return Err(ActionError::LockedOut);
                }
                if cd > 0 {
                    return Err(ActionError::OnCooldown);
                }
                if kind == ActionKind::Mystery && self.round < u.mystery_unlock_round {
                    return Err(ActionError::IllegalAction);
                }
                if u.fluxo < custo {
                    return Err(ActionError::NotEnoughFluxo);
                }
                if tier.targets == TargetSpec::AllySingle || tier.targets == TargetSpec::SelfTarget {
                    let tslot = self.tier_ally_slot(side, tier, target, slot);
                    let (_, shield) = self.tier_support_value(side, slot, tier);
                    let falta = (self.sides[side].units[tslot].hp_max - self.sides[side].units[tslot].hp).max(0);
                    let heal_real = self.sides[side].units[slot].atk_eff() * tier.heal_pct / 100;
                    let heal_real = heal_real.min(falta);
                    Ok(ActionPreview {
                        damage: 0,
                        heal: heal_real,
                        shield,
                        states: Vec::new(),
                        fluxo_delta: -custo,
                        targets: vec![(side, tslot)],
                        notes,
                    })
                } else {
                    let slots = self.tier_enemy_slots(side, tier, target);
                    if tier.targets == TargetSpec::Random2 {
                        notes.push(format!(
                            "alvos aleatórios: 2 sorteados entre {} vivos (prévia lista candidatos)",
                            slots.len()
                        ));
                    }
                    if tier.targets == TargetSpec::EnemySingle && target.is_none() {
                        notes.push("alvo automático: primeiro vivo da frente (alvo manual disponível)".to_string());
                    }
                    let damage: i32 = slots.iter().map(|&ts| self.tier_damage(side, slot, tier, ts)).sum();
                    let states: Vec<(usize, usize, ComboState)> = slots
                        .iter()
                        .filter(|&_ts| tier.applies != ComboState::None)
                        .map(|&ts| (1 - side, ts, tier.applies))
                        .collect();
                    let targets: Vec<(usize, usize)> = slots.iter().map(|&ts| (1 - side, ts)).collect();
                    Ok(ActionPreview { damage, heal: 0, shield: 0, states, fluxo_delta: -custo, targets, notes })
                }
            }
            ActionKind::Interrupt => {
                let Some(def) = u.interrupt else {
                    return Err(ActionError::IllegalAction);
                };
                if u.interrupt_cd > 0 {
                    return Err(ActionError::OnCooldown);
                }
                if u.fluxo < INTERRUPT_COST {
                    return Err(ActionError::NotEnoughFluxo);
                }
                let t = target.ok_or(ActionError::BadTarget)?;
                let enemy = 1 - side;
                if t >= 4 || !self.sides[enemy].units[t].alive || self.sides[enemy].acted[t] {
                    return Err(ActionError::BadTarget);
                }
                let _ = def;
                Ok(ActionPreview {
                    damage: 0,
                    heal: 0,
                    shield: 0,
                    states: Vec::new(),
                    fluxo_delta: -INTERRUPT_COST,
                    targets: vec![(enemy, t)],
                    notes,
                })
            }
        }
    }

    /// Executa UMA decisão atômica e devolve os eventos gerados.
    pub fn execute_action(&mut self, action: ChosenAction) -> Result<Vec<BattleEvent>, ActionError> {
        if self.over {
            return Err(ActionError::BattleOver);
        }
        if !self.round_active {
            self.open_round();
            if self.over {
                return Ok(std::mem::take(&mut self.outbox));
            }
        }
        if !self.chase_queue.is_empty() {
            let side = self.chase_side;
            match action {
                ChosenAction::Chase { chaser_slot } => self.run_chase(side, chaser_slot)?,
                ChosenAction::RejectChase => self.reject_chase(side),
                ChosenAction::Act { .. } | ChosenAction::PickTactic { .. } => {
                    return Err(ActionError::WrongPhase)
                }
            }
            self.decision_gen += 1;
            return Ok(std::mem::take(&mut self.outbox));
        }
        if let Some(oferta) = self.tactic_offer {
            let ChosenAction::PickTactic { index } = action else {
                return Err(ActionError::WrongPhase);
            };
            if index >= oferta.len() {
                return Err(ActionError::IllegalAction);
            }
            self.apply_tactic(0, oferta[index]);
            self.tactic_offer = None;
            self.decision_gen += 1;
            return Ok(std::mem::take(&mut self.outbox));
        }
        let (side, slot) = match self.current_entry() {
            Some(p) => p,
            None => return Err(ActionError::NotYourTurn),
        };
        let ChosenAction::Act { kind, target } = action else {
            return Err(ActionError::WrongPhase);
        };
        self.validate(side, slot, kind, target)?;
        self.decision_gen += 1;
        match kind {
            ActionKind::Standard => self.do_standard(side, slot, target),
            ActionKind::Skill => self.do_tier(side, slot, &self.sides[side].units[slot].skill.clone(), SKILL_COST, SKILL_CD, target),
            ActionKind::Mystery => {
                let tier = self.sides[side].units[slot].mystery.clone();
                self.do_tier(side, slot, &tier, MYSTERY_COST, MYSTERY_CD, target);
            }
            ActionKind::Interrupt => self.do_interrupt(side, slot, target.expect("validado"))?,
        }
        self.sides[side].acted[slot] = true;
        if self.chase_queue.is_empty() {
            // Turno consumido sem janela de perseguição.
            self.cursor += 1;
        }
        self.finish_if_defeated();
        Ok(std::mem::take(&mut self.outbox))
    }

    /// Validação defensiva (a UI previne; a engine recusa com erro claro).
    fn validate(&self, side: usize, slot: usize, kind: ActionKind, target: Option<usize>) -> Result<(), ActionError> {
        let u = &self.sides[side].units[slot];
        if !u.alive {
            return Err(ActionError::NotYourTurn);
        }
        match kind {
            ActionKind::Standard => {
                if let Some(t) = target {
                    if t >= 4 || !self.sides[1 - side].units[t].alive {
                        return Err(ActionError::BadTarget);
                    }
                }
            }
            ActionKind::Skill | ActionKind::Mystery => {
                let tier = if kind == ActionKind::Skill { &u.skill } else { &u.mystery };
                if kind == ActionKind::Skill {
                    if u.silence_turns > 0 {
                        return Err(ActionError::LockedOut);
                    }
                    if u.skill_cd > 0 {
                        return Err(ActionError::OnCooldown);
                    }
                } else {
                    if u.silence_turns > 0 {
                        return Err(ActionError::LockedOut);
                    }
                    if u.mystery_cd > 0 {
                        return Err(ActionError::OnCooldown);
                    }
                    if self.round < u.mystery_unlock_round {
                        return Err(ActionError::IllegalAction);
                    }
                }
                if u.fluxo < SKILL_COST && kind == ActionKind::Skill {
                    return Err(ActionError::NotEnoughFluxo);
                }
                if u.fluxo < MYSTERY_COST && kind == ActionKind::Mystery {
                    return Err(ActionError::NotEnoughFluxo);
                }
                if tier.targets == TargetSpec::AllySingle {
                    if let Some(t) = target {
                        if t >= 4 || !self.sides[side].units[t].alive {
                            return Err(ActionError::BadTarget);
                        }
                    }
                }
                // v0.8: alvo único inimigo — explícito deve estar vivo;
                // `None` cai no auto (primeiro vivo da frente).
                if tier.targets == TargetSpec::EnemySingle {
                    if let Some(t) = target {
                        if t >= 4 || !self.sides[1 - side].units[t].alive {
                            return Err(ActionError::BadTarget);
                        }
                    }
                }
            }
            ActionKind::Interrupt => {
                let Some(def) = u.interrupt else {
                    return Err(ActionError::IllegalAction);
                };
                if u.interrupt_cd > 0 {
                    return Err(ActionError::OnCooldown);
                }
                if u.fluxo < INTERRUPT_COST {
                    return Err(ActionError::NotEnoughFluxo);
                }
                let t = target.ok_or(ActionError::BadTarget)?;
                let enemy = 1 - side;
                if t >= 4 || !self.sides[enemy].units[t].alive || self.sides[enemy].acted[t] {
                    return Err(ActionError::BadTarget);
                }
                let _ = def;
            }
        }
        Ok(())
    }

    // ── Rodada ───────────────────────────────────────────────────────────────

    fn open_round(&mut self) {
        if self.sides[0].defeated() || self.sides[1].defeated() {
            self.finish();
            return;
        }
        if self.round == 0 {
            self.first_starter = self.rng.range(2);
        }
        self.round += 1;
        self.starter = (self.first_starter + self.round - 1) % 2;
        self.outbox.push(BattleEvent::RoundStart { round: self.round, starter: self.starter });
        self.round_active = true;
        self.chase_queue.clear();

        // Início de rodada: fluxo, cooldowns, pulso de DoT, reset de chase.
        self.chain_links_round = 0;
        for si in 0..2 {
            self.sides[si].first_chase_used = false;
            for slot in 0..4 {
                let u = &mut self.sides[si].units[slot];
                if !u.alive {
                    continue;
                }
                u.chased = false;
                u.chase_used = 0;
                u.acted = false;
                u.skill_cd = u.skill_cd.saturating_sub(1);
                u.mystery_cd = u.mystery_cd.saturating_sub(1);
                u.interrupt_cd = u.interrupt_cd.saturating_sub(1);
                let ganho =
                    FLUXO_PER_ROUND + self.edges[si].fluxo_per_round + u.round_start_fluxo.min(FLUXO_PASSIVE_CAP);
                u.fluxo = (u.fluxo + ganho).min(FLUXO_CAP);
                self.outbox.push(BattleEvent::FluxoGain { side: si, slot, amount: ganho });
                if u.dot_turns > 0 {
                    u.dot_turns -= 1;
                    let d = pace(u.dot_dmg);
                    if d > 0 {
                        let t = &mut self.sides[si].units[slot];
                        t.hp -= d;
                        let mut killed = false;
                        if t.hp <= 0 {
                            t.hp = 0;
                            t.alive = false;
                            killed = true;
                        }
                        self.outbox.push(BattleEvent::Damage { side: si, slot, amount: d, killed });
                    }
                }
            }
        }
        self.finish_if_defeated();
        if self.over {
            return;
        }

        // R1 (feedback 2026-10-18): a cada 2 rodadas (2, 4, 6, 8) abre
        // oferta de tática para o jogador; o inimigo adota a dele EM
        // SILÊNCIO — simetria de poder, zero UI. Efeito de escopo da
        // batalha: não entra no teto de poder da run.
        if self.round.is_multiple_of(TACTIC_INTERVAL) && self.round + TACTIC_INTERVAL <= MAX_ROUNDS {
            let pool = crate::rewards::TACTIC_POOL.len();
            let a = self.rng.range(pool);
            let mut b = self.rng.range(pool - 1);
            if b >= a {
                b += 1;
            }
            let inimigo = self.rng.range(pool);
            self.enemy_tactic = Some(inimigo);
            self.apply_tactic(1, inimigo);
            self.tactic_offer = Some([a, b]);
        }

        // Timeline: SPD desc dentro do lado, intercalada A1→B1→A2→B2…
        self.sides[0].order = self.spd_order(0);
        self.sides[1].order = self.spd_order(1);
        self.timeline.clear();
        let (first, second) = (self.starter, 1 - self.starter);
        let len = self.sides[0].order.len().max(self.sides[1].order.len());
        for i in 0..len {
            if let Some(&s) = self.sides[first].order.get(i) {
                self.timeline.push((first, s));
            }
            if let Some(&s) = self.sides[second].order.get(i) {
                self.timeline.push((second, s));
            }
        }
        self.cursor = 0;
    }

    fn spd_order(&self, side: usize) -> Vec<usize> {
        let mut slots: Vec<usize> = self.sides[side].living_slots();
        slots.sort_by(|&a, &b| {
            self.sides[side].units[b].spd.cmp(&self.sides[side].units[a].spd).then(a.cmp(&b))
        });
        slots
    }

    fn current_entry(&self) -> Option<(usize, usize)> {
        self.timeline.get(self.cursor).copied()
    }

    /// Avança o cursor pulando mortas e atordoadas (eventos de skip).
    fn advance_cursor(&mut self) {
        loop {
            if self.round >= MAX_ROUNDS && self.cursor >= self.timeline.len() {
                self.finish();
                return;
            }
            let Some((side, slot)) = self.current_entry() else {
                self.close_round();
                if self.over {
                    return;
                }
                if !self.round_active {
                    return; // open_round já deixou a próxima rodada pronta
                }
                continue;
            };
            let u = &self.sides[side].units[slot];
            if !u.alive {
                self.decision_gen += 1;
                self.outbox.push(BattleEvent::Skip { side, slot, reason: SkipReason::Dead });
                self.cursor += 1;
                continue;
            }
            if u.stun_turns > 0 {
                self.sides[side].units[slot].stun_turns -= 1;
                self.sides[side].acted[slot] = true;
                self.decision_gen += 1;
                self.outbox.push(BattleEvent::Skip { side, slot, reason: SkipReason::Stunned });
                self.cursor += 1;
                continue;
            }
            return;
        }
    }

    fn close_round(&mut self) {
        // Fim de rodada: curas de passiva (o aliado com menos HP%).
        for si in 0..2 {
            let pcts: Vec<i32> =
                self.sides[si].units.iter().map(|u| if u.alive { u.round_end_heal_pct } else { 0 }).collect();
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
                    self.outbox.push(BattleEvent::Heal { side: si, slot: best, amount: heal });
                }
            }
        }
        self.outbox.push(BattleEvent::RoundEnd { round: self.round });
        self.round_active = false;
        self.chase_queue.clear();
        if self.round >= MAX_ROUNDS || self.sides[0].defeated() || self.sides[1].defeated() {
            self.finish();
        }
    }

    fn finish_if_defeated(&mut self) {
        if self.sides[0].defeated() || self.sides[1].defeated() {
            self.finish();
        }
    }

    /// Registra o resultado (teto de rodadas → desempate por % de HP).
    fn finish(&mut self) {
        if self.outcome.is_some() {
            self.over = true;
            return;
        }
        let outcome = if self.sides[1].defeated() {
            Outcome::TeamAWin
        } else if self.sides[0].defeated() {
            Outcome::TeamBWin
        } else {
            match self.sides[0].total_hp_pct().cmp(&self.sides[1].total_hp_pct()) {
                std::cmp::Ordering::Greater => Outcome::TeamAWin,
                std::cmp::Ordering::Less => Outcome::TeamBWin,
                std::cmp::Ordering::Equal => Outcome::Draw,
            }
        };
        self.outbox.push(BattleEvent::BattleOver { outcome });
        self.outcome = Some(outcome);
        self.over = true;
        self.round_active = false;
    }

    // ── Ações ────────────────────────────────────────────────────────────────

    // ── Cálculos puros (compartilhados por execução e pré-visualização) ─────

    /// Slot inimigo resolvido do ataque padrão (`None` = sem vivos).
    /// U1 (feedback 2026-10-18): o padrão mira o slot ESPELHADO (1↔1,
    /// 2↔2…); se o espelhado caiu, ataca o primeiro vivo da frente para
    /// trás — sem seleção manual na UI (alvo automático).
    fn standard_target(&self, side: usize, slot: usize, target: Option<usize>) -> Option<usize> {
        let enemy = 1 - side;
        if let Some(t) = target {
            if t < 4 && self.sides[enemy].units[t].alive {
                return Some(t);
            }
        }
        if self.sides[enemy].units[slot].alive {
            return Some(slot);
        }
        self.sides[enemy].living_slots().first().copied()
    }

    /// Dano final do padrão no alvo (com bônus e mitigações), sem mutação.
    fn standard_damage(&self, side: usize, slot: usize, tslot: usize) -> i32 {
        let u = &self.sides[side].units[slot];
        let (tdef, t_red, t_dot) = {
            let t = &self.sides[1 - side].units[tslot];
            (t.def, t.dmg_reduction_permille, t.dot_turns > 0)
        };
        let mut dmg = pace(crate::combat::damage_after_def(u.atk_eff(), u.standard.damage_pct, tdef));
        if t_dot && u.bonus_vs_dot > 0 {
            dmg = dmg * (PER_MILLE + u.bonus_vs_dot) / PER_MILLE;
        }
        if u.standard.applies_state != ComboState::None {
            if let Some((_, p)) = u.team_bonus_vs.iter().find(|(st, _)| *st == u.standard.applies_state) {
                dmg = dmg * (PER_MILLE + p) / PER_MILLE;
            }
        }
        if t_red > 0 {
            dmg = dmg * (PER_MILLE - t_red) / PER_MILLE;
        }
        dmg
    }

    /// Slots inimigos efetivos de um tier (determinístico exceto Random2).
    /// `target` é o alvo explícito de `EnemySingle` (None = auto frente).
    fn tier_enemy_slots(&self, side: usize, tier: &Tier, target: Option<usize>) -> Vec<usize> {
        let enemy = 1 - side;
        let mut slots: Vec<usize> = match tier.targets {
            TargetSpec::EnemiesFront => {
                let living = self.sides[enemy].living_slots();
                let front: Vec<usize> = living.iter().copied().take(2).collect();
                if front.is_empty() { living } else { front }
            }
            TargetSpec::EnemiesAll => self.sides[enemy].living_slots(),
            // v0.8: alvo único — o escolhido, ou o primeiro vivo da frente.
            TargetSpec::EnemySingle => {
                let escolhido = target.filter(|&t| t < 4 && self.sides[enemy].units[t].alive);
                match escolhido {
                    Some(t) => vec![t],
                    None => self.sides[enemy].living_slots().into_iter().take(1).collect(),
                }
            }
            // Random2 é resolvido na execução (RNG); prévia lista candidatos
            TargetSpec::Random2 => self.sides[enemy].living_slots(),
            _ => Vec::new(),
        };
        slots.retain(|&s| self.sides[enemy].units[s].alive);
        slots
    }

    /// Slot aliado efetivo de um tier (auto = o mais ferido).
    fn tier_ally_slot(&self, side: usize, tier: &Tier, target: Option<usize>, slot: usize) -> usize {
        if tier.targets == TargetSpec::SelfTarget {
            slot
        } else {
            match target {
                Some(t) if t < 4 && self.sides[side].units[t].alive => t,
                _ => self.most_wounded_ally(side),
            }
        }
    }

    /// Dano final do tier em um alvo (com perfuração/aura/mitigação).
    fn tier_damage(&self, side: usize, slot: usize, tier: &Tier, tslot: usize) -> i32 {
        let (tdef, t_red) = {
            let t = &self.sides[1 - side].units[tslot];
            (t.def, t.dmg_reduction_permille)
        };
        let eff_def = tdef * (100 - tier.pierce_def_pct) / 100;
        let mut dmg = pace(crate::combat::damage_after_def(
            self.sides[side].units[slot].atk_eff(),
            tier.dmg_pct,
            eff_def,
        ));
        if tier.applies != ComboState::None {
            if let Some((_, p)) = self.sides[side].units[slot]
                .team_bonus_vs
                .iter()
                .find(|(st, _)| *st == tier.applies)
            {
                dmg = dmg * (PER_MILLE + p) / PER_MILLE;
            }
        }
        if t_red > 0 {
            dmg = dmg * (PER_MILLE - t_red) / PER_MILLE;
        }
        dmg
    }

    /// Cura/escudo do tier em aliado (valor bruto; cap de HP é da execução).
    fn tier_support_value(&self, side: usize, slot: usize, tier: &Tier) -> (i32, i32) {
        let atk = self.sides[side].units[slot].atk_eff();
        (atk * tier.heal_pct / 100, atk * tier.shield_pct / 100)
    }

    /// Dano do elo de perseguição: bônus da 1ª perseguição do time (passiva)
    /// + Eco de Combate (+N‰ por elo da cadeia na rodada, com teto).
    fn chase_damage(&self, side: usize, chaser_slot: usize, enemy: usize, tslot: usize) -> i32 {
        let c = &self.sides[side].units[chaser_slot];
        let tdef = self.sides[enemy].units[tslot].def;
        let primeiro_elo = !self.sides[side].first_chase_used;
        let mut bonus = if primeiro_elo { c.chase_dmg_bonus_permille } else { 0 };
        let eco = self.edges[side].chain_dmg_per_link * (self.chain_links_round as i32);
        bonus += eco.min(self.edges[side].chain_dmg_cap);
        let mut dmg = pace(crate::combat::damage_after_def(c.atk_eff(), c.chase.damage_pct, tdef));
        if bonus > 0 {
            dmg = dmg * (PER_MILLE + bonus) / PER_MILLE;
        }
        dmg
    }

    fn do_standard(&mut self, side: usize, slot: usize, target: Option<usize>) {
        let (sname, _dmg_pct, applies) = {
            let u = &self.sides[side].units[slot];
            (u.standard.name.clone(), u.standard.damage_pct, u.standard.applies_state)
        };
        let Some(tslot) = self.standard_target(side, slot, target) else {
            return;
        };
        self.outbox.push(BattleEvent::ActionDeclared {
            side,
            slot,
            kind: ActionKind::Standard,
            name: sname,
            target: Some((1 - side, tslot)),
        });
        let dmg = self.standard_damage(side, slot, tslot);
        let atk = self.sides[side].units[slot].atk_eff();
        let dot_on_hit = self.sides[side].units[slot].dot_on_hit;
        if dot_on_hit {
            let t = &mut self.sides[1 - side].units[tslot];
            if t.alive {
                let novo = atk * DOT_ATK_PCT / 100;
                if t.dot_turns == 0 || novo >= t.dot_dmg {
                    t.dot_dmg = novo;
                }
                t.dot_turns = DOT_TURNS;
            }
        }
        self.deal_damage(1 - side, tslot, dmg, 0);
        if applies != ComboState::None && self.sides[1 - side].units[tslot].alive {
            self.chase_queue.push((1 - side, tslot, applies));
            self.outbox.push(BattleEvent::StateApplied { side: 1 - side, slot: tslot, state: applies });
        }
        // Reflexo do alvo (só em ataque padrão)
        let refl = self.sides[1 - side].units[tslot].reflect_permille;
        if refl > 0 && self.sides[side].units[slot].alive {
            let back = dmg * refl / PER_MILLE;
            self.deal_damage(side, slot, back, 0);
        }
        // Ataque padrão gera fluxo
        self.gain_fluxo(side, slot, FLUXO_STD_GAIN);
        if !self.chase_queue.is_empty() {
            self.open_chase_window(side);
        }
    }

    fn do_tier(
        &mut self,
        side: usize,
        slot: usize,
        tier: &Tier,
        cost: i32,
        cd: u32,
        target: Option<usize>,
    ) {
        self.outbox.push(BattleEvent::ActionDeclared {
            side,
            slot,
            kind: if cost == SKILL_COST { ActionKind::Skill } else { ActionKind::Mystery },
            name: tier.name.clone(),
            target: None,
        });
        self.sides[side].units[slot].fluxo -= cost;
        if cost == SKILL_COST {
            self.sides[side].units[slot].skill_cd = cd.max(1);
        } else {
            self.sides[side].units[slot].mystery_cd = cd.max(1);
        }
        let mut queue: Vec<(usize, usize, ComboState)> = Vec::new();

        if tier.team_fluxo > 0 {
            for s in 0..4 {
                if self.sides[side].units[s].alive {
                    self.gain_fluxo(side, s, tier.team_fluxo);
                }
            }
        }

        match tier.targets {
            TargetSpec::AllySingle | TargetSpec::SelfTarget => {
                let tslot = self.tier_ally_slot(side, tier, target, slot);
                let (heal, shield) = self.tier_support_value(side, slot, tier);
                if heal > 0 {
                    let t = &mut self.sides[side].units[tslot];
                    t.hp = (t.hp + heal).min(t.hp_max);
                    self.outbox.push(BattleEvent::Heal { side, slot: tslot, amount: heal });
                }
                if shield > 0 {
                    self.sides[side].units[tslot].shield += shield;
                    self.outbox.push(BattleEvent::Shield { side, slot: tslot, amount: shield });
                }
                if tier.reflect_pct > 0 {
                    self.sides[side].units[tslot].reflect_permille = tier.reflect_pct * 10;
                }
            }
            TargetSpec::EnemiesFront | TargetSpec::EnemiesAll | TargetSpec::EnemySingle | TargetSpec::Random2 => {
                let enemy = 1 - side;
                let mut slots = self.tier_enemy_slots(side, tier, target);
                if tier.targets == TargetSpec::Random2 {
                    let n = slots.len().min(2);
                    self.rng.shuffle(&mut slots);
                    slots.truncate(n);
                }
                let dot_novo = self.sides[side].units[slot].atk_eff() * DOT_ATK_PCT / 100;
                for tslot in slots {
                    let dmg = self.tier_damage(side, slot, tier, tslot);
                    self.deal_damage(enemy, tslot, dmg, tier.pierce_shield_pct);
                    if self.sides[enemy].units[tslot].alive {
                        let t = &mut self.sides[enemy].units[tslot];
                        if tier.dot {
                            if t.dot_turns == 0 || dot_novo >= t.dot_dmg {
                                t.dot_dmg = dot_novo;
                            }
                            t.dot_turns = DOT_TURNS;
                        }
                    }
                    if tier.applies != ComboState::None && self.sides[enemy].units[tslot].alive {
                        queue.push((enemy, tslot, tier.applies));
                        self.outbox.push(BattleEvent::StateApplied { side: enemy, slot: tslot, state: tier.applies });
                    }
                }
            }
        }
        self.chase_queue = queue;
        if !self.chase_queue.is_empty() {
            self.open_chase_window(side);
        }
    }

    fn do_interrupt(&mut self, side: usize, slot: usize, target: usize) -> Result<(), ActionError> {
        let def = self.sides[side].units[slot].interrupt.ok_or(ActionError::IllegalAction)?;
        let enemy = 1 - side;
        if target >= 4 || !self.sides[enemy].units[target].alive || self.sides[enemy].acted[target] {
            return Err(ActionError::BadTarget);
        }
        self.outbox.push(BattleEvent::ActionDeclared {
            side,
            slot,
            kind: ActionKind::Interrupt,
            name: "Interrupt".to_string(),
            target: Some((enemy, target)),
        });
        self.sides[side].units[slot].fluxo -= INTERRUPT_COST;
        self.sides[side].units[slot].interrupt_cd = INTERRUPT_CD;
        {
            let t = &mut self.sides[enemy].units[target];
            if def.kind == crate::data::InterruptKind::Stun {
                t.stun_turns = 1;
            }
            if def.kind == crate::data::InterruptKind::Silence {
                t.silence_turns = 1;
            }
        }
        self.outbox.push(BattleEvent::Debuffed { side: enemy, slot: target, kind: def.kind });
        Ok(())
    }

    fn most_wounded_ally(&self, side: usize) -> usize {
        let side_s = &self.sides[side];
        (0..4)
            .filter(|&s| side_s.units[s].alive)
            .min_by_key(|&s| side_s.units[s].hp_pct())
            .unwrap_or(0)
    }

    fn gain_fluxo(&mut self, side: usize, slot: usize, amount: i32) {
        let u = &mut self.sides[side].units[slot];
        if !u.alive || amount <= 0 {
            return;
        }
        let antes = u.fluxo;
        u.fluxo = (u.fluxo + amount).min(FLUXO_CAP);
        let delta = u.fluxo - antes;
        if delta > 0 {
            self.outbox.push(BattleEvent::FluxoGain { side, slot, amount: delta });
        }
    }

    fn deal_damage(&mut self, side: usize, slot: usize, dmg: i32, pierce_shield_pct: i32) {
        let u = &mut self.sides[side].units[slot];
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
        let mut killed = false;
        if u.hp <= 0 {
            u.hp = 0;
            u.alive = false;
            killed = true;
        }
        self.outbox.push(BattleEvent::Damage { side, slot, amount: d, killed });
        // Último Suspiro: a 1ª morte do time revive com % do HP máximo.
        if killed && !self.revive_used[side] && self.edges[side].revive_pct > 0 {
            self.revive_used[side] = true;
            let pct = self.edges[side].revive_pct;
            let t = &mut self.sides[side].units[slot];
            let hp = (t.hp_max * pct / 100).max(1);
            t.hp = hp;
            t.alive = true;
            self.outbox.push(BattleEvent::Revive { side, slot, amount: hp });
        }
    }

    // ── Chase ────────────────────────────────────────────────────────────────

    fn open_chase_window(&mut self, side: usize) {
        self.chase_side = side;
        let (target, state) = match self.chase_queue.first() {
            Some(&(t, s, st)) => ((t, s), st),
            None => return,
        };
        let eligible = self.chase_eligible();
        self.outbox.push(BattleEvent::ChaseOffered { side, target, state, eligible });
    }

    /// Perseguidores elegíveis: vivos, com gatilho compatível e que ainda
    /// não perseguiram nesta rodada.
    fn chase_eligible(&self) -> Vec<usize> {
        let side = self.chase_side;
        let Some(&(_, tslot, st)) = self.chase_queue.first() else {
            return Vec::new();
        };
        if st == ComboState::None || !self.sides[1 - side].units[tslot].alive {
            return Vec::new();
        }
        let extra = self.edges[side].extra_chases;
        let mut out = Vec::new();
        for &s in &self.sides[side].order {
            let u = &self.sides[side].units[s];
            if u.alive
                && u.chase_used < u.chase.max_per_round + extra
                && u.chase.trigger == st
                && u.chase.max_per_round > 0
            {
                out.push(s);
            }
        }
        out
    }

    fn run_chase(&mut self, side: usize, chaser_slot: usize) -> Result<(), ActionError> {
        let Some(&(enemy, tslot, st)) = self.chase_queue.first() else {
            return Err(ActionError::WrongPhase);
        };
        if self.chase_queue.len() > CHAIN_CAP {
            // Prova contra loops: teto de estados pendentes por janela.
            self.chase_queue.clear();
            self.cursor += 1;
            self.outbox.push(BattleEvent::ChaseRejected { side });
            return Ok(());
        }
        let elegivel = self
            .chase_eligible()
            .into_iter()
            .any(|s| s == chaser_slot && self.sides[side].units[chaser_slot].chase.trigger == st);
        if !elegivel {
            return Err(ActionError::BadTarget);
        }
        let capplies = self.sides[side].units[chaser_slot].chase.applies_state;
        let primeiro_elo = !self.sides[side].first_chase_used;
        self.sides[side].units[chaser_slot].chased = true;
        self.sides[side].units[chaser_slot].chase_used += 1;
        self.sides[side].first_chase_used = true;
        let dmg = self.chase_damage(side, chaser_slot, enemy, tslot);
        self.deal_damage(enemy, tslot, dmg, 0);
        self.chain_links += 1;
        self.chain_links_round += 1;
        let killed = !self.sides[enemy].units[tslot].alive;
        self.outbox.push(BattleEvent::ChaseResolved {
            chaser: (side, chaser_slot),
            target: (enemy, tslot),
            amount: dmg,
            killed,
        });
        if capplies != ComboState::None && self.sides[enemy].units[tslot].alive {
            self.chase_queue.push((enemy, tslot, capplies));
        }
        if primeiro_elo {
            let fluxo_bonus = self.sides[side].units[chaser_slot].chase_fluxo_bonus;
            if fluxo_bonus > 0 {
                for s in 0..4 {
                    if self.sides[side].units[s].alive {
                        self.gain_fluxo(side, s, fluxo_bonus);
                    }
                }
            }
        }
        self.chase_queue.remove(0);
        self.finish_if_defeated();
        if !self.over && !self.chase_queue.is_empty() {
            self.open_chase_window(side);
        } else {
            // Cadeia encerrada: turno do agente consumido.
            self.cursor += 1;
        }
        Ok(())
    }

    fn reject_chase(&mut self, side: usize) {
        self.chase_queue.clear();
        self.cursor += 1;
        self.outbox.push(BattleEvent::ChaseRejected { side });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// IA simétrica (E.9): decide com a MESMA informação do jogador — legal_actions
// + preview_action + chase_eligible_slots, zero leitura de estado interno.
// Auditoria: se esta função referencia algo além da API pública, o contrato
// de simetria quebra.
// ─────────────────────────────────────────────────────────────────────────────

/// IA greedy simétrica. Prioridade: chase elegível > interrupt de janela >
/// cura com aliado ferido > mystery > skill de dano > padrão. Desempate pelo
/// maior número previsto (dano×2; cura×2 quando alguém está <45% de HP).
pub fn greedy_decide(bt: &InteractiveBattle, p: &TurnPrompt) -> ChosenAction {
    if p.phase == PromptPhase::ChooseTactic {
        // Determinístico e sem favoritismo: alterna pela geração de decisão
        // (a oferta já é sorteada pela seed; a escolha espelha o inimigo).
        return ChosenAction::PickTactic { index: (bt.generation() % 2) as usize };
    }
    if p.phase == PromptPhase::ConfirmChase {
        return match bt.chase_eligible_slots(p.side).first() {
            Some(&s) => ChosenAction::Chase { chaser_slot: s },
            None => ChosenAction::RejectChase,
        };
    }
    let legais = bt.legal_actions(p.side, p.slot);
    let mut melhor: Option<(i64, ChosenAction)> = None;
    for l in &legais {
        let alvo = match l.kind {
            ActionKind::Standard => melhor_alvo_inimigo(bt, p),
            ActionKind::Interrupt => bt.interrupt_targets(p.side).first().copied(),
            _ => None,
        };
        let acao = ChosenAction::Act { kind: l.kind, target: alvo };
        let Ok(prev) = bt.preview_action(p.side, p.slot, acao) else {
            continue;
        };
        let valor = if prev.heal > 0 {
            let ferido = (0..4).any(|k| {
                bt.unit(p.side, k).alive && bt.unit(p.side, k).hp * 100 < bt.unit(p.side, k).hp_max * 45
            });
            if ferido {
                prev.heal as i64 * 2 + peso(l.kind)
            } else {
                peso(l.kind) / 4
            }
        } else {
            prev.damage as i64 * 2 + peso(l.kind)
        };
        if melhor.as_ref().map(|(v, _)| valor > *v).unwrap_or(true) {
            melhor = Some((valor, acao));
        }
    }
    melhor.map(|(_, a)| a).unwrap_or(ChosenAction::Act { kind: ActionKind::Standard, target: None })
}

/// Peso de prioridade da IA por tipo de ação (tie-break macro).
const fn peso(kind: ActionKind) -> i64 {
    match kind {
        ActionKind::Standard => 0,
        ActionKind::Skill => 4,
        ActionKind::Mystery => 8,
        ActionKind::Interrupt => 12,
    }
}

/// Alvo padrão da IA: inimigo vivo com menos HP (fogo concentrado).
fn melhor_alvo_inimigo(bt: &InteractiveBattle, p: &TurnPrompt) -> Option<usize> {
    let enemy = 1 - p.side;
    (0..4)
        .filter(|&k| bt.unit(enemy, k).alive)
        .min_by_key(|&k| bt.unit(enemy, k).hp)
}

// ─────────────────────────────────────────────────────────────────────────────
// Testes
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Roster;

    fn roster() -> Roster {
        // Roster real de lançamento (16 personagens) — fonte da verdade do jogo.
        Roster::from_json(include_str!("../../../data/roster/roster-v2.json")).expect("roster v2")
    }

    /// Roster sintético controlado: lane limpa para testes de regra.
    /// Slots A: 0 striker (spd 50, mystery R3), 1 striker com chase launch.
    /// Slots B: 4 bonecos guardian lentos.
    const TEST_ROSTER: &str = r#"{"version":2,"characters":[
        {"id":"a1","name":"Atacante","faction":"ember","role":"striker","hp":1000,"attack":100,"defense":40,"speed":50,
         "mystery":{"name":"M1","chakra":20,"cooldown":3,"targets":"enemies_front","damage_pct":120,"applies_state":"launch","effect":""},
         "standard":{"name":"P","damage_pct":70,"applies_state":"none"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}},
        {"id":"a2","name":"Perseguidor","faction":"ember","role":"striker","hp":1000,"attack":100,"defense":40,"speed":45,
         "mystery":{"name":"M2","chakra":20,"cooldown":3,"targets":"enemies_front","damage_pct":90,"applies_state":"none","effect":""},
         "standard":{"name":"P","damage_pct":70,"applies_state":"none"},
         "chase":{"name":"C","trigger":"launch","damage_pct":25,"applies_state":"none","max_per_round":1},
         "passive":{"name":"","effect":""}},
        {"id":"a3","name":"Controlador","faction":"ember","role":"controller","hp":900,"attack":100,"defense":40,"speed":40,
         "mystery":{"name":"M3","chakra":25,"cooldown":3,"targets":"enemies_front","damage_pct":85,"applies_state":"knockdown","effect":"Dano e Derrubada nos inimigos da frente."},
         "standard":{"name":"P","damage_pct":60,"applies_state":"none"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}},
        {"id":"a4","name":"Curandeiro","faction":"ember","role":"support","hp":900,"attack":100,"defense":40,"speed":35,
         "mystery":{"name":"M4","chakra":15,"cooldown":2,"targets":"ally_single","damage_pct":100,"applies_state":"none","effect":"Cura 100% do ataque."},
         "standard":{"name":"P","damage_pct":60,"applies_state":"none"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}},
        {"id":"b1","name":"Boneco1","faction":"tide","role":"guardian","hp":1200,"attack":90,"defense":50,"speed":30,
         "mystery":{"name":"MB","chakra":20,"cooldown":3,"targets":"self","damage_pct":80,"applies_state":"none","effect":"Escudo de 80% do ataque em si."},
         "standard":{"name":"P","damage_pct":65,"applies_state":"none"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}},
        {"id":"b2","name":"Boneco2","faction":"tide","role":"guardian","hp":1200,"attack":90,"defense":50,"speed":28,
         "mystery":{"name":"MB","chakra":20,"cooldown":3,"targets":"self","damage_pct":80,"applies_state":"none","effect":"Escudo de 80% do ataque em si."},
         "standard":{"name":"P","damage_pct":65,"applies_state":"none"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}},
        {"id":"b3","name":"Boneco3","faction":"tide","role":"guardian","hp":1200,"attack":90,"defense":50,"speed":26,
         "mystery":{"name":"MB","chakra":20,"cooldown":3,"targets":"self","damage_pct":80,"applies_state":"none","effect":"Escudo de 80% do ataque em si."},
         "standard":{"name":"P","damage_pct":65,"applies_state":"none"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}},
        {"id":"b4","name":"Boneco4","faction":"tide","role":"guardian","hp":1200,"attack":90,"defense":50,"speed":24,
         "mystery":{"name":"MB","chakra":20,"cooldown":3,"targets":"self","damage_pct":80,"applies_state":"none","effect":"Escudo de 80% do ataque em si."},
         "standard":{"name":"P","damage_pct":65,"applies_state":"none"},
         "chase":{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0},
         "passive":{"name":"","effect":""}}
    ]}"#;

    fn test_roster() -> Roster {
        Roster::from_json(TEST_ROSTER).expect("roster de teste")
    }

    /// Condução mínima: recusa chase e ataca com o padrão (alvo vivo dado;
    /// morto → auto-alvo da regra da IA).
    fn acao_padrao(bt: &InteractiveBattle, p: &TurnPrompt, alvo: usize) -> ChosenAction {
        if p.phase == PromptPhase::ConfirmChase {
            return ChosenAction::RejectChase;
        }
        let inimigo = 1 - p.side;
        let t = if bt.unit(inimigo, alvo).alive { Some(alvo) } else { None };
        ChosenAction::Act { kind: ActionKind::Standard, target: t }
    }

    /// Força a vez para (side, slot): reposiciona o cursor e limpa `acted`.
    /// Utilidade de teste — a UI nunca precisa disto.
    fn forcar_vez(bt: &mut InteractiveBattle, side: usize, slot: usize) {
        let _ = bt.prompt();
        let _ = bt.drain();
        if let Some(pos) = bt.timeline.iter().position(|&e| e == (side, slot)) {
            bt.cursor = pos;
        }
        bt.sides[side].acted[slot] = false;
    }

    #[test]
    fn interativo_deterministico() {
        let r = roster();
        let roda = || {
            let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 0], [3, 4, 5, 3], 42);
            let mut evs = Vec::new();
            loop {
                let p = bt.prompt();
                evs.extend(bt.drain());
                if p.phase == PromptPhase::Over {
                    break;
                }
                if p.phase == PromptPhase::ChooseTactic {
                    evs.append(&mut bt.execute_action(ChosenAction::PickTactic { index: 0 }).expect("tática legal"));
                    continue;
                }
                evs.append(&mut bt.execute_action(acao_padrao(&bt, &p, 0)).expect("ok"));
            }
            (bt.outcome, evs, bt.chain_links())
        };
        let (o1, e1, c1) = roda();
        let (o2, e2, c2) = roda();
        assert_eq!(o1, o2);
        assert_eq!(e1, e2, "mesma seed ⇒ mesmos eventos");
        assert_eq!(c1, c2);
    }

    #[test]
    fn timeline_intercalada_e_alternante() {
        let r = test_roster();
        // seed 0: lado 0 abre; seed 1: rotação garante alternância
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        let p = bt.prompt();
        let _ = bt.drain();
        assert_eq!(p.phase, PromptPhase::ChooseAction);
        let tl = bt.timeline();
        assert_eq!(tl.len(), 8, "8 ações (4v4 vivos)");
        // alternância estrita A-B-A-B…
        for (i, (side, _)) in tl.iter().enumerate() {
            let esperado = if i % 2 == 0 { bt.starter() } else { 1 - bt.starter() };
            assert_eq!(*side, esperado, "posição {i} da timeline deve alternar");
        }
        // SPD desc dentro do lado A (50,45,40,35 → slots 0,1,2,3)
        let ordem_a: Vec<usize> = tl
            .iter()
            .filter(|(s, _)| *s == 0)
            .map(|(_, k)| *k)
            .collect();
        assert_eq!(ordem_a, vec![0, 1, 2, 3], "SPD decrescente dentro do lado");
        // rodada 2 abre com o OUTRO lado
        let primeiro = bt.starter();
        // termina a rodada 1 rapidamente (padrão recusando chase)
        for _ in 0..16 {
            let p = bt.prompt();
            let _ = bt.drain();
            if p.phase == PromptPhase::Over {
                break;
            }
            let _ = bt.execute_action(acao_padrao(&bt, &p, 0));
        }
        if !bt.is_over() {
            assert_ne!(bt.starter(), primeiro, "rodada 2 deve alternar o abridor");
        }
    }

    #[test]
    fn fluxo_economia_respeitada() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        let _p = bt.prompt();
        let _ = bt.drain();
        let u0 = bt.unit(0, 0);
        assert_eq!(u0.fluxo, FLUXO_PER_ROUND, "abre com +10 da rodada 1");
        // Skill (30) ilegal com 10 de fluxo; padrão legal
        let legais = bt.legal_actions(0, 0);
        assert!(!legais.iter().any(|l| l.kind == ActionKind::Skill), "skill exige 30 de fluxo");
        assert!(legais.iter().any(|l| l.kind == ActionKind::Standard));
        // padrão gera +15 (força a vez do slot 0 do lado 0)
        forcar_vez(&mut bt, 0, 0);
        let _ = bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: Some(0) }).expect("ok");
        let u0 = bt.unit(0, 0);
        assert_eq!(u0.fluxo, FLUXO_PER_ROUND + FLUXO_STD_GAIN, "padrão gera +15");
        // teto de 100
        let mut bt2 = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        let _ = bt2.prompt();
        let _ = bt2.drain();
        for _ in 0..12 {
            let p = bt2.prompt();
            let _ = bt2.drain();
            if p.phase == PromptPhase::Over {
                break;
            }
            let _ = bt2.execute_action(acao_padrao(&bt2, &p, 0));
        }
        let u = bt2.unit(0, 0);
        assert!(u.fluxo <= FLUXO_CAP, "fluxo respeita teto 100 ({})", u.fluxo);
    }

    #[test]
    fn skill_custo_cd_e_silencio() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        // força fluxo para testar skill direto
        bt.sides[0].units[0].fluxo = 100;
        forcar_vez(&mut bt, 0, 0);
        let evs = bt
            .execute_action(ChosenAction::Act { kind: ActionKind::Skill, target: None })
            .expect("skill legal com fluxo");
        assert!(evs.iter().any(|e| matches!(e, BattleEvent::ActionDeclared { kind: ActionKind::Skill, .. })));
        let u0 = bt.unit(0, 0);
        assert_eq!(u0.fluxo, 100 - SKILL_COST);
        assert_eq!(u0.skill_cd, SKILL_CD, "skill entra em CD 2");
        // skill em CD → ilegal
        let legais = bt.legal_actions(0, 0);
        assert!(!legais.iter().any(|l| l.kind == ActionKind::Skill), "skill em CD não é legal");
        // silêncio bloqueia skill/mystery mas não padrão
        bt.sides[0].units[0].silence_turns = 1;
        bt.sides[0].units[0].skill_cd = 0;
        let legais = bt.legal_actions(0, 0);
        assert!(!legais.iter().any(|l| l.kind == ActionKind::Skill));
        assert!(!legais.iter().any(|l| l.kind == ActionKind::Mystery));
        assert!(legais.iter().any(|l| l.kind == ActionKind::Standard));
    }

    #[test]
    fn mystery_destrava_por_rodada_e_custo() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        // v0.8: unlock R3 uniforme — striker (slot 0) e controller (slot 2)
        bt.sides[0].units[0].fluxo = 100;
        bt.sides[0].units[2].fluxo = 100;
        let _ = bt.prompt();
        let _ = bt.drain();
        bt.round = 1;
        let legais_striker = bt.legal_actions(0, 0);
        assert!(!legais_striker.iter().any(|l| l.kind == ActionKind::Mystery), "striker: mystery só na R3");
        let legais_ctrl = bt.legal_actions(0, 2);
        assert!(!legais_ctrl.iter().any(|l| l.kind == ActionKind::Mystery), "controller: mystery só na R3");
        bt.round = 2;
        let legais_ctrl = bt.legal_actions(0, 2);
        assert!(!legais_ctrl.iter().any(|l| l.kind == ActionKind::Mystery), "controller ainda bloqueado na R2");
        bt.round = 3;
        let legais_striker = bt.legal_actions(0, 0);
        assert!(legais_striker.iter().any(|l| l.kind == ActionKind::Mystery), "striker destrava na R3");
        let legais_ctrl = bt.legal_actions(0, 2);
        assert!(legais_ctrl.iter().any(|l| l.kind == ActionKind::Mystery), "controller destrava na R3");
        // mystery custa 60
        let m = legais_striker.iter().find(|l| l.kind == ActionKind::Mystery).expect("mystery legal");
        assert_eq!(m.fluxo_cost, MYSTERY_COST);
        // executa: dano da banda mystery frente (multi) = 100%
        forcar_vez(&mut bt, 0, 0);
        let evs = bt
            .execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: None })
            .expect("mystery executada");
        let dano: i32 = evs
            .iter()
            .filter_map(|e| match e {
                BattleEvent::Damage { amount, .. } => Some(*amount),
                _ => None,
            })
            .sum();
        assert!(dano > 0, "mystery deve causar dano ({dano})");
        // aplica launch → janela de chase
        assert!(evs.iter().any(|e| matches!(e, BattleEvent::ChaseOffered { .. })));
    }

    #[test]
    fn chase_confirmado_e_rejeitado() {
        let r = test_roster();
        // Confirmado: mystery do striker aplica launch → perseguidor slot 1 dá dano
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        bt.sides[0].units[0].fluxo = 100;
        // round=2 ⇒ a abertura preguiçosa (dentro de forcar_vez) entra na R3
        // (ímpar, sem oferta de tática) — striker destrava a mystery na R3
        bt.round = 2;
        forcar_vez(&mut bt, 0, 0);
        let _ = bt.execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: None }).expect("mystery");
        let p = bt.prompt();
        let _ = bt.drain();
        assert_eq!(p.phase, PromptPhase::ConfirmChase, "launch abre janela");
        let hp_antes = bt.unit(1, 0).hp;
        let _ = bt.execute_action(ChosenAction::Chase { chaser_slot: 1 }).expect("chase confirmado");
        let hp_depois = bt.unit(1, 0).hp;
        assert!(hp_depois < hp_antes, "chase deve causar dano ({hp_antes} → {hp_depois})");
        assert_eq!(bt.chain_links(), 1);

        // Rejeitado: nenhuma perseguição acontece
        let mut bt2 = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        bt2.sides[0].units[0].fluxo = 100;
        // round=2 ⇒ abertura preguiçosa entra na R3 (sem oferta de tática)
        bt2.round = 2;
        forcar_vez(&mut bt2, 0, 0);
        let _ = bt2.execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: None }).expect("mystery");
        let p = bt2.prompt();
        let _ = bt2.drain();
        assert_eq!(p.phase, PromptPhase::ConfirmChase);
        let _ = bt2.execute_action(ChosenAction::RejectChase).expect("recusa legal");
        assert_eq!(bt2.chain_links(), 0, "recusa não gera elo");
        // avança algumas ações: nenhuma ChaseResolved pode aparecer
        for _ in 0..12 {
            let p = bt2.prompt();
            let ev = bt2.drain();
            if p.phase == PromptPhase::Over {
                break;
            }
            assert!(
                ev.iter().all(|e| !matches!(e, BattleEvent::ChaseResolved { .. })),
                "recusa proíbe elos de chase"
            );
            if p.phase == PromptPhase::ChooseTactic {
                let _ = bt2.execute_action(ChosenAction::PickTactic { index: 0 });
                continue;
            }
            let _ = bt2.execute_action(acao_padrao(&bt2, &p, 0)).expect("ok");
        }
        assert_eq!(bt2.chain_links(), 0, "nenhum elo após recusa");
    }

    #[test]
    fn chase_respeita_1_por_unidade_e_cap() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        bt.sides[0].units[0].fluxo = 100;
        // round=2 ⇒ abertura preguiçosa entra na R3 (sem oferta de tática)
        bt.round = 2;
        forcar_vez(&mut bt, 0, 0);
        let _ = bt.execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: None }).expect("mystery");
        let p = bt.prompt();
        let _ = bt.drain();
        assert_eq!(p.phase, PromptPhase::ConfirmChase);
        // slot 1 persegue
        let _ = bt.execute_action(ChosenAction::Chase { chaser_slot: 1 }).expect("1º chase");
        // sem perseguidor restante para o launch do 2º alvo → janela fecha sozinha
        let p = bt.prompt();
        let _ = bt.drain();
        assert_eq!(p.phase, PromptPhase::ChooseAction, "janelas sem elegíveis fecham sozinhas");
        let u1 = bt.unit(0, 1);
        assert!(u1.chased, "flag chased marca a unidade na rodada");
        assert_eq!(bt.chain_links(), 1, "1 chase por unidade por rodada");
    }

    #[test]
    fn interrupt_janela_stun_e_silencio() {
        // Controlador com interrupt stun; boneco ainda vai agir → janela aberta
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [2, 1, 0, 3], [4, 5, 6, 7], 0);
        bt.sides[0].units[0].fluxo = 100;
        bt.sides[0].units[0].interrupt = Some(InterruptDef {
            kind: crate::data::InterruptKind::Stun,
            window: 1,
        });
        forcar_vez(&mut bt, 0, 0);
        // alvos de interrupt = todos os bonecos (nenhum agiu ainda)
        let alvos = bt.interrupt_targets(0);
        assert_eq!(alvos.len(), 4, "nenhum inimigo agiu: janela total");
        let evs = bt
            .execute_action(ChosenAction::Act { kind: ActionKind::Interrupt, target: Some(0) })
            .expect("interrupt legal");
        assert!(evs.iter().any(|e| matches!(e, BattleEvent::Debuffed { kind: crate::data::InterruptKind::Stun, .. })));
        let u0 = bt.unit(0, 0);
        assert_eq!(u0.fluxo, 100 - INTERRUPT_COST);
        assert_eq!(u0.interrupt_cd, INTERRUPT_CD);
        // o boneco 0 foi atordoado: quando a vez dele chegar, evento Skip
        let mut viu_skip = false;
        for _ in 0..10 {
            let p = bt.prompt();
            let evs = bt.drain();
            viu_skip |= evs.iter().any(|e| matches!(e, BattleEvent::Skip { side: 1, slot: 0, reason: SkipReason::Stunned }));
            if p.phase == PromptPhase::Over || viu_skip {
                break;
            }
            if p.phase == PromptPhase::ChooseTactic {
                let _ = bt.execute_action(ChosenAction::PickTactic { index: 0 });
                continue;
            }
            let _ = bt.execute_action(acao_padrao(&bt, &p, 0));
        }
        assert!(viu_skip, "stun deve pular a ação do alvo");

        // Silence: bloqueia skill/mystery, padrão passa
        let mut bt2 = InteractiveBattle::new(&r, [2, 1, 0, 3], [4, 5, 6, 7], 0);
        bt2.sides[0].units[0].fluxo = 100;
        bt2.sides[0].units[0].interrupt = Some(InterruptDef {
            kind: crate::data::InterruptKind::Silence,
            window: 1,
        });
        forcar_vez(&mut bt2, 0, 0);
        let _ = bt2.execute_action(ChosenAction::Act { kind: ActionKind::Interrupt, target: Some(0) }).expect("ok");
        assert_eq!(bt2.unit(1, 0).silence_turns, 1);
        // silenciado: legal_actions dele não pode ter skill/mystery
        bt2.sides[1].units[0].fluxo = 100;
        let legais = bt2.legal_actions(1, 0);
        assert!(!legais.iter().any(|l| l.kind != ActionKind::Standard), "silenciado só tem padrão");
    }

    #[test]
    fn interrupt_exige_alvo_que_ainda_vai_agir() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [2, 1, 0, 3], [4, 5, 6, 7], 0);
        bt.sides[0].units[0].fluxo = 100;
        bt.sides[0].units[0].interrupt = Some(InterruptDef {
            kind: crate::data::InterruptKind::Stun,
            window: 1,
        });
        forcar_vez(&mut bt, 0, 0);
        // janela fechada: todos os inimigos já agiram nesta rodada
        bt.sides[1].acted = [true, true, true, true];
        let legais = bt.legal_actions(0, 0);
        assert!(
            !legais.iter().any(|l| l.kind == ActionKind::Interrupt),
            "janela fechada: todos os inimigos já agiram"
        );
        let err = bt.execute_action(ChosenAction::Act { kind: ActionKind::Interrupt, target: Some(0) });
        assert_eq!(err, Err(ActionError::BadTarget), "alvo que já agiu é recusado");
    }

    #[test]
    fn teto_dez_rodadas_desempate_por_hp() {
        let r = test_roster();
        // batalha passiva: guardian self-shield vs self-shield → vai ao limite
        let mut bt = InteractiveBattle::new(&r, [4, 5, 6, 7], [4, 5, 6, 7], 9);
        let mut guard = 0;
        loop {
            guard += 1;
            assert!(guard < 30_000, "não converge no teto de rodadas");
            let p = bt.prompt();
            let _ = bt.drain();
            if p.phase == PromptPhase::Over {
                break;
            }
            if p.phase == PromptPhase::ChooseTactic {
                let _ = bt.execute_action(ChosenAction::PickTactic { index: 0 });
                continue;
            }
            let acao = if p.phase == PromptPhase::ConfirmChase {
                ChosenAction::RejectChase
            } else {
                let skill_legal = bt.legal_actions(p.side, p.slot).iter().any(|l| l.kind == ActionKind::Skill);
                if skill_legal {
                    ChosenAction::Act { kind: ActionKind::Skill, target: None }
                } else {
                    acao_padrao(&bt, &p, 0)
                }
            };
            let _ = bt.execute_action(acao);
        }
        assert!(bt.round() <= MAX_ROUNDS, "batalha termina em ≤10 rodadas ({})", bt.round());
        assert!(bt.outcome().is_some());
    }

    #[test]
    fn dot_30_por_cento_durante_duas_rodadas() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        let atk = bt.sides[0].units[0].atk;
        // abre a rodada 1 ANTES de semear o DoT (2 ticks: abre R2 e R3)
        let _ = bt.prompt();
        let _ = bt.drain();
        bt.sides[1].units[0].dot_dmg = atk * DOT_ATK_PCT / 100;
        bt.sides[1].units[0].dot_turns = DOT_TURNS;
        let esperado = pace(atk * DOT_ATK_PCT / 100);
        let hp_antes = bt.unit(1, 0).hp;
        for _ in 0..12 {
            let p = bt.prompt();
            let _ = bt.drain();
            if p.phase == PromptPhase::Over || bt.round() >= 2 {
                break;
            }
            let _ = bt.execute_action(acao_padrao(&bt, &p, 0));
        }
        let u = bt.unit(1, 0);
        let dano_dot = hp_antes - u.hp;
        assert!(
            dano_dot >= esperado,
            "abertura da rodada 2 deve trazer tick de DoT ≥ {esperado} (dano total {dano_dot})"
        );
        assert_eq!(u.dot_turns, 1, "resta 1 rodada de DoT");
    }

    #[test]
    fn ttk_striker_morre_em_poucos_padroes() {
        let r = roster();
        // Ignia (0) vs composição inimiga; contagem de padrões até cair
        let mut bt = InteractiveBattle::new(&r, [4, 5, 6, 7], [0, 1, 2, 3], 5);
        let mut hits = 0usize;
        let mut guard = 0;
        loop {
            guard += 1;
            assert!(guard < 30_000);
            let p = bt.prompt();
            let _ = bt.drain();
            if p.phase == PromptPhase::Over {
                break;
            }
            if p.phase == PromptPhase::ChooseTactic {
                let _ = bt.execute_action(ChosenAction::PickTactic { index: 0 });
                continue;
            }
            let acao = if p.phase == PromptPhase::ConfirmChase {
                ChosenAction::RejectChase
            } else {
                if p.side == 0 {
                    hits += 1;
                }
                acao_padrao(&bt, &p, 0)
            };
            let _ = bt.execute_action(acao);
            if !bt.unit(1, 0).alive {
                break;
            }
        }
        assert!(
            (4..=12).contains(&hits),
            "TTK do striker em banda tática (hits de padrão no alvo: {hits})"
        );
    }

    #[test]
    fn decisao_fora_da_vez_e_recusada() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        let p = bt.prompt();
        let _ = bt.drain();
        assert_eq!(p.phase, PromptPhase::ChooseAction);
        // a vez é de (p.side, p.slot): padrão com alvo válido passa
        let ev = bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: Some(0) });
        assert!(ev.is_ok(), "turno corrente é aceito");
        // padrão sem estado → sem janela de chase
        let p2 = bt.prompt();
        let _ = bt.drain();
        assert_eq!(p2.phase, PromptPhase::ChooseAction);
        assert_ne!(p2, p, "a vez passou para outra unidade");
        // fora de janela de chase, recusar não faz sentido
        let err = bt.execute_action(ChosenAction::RejectChase);
        assert_eq!(err, Err(ActionError::WrongPhase), "recusa fora de janela de chase");
    }

    #[test]
    fn passivas_ganham_fluxo_com_teto() {
        // Mel (tide support, +8 chakra no início da rodada) → fluxo passivo ≤ 20
        let r = roster();
        let idx_mel = r.characters.iter().position(|c| c.id == "mel").expect("mel no roster");
        let idx_dalva = r.characters.iter().position(|c| c.id == "dalva").expect("dalva no roster");
        let mut bt = InteractiveBattle::new(&r, [idx_mel, idx_mel, idx_mel, idx_mel], [idx_dalva, idx_dalva, idx_dalva, idx_dalva], 3);
        let _ = bt.prompt();
        let evs = bt.drain();
        // Mel ganha 10 (rodada) + 8 (passiva) = 18 por rodada
        let ganho_mel: i32 = evs
            .iter()
            .filter(|e| matches!(e, BattleEvent::FluxoGain { side: 0, slot: 0, amount: _ }))
            .map(|e| match e {
                BattleEvent::FluxoGain { amount, .. } => *amount,
                _ => 0,
            })
            .sum();
        assert_eq!(ganho_mel, FLUXO_PER_ROUND + 8, "passiva +8 soma ao fluxo da rodada");
    }

    #[test]
    fn unmodeled_fica_visivel() {
        let r = roster();
        let idx_ivo = r.characters.iter().position(|c| c.id == "ivo").expect("ivo no roster");
        let bt = InteractiveBattle::new(&r, [idx_ivo, 0, 1, 2], [3, 4, 5, 6], 1);
        assert!(
            !bt.unmodeled().is_empty(),
            "passiva condicional não modelada deve aparecer na lista honesta"
        );
    }

    // ── E.2: preview == resultado ────────────────────────────────────────────

    #[test]
    fn preview_padrao_igual_resultado() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        forcar_vez(&mut bt, 0, 0);
        let prev = bt
            .preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Standard, target: Some(0) })
            .expect("preview legal");
        assert_eq!(prev.targets, vec![(1, 0)]);
        assert_eq!(prev.fluxo_delta, FLUXO_STD_GAIN, "padrão gera fluxo");
        let evs = bt
            .execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: Some(0) })
            .expect("executa");
        let dano: i32 = evs
            .iter()
            .filter_map(|e| match e {
                BattleEvent::Damage { side: 1, slot: 0, amount, .. } => Some(*amount),
                _ => None,
            })
            .sum();
        assert_eq!(prev.damage, dano, "prévia do padrão == dano executado");
    }

    #[test]
    fn preview_skill_igual_resultado() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        bt.sides[0].units[0].fluxo = 100;
        forcar_vez(&mut bt, 0, 0);
        let prev = bt
            .preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Skill, target: None })
            .expect("preview skill");
        assert_eq!(prev.fluxo_delta, -SKILL_COST);
        let evs = bt
            .execute_action(ChosenAction::Act { kind: ActionKind::Skill, target: None })
            .expect("executa skill");
        let dano: i32 = evs
            .iter()
            .filter_map(|e| match e {
                BattleEvent::Damage { side: 1, amount, .. } => Some(*amount),
                _ => None,
            })
            .sum();
        assert_eq!(prev.damage, dano, "prévia da skill == dano executado (frente)");
        assert!(prev.targets.contains(&(1, 0)), "alvo da frente na prévia");
    }

    #[test]
    fn preview_mystery_estados_igual_resultado() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        bt.sides[0].units[0].fluxo = 100;
        // round=2 ⇒ abertura preguiçosa entra na R3 (sem oferta de tática)
        bt.round = 2;
        forcar_vez(&mut bt, 0, 0);
        let prev = bt
            .preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Mystery, target: None })
            .expect("preview mystery");
        assert!(!prev.states.is_empty(), "mystery do striker aplica launch");
        assert!(prev.states.iter().all(|(s, _, st)| *s == 1 && *st == ComboState::Launch));
        let evs = bt
            .execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: None })
            .expect("executa mystery");
        let dano: i32 = evs
            .iter()
            .filter_map(|e| match e {
                BattleEvent::Damage { side: 1, amount, .. } => Some(*amount),
                _ => None,
            })
            .sum();
        assert_eq!(prev.damage, dano, "prévia da mystery == dano executado");
        let aplicados: Vec<(usize, usize, ComboState)> = evs
            .iter()
            .filter_map(|e| match e {
                BattleEvent::StateApplied { side, slot, state } => Some((*side, *slot, *state)),
                _ => None,
            })
            .collect();
        // exclui estados abertos por chase (só os da ação direta contam)
        assert_eq!(prev.states, aplicados, "estados previstos == aplicados");
    }

    #[test]
    fn preview_cura_escudo_igual_resultado() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        // machuca o slot 0 do lado 0 para a cura ter efeito parcial
        bt.sides[0].units[0].hp -= 250;
        bt.sides[0].units[3].fluxo = 100;
        forcar_vez(&mut bt, 0, 3);
        let prev = bt
            .preview_action(0, 3, ChosenAction::Act { kind: ActionKind::Skill, target: Some(0) })
            .expect("preview cura");
        assert!(prev.heal > 0 && prev.damage == 0);
        let evs = bt
            .execute_action(ChosenAction::Act { kind: ActionKind::Skill, target: Some(0) })
            .expect("executa cura");
        let curas: i32 = evs
            .iter()
            .filter_map(|e| match e {
                BattleEvent::Heal { side: 0, slot: 0, amount } => Some(*amount),
                _ => None,
            })
            .sum();
        assert_eq!(prev.heal, curas, "prévia da cura == cura executada (cap de HP incluso)");
    }

    #[test]
    fn preview_chase_igual_resultado() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        bt.sides[0].units[0].fluxo = 100;
        // round=2 ⇒ abertura preguiçosa entra na R3 (sem oferta de tática)
        bt.round = 2;
        forcar_vez(&mut bt, 0, 0);
        let _ = bt.execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: None }).expect("mystery");
        let p = bt.prompt();
        let _ = bt.drain();
        assert_eq!(p.phase, PromptPhase::ConfirmChase);
        let prev = bt
            .preview_action(0, 1, ChosenAction::Chase { chaser_slot: 1 })
            .expect("preview chase");
        assert!(prev.damage > 0);
        let evs = bt.execute_action(ChosenAction::Chase { chaser_slot: 1 }).expect("executa chase");
        let dano: i32 = evs
            .iter()
            .filter_map(|e| match e {
                BattleEvent::ChaseResolved { amount, .. } => Some(*amount),
                _ => None,
            })
            .sum();
        assert_eq!(prev.damage, dano, "prévia do chase == dano do elo");
    }

    #[test]
    fn preview_recusa_e_erros_espelham_execucao() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        let _ = bt.prompt();
        let _ = bt.drain();
        // fora de janela de chase: prévia espelha o erro da execução
        let pv = bt.preview_action(0, 0, ChosenAction::RejectChase);
        let ex = bt.execute_action(ChosenAction::RejectChase);
        assert_eq!(pv.err(), ex.err(), "prévia e execução concordam no erro");
        // mystery sem fluxo: erro idêntico
        let pv = bt.preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Mystery, target: None });
        let ex = bt.execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: None });
        assert_eq!(pv.err(), ex.err());
        // padrão em alvo morto → mesmo erro
        bt.sides[1].units[0].alive = false;
        let pv = bt.preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Standard, target: Some(0) });
        assert!(pv.is_ok(), "alvo morto cai no auto-alvo com nota");
        assert!(pv.expect("ok").notes.iter().any(|n| n.contains("auto-alvo")));
    }

    #[test]
    fn preview_nao_consome_rng_nem_muta_estado() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        bt.sides[0].units[0].fluxo = 100;
        forcar_vez(&mut bt, 0, 0);
        let mut hp_antes: Vec<i32> = Vec::new();
        for s in 0..2 {
            for k in 0..4 {
                hp_antes.push(bt.unit(s, k).hp);
            }
        }
        let prev1 = bt
            .preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Skill, target: None })
            .expect("ok");
        let prev2 = bt
            .preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Skill, target: None })
            .expect("ok");
        assert_eq!(prev1, prev2, "prévia é pura (idempotente)");
        let mut hp_depois: Vec<i32> = Vec::new();
        for s in 0..2 {
            for k in 0..4 {
                hp_depois.push(bt.unit(s, k).hp);
            }
        }
        assert_eq!(hp_antes, hp_depois, "prévia não muta HP");
    }

    // ── E.9: contrato de simetria da IA ──────────────────────────────────────

    /// Batalha IA vs IA via APENAS a API pública — deve concluir sempre.
    #[test]
    fn ia_simetrica_conclui_por_api_publica() {
        let r = roster();
        for seed in [1u64, 2, 3, 4, 5] {
            let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], seed);
            let mut guard = 0;
            loop {
                guard += 1;
                assert!(guard < 50_000, "IA não converge (seed {seed})");
                let p = bt.prompt();
                let _ = bt.drain();
                if p.phase == PromptPhase::Over {
                    break;
                }
                let acao = greedy_decide(&bt, &p);
                // a submissão DEVE ser aceita (IA só usa ações legais)
                if let Err(e) = bt.execute_action(acao) {
                    panic!("IA submeteu ação ilegal (seed {seed}): {acao:?} → {e:?}");
                }
            }
            assert!(bt.outcome().is_some());
        }
    }

    /// Espelho com IA dos dois lados: nenhum lado domina (janela 25-75%).
    #[test]
    fn ia_espelho_nao_domina() {
        let r = roster();
        let mut a = 0;
        let n = 101;
        for seed in 0..n as u64 {
            let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [0, 1, 2, 3], seed);
            loop {
                let p = bt.prompt();
                let _ = bt.drain();
                if p.phase == PromptPhase::Over {
                    break;
                }
                let _ = bt.execute_action(greedy_decide(&bt, &p));
            }
            if bt.outcome() == Some(Outcome::TeamAWin) {
                a += 1;
            }
        }
        let wr = a as f64 / n as f64;
        assert!(
            (0.25..=0.75).contains(&wr),
            "espelho com IA deve ficar perto de 50% (A={wr:.2} em {n})"
        );
    }

    /// IA determinística: mesma seed ⇒ mesma sequência de decisões.
    #[test]
    fn ia_deterministica() {
        let r = roster();
        let roda = |seed: u64| {
            let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 0], [3, 4, 5, 3], seed);
            let mut decisoes: Vec<(usize, usize, String)> = Vec::new();
            loop {
                let p = bt.prompt();
                let _ = bt.drain();
                if p.phase == PromptPhase::Over {
                    break;
                }
                let acao = greedy_decide(&bt, &p);
                decisoes.push((p.side, p.slot, format!("{acao:?}")));
                let _ = bt.execute_action(acao);
            }
            (bt.outcome(), decisoes)
        };
        let (o1, d1) = roda(77);
        let (o2, d2) = roda(77);
        assert_eq!(o1, o2);
        assert_eq!(d1, d2, "mesma seed ⇒ mesmas decisões da IA");
    }

    #[test]
    fn prefacios_de_tier_respeitam_bandas_da_spec() {
        let r = roster();
        let idx_reia = r.characters.iter().position(|c| c.id == "reia").expect("reia");
        let idx_volta = r.characters.iter().position(|c| c.id == "volta").expect("volta");
        let idx_mel = r.characters.iter().position(|c| c.id == "mel").expect("mel");
        let bt = InteractiveBattle::new(&r, [idx_reia, idx_volta, idx_mel, idx_mel], [idx_mel, idx_mel, idx_mel, idx_mel], 1);
        let reia = &bt.sides[0].units[0];
        // v0.8: striker single-target — banda de ALVO ÚNICO na skill E mystery
        assert_eq!(reia.skill.targets, TargetSpec::EnemySingle, "skill single");
        assert_eq!(reia.skill.dmg_pct, SKILL_DMG_SINGLE, "skill single = 120%");
        assert_eq!(reia.mystery.targets, TargetSpec::EnemySingle, "mystery single");
        assert_eq!(reia.mystery.dmg_pct, MYSTERY_DMG_SINGLE, "mystery single = 200%");
        assert_eq!(reia.mystery.applies, ComboState::Launch, "mystery preserva estado do contrato");
        assert_eq!(reia.skill.applies, ComboState::None, "skill NÃO aplica estado");
        let volta = &bt.sides[0].units[1];
        assert_eq!(volta.mystery.dmg_pct, MYSTERY_DMG_AOE, "mystery AoE = 100%");
        let mel = &bt.sides[0].units[2];
        assert_eq!(mel.skill.heal_pct, 100, "cura da skill = contrato (banda 80-100)");
        assert_eq!(mel.mystery.heal_pct, 165, "cura da mystery amplificada (banda 150-180)");
        assert_eq!(mel.mystery.dmg_pct, 0, "suporte não tem dano na mystery");
        let _ = idx_reia + idx_volta + idx_mel;
    }

    /// U1 (feedback 2026-10-18): o ataque padrão tem ALVO AUTOMÁTICO —
    /// mira o slot espelhado (1↔1, 2↔2…); a prévia resolve sem alvo dado.
    #[test]
    fn padrao_auto_mira_espelhado() {
        let r = roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 11);
        let p = bt.prompt();
        assert_eq!(p.phase, PromptPhase::ChooseAction);
        let pv = bt
            .preview_action(p.side, p.slot, ChosenAction::Act { kind: ActionKind::Standard, target: None })
            .expect("prévia do padrão");
        assert_eq!(pv.targets, vec![(1 - p.side, p.slot)], "espelhado é o alvo automático");
        // e o legal_actions NÃO pede fila de alvos (UI executa direto)
        let legal = bt.legal_actions(p.side, p.slot).into_iter().find(|l| l.kind == ActionKind::Standard).expect("legal");
        assert_eq!(legal.targeting, TargetKind::Auto, "padrão sem seleção de alvo");
    }

    /// U1 (feedback): espelhado caído → primeiro vivo da frente para trás.
    #[test]
    fn padrao_cai_no_primeiro_vivo_se_espelhado_morto() {
        let r = roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 5);
        let p = bt.prompt();
        // mata o ESPELHADO da unidade na vez (slot p.slot do inimigo)
        bt.sides[1 - p.side].units[p.slot].hp = 0;
        bt.sides[1 - p.side].units[p.slot].alive = false;
        let pv = bt
            .preview_action(p.side, p.slot, ChosenAction::Act { kind: ActionKind::Standard, target: None })
            .expect("prévia");
        // primeiro vivo da frente para trás = menor slot vivo (todos os
        // outros estão de pé)
        let esperado = (0..4).find(|&s| s != p.slot).unwrap();
        assert_eq!(pv.targets.first().copied(), Some((1 - p.side, esperado)), "cai no primeiro vivo da frente");
    }

    /// v0.8 — alvo único (enemy_single): o jogador mira QUALQUER inimigo
    /// vivo, incluindo a linha de trás; a prévia é exata e casa com a
    /// execução. Sem alvo explícito, cai no primeiro vivo da frente.
    #[test]
    fn alvo_unico_mira_linha_de_tras_e_casa_com_execucao() {
        let r = roster();
        let idx_reia = r.characters.iter().position(|c| c.id == "reia").expect("reia");
        let mut bt = InteractiveBattle::new(&r, [idx_reia, 1, 2, 3], [4, 5, 6, 7], 9);
        // ronda 3 + fluxo pra mystery destravar
        bt.round = 2;
        bt.sides[0].units[0].fluxo = 100;
        forcar_vez(&mut bt, 0, 0);
        // legal: mystery pede fila de alvos inimigos (TargetKind::Enemy)
        let legal = bt.legal_actions(0, 0).into_iter().find(|l| l.kind == ActionKind::Mystery).expect("mystery legal");
        assert_eq!(legal.targeting, TargetKind::Enemy, "mystery single pede alvo");
        // prévia com alvo da LINHA DE TRÁS (slot 3) = um alvo só, dano exato
        let pv = bt
            .preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Mystery, target: Some(3) })
            .expect("prévia single");
        assert_eq!(pv.targets, vec![(1, 3)], "mira o slot 3 (linha de trás)");
        assert!(pv.damage > 0);
        // execução no MESMO alvo gera exatamente 1 evento de dano no slot 3,
        // com o MESMO valor da prévia (preview == result)
        let evs = bt
            .execute_action(ChosenAction::Act { kind: ActionKind::Mystery, target: Some(3) })
            .expect("executada");
        let danos: Vec<i32> = evs
            .iter()
            .filter_map(|e| match e {
                BattleEvent::Damage { side, slot, amount, .. } if *side == 1 && *slot == 3 => Some(*amount),
                _ => None,
            })
            .collect();
        assert_eq!(danos.len(), 1, "um alvo = um hit");
        assert_eq!(danos[0], pv.damage, "preview == result");
    }

    /// v0.8 — alvo único sem alvo explícito cai no primeiro vivo da frente.
    #[test]
    fn alvo_unico_sem_alvo_cai_no_auto_frente() {
        let r = roster();
        let idx_reia = r.characters.iter().position(|c| c.id == "reia").expect("reia");
        let mut bt = InteractiveBattle::new(&r, [idx_reia, 1, 2, 3], [4, 5, 6, 7], 9);
        bt.round = 2;
        bt.sides[0].units[0].fluxo = 100;
        forcar_vez(&mut bt, 0, 0);
        let pv = bt
            .preview_action(0, 0, ChosenAction::Act { kind: ActionKind::Mystery, target: None })
            .expect("prévia auto");
        assert_eq!(pv.targets, vec![(1, 0)], "auto = primeiro vivo da frente");
    }

    /// v0.8 — `skill_targets` no contrato: ausente herda a mystery;
    /// presente permite formas distintas por tier.
    #[test]
    fn skill_targets_override_no_contrato() {
        let base = r#"{"version":2,"characters":[
            {"id":"s1","name":"S1","faction":"ember","role":"striker","hp":900,"attack":100,"defense":40,"speed":50,
             "mystery":{"name":"M","chakra":20,"cooldown":3,"targets":"enemies_front","damage_pct":100,"applies_state":"none","effect":""},
             "standard":{"name":"S","damage_pct":70,"applies_state":"none"},
             "chase":{"name":"C","trigger":"launch","damage_pct":30,"applies_state":"none","max_per_round":1},
             "passive":{"name":"","effect":""}}
        ]}"#;
        let r_herdado = Roster::from_json(base).expect("herdado");
        let bt = InteractiveBattle::new(&r_herdado, [0, 0, 0, 0], [0, 0, 0, 0], 0);
        assert_eq!(bt.sides[0].units[0].skill.targets, TargetSpec::EnemiesFront, "skill herda a mystery");
        let com_override = base.replace(
            "\"passive\":{\"name\":\"\",\"effect\":\"\"}",
            "\"passive\":{\"name\":\"\",\"effect\":\"\"},\"skill_targets\":\"enemy_single\"",
        );
        let r2 = Roster::from_json(&com_override).expect("override");
        let bt2 = InteractiveBattle::new(&r2, [0, 0, 0, 0], [0, 0, 0, 0], 0);
        let u = &bt2.sides[0].units[0];
        assert_eq!(u.skill.targets, TargetSpec::EnemySingle, "skill com forma própria");
        assert_eq!(u.skill.dmg_pct, SKILL_DMG_SINGLE, "single → banda 120%");
        assert_eq!(u.mystery.targets, TargetSpec::EnemiesFront, "mystery inalterada");
        assert_eq!(u.mystery.dmg_pct, MYSTERY_DMG_AOE, "multi → banda 100%");
    }

    /// B1 (feedback 2026-10-18): geração de decisão — estável enquanto a
    /// MESMA vez continua; incrementa a cada decisão consumida.
    #[test]
    fn generation_estavel_na_mesma_vez() {
        let r = roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 3);
        let p = bt.prompt();
        if p.phase == PromptPhase::ChooseAction {
            let g1 = bt.generation();
            let _ = bt.prompt(); // consultas repetidas NÃO consomem decisão
            let _ = bt.prompt();
            assert_eq!(bt.generation(), g1, "vez repetida não muda a geração");
            let _ = bt.execute_action(ChosenAction::Act { kind: ActionKind::Standard, target: None });
            assert_eq!(bt.generation(), g1 + 1, "decisão consumida incrementa a geração");
        }
    }

    /// R1 (feedback 2026-10-18): a cada 2 rodadas abre oferta de tática
    /// (rodadas 2, 4, 6, 8); o inimigo adota a dele em silêncio.
    #[test]
    fn tatica_ofertada_a_cada_2_rodadas() {
        let r = roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 21);
        let mut rodadas_com_oferta = Vec::new();
        let mut inimigo_sorteou = 0;
        let mut guard = 0;
        loop {
            guard += 1;
            assert!(guard < 50_000, "não converge com táticas");
            let p = bt.prompt();
            let _ = bt.drain();
            match p.phase {
                PromptPhase::Over => break,
                PromptPhase::ChooseTactic => {
                    rodadas_com_oferta.push(bt.round());
                    assert!(bt.enemy_tactic_pick().is_some(), "inimigo já sorteou a tática dele");
                    inimigo_sorteou += 1;
                    let _ = bt.execute_action(ChosenAction::PickTactic { index: 0 });
                }
                _ => {
                    let _ = bt.execute_action(greedy_decide(&bt, &p));
                }
            }
        }
        assert_eq!(rodadas_com_oferta.first(), Some(&2), "primeira oferta na rodada 2");
        assert_eq!(inimigo_sorteou, rodadas_com_oferta.len(), "cada oferta tem equivalente silencioso");
        assert!(
            rodadas_com_oferta.iter().all(|&r| r % TACTIC_INTERVAL == 0 && r < MAX_ROUNDS),
            "ofertas só em rodadas pares pré-finais: {rodadas_com_oferta:?}"
        );
    }

    /// R1: adotar tática muda o estado do time (efeito real, escopo da
    /// batalha) e emite evento nomeado.
    #[test]
    fn tatica_aplica_efeito_e_emite_evento() {
        let r = roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 21);
        // avança até a 1ª janela de tática (rodada 2)
        loop {
            let p = bt.prompt();
            let _ = bt.drain();
            match p.phase {
                PromptPhase::Over => panic!("batalha terminou antes da rodada 2"),
                PromptPhase::ChooseTactic => break,
                _ => {
                    let _ = bt.execute_action(greedy_decide(&bt, &p));
                }
            }
        }
        // retrato antes: ATK/mitigação/fluxo/cura de fim de rodada do lado 0
        let antes = (
            bt.sides[0].units.iter().map(|u| u.atk).sum::<i32>(),
            bt.sides[0].units.iter().map(|u| u.dmg_reduction_permille).sum::<i32>(),
            bt.edges[0].fluxo_per_round,
            bt.sides[0].units.iter().map(|u| u.round_end_heal_pct).sum::<i32>(),
        );
        let eventos = bt.execute_action(ChosenAction::PickTactic { index: 0 }).expect("tática legal");
        let depois = (
            bt.sides[0].units.iter().map(|u| u.atk).sum::<i32>(),
            bt.sides[0].units.iter().map(|u| u.dmg_reduction_permille).sum::<i32>(),
            bt.edges[0].fluxo_per_round,
            bt.sides[0].units.iter().map(|u| u.round_end_heal_pct).sum::<i32>(),
        );
        assert_ne!(antes, depois, "adotar tática muda o estado do time");
        assert!(
            eventos
                .iter()
                .any(|e| matches!(e, BattleEvent::TacticPicked { side: 0, .. })),
            "evento TacticPicked do jogador emitido"
        );
        // oferta encerrada — volta a pedir ação normal
        assert!(bt.tactic_offer().is_none());
        assert_ne!(bt.prompt().phase, PromptPhase::ChooseTactic);
    }
}
