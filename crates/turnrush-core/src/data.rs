//! Roster de lançamento: 12 personagens com habilidades **fixas** (estilo
//! souls-like tático: cada carta/personagem tem poder pré-definido e testado).
//!
//! Balanceamento inicial por orçamento de poder aproximado; a calibração fina
//! é feita pelo DWC (simulador Monte Carlo) com modificadores por composição.
//!
//! **v0.3 — roster data-driven**: além do roster interno (`ROSTER`, fonte do
//! modo 1v1), este módulo carrega rosters do **contrato JSON v2** com
//! `Roster::from_json` — o mesmo schema compartilhado com o agente de dados
//! (`game/data/roster/roster-v2.json`). Sem arquivo, o fallback embutido
//! (`Roster::builtin`, os mesmos 12 personagens no schema v2) é usado.

use crate::logger::{self, Level, V};
use crate::types::{Ability, Effect, Stats, Targeting};
use std::path::{Path, PathBuf};

/// Ataque básico universal: grátis, sem cooldown, 90% do ATK.
pub const fn basic() -> Ability {
    Ability {
        id: "basic",
        name: "Ataque",
        cost: 0,
        cooldown: 0,
        targeting: Targeting::Enemy,
        effect: fx(90),
        is_chase: false,
    }
}

pub const fn fx(dmg_pct: i32) -> Effect {
    Effect {
        dmg_pct,
        heal: 0,
        shield: 0,
        stun: 0,
        poison_dmg: 0,
        poison_turns: 0,
        atk_pct: 0,
        buff_turns: 0,
        is_launch: false,
    }
}

#[allow(clippy::too_many_arguments)] // construtor const de tabela — legível como linha de dados
pub const fn ability(
    id: &'static str,
    name: &'static str,
    cost: i32,
    cooldown: u8,
    targeting: Targeting,
    effect: Effect,
    is_chase: bool,
) -> Ability {
    Ability {
        id,
        name,
        cost,
        cooldown,
        targeting,
        effect,
        is_chase,
    }
}

pub struct CharacterDef {
    pub id: usize,
    pub name: &'static str,
    pub role: &'static str,
    pub stats: Stats,
    /// [básico, habilidade, ultimate]
    pub abilities: [Ability; 3],
}

#[allow(clippy::too_many_arguments)] // construtor const de tabela — legível como linha de dados
const fn ch(
    id: usize,
    name: &'static str,
    role: &'static str,
    hp: i32,
    atk: i32,
    def: i32,
    spd: i32,
    skill: Ability,
    ult: Ability,
) -> CharacterDef {
    CharacterDef {
        id,
        name,
        role,
        stats: Stats { hp, atk, def, spd },
        abilities: [basic(), skill, ult],
    }
}

/// Base de referência: HP 90 · ATK 22 · DEF 20 · SPD 10.
/// Mitigação: dano_final = ATK × pct% × 100/(100+DEF).
pub const ROSTER: [CharacterDef; 12] = [
    ch(
        0,
        "Golem",
        "Tanque",
        150, 20, 32, 8,
        ability("wall", "Muralha de Pedra", 3, 3, Targeting::SelfTarget, Effect { shield: 30, ..fx(0) }, false),
        ability("quake", "Terremoto", 6, 4, Targeting::EnemyAll, Effect { is_launch: true, ..fx(70) }, false),
    ),
    ch(
        1,
        "Bruto",
        "Berserker",
        115, 24, 26, 9,
        ability("smash", "Golpe Esmagador", 4, 3, Targeting::Enemy, Effect { is_launch: true, ..fx(150) }, false),
        ability("rage", "Fúria", 3, 4, Targeting::SelfTarget, Effect { atk_pct: 30, buff_turns: 2, ..fx(0) }, false),
    ),
    ch(
        2,
        "Sombra",
        "Assassino",
        85, 26, 14, 15,
        ability("pierce", "Lâmina Traspassa", 3, 2, Targeting::Enemy, fx(130), true),
        ability("execute", "Execução", 6, 4, Targeting::Enemy, Effect { is_launch: true, ..fx(190) }, false),
    ),
    ch(
        3,
        "Arqueira",
        "Atiradora",
        92, 26, 20, 12,
        ability("snipe", "Tiro Certeiro", 3, 2, Targeting::Enemy, fx(125), false),
        ability("rain", "Chuva de Flechas", 6, 4, Targeting::EnemyAll, Effect { is_launch: true, ..fx(70) }, false),
    ),
    ch(
        4,
        "Piromante",
        "Mago",
        88, 28, 15, 10,
        ability("fireball", "Bola de Fogo", 4, 3, Targeting::Enemy, Effect { poison_dmg: 6, poison_turns: 2, is_launch: true, ..fx(125) }, false),
        ability("blast", "Deflagrar", 7, 5, Targeting::EnemyAll, fx(90), false),
    ),
    ch(
        5,
        "Oráculo",
        "Curandeiro",
        95, 19, 26, 10,
        ability("bless", "Bênção", 3, 3, Targeting::Ally, Effect { heal: 26, ..fx(0) }, false),
        ability("circle", "Círculo Sagrado", 6, 4, Targeting::AllyAll, Effect { heal: 14, shield: 10, ..fx(0) }, false),
    ),
    ch(
        6,
        "Comandante",
        "Suporte",
        105, 22, 26, 10,
        ability("order", "Ordem de Ataque", 4, 3, Targeting::AllyAll, Effect { atk_pct: 35, buff_turns: 2, ..fx(0) }, false),
        ability("counter", "Contra-Ataque", 3, 2, Targeting::Enemy, Effect { shield: 22, ..fx(115) }, false),
    ),
    ch(
        7,
        "Feiticeira",
        "Maldição",
        90, 23, 20, 10,
        ability("curse", "Maldição", 4, 3, Targeting::Enemy, Effect { atk_pct: -25, buff_turns: 3, poison_dmg: 6, poison_turns: 3, ..fx(0) }, false),
        ability("vortex", "Vórtice", 6, 4, Targeting::EnemyAll, Effect { atk_pct: -15, buff_turns: 2, is_launch: true, ..fx(55) }, false),
    ),
    ch(
        8,
        "Duelista",
        "Duelista",
        90, 25, 20, 13,
        ability("double", "Estocada Dupla", 3, 2, Targeting::Enemy, fx(135), true),
        ability("stance", "Postura Precisa", 2, 3, Targeting::SelfTarget, Effect { atk_pct: 35, buff_turns: 1, ..fx(0) }, false),
    ),
    ch(
        9,
        "Monge",
        "Lutador",
        100, 24, 20, 11,
        ability("palm", "Palma Sônica", 4, 3, Targeting::Enemy, Effect { stun: 1, ..fx(115) }, true),
        ability("flow", "Fluxo Interior", 2, 3, Targeting::Ally, Effect { heal: 20, ..fx(0) }, false),
    ),
    ch(
        10,
        "Necromante",
        "Evocador",
        90, 26, 20, 10,
        ability("harvest", "Colheita", 3, 2, Targeting::Enemy, Effect { heal: 15, ..fx(125) }, false),
        ability("crows", "Enxame de Corvos", 6, 4, Targeting::EnemyAll, Effect { poison_dmg: 4, poison_turns: 2, is_launch: true, ..fx(65) }, false),
    ),
    ch(
        11,
        "Guardiã",
        "Paladino",
        125, 23, 30, 10,
        ability("aegis", "Escudo Divino", 3, 3, Targeting::Ally, Effect { heal: 8, shield: 26, ..fx(0) }, false),
        ability("judge", "Julgamento", 4, 3, Targeting::Enemy, Effect { shield: 15, is_launch: true, ..fx(145) }, false),
    ),
];

/// Tamanho da composição por time no modo 1v1 (C(12,3) = 220 composições).
pub const TEAM_SIZE: usize = 3;

// ═════════════════════════════════════════════════════════════════════════════
// Contrato de dados v2 (compartilhado com o agente de dados — game/data/roster)
// ═════════════════════════════════════════════════════════════════════════════

/// Facção de um personagem do contrato v2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Faction {
    Ember,
    Tide,
    Verdant,
    Storm,
}

impl Faction {
    pub fn parse(s: &str) -> Result<Faction, String> {
        match s {
            "ember" => Ok(Faction::Ember),
            "tide" => Ok(Faction::Tide),
            "verdant" => Ok(Faction::Verdant),
            "storm" => Ok(Faction::Storm),
            other => Err(format!(
                "faction inválida: \"{other}\" (esperado ember|tide|verdant|storm)"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Faction::Ember => "ember",
            Faction::Tide => "tide",
            Faction::Verdant => "verdant",
            Faction::Storm => "storm",
        }
    }
}

/// Papel tático de um personagem do contrato v2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Striker,
    Guardian,
    Support,
    Controller,
}

impl Role {
    pub fn parse(s: &str) -> Result<Role, String> {
        match s {
            "striker" => Ok(Role::Striker),
            "guardian" => Ok(Role::Guardian),
            "support" => Ok(Role::Support),
            "controller" => Ok(Role::Controller),
            other => Err(format!(
                "role inválida: \"{other}\" (esperado striker|guardian|support|controller)"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Striker => "striker",
            Role::Guardian => "guardian",
            Role::Support => "support",
            Role::Controller => "controller",
        }
    }
}

/// Estado de combo (contrato v2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ComboState {
    #[default]
    None,
    Launch,
    Knockback,
    Knockdown,
}

impl ComboState {
    pub fn parse(s: &str) -> Result<ComboState, String> {
        match s {
            "none" => Ok(ComboState::None),
            "launch" => Ok(ComboState::Launch),
            "knockback" => Ok(ComboState::Knockback),
            "knockdown" => Ok(ComboState::Knockdown),
            other => Err(format!(
                "applies_state/trigger inválido: \"{other}\" (esperado launch|knockback|knockdown|none)"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ComboState::None => "none",
            ComboState::Launch => "launch",
            ComboState::Knockback => "knockback",
            ComboState::Knockdown => "knockdown",
        }
    }

    /// Rótulo legível para logs.
    pub fn label(self) -> &'static str {
        match self {
            ComboState::None => "—",
            ComboState::Launch => "lançamento",
            ComboState::Knockback => "empurrão",
            ComboState::Knockdown => "derrubada",
        }
    }
}

/// Alvo de uma mystery no contrato v2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TargetSpec {
    /// Inimigos da fileira da frente (fallback: todos, se a frente caiu).
    #[default]
    EnemiesFront,
    /// Todos os inimigos vivos.
    EnemiesAll,
    /// Um aliado (regra: mais ferido para cura/escudo; maior ATK para buff).
    AllySingle,
    /// O próprio usuário.
    SelfTarget,
    /// Dois inimigos vivos aleatórios.
    Random2,
}

impl TargetSpec {
    pub fn parse(s: &str) -> Result<TargetSpec, String> {
        match s {
            "enemies_front" => Ok(TargetSpec::EnemiesFront),
            "enemies_all" => Ok(TargetSpec::EnemiesAll),
            "ally_single" => Ok(TargetSpec::AllySingle),
            "self" => Ok(TargetSpec::SelfTarget),
            "random_2" => Ok(TargetSpec::Random2),
            other => Err(format!(
                "targets inválido: \"{other}\" (esperado enemies_front|enemies_all|ally_single|self|random_2)"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TargetSpec::EnemiesFront => "enemies_front",
            TargetSpec::EnemiesAll => "enemies_all",
            TargetSpec::AllySingle => "ally_single",
            TargetSpec::SelfTarget => "self",
            TargetSpec::Random2 => "random_2",
        }
    }
}

/// Habilidade definitiva (mystery) do contrato v2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MysteryDef {
    pub name: String,
    pub chakra: i32,
    pub cooldown: u32,
    pub targets: TargetSpec,
    pub damage_pct: i32,
    pub applies_state: ComboState,
    /// Efeito auxiliar em tokens `chave=valor;...` (heal, shield, buffatk,
    /// debuffatk, turns) — interpretado pelo modo de formação.
    pub effect: String,
}

/// Ataque padrão (standard) do contrato v2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StandardDef {
    pub name: String,
    pub damage_pct: i32,
    pub applies_state: ComboState,
}

/// Perseguição (chase) do contrato v2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChaseDef {
    pub name: String,
    /// Estado que dispara esta perseguição.
    pub trigger: ComboState,
    pub damage_pct: i32,
    pub applies_state: ComboState,
    /// Máximo de disparos desta chase por rodada (0 = não persegue).
    pub max_per_round: u32,
}

/// Passiva do contrato v2 (efeito em tokens; ver `parse_passive` em formation.rs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PassiveDef {
    pub name: String,
    pub effect: String,
}

/// Personagem do contrato v2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RosterChar {
    pub id: String,
    pub name: String,
    pub faction: Faction,
    pub role: Role,
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub mystery: MysteryDef,
    pub standard: StandardDef,
    pub chase: ChaseDef,
    pub passive: PassiveDef,
}

/// Roster carregado do contrato JSON v2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roster {
    pub version: u32,
    pub characters: Vec<RosterChar>,
}

impl Roster {
    /// Carrega um roster do **contrato JSON v2** com validação e mensagens de
    /// erro claras (incluindo o caminho do campo problemático).
    pub fn from_json(src: &str) -> Result<Roster, String> {
        let root = crate::json::parse(src).map_err(|e| format!("roster: {e}"))?;
        let version = root
            .get("version")
            .ok_or_else(|| "roster: campo 'version' ausente".to_string())?
            .as_int()
            .ok_or_else(|| "roster: 'version' deve ser inteiro".to_string())?;
        if version != 2 {
            return Err(format!("roster: 'version' deve ser 2, encontrado {version}"));
        }
        let chars = root
            .get("characters")
            .ok_or_else(|| "roster: campo 'characters' ausente".to_string())?
            .as_arr()
            .ok_or_else(|| "roster: 'characters' deve ser um array".to_string())?;
        if chars.is_empty() {
            return Err("roster: 'characters' não pode ser vazio".to_string());
        }
        let mut out = Vec::with_capacity(chars.len());
        let mut ids: Vec<String> = Vec::with_capacity(chars.len());
        for (i, cj) in chars.iter().enumerate() {
            let c = parse_char(i, cj)?;
            if ids.contains(&c.id) {
                return Err(format!("roster: id duplicado '{}' (personagem {i})", c.id));
            }
            ids.push(c.id.clone());
            out.push(c);
        }
        Ok(Roster { version: 2, characters: out })
    }

    /// Roster embutido: os 12 personagens de lançamento no schema v2.
    /// Fonte da verdade do fallback (parseado e verificado por teste).
    pub fn builtin() -> Roster {
        match Roster::from_json(BUILTIN_ROSTER_JSON_V2) {
            Ok(r) => r,
            Err(e) => unreachable!("roster embutido inválido (bug): {e}"),
        }
    }

    pub fn len(&self) -> usize {
        self.characters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.characters.is_empty()
    }
}

fn req_obj<'a>(parent: &'a crate::json::Json, field: &str, ctx: &str) -> Result<&'a crate::json::Json, String> {
    parent
        .get(field)
        .ok_or_else(|| format!("roster: '{ctx}.{field}' ausente"))
}

fn req_str(parent: &crate::json::Json, field: &str, ctx: &str) -> Result<String, String> {
    let v = req_obj(parent, field, ctx)?;
    v.as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("roster: '{ctx}.{field}' deve ser string (encontrado {})", v.type_name()))
}

fn req_int(parent: &crate::json::Json, field: &str, ctx: &str) -> Result<i64, String> {
    let v = req_obj(parent, field, ctx)?;
    v.as_int().ok_or_else(|| {
        format!("roster: '{ctx}.{field}' deve ser inteiro (encontrado {})", v.type_name())
    })
}

fn req_enum<T>(
    parent: &crate::json::Json,
    field: &str,
    ctx: &str,
    parse: fn(&str) -> Result<T, String>,
) -> Result<T, String> {
    let s = req_str(parent, field, ctx)?;
    parse(&s).map_err(|e| format!("roster: '{ctx}.{field}': {e}"))
}

fn int_in(v: i64, min: i64, max: i64, field: &str, ctx: &str) -> Result<i64, String> {
    if (min..=max).contains(&v) {
        Ok(v)
    } else {
        Err(format!("roster: '{ctx}.{field}' = {v} fora do intervalo [{min}, {max}]"))
    }
}

fn parse_char(i: usize, j: &crate::json::Json) -> Result<RosterChar, String> {
    let ctx = format!("characters[{i}]");
    if j.as_obj().is_none() {
        return Err(format!("roster: '{ctx}' deve ser um objeto"));
    }
    let id = req_str(j, "id", &ctx)?;
    if id.trim().is_empty() {
        return Err(format!("roster: '{ctx}.id' não pode ser vazio"));
    }
    let name = req_str(j, "name", &ctx)?;
    if name.trim().is_empty() {
        return Err(format!("roster: '{ctx}.name' não pode ser vazio"));
    }
    let faction = req_enum(j, "faction", &ctx, Faction::parse)?;
    let role = req_enum(j, "role", &ctx, Role::parse)?;
    let hp = int_in(req_int(j, "hp", &ctx)?, 1, 100_000, "hp", &ctx)? as i32;
    let attack = int_in(req_int(j, "attack", &ctx)?, 1, 100_000, "attack", &ctx)? as i32;
    let defense = int_in(req_int(j, "defense", &ctx)?, 0, 100_000, "defense", &ctx)? as i32;
    let speed = int_in(req_int(j, "speed", &ctx)?, 1, 1_000, "speed", &ctx)? as i32;

    let mctx = format!("{ctx}.mystery");
    let m = req_obj(j, "mystery", &ctx)?;
    if m.as_obj().is_none() {
        return Err(format!("roster: '{mctx}' deve ser um objeto"));
    }
    let mystery = MysteryDef {
        name: req_str(m, "name", &mctx)?,
        chakra: int_in(req_int(m, "chakra", &mctx)?, 0, 40, "chakra", &mctx)? as i32,
        cooldown: int_in(req_int(m, "cooldown", &mctx)?, 0, 20, "cooldown", &mctx)? as u32,
        targets: req_enum(m, "targets", &mctx, TargetSpec::parse)?,
        damage_pct: int_in(req_int(m, "damage_pct", &mctx)?, 0, 1_000, "damage_pct", &mctx)? as i32,
        applies_state: req_enum(m, "applies_state", &mctx, ComboState::parse)?,
        effect: m
            .get("effect")
            .map(|e| e.as_str().unwrap_or_default().to_string())
            .unwrap_or_default(),
    };

    let sctx = format!("{ctx}.standard");
    let s = req_obj(j, "standard", &ctx)?;
    if s.as_obj().is_none() {
        return Err(format!("roster: '{sctx}' deve ser um objeto"));
    }
    let standard = StandardDef {
        name: req_str(s, "name", &sctx)?,
        damage_pct: int_in(req_int(s, "damage_pct", &sctx)?, 0, 1_000, "damage_pct", &sctx)? as i32,
        applies_state: req_enum(s, "applies_state", &sctx, ComboState::parse)?,
    };

    let cctx = format!("{ctx}.chase");
    let c = req_obj(j, "chase", &ctx)?;
    if c.as_obj().is_none() {
        return Err(format!("roster: '{cctx}' deve ser um objeto"));
    }
    let chase = ChaseDef {
        name: req_str(c, "name", &cctx)?,
        trigger: req_enum(c, "trigger", &cctx, ComboState::parse)?,
        damage_pct: int_in(req_int(c, "damage_pct", &cctx)?, 0, 1_000, "damage_pct", &cctx)? as i32,
        applies_state: req_enum(c, "applies_state", &cctx, ComboState::parse)?,
        max_per_round: int_in(req_int(c, "max_per_round", &cctx)?, 0, 4, "max_per_round", &cctx)? as u32,
    };

    let pctx = format!("{ctx}.passive");
    let p = req_obj(j, "passive", &ctx)?;
    if p.as_obj().is_none() {
        return Err(format!("roster: '{pctx}' deve ser um objeto"));
    }
    let passive = PassiveDef { name: req_str(p, "name", &pctx)?, effect: req_str(p, "effect", &pctx)? };

    Ok(RosterChar {
        id,
        name,
        faction,
        role,
        hp,
        attack,
        defense,
        speed,
        mystery,
        standard,
        chase,
        passive,
    })
}

/// Caminhos candidatos para `roster-v2.json` (usados por `load_roster_auto`).
pub fn roster_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(p) = std::env::var("TURNRUSH_ROSTER") {
        if !p.trim().is_empty() {
            out.push(PathBuf::from(p));
        }
    }
    out.push(PathBuf::from("game/data/roster/roster-v2.json"));
    out.push(PathBuf::from("data/roster/roster-v2.json"));
    out
}

/// Carrega o primeiro candidato existente; sem arquivo válido → fallback
/// embutido. Registra o resultado no log de sessão.
pub fn load_roster_from_candidates(paths: &[PathBuf]) -> (Roster, Option<PathBuf>) {
    for path in paths {
        match std::fs::read_to_string(path) {
            Ok(src) => match Roster::from_json(&src) {
                Ok(r) => {
                    logger::kv(
                        Level::Info,
                        "roster_load",
                        &format!("roster carregado de {}", path.display()),
                        &[
                            ("fonte", V::S(&path.display().to_string())),
                            ("personagens", V::U(r.len() as u64)),
                            ("versao", V::U(r.version as u64)),
                        ],
                    );
                    return (r, Some(path.clone()));
                }
                Err(e) => {
                    logger::kv(
                        Level::Warn,
                        "roster_load",
                        &format!("arquivo inválido em {}: {e}", path.display()),
                        &[("fonte", V::S(&path.display().to_string())), ("erro", V::S(&e))],
                    );
                }
            },
            Err(_) => continue, // ausente → próximo candidato (tolerado)
        }
    }
    let fallback = Roster::builtin();
    logger::kv(
        Level::Info,
        "roster_load",
        "nenhum roster-v2.json encontrado — usando fallback embutido (12 personagens)",
        &[
            ("fonte", V::S("embutido")),
            ("personagens", V::U(fallback.len() as u64)),
            ("versao", V::U(fallback.version as u64)),
        ],
    );
    (fallback, None)
}

/// Carrega o roster do jogo (env `TURNRUSH_ROSTER` → `game/data/roster/…` →
/// embutido), logando a fonte. Nunca falha.
pub fn load_roster_auto() -> Roster {
    let candidates = roster_candidates();
    load_roster_from_candidates(&candidates).0
}

/// Carrega um roster de um caminho específico (loga e valida).
pub fn load_roster(path: &Path) -> Result<Roster, String> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| format!("roster: ler {}: {e}", path.display()))?;
    let r = Roster::from_json(&src)?;
    logger::kv(
        Level::Info,
        "roster_load",
        &format!("roster carregado de {}", path.display()),
        &[
            ("fonte", V::S(&path.display().to_string())),
            ("personagens", V::U(r.len() as u64)),
            ("versao", V::U(r.version as u64)),
        ],
    );
    Ok(r)
}

/// Roster embutido (fallback) no **contrato JSON v2 exato** — mesmo schema de
/// `game/data/roster/roster-v2.json` (contrato compartilhado com o agente de
/// dados). Os 12 personagens de lançamento, com kits adaptados ao 4v4.
pub const BUILTIN_ROSTER_JSON_V2: &str = r#"{
  "version": 2,
  "characters": [
    {
      "id": "golem", "name": "Golem", "faction": "ember", "role": "guardian",
      "hp": 150, "attack": 20, "defense": 32, "speed": 8,
      "mystery": { "name": "Terremoto", "chakra": 6, "cooldown": 4, "targets": "enemies_all", "damage_pct": 70, "applies_state": "launch", "effect": "" },
      "standard": { "name": "Punho de Pedra", "damage_pct": 90, "applies_state": "none" },
      "chase": { "name": "", "trigger": "none", "damage_pct": 0, "applies_state": "none", "max_per_round": 0 },
      "passive": { "name": "Pele de Pedra", "effect": "def+15" }
    },
    {
      "id": "bruto", "name": "Brute", "faction": "ember", "role": "striker",
      "hp": 115, "attack": 24, "defense": 26, "speed": 9,
      "mystery": { "name": "Golpe Esmagador", "chakra": 4, "cooldown": 3, "targets": "enemies_front", "damage_pct": 95, "applies_state": "launch", "effect": "" },
      "standard": { "name": "Golpe", "damage_pct": 90, "applies_state": "knockback" },
      "chase": { "name": "", "trigger": "none", "damage_pct": 0, "applies_state": "none", "max_per_round": 0 },
      "passive": { "name": "Fúria Contida", "effect": "atk+10" }
    },
    {
      "id": "sombra", "name": "Shade", "faction": "tide", "role": "striker",
      "hp": 85, "attack": 26, "defense": 14, "speed": 15,
      "mystery": { "name": "Execução", "chakra": 6, "cooldown": 4, "targets": "enemies_front", "damage_pct": 130, "applies_state": "launch", "effect": "" },
      "standard": { "name": "Adaga", "damage_pct": 90, "applies_state": "none" },
      "chase": { "name": "Lâmina Traspassa", "trigger": "launch", "damage_pct": 130, "applies_state": "knockback", "max_per_round": 1 },
      "passive": { "name": "Passo Sombrio", "effect": "spd+3" }
    },
    {
      "id": "arqueira", "name": "Archer", "faction": "verdant", "role": "striker",
      "hp": 92, "attack": 26, "defense": 20, "speed": 12,
      "mystery": { "name": "Chuva de Flechas", "chakra": 6, "cooldown": 4, "targets": "enemies_all", "damage_pct": 70, "applies_state": "launch", "effect": "" },
      "standard": { "name": "Flecha", "damage_pct": 90, "applies_state": "none" },
      "chase": { "name": "", "trigger": "none", "damage_pct": 0, "applies_state": "none", "max_per_round": 0 },
      "passive": { "name": "Mira Estável", "effect": "spd+2" }
    },
    {
      "id": "piromante", "name": "Pyromancer", "faction": "ember", "role": "controller",
      "hp": 88, "attack": 28, "defense": 15, "speed": 10,
      "mystery": { "name": "Deflagrar", "chakra": 7, "cooldown": 5, "targets": "enemies_all", "damage_pct": 90, "applies_state": "none", "effect": "" },
      "standard": { "name": "Faísca", "damage_pct": 90, "applies_state": "none" },
      "chase": { "name": "", "trigger": "none", "damage_pct": 0, "applies_state": "none", "max_per_round": 0 },
      "passive": { "name": "Sobrecarga", "effect": "atk+8" }
    },
    {
      "id": "oraculo", "name": "Oracle", "faction": "verdant", "role": "support",
      "hp": 95, "attack": 19, "defense": 26, "speed": 10,
      "mystery": { "name": "Círculo Sagrado", "chakra": 6, "cooldown": 4, "targets": "ally_single", "damage_pct": 0, "applies_state": "none", "effect": "heal=30;shield=12" },
      "standard": { "name": "Cajado", "damage_pct": 90, "applies_state": "none" },
      "chase": { "name": "", "trigger": "none", "damage_pct": 0, "applies_state": "none", "max_per_round": 0 },
      "passive": { "name": "Fé Serena", "effect": "chakra+1" }
    },
    {
      "id": "comandante", "name": "Commander", "faction": "storm", "role": "support",
      "hp": 105, "attack": 22, "defense": 26, "speed": 10,
      "mystery": { "name": "Ordem de Ataque", "chakra": 4, "cooldown": 3, "targets": "ally_single", "damage_pct": 0, "applies_state": "none", "effect": "buffatk=35;turns=2" },
      "standard": { "name": "Sabre", "damage_pct": 90, "applies_state": "none" },
      "chase": { "name": "", "trigger": "none", "damage_pct": 0, "applies_state": "none", "max_per_round": 0 },
      "passive": { "name": "Disciplina", "effect": "def+8" }
    },
    {
      "id": "feiticeira", "name": "Sorceress", "faction": "tide", "role": "controller",
      "hp": 90, "attack": 23, "defense": 20, "speed": 10,
      "mystery": { "name": "Vórtice", "chakra": 6, "cooldown": 4, "targets": "enemies_all", "damage_pct": 55, "applies_state": "knockback", "effect": "debuffatk=15;turns=2" },
      "standard": { "name": "Hex", "damage_pct": 90, "applies_state": "none" },
      "chase": { "name": "", "trigger": "none", "damage_pct": 0, "applies_state": "none", "max_per_round": 0 },
      "passive": { "name": "Má Vontade", "effect": "atk+5" }
    },
    {
      "id": "duelista", "name": "Duelist", "faction": "storm", "role": "striker",
      "hp": 90, "attack": 25, "defense": 20, "speed": 13,
      "mystery": { "name": "Postura Precisa", "chakra": 2, "cooldown": 3, "targets": "self", "damage_pct": 0, "applies_state": "none", "effect": "buffatk=35;turns=1" },
      "standard": { "name": "Estocada", "damage_pct": 90, "applies_state": "none" },
      "chase": { "name": "Estocada Dupla", "trigger": "launch", "damage_pct": 135, "applies_state": "none", "max_per_round": 1 },
      "passive": { "name": "Leitura", "effect": "spd+2" }
    },
    {
      "id": "monge", "name": "Monk", "faction": "verdant", "role": "striker",
      "hp": 100, "attack": 24, "defense": 20, "speed": 11,
      "mystery": { "name": "Fluxo Interior", "chakra": 2, "cooldown": 3, "targets": "ally_single", "damage_pct": 0, "applies_state": "none", "effect": "heal=20" },
      "standard": { "name": "Soco", "damage_pct": 90, "applies_state": "none" },
      "chase": { "name": "Palma Sônica", "trigger": "knockback", "damage_pct": 115, "applies_state": "knockdown", "max_per_round": 1 },
      "passive": { "name": "Chi Interno", "effect": "hp+8" }
    },
    {
      "id": "necromante", "name": "Necromancer", "faction": "tide", "role": "controller",
      "hp": 90, "attack": 26, "defense": 20, "speed": 10,
      "mystery": { "name": "Enxame de Corvos", "chakra": 6, "cooldown": 4, "targets": "enemies_all", "damage_pct": 65, "applies_state": "launch", "effect": "" },
      "standard": { "name": "Toque Vampírico", "damage_pct": 90, "applies_state": "none" },
      "chase": { "name": "", "trigger": "none", "damage_pct": 0, "applies_state": "none", "max_per_round": 0 },
      "passive": { "name": "Colheita", "effect": "atk+5" }
    },
    {
      "id": "guardia", "name": "Guardian", "faction": "storm", "role": "guardian",
      "hp": 125, "attack": 23, "defense": 30, "speed": 10,
      "mystery": { "name": "Julgamento", "chakra": 4, "cooldown": 3, "targets": "enemies_front", "damage_pct": 90, "applies_state": "launch", "effect": "shield=15" },
      "standard": { "name": "Martelo", "damage_pct": 90, "applies_state": "none" },
      "chase": { "name": "", "trigger": "none", "damage_pct": 0, "applies_state": "none", "max_per_round": 0 },
      "passive": { "name": "Voto de Proteção", "effect": "def+10" }
    }
  ]
}"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roster_ids_consistentes() {
        for (i, c) in ROSTER.iter().enumerate() {
            assert_eq!(c.id, i);
            assert_eq!(c.abilities.len(), 3);
            // básico sempre disponível (custo 0, cd 0)
            assert_eq!(c.abilities[0].cost, 0);
            assert_eq!(c.abilities[0].cooldown, 0);
        }
    }

    #[test]
    fn roster_embutido_v2_valido() {
        let r = Roster::builtin();
        assert_eq!(r.version, 2);
        assert_eq!(r.len(), 12);
        for (i, c) in r.characters.iter().enumerate() {
            assert!(!c.id.is_empty());
            assert!(!c.name.is_empty());
            assert!(c.hp > 0 && c.attack > 0 && c.speed > 0);
            assert!(c.mystery.chakra >= 0 && c.mystery.cooldown <= 20);
            // chase existente ⇒ trigger != none e max_per_round >= 1
            if c.chase.trigger != ComboState::None {
                assert!(c.chase.max_per_round >= 1, "{} deveria perseguir", c.name);
            }
            let _ = i;
        }
    }

    #[test]
    fn from_json_aceita_contrato_e_valida_campos() {
        let src = r#"{"version":2,"characters":[
            {"id":"x1","name":"X1","faction":"tide","role":"controller",
             "hp":100,"attack":20,"defense":10,"speed":9,
             "mystery":{"name":"M","chakra":4,"cooldown":3,"targets":"random_2","damage_pct":80,"applies_state":"knockdown","effect":"heal=5"},
             "standard":{"name":"S","damage_pct":90,"applies_state":"launch"},
             "chase":{"name":"C","trigger":"knockdown","damage_pct":60,"applies_state":"launch","max_per_round":2},
             "passive":{"name":"P","effect":"hp+10"}}]}"#;
        let r = Roster::from_json(src).expect("contrato válido");
        assert_eq!(r.len(), 1);
        let c = &r.characters[0];
        assert_eq!(c.faction, Faction::Tide);
        assert_eq!(c.role, Role::Controller);
        assert_eq!(c.mystery.targets, TargetSpec::Random2);
        assert_eq!(c.mystery.applies_state, ComboState::Knockdown);
        assert_eq!(c.chase.trigger, ComboState::Knockdown);
        assert_eq!(c.chase.max_per_round, 2);
        assert_eq!(c.standard.applies_state, ComboState::Launch);
    }

    #[test]
    fn from_json_erro_claro_em_cada_campo() {
        let base = |campo: &str| {
            format!(
                r#"{{"version":2,"characters":[{{"id":"x","name":"X","faction":"ember","role":"striker",
                "hp":10,"attack":5,"defense":0,"speed":5,
                "mystery":{{"name":"M","chakra":1,"cooldown":1,"targets":"self","damage_pct":0,"applies_state":"none","effect":""}},
                "standard":{{"name":"S","damage_pct":10,"applies_state":"none"}},
                "chase":{{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0}},
                "passive":{{"name":"P","effect":""}}{campo}}}]}}"#
            )
        };
        // faction inválida
        let bad = base("").replace("\"faction\":\"ember\"", "\"faction\":\"fogo\"");
        let e = Roster::from_json(&bad).unwrap_err();
        assert!(e.contains("faction") && e.contains("fogo"), "{e}");
        // versão errada
        let bad = base("").replace("\"version\":2", "\"version\":1");
        assert!(Roster::from_json(&bad).unwrap_err().contains("'version' deve ser 2"));
        // campo ausente
        let bad = base("").replace("\"hp\":10,", "");
        assert!(Roster::from_json(&bad).unwrap_err().contains("characters[0].hp"),);
        // chase com max_per_round fora do intervalo
        let bad = base("").replace("\"max_per_round\":0", "\"max_per_round\":9");
        assert!(Roster::from_json(&bad).unwrap_err().contains("max_per_round"));
        // id duplicado
        let dup = format!("{{\"version\":2,\"characters\":[{},{}]}}", char_json("a"), char_json("a"));
        assert!(Roster::from_json(&dup).unwrap_err().contains("duplicado"));
        // JSON inválido
        assert!(Roster::from_json("{ sem fecho").unwrap_err().contains("roster:"));
    }

    fn char_json(id: &str) -> String {
        format!(
            r#"{{"id":"{id}","name":"N{id}","faction":"storm","role":"support","hp":10,"attack":5,"defense":0,"speed":5,
            "mystery":{{"name":"M","chakra":1,"cooldown":1,"targets":"self","damage_pct":0,"applies_state":"none","effect":""}},
            "standard":{{"name":"S","damage_pct":10,"applies_state":"none"}},
            "chase":{{"name":"C","trigger":"none","damage_pct":0,"applies_state":"none","max_per_round":0}},
            "passive":{{"name":"P","effect":""}}}}"#
        )
    }

    #[test]
    fn loader_de_arquivo_tolerante_a_ausencia() {
        // caminho que não existe → fallback embutido
        let (r, fonte) =
            load_roster_from_candidates(&[PathBuf::from("/tmp/nao-existe-roster-8a.json")]);
        assert!(fonte.is_none());
        assert_eq!(r.len(), 12);

        // arquivo válido em /tmp → carregado e logado
        let path = std::env::temp_dir().join(format!("turnrush-roster-test-{}.json", std::process::id()));
        std::fs::write(&path, BUILTIN_ROSTER_JSON_V2).expect("escrever roster tmp");
        let (r2, fonte2) = load_roster_from_candidates(std::slice::from_ref(&path));
        assert_eq!(fonte2.as_deref(), Some(path.as_path()));
        assert_eq!(r2.len(), 12);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn enums_do_contrato() {
        assert_eq!(Faction::parse("ember").unwrap(), Faction::Ember);
        assert!(Faction::parse("fogo").is_err());
        assert_eq!(ComboState::parse("knockdown").unwrap(), ComboState::Knockdown);
        assert!(ComboState::parse("flying").is_err());
        assert_eq!(TargetSpec::parse("random_2").unwrap(), TargetSpec::Random2);
        assert!(TargetSpec::parse("all").is_err());
        assert_eq!(Role::parse("guardian").unwrap(), Role::Guardian);
    }
}
