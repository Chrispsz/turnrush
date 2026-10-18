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
    /// Skill = forma base (dano 120/100, utilidade do contrato, SEM estado);
    /// Mystery = forma final (200 único / 100 multi, utilidade ×1650‰, COM
    /// estado/DoT/perfuração).
    fn derive(m: &MysteryDef) -> (Tier, Tier) {
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

        let skill = Tier {
            name: m.name.clone(),
            targets: m.targets,
            dmg_pct: if heal_json > 0 || shield_json > 0 { 0 } else if multi { SKILL_DMG_AOE } else { SKILL_DMG_SINGLE },
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
    chain_links: u64,
    unmodeled: Vec<String>,
}

impl InteractiveBattle {
    /// Monta a batalha interativa (`comp_a`/`comp_b` = 4 índices no roster).
    /// Alias de [`Self::new`] com o nome da especificação.
    pub fn start_interactive(roster: &Roster, comp_a: [usize; 4], comp_b: [usize; 4], seed: u64) -> Self {
        Self::new(roster, comp_a, comp_b, seed)
    }

    /// Monta a batalha interativa. Não sorteia nada ainda: a 1ª rodada abre
    /// de forma preguiçosa no primeiro `prompt`/`execute_action`.
    pub fn new(roster: &Roster, comp_a: [usize; 4], comp_b: [usize; 4], seed: u64) -> Self {
        let mut unmodeled: Vec<String> = Vec::new();
        let mk = |comp: [usize; 4], unmodeled: &mut Vec<String>| -> ISide {
            let units = comp
                .iter()
                .map(|&ci| {
                    let last = roster.characters.len().saturating_sub(1);
                    let c: &RosterChar = &roster.characters[ci.min(last)];
                    let (u, notas) = Self::build_unit(c);
                    unmodeled.extend(notas);
                    u
                })
                .collect();
            ISide { units, order: Vec::new(), acted: [false; 4], first_chase_used: false }
        };
        let sides = [mk(comp_a, &mut unmodeled), mk(comp_b, &mut unmodeled)];
        InteractiveBattle {
            sides,
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
            unmodeled,
        }
    }

    fn build_unit(c: &RosterChar) -> (IU, Vec<String>) {
        let mut unmodeled: Vec<String> = Vec::new();
        let low = c.passive.effect.to_lowercase();
        let (skill, mystery) = Tier::derive(&c.mystery);
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

        (
            IU {
                name: c.name.clone(),
                role: c.role,
                hp_max: c.hp.max(1),
                hp: c.hp.max(1),
                atk: c.attack * (PER_MILLE + atk_buff_permille) / PER_MILLE,
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
                acted: false,
                alive: true,
                standard: c.standard.clone(),
                skill,
                mystery,
                chase: c.chase.clone(),
                mystery_unlock_round: c.mystery_unlock_round,
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
            if !self.chase_queue.is_empty() {
                if self.chase_eligible().is_empty() {
                    // Ninguém pode perseguir: a janela fecha sozinha (transparente).
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
            targeting: TargetKind::Enemy,
            name: u.standard.name.clone(),
        });
        if u.silence_turns == 0 && u.skill_cd == 0 && u.fluxo >= SKILL_COST {
            let targeting = match u.skill.targets {
                TargetSpec::AllySingle => TargetKind::Ally,
                TargetSpec::SelfTarget | TargetSpec::EnemiesFront | TargetSpec::EnemiesAll | TargetSpec::Random2 => TargetKind::Auto,
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
                TargetSpec::SelfTarget | TargetSpec::EnemiesFront | TargetSpec::EnemiesAll | TargetSpec::Random2 => TargetKind::Auto,
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

    /// Slots inimigos que ainda vão agiar nesta rodada (janela de interrupt).
    pub fn interrupt_targets(&self, side: usize) -> Vec<usize> {
        let enemy = 1 - side;
        (0..4).filter(|&s| self.sides[enemy].units[s].alive && !self.sides[enemy].acted[s]).collect()
    }

    // ── Execução ─────────────────────────────────────────────────────────────

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
                ChosenAction::Act { .. } => return Err(ActionError::WrongPhase),
            }
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
        for si in 0..2 {
            self.sides[si].first_chase_used = false;
            for slot in 0..4 {
                let u = &mut self.sides[si].units[slot];
                if !u.alive {
                    continue;
                }
                u.chased = false;
                u.acted = false;
                u.skill_cd = u.skill_cd.saturating_sub(1);
                u.mystery_cd = u.mystery_cd.saturating_sub(1);
                u.interrupt_cd = u.interrupt_cd.saturating_sub(1);
                let ganho = FLUXO_PER_ROUND + u.round_start_fluxo.min(FLUXO_PASSIVE_CAP);
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
                self.outbox.push(BattleEvent::Skip { side, slot, reason: SkipReason::Dead });
                self.cursor += 1;
                continue;
            }
            if u.stun_turns > 0 {
                self.sides[side].units[slot].stun_turns -= 1;
                self.sides[side].acted[slot] = true;
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

    fn do_standard(&mut self, side: usize, slot: usize, target: Option<usize>) {
        let (sname, dmg_pct, applies) = {
            let u = &self.sides[side].units[slot];
            (u.standard.name.clone(), u.standard.damage_pct, u.standard.applies_state)
        };
        self.outbox.push(BattleEvent::ActionDeclared {
            side,
            slot,
            kind: ActionKind::Standard,
            name: sname,
            target: target.map(|t| (1 - side, t)),
        });
        let enemy = 1 - side;
        let targets = self.sides[enemy].living_slots();
        if targets.is_empty() {
            return;
        }
        let tslot = match target {
            Some(t) if t < 4 && self.sides[enemy].units[t].alive => t,
            _ => targets[0],
        };
        let atk = self.sides[side].units[slot].atk_eff();
        let dot_on_hit = self.sides[side].units[slot].dot_on_hit;
        let bonus_vs_dot = self.sides[side].units[slot].bonus_vs_dot;
        let (tdef, t_red, t_dot) = {
            let t = &self.sides[enemy].units[tslot];
            (t.def, t.dmg_reduction_permille, t.dot_turns > 0)
        };
        let mut dmg = pace(crate::combat::damage_after_def(atk, dmg_pct, tdef));
        if t_dot && bonus_vs_dot > 0 {
            dmg = dmg * (PER_MILLE + bonus_vs_dot) / PER_MILLE;
        }
        if !self.sides[side].units[slot].team_bonus_vs.is_empty() && applies != ComboState::None {
            if let Some((_, p)) =
                self.sides[side].units[slot].team_bonus_vs.iter().find(|(st, _)| *st == applies)
            {
                dmg = dmg * (PER_MILLE + p) / PER_MILLE;
            }
        }
        if t_red > 0 {
            dmg = dmg * (PER_MILLE - t_red) / PER_MILLE;
        }
        if dot_on_hit {
            let t = &mut self.sides[enemy].units[tslot];
            if t.alive {
                let novo = atk * DOT_ATK_PCT / 100;
                if t.dot_turns == 0 || novo >= t.dot_dmg {
                    t.dot_dmg = novo;
                }
                t.dot_turns = DOT_TURNS;
            }
        }
        self.deal_damage(enemy, tslot, dmg, 0);
        if applies != ComboState::None && self.sides[enemy].units[tslot].alive {
            self.chase_queue.push((enemy, tslot, applies));
        }
        // Reflexo do alvo (só em ataque padrão)
        let refl = self.sides[enemy].units[tslot].reflect_permille;
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
        let atk = self.sides[side].units[slot].atk_eff();
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
                let tslot = if tier.targets == TargetSpec::SelfTarget {
                    slot
                } else {
                    match target {
                        Some(t) if t < 4 && self.sides[side].units[t].alive => t,
                        _ => self.most_wounded_ally(side),
                    }
                };
                let heal = atk * tier.heal_pct / 100;
                if heal > 0 {
                    let t = &mut self.sides[side].units[tslot];
                    t.hp = (t.hp + heal).min(t.hp_max);
                    self.outbox.push(BattleEvent::Heal { side, slot: tslot, amount: heal });
                }
                let shield = atk * tier.shield_pct / 100;
                if shield > 0 {
                    self.sides[side].units[tslot].shield += shield;
                    self.outbox.push(BattleEvent::Shield { side, slot: tslot, amount: shield });
                }
                if tier.reflect_pct > 0 {
                    self.sides[side].units[tslot].reflect_permille = tier.reflect_pct * 10;
                }
            }
            TargetSpec::EnemiesFront | TargetSpec::EnemiesAll | TargetSpec::Random2 => {
                let enemy = 1 - side;
                let mut slots: Vec<usize> = match tier.targets {
                    TargetSpec::EnemiesFront => {
                        let living = self.sides[enemy].living_slots();
                        let front: Vec<usize> = living.iter().copied().take(2).collect();
                        if front.is_empty() {
                            living
                        } else {
                            front
                        }
                    }
                    TargetSpec::EnemiesAll => self.sides[enemy].living_slots(),
                    TargetSpec::Random2 => {
                        let mut living = self.sides[enemy].living_slots();
                        let n = living.len().min(2);
                        self.rng.shuffle(&mut living);
                        living.truncate(n);
                        living
                    }
                    _ => Vec::new(),
                };
                slots.retain(|&s| self.sides[enemy].units[s].alive);
                for tslot in slots {
                    let (tdef, t_red) = {
                        let t = &self.sides[enemy].units[tslot];
                        (t.def, t.dmg_reduction_permille)
                    };
                    let eff_def = tdef * (100 - tier.pierce_def_pct) / 100;
                    let mut dmg = pace(crate::combat::damage_after_def(atk, tier.dmg_pct, eff_def));
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
                    self.deal_damage(enemy, tslot, dmg, tier.pierce_shield_pct);
                    if self.sides[enemy].units[tslot].alive {
                        let t = &mut self.sides[enemy].units[tslot];
                        if tier.dot {
                            let novo = atk * DOT_ATK_PCT / 100;
                            if t.dot_turns == 0 || novo >= t.dot_dmg {
                                t.dot_dmg = novo;
                            }
                            t.dot_turns = DOT_TURNS;
                        }
                    }
                    if tier.applies != ComboState::None && self.sides[enemy].units[tslot].alive {
                        queue.push((enemy, tslot, tier.applies));
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
        let mut out = Vec::new();
        for &s in &self.sides[side].order {
            let u = &self.sides[side].units[s];
            if u.alive && !u.chased && u.chase.trigger == st && u.chase.max_per_round > 0 {
                out.push(s);
            }
        }
        out
    }

    fn run_chase(&mut self, side: usize, chaser_slot: usize) -> Result<(), ActionError> {
        let Some(&(enemy, tslot, st)) = self.chase_queue.first() else {
            return Err(ActionError::WrongPhase);
        };
        if self.chain_links >= CHAIN_CAP as u64 {
            // Prova contra loops: teto absoluto de elos na batalha.
            self.chase_queue.clear();
            return Err(ActionError::WrongPhase);
        }
        let elegivel = self
            .chase_eligible()
            .into_iter()
            .any(|s| s == chaser_slot && self.sides[side].units[chaser_slot].chase.trigger == st);
        if !elegivel {
            return Err(ActionError::BadTarget);
        }
        let c = &self.sides[side].units[chaser_slot];
        let (_cname, cdmg_pct, capplies) =
            (c.chase.name.clone(), c.chase.damage_pct, c.chase.applies_state);
        let primeiro_elo = !self.sides[side].first_chase_used;
        let bonus = if primeiro_elo { self.sides[side].units[chaser_slot].chase_dmg_bonus_permille } else { 0 };
        self.sides[side].units[chaser_slot].chased = true;
        self.sides[side].first_chase_used = true;
        let atk = self.sides[side].units[chaser_slot].atk_eff();
        let tdef = self.sides[enemy].units[tslot].def;
        let mut dmg = pace(crate::combat::damage_after_def(atk, cdmg_pct, tdef));
        if bonus > 0 {
            dmg = dmg * (PER_MILLE + bonus) / PER_MILLE;
        }
        self.deal_damage(enemy, tslot, dmg, 0);
        self.chain_links += 1;
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
        // striker (slot 0) destrava na R3; controller (slot 2) na R2
        bt.sides[0].units[0].fluxo = 100;
        bt.sides[0].units[2].fluxo = 100;
        let _ = bt.prompt();
        let _ = bt.drain();
        bt.round = 1;
        let legais_striker = bt.legal_actions(0, 0);
        assert!(!legais_striker.iter().any(|l| l.kind == ActionKind::Mystery), "striker: mystery só na R3");
        let legais_ctrl = bt.legal_actions(0, 2);
        assert!(!legais_ctrl.iter().any(|l| l.kind == ActionKind::Mystery), "controller: mystery só na R2");
        bt.round = 2;
        let legais_ctrl = bt.legal_actions(0, 2);
        assert!(legais_ctrl.iter().any(|l| l.kind == ActionKind::Mystery), "controller destrava na R2");
        let legais_striker = bt.legal_actions(0, 0);
        assert!(!legais_striker.iter().any(|l| l.kind == ActionKind::Mystery), "striker ainda bloqueado na R2");
        bt.round = 3;
        let legais_striker = bt.legal_actions(0, 0);
        assert!(legais_striker.iter().any(|l| l.kind == ActionKind::Mystery), "striker destrava na R3");
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
        bt.round = 3; // striker destrava a mystery na R3
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
        bt2.round = 3;
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
            let _ = bt2.execute_action(acao_padrao(&bt2, &p, 0)).expect("ok");
        }
        assert_eq!(bt2.chain_links(), 0, "nenhum elo após recusa");
    }

    #[test]
    fn chase_respeita_1_por_unidade_e_cap() {
        let r = test_roster();
        let mut bt = InteractiveBattle::new(&r, [0, 1, 2, 3], [4, 5, 6, 7], 0);
        bt.sides[0].units[0].fluxo = 100;
        bt.round = 3;
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

    #[test]
    fn prefacios_de_tier_respeitam_bandas_da_spec() {
        let r = roster();
        let idx_reia = r.characters.iter().position(|c| c.id == "reia").expect("reia");
        let idx_volta = r.characters.iter().position(|c| c.id == "volta").expect("volta");
        let idx_mel = r.characters.iter().position(|c| c.id == "mel").expect("mel");
        let bt = InteractiveBattle::new(&r, [idx_reia, idx_volta, idx_mel, idx_mel], [idx_mel, idx_mel, idx_mel, idx_mel], 1);
        let reia = &bt.sides[0].units[0];
        // enemies_front atinge até 2 alvos = banda multi-alvo na skill E mystery
        assert_eq!(reia.skill.dmg_pct, SKILL_DMG_AOE, "skill frente (multi) = 100%");
        assert_eq!(reia.mystery.dmg_pct, MYSTERY_DMG_AOE, "mystery frente (multi) = 100%");
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
}
