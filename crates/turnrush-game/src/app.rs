//! TurnRush 2D — cliente visual (Bevy).
//!
//! Fluxo jogável: Menu → (Como Jogar | Elenco |) Draft (escolha 4 de 8) →
//! Batalha 4v4 (playback do engine determinístico com barras de HP, flashes e
//! log, com botão Acelerar) → Recompensa → próxima batalha (inimigos escalam)
//! → Fim de run (3 vitórias). O adversário é a IA do engine — modo OFFLINE
//! completo para testar a mecânica sem servidor.
//!
//! Decisões de arquitetura:
//! - **Zero assets externos**: UI via nodes/texto do Bevy e fonte embutida
//!   (`default_font`) — nada para falhar ao carregar.
//! - **Raízes persistentes por estado**: construídas uma vez no `Startup` e
//!   mostradas/escondidas por `Visibility` — sem despawn recursivo.
//! - **Seleção por marcador** (`CardSelected`), não por cor.
//! - **Batalha = playback do engine**: `Formation4v4` (determinístico, com
//!   cadeias de chase) roda uma vez com log; o cliente toca os
//!   eventos em ritmo legível com sprites, animações e efeitos (scene.rs).
//!   Linhas desconhecidas continuam visíveis.
//! - **Logs de sessão completos**: o mesmo logger do engine (`~/TurnRush/logs`).
//!
//! Lições de depuração (v0.3.0 → v0.4.0) — a janela abria em BRANCO na
//! v0.3.0 e eram TRÊS causas somadas, nenhuma visível no `--check` headless:
//! - **`Camera2d` é obrigatório**: UI do Bevy não renderiza sem câmera.
//! - **`bevy_ui_render` é obrigatório no Bevy 0.20**: o render da UI virou
//!   crate separado; só a feature `bevy_ui` roda layout/interação e DESENHA
//!   NADA. E `Node::default()` é `Row` — telas em coluna precisam de
//!   `FlexDirection::Column` explícito.
//! - **Queries de `Text` devem ser disjuntas**: dois `Query<&mut Text>` com
//!   acesso sobreposto panica ao inicializar o sistema (`battle_enter`).
//! - **Fonte default é subset ASCII**: substituímos por DejaVu Sans Mono
//!   embutida (via `AssetId::default()`) — acentos pt-BR agora renderizam.
//! - **`--shot <base>`**: navega menu → como-jogar → elenco → draft →
//!   batalha, salva 5 PNGs e sai — prova de renderização real sem interação
//!   (Xvfb + lavapipe/`WGPU_BACKEND`).
//!
//! NOTA: usamos `Button`/`Interaction` (deprecados no Bevy 0.20, removidos
//! depois) porque ainda funcionam e mantêm o MVP simples; migração para
//! picking + `Pressed` fica para a v0.5.

#![allow(deprecated)]

use bevy::prelude::*;
use bevy::render::view::window::screenshot::{save_to_disk, Screenshot};
use bevy::text::FontSize;
use bevy::window::WindowResolution;
use std::collections::HashMap;

use turnrush_core::data::{ComboState, Roster, RosterChar, TargetSpec};
use turnrush_core::formation::{ActionChoice, Formation4v4, FormationMods, Outcome};
use turnrush_core::rng::Rng;

use crate::battle_log::{parse_all, parse_line, EvKind, ParsedEvent};
use crate::scene;
use crate::scene::{despawn_battle_scene, spawn_battle_scene, BattleScene, FxQueue, VisEvent};
use crate::sprites::SpriteAssets;

/// Tamanho de fonte em pixels (convenção Bevy 0.20).
pub fn px(v: f32) -> FontSize {
    FontSize::Px(v)
}

/// Vitórias necessárias para completar a run.
pub const VICTORIES_TO_WIN: usize = 3;
/// Segundos entre eventos do playback.
const STEP_SECS: f32 = 0.30;
/// Pausa extra ao abrir rodada.
const ROUND_SECS: f32 = 0.80;
/// Duração do flash de dano.

// ─────────────────────────────────────────────────────────────────────────────
// Estados
// ─────────────────────────────────────────────────────────────────────────────

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Menu,
    /// Tela de regras/controles ("Como Jogar").
    ComoJogar,
    /// Visualizador do elenco completo (16 cartas, esboço).
    Elenco,
    Draft,
    Battle,
    Reward,
    GameOver,
}

// ─────────────────────────────────────────────────────────────────────────────
// Recursos
// ─────────────────────────────────────────────────────────────────────────────

/// Estado da run corrente.
#[derive(Resource)]
pub struct RunRes {
    pub roster: Roster,
    pub rng: Rng,
    pub team: Vec<usize>,
    pub enemy: Vec<usize>,
    pub mods: FormationMods,
    pub wins: usize,
    pub rounds_total: u64,
    pub chains_total: u64,
}

/// Playback da batalha corrente.
#[derive(Resource)]
pub struct PlaybackRes {
    pub events: Vec<ParsedEvent>,
    pub idx: usize,
    pub wait: f32,
    pub finished: bool,
    pub outcome: Outcome,
    pub chains: u64,
    pub rounds: u64,
    /// nome → (lado, slot)
    pub map: HashMap<String, (usize, usize)>,
    pub max_hp: [i64; 8],
    pub taken: [i64; 8],
    pub healed: [i64; 8],
    pub alive: [bool; 8],
}

impl PlaybackRes {
    fn unit(&self, name: &str) -> Option<usize> {
        self.map.get(name).map(|&(s, k)| s * 4 + k)
    }
    pub fn hp_pct(&self, u: usize) -> f32 {
        let max = self.max_hp[u].max(1);
        let cur = (max - self.taken[u] + self.healed[u]).clamp(0, max);
        cur as f32 / max as f32
    }
}

/// Ofertas do draft corrente (8 índices do roster, `None` = vazio).
#[derive(Resource, Default)]
pub struct DraftRes {
    pub offers: [Option<usize>; 8],
}

/// Fonte embutida: DejaVu Sans Mono (licença Bitstream Vera/DejaVu — livre,
/// redistribuição permitida). Cobertura pt-BR completa — substitui o subset
/// ASCII da fonte default do Bevy, que renderizava acentos como □.
const DEFAULT_FONT_TTF: &[u8] = include_bytes!("../assets/DejaVuSansMono.ttf");

/// Opções do modo de diagnóstico `--shot <caminho_base>`:
/// navega menu → draft → batalha capturando um PNG de cada tela e sai —
/// prova de renderização real sem interação (Xvfb/CI).
#[derive(Clone, Debug)]
pub struct ShotOpts {
    pub path: String,
}

/// Fases do modo `--shot`.
#[derive(Clone, Copy, PartialEq)]
enum ShotPhase {
    Menu,
    MenuSalvo,
    Ajuda,
    AjudaSalvo,
    Elenco,
    ElencoSalvo,
    Draft,
    DraftSalvo,
    Batalha,
    BatalhaSalvo,
}

/// Estado interno do modo `--shot`.
#[derive(Resource)]
struct ShotState {
    base: String,
    phase: ShotPhase,
    timer: f32,
    frames: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Marcadores de UI
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy, PartialEq)]
pub struct StateRoot(pub GameState);

#[derive(Component)]
pub struct MenuBtn(pub MenuAction);

#[derive(Clone, Copy, PartialEq)]
pub enum MenuAction {
    Play,
    Again,
    Help,
    Elenco,
    Exit,
}

#[derive(Component)]
pub struct DraftCard {
    pub roster_idx: Option<usize>,
    pub name_text: Entity,
    pub stat_text: Entity,
    pub sprite_node: Entity,
}

#[derive(Component)]
pub struct CardSelected;

#[derive(Component)]
pub struct DraftConfirm;

#[derive(Component)]
pub struct DraftCounter;

/// Rótulo de vitórias da run (canto esquerdo do header da batalha).
#[derive(Component)]
pub struct WinsText;

/// Rótulo de cadeias de combo ao vivo (canto direito do header).
#[derive(Component)]
pub struct ChainsText;

#[derive(Component)]
pub struct BattleHeader;

#[derive(Component)]
pub struct BattleLogText;

#[derive(Component)]
pub struct RewardBtn(pub RewardKind);

#[derive(Clone, Copy, PartialEq)]
pub enum RewardKind {
    Dano,
    Vida,
    Equilibrio,
}

#[derive(Component)]
pub struct GameOverText;

/// Botão "Voltar" (telas Como Jogar e Elenco).
#[derive(Component)]
pub struct BackBtn;

/// Botão de aceleração do playback (batalha).
#[derive(Component)]
pub struct SpeedBtn;

/// Rótulo do botão de aceleração.
#[derive(Component)]
pub struct SpeedLabel;

/// Playback acelerado (toggle do botão Acelerar; 6× a velocidade base).
#[derive(Resource, Default)]
pub struct FastForward(pub bool);

// ────────────────────────────────────────────────────────────────────────────
// Batalha manual (v0.6): engine por passos com decisão do jogador
// ────────────────────────────────────────────────────────────────────────────

/// Modo de condução da batalha corrente.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub enum BattleMode {
    /// Playback integral (log pronto) — usado pelo modo `--shot`.
    #[default]
    Playback,
    /// Manual: engine avança por passos e o jogador decide cada ação.
    Manual,
}

/// Condição de sistema: batalha em playback automático.
fn is_playback(mode: Res<BattleMode>) -> bool {
    *mode == BattleMode::Playback
}

/// Condição de sistema: batalha manual.
fn is_manual(mode: Res<BattleMode>) -> bool {
    *mode == BattleMode::Manual
}

/// Estado da batalha manual: engine vivo + fila de eventos por exibir.
#[derive(Resource, Default)]
pub struct ManualRes {
    /// Engine da batalha corrente (lado 0 manual). `None` fora de batalha.
    pub f: Option<Formation4v4>,
    /// Índice do próximo evento cru a converter em [`ParsedEvent`].
    pub ev_cursor: usize,
    /// Eventos parseados aguardando exibição (aplicados 1 por tick).
    pub parsed: Vec<ParsedEvent>,
    /// Botão Auto: a IA do engine resolve as decisões do jogador.
    pub auto: bool,
}

/// Decisão pendente exibida no painel manual (dados já resolvidos p/ UI).
#[derive(Resource, Default)]
pub struct PendingUi {
    pub active: bool,
    pub choosing_target: bool,
    pub name: String,
    pub chakra: i32,
    pub mystery_ready: bool,
    pub mystery_cost: i32,
    pub mystery_name: String,
    pub mystery_desc: String,
    pub standard_name: String,
    /// (nome, hp%, vivo) por slot inimigo.
    pub targets: [(String, i32, bool); 4],
}

// ── Marcadores do painel de decisão ──

#[derive(Component)]
pub struct DecPanel;
#[derive(Component)]
pub struct DecTargetRow;
#[derive(Component)]
pub struct DecTitleText;
#[derive(Component)]
pub struct DecMysteryLabel;
#[derive(Component)]
pub struct DecStandardLabel;
#[derive(Component)]
pub struct DecTargetLabel(pub usize);

/// Botão do painel de decisão.
#[derive(Component, Clone, Copy, PartialEq)]
pub struct DecBtn(pub DecKind);

#[derive(Clone, Copy, PartialEq)]
pub enum DecKind {
    Mystery,
    Standard,
    Auto,
}

/// Botão de alvo inimigo (slot 0..4).
#[derive(Component, Clone, Copy, PartialEq)]
pub struct DecTargetBtn(pub usize);

/// Botão “Auto-alvo” (regra da IA: primeiro inimigo vivo).
#[derive(Component)]
pub struct DecTargetAutoBtn;

/// Conteúdo da tela "Como Jogar" — regras FIÉIS ao engine (formation.rs).
const AJUDA: [(&str, &str); 7] = [
    (
        "1 · DRAFT — 8 cartas na mesa, escolha 4",
        "A oferta é a mesma para os dois lados (simetria por construção).\nA IA monta o time inimigo entre os personagens restantes do elenco.",
    ),
    (
        "2 · BATALHA 4v4 POR TURNOS (VOCÊ COMANDA)",
        "Turnos alternados A-B-A-B por posição; quem abre a 1ª rodada é sorteado.\nNa vez de cada unidade sua, o painel inferior oferece a Mystery (custo de\nchakra) ou o Padrão com alvo clicável. Máximo de 12 rodadas — desempate\npor % de vida. Chakra: 30 inicial · +14 por rodada · teto 100.",
    ),
    (
        "3 · CHAIN COMBOS (CADEIAS DE PERSEGUIÇÃO)",
        "Mysteries e ataques aplicam estados: Launch, Knockback e Knockdown.\nAliados com chase de gatilho compatível encadeiam golpes no mesmo turno\n— até 12 elos por cadeia, 1 chase por unidade por rodada.",
    ),
    (
        "4 · PASSIVAS E DANO CONTÍNUO",
        "Cada carta tem uma passiva sempre ativa (reflexo, cura de rodada, chakra\nextra, bônus contra estados…). Queimadura/Veneno: 10% do ATK do aplicador\npor rodada, durante 2 rodadas, sem acumular.",
    ),
    (
        "5 · RECOMPENSA E ESCALADA",
        "Vitória → escolha: +10% dano · +10% vida · Equilíbrio +5%/+5%.\nO próximo inimigo escala +11% de dano e +7% de vida POR VITÓRIA sua.\n3 vitórias completam a run; uma derrota encerra (roguelike).",
    ),
    (
        "6 · CLÃS E BALANCEAMENTO",
        "Pyreborn, Duskmere, Thornvale e Skyrend dão identidade — NÃO existe\ncontador duro entre clãs. O calibrador DWC v2 mantém todas as composições\nem ~50% de winrate médio: a vantagem vem da sua leitura do confronto.",
    ),
    (
        "7 · CONTROLES (modo offline)",
        "Tudo no mouse. Na batalha: Mystery/Padrão no painel inferior — Padrão\nabre os alvos inimigos (clique num inimigo vivo ou use Auto-alvo).\nBotão Auto entrega as decisões à IA; Acelerar alterna playback 1×/6×.",
    ),
];

/// Nomes de exibição oficiais (docs/NAMING.md) — ids internos estáveis.
pub fn faction_display(f: &str) -> &'static str {
    match f {
        "ember" => "PYREBORN",
        "tide" => "DUSKMERE",
        "verdant" => "THORNVALE",
        "storm" => "SKYREND",
        _ => "—",
    }
}

/// Classes de exibição oficiais (docs/NAMING.md).
pub fn role_display(r: &str) -> &'static str {
    match r {
        "striker" => "Reaver",
        "guardian" => "Bastion",
        "support" => "Mender",
        "controller" => "Tactician",
        _ => "—",
    }
}

fn rotulo_alvo(t: TargetSpec) -> &'static str {
    match t {
        TargetSpec::EnemiesFront => "frente",
        TargetSpec::EnemiesAll => "todos",
        TargetSpec::AllySingle => "aliado",
        TargetSpec::SelfTarget => "si",
        TargetSpec::Random2 => "2 aleatórios",
    }
}

fn estado(s: ComboState) -> String {
    if s == ComboState::None {
        String::new()
    } else {
        format!(" · {}", s.label())
    }
}

fn chase_line(c: &RosterChar) -> String {
    if c.chase.trigger == ComboState::None || c.chase.max_per_round == 0 {
        "C: —".to_string()
    } else {
        format!(
            "C: {} · gatilho {} · {}% · {}×/rodada",
            c.chase.name,
            c.chase.trigger.label(),
            c.chase.damage_pct,
            c.chase.max_per_round
        )
    }
}

/// Carta do elenco: sprite genérico + identidade + kit completo (esboço).
fn spawn_elenco_card(grid: &mut ChildSpawnerCommands, tex: &SpriteAssets, i: usize, c: &RosterChar) {
    let fac = c.faction.as_str();
    let sprite = grid
        .spawn((
            Node { width: Val::Px(46.0), height: Val::Px(46.0), ..default() },
            ImageNode::new(tex.units[i].clone()),
        ))
        .id();
    grid.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            padding: UiRect::all(Val::Px(9.0)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(faction_color(fac)),
    ))
    .add_child(sprite)
    .with_children(|p| {
        p.spawn((Text::new(c.name.clone()), TextFont { font_size: px(16.0), ..default() }, TextColor(TXT)));
        p.spawn((
            Text::new(format!("{} · {}", faction_display(fac), role_display(c.role.as_str()))),
            TextFont { font_size: px(11.0), ..default() },
            TextColor(faction_color(fac)),
        ));
        p.spawn((
            Text::new(format!(
                "HP {} · ATQ {} · DEF {} · VEL {}",
                c.hp, c.attack, c.defense, c.speed
            )),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(TXT_DIM),
        ));
        p.spawn((
            Text::new(format!(
                "M: {} · {}% · chakra {} · cd {} · {}{}",
                c.mystery.name,
                c.mystery.damage_pct,
                c.mystery.chakra,
                c.mystery.cooldown,
                rotulo_alvo(c.mystery.targets),
                estado(c.mystery.applies_state)
            )),
            TextFont { font_size: FontSize::Px(10.0), ..default() },
            TextColor(AMBER),
        ));
        p.spawn((
            Text::new(chase_line(c)),
            TextFont { font_size: FontSize::Px(10.0), ..default() },
            TextColor(TXT_DIM),
        ));
        p.spawn((
            Text::new(format!("P: {} — {}", c.passive.name, c.passive.effect)),
            TextFont { font_size: FontSize::Px(10.0), ..default() },
            TextColor(TXT_DIM),
        ));
    });
}

// Cores do tema (sem azul/indigo — âmbar × rosa; clã na borda)
pub const BG: Color = Color::srgb(0.075, 0.075, 0.11);
pub const PANEL: Color = Color::srgb(0.13, 0.13, 0.18);
pub const PANEL_DARK: Color = Color::srgb(0.10, 0.10, 0.14);
pub const TXT: Color = Color::srgb(0.92, 0.92, 0.95);
pub const TXT_DIM: Color = Color::srgb(0.62, 0.62, 0.70);
pub const AMBER: Color = Color::srgb(0.96, 0.62, 0.04);
pub const ROSE: Color = Color::srgb(0.96, 0.30, 0.40);
pub const GOOD: Color = Color::srgb(0.30, 0.78, 0.40);
pub const WARN: Color = Color::srgb(0.92, 0.76, 0.20);
pub const BAD: Color = Color::srgb(0.88, 0.28, 0.28);

fn faction_color(f: &str) -> Color {
    match f {
        "ember" => Color::srgb(0.94, 0.45, 0.15),
        "verdant" => Color::srgb(0.35, 0.78, 0.35),
        "tide" => Color::srgb(0.25, 0.75, 0.72),
        "storm" => Color::srgb(0.85, 0.78, 0.25),
        _ => TXT_DIM,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// App
// ─────────────────────────────────────────────────────────────────────────────

pub fn run(shot: Option<ShotOpts>) {
    // Logs de sessão (o "script de tudo") — o mesmo diretório do engine.
    let seed = turnrush_core::logger::unix_nanos() as u64;
    let _ = turnrush_core::logger::init(seed);
    turnrush_core::logger::kv(
        turnrush_core::logger::Level::Info,
        "game_start",
        "TurnRush 2D iniciado",
        &[("janela", turnrush_core::logger::V::S("1280x720"))],
    );

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(bevy::log::LogPlugin {
                    filter: "info,wgpu=info,naga=warn".into(),
                    level: bevy::log::Level::INFO,
                    ..default()
                })
                .set(bevy::window::WindowPlugin {
                    primary_window: Some(Window {
                        title: "TurnRush".into(),
                        resolution: WindowResolution::new(1280, 720),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .init_state::<GameState>()
        .insert_resource(ClearColor(BG))
        .insert_resource(new_run())
        .init_resource::<DraftRes>()
        .init_resource::<FastForward>()
        .init_resource::<BattleMode>()
        .init_resource::<ManualRes>()
        .init_resource::<PendingUi>()
        .init_resource::<FxQueue>()
        .init_resource::<BattleScene>()
        .insert_resource(new_playback_placeholder())
        // Sprites em PreStartup: o Commands flusha na fronteira de schedule,
        // então Startup (setup_roots, embers) já enxerga Res<SpriteAssets>.
        .add_systems(PreStartup, spawn_sprite_assets)
        .add_systems(
            Startup,
            (
                spawn_camera,
                override_default_font,
                scene::spawn_embers,
                setup_roots,
                move |mut cmds: Commands| {
                    if let Some(s) = shot.clone() {
                        cmds.insert_resource(ShotState {
                            base: s.path,
                            phase: ShotPhase::Menu,
                            timer: 2.0,
                            frames: 0,
                        });
                    }
                },
            ),
        )
        .add_systems(Update, (sync_root_visibility, shot_tick, back_click, speed_click, scene::embers_tick))
        // Menu
        .add_systems(Update, menu_interactions.run_if(in_state(GameState::Menu)))
        // Draft
        .add_systems(OnEnter(GameState::Draft), draft_enter)
        .add_systems(
            Update,
            (draft_card_click, draft_confirm).run_if(in_state(GameState::Draft)),
        )
        // Batalha (cena em mundo 2D + playback + efeitos)
        .add_systems(OnEnter(GameState::Battle), (battle_enter, spawn_battle_scene).chain())
        .add_systems(
            Update,
            (
                battle_playback.run_if(is_playback),
                manual_tick.run_if(is_manual),
                sync_decision_panel,
                decision_click,
                target_click,
                scene::scene_animate,
                scene::scene_fx,
                scene::fx_motion,
                scene::scene_shake,
                battle_finish,
            )
                .run_if(in_state(GameState::Battle)),
        )
        .add_systems(OnExit(GameState::Battle), despawn_battle_scene)
        // Recompensa
        .add_systems(Update, reward_click.run_if(in_state(GameState::Reward)))
        // Fim de run
        .add_systems(OnEnter(GameState::GameOver), gameover_enter)
        .add_systems(Update, gameover_click.run_if(in_state(GameState::GameOver)))
        .run();
}

/// Gera todos os sprites procedurais (um por personagem do roster) + utilitários.
fn spawn_sprite_assets(mut cmds: Commands, mut images: ResMut<Assets<Image>>, run: Res<RunRes>) {
    let units = run
        .roster
        .characters
        .iter()
        .map(|c| images.add(crate::sprites::unit_image(c.faction.as_str(), c.role.as_str())))
        .collect();
    cmds.insert_resource(SpriteAssets {
        units,
        white: images.add(crate::sprites::white_image()),
        glow: images.add(crate::sprites::glow_image()),
    });
    turnrush_core::logger::kv(
        turnrush_core::logger::Level::Info,
        "sprites_generated",
        "sprites procedurais gerados em runtime",
        &[("units", turnrush_core::logger::V::U(run.roster.len() as u64))],
    );
}

/// Câmera 2D — SEM ela o Bevy UI não renderiza nada (janela em branco).
fn spawn_camera(mut cmds: Commands) {
    cmds.spawn(Camera2d);
}

/// Substitui a fonte default (subset ASCII do Bevy) pela DejaVu Sans Mono
/// embutida — vale para todo `TextFont` sem fonte explícita.
fn override_default_font(mut fonts: ResMut<Assets<Font>>) {
    let fonte = Font::from_bytes(DEFAULT_FONT_TTF.to_vec());
    fonts
        .insert(AssetId::default(), fonte)
        .expect("falha ao substituir a fonte default");
}

/// `base.png` → `base_menu.png` (sufixo antes da extensão; sem extensão → .png).
fn variante(base: &str, sufixo: &str) -> String {
    let (stem, ext) = match base.rfind('.') {
        // ponto só é extensão se não fizer parte de um diretório (ex.: "./x")
        Some(i) if !base[i..].contains('/') => (&base[..i], &base[i + 1..]),
        _ => (base, "png"),
    };
    format!("{stem}{sufixo}.{ext}")
}

/// PNG já gravado (não vazio) no disco?
fn gravado(path: &str) -> bool {
    std::path::Path::new(path)
        .metadata()
        .map(|m| m.len() > 0)
        .unwrap_or(false)
}

/// Diagnóstico de UI: estado das raízes + câmera default (log de sessão).
fn logar_ui(
    roots: &Query<(&StateRoot, &Visibility, &InheritedVisibility, &ViewVisibility)>,
    ui_nodes: &Query<(), With<Node>>,
    cameras: &Query<(), With<Camera2d>>,
    default_ui_camera: &DefaultUiCamera,
) {
    for (root, vis, inh, view) in roots.iter() {
        turnrush_core::logger::kv(
            turnrush_core::logger::Level::Info,
            "ui_root",
            "raiz de estado",
            &[
                ("estado", turnrush_core::logger::V::S(&format!("{:?}", root.0))),
                ("vis", turnrush_core::logger::V::S(&format!("{:?}", vis))),
                ("inherited", turnrush_core::logger::V::S(&format!("{:?}", *inh))),
                ("view", turnrush_core::logger::V::S(&format!("{:?}", *view))),
            ],
        );
    }
    turnrush_core::logger::kv(
        turnrush_core::logger::Level::Info,
        "ui_stats",
        "resumo da UI",
        &[
            ("nos_ui", turnrush_core::logger::V::U(ui_nodes.iter().count() as u64)),
            ("cameras_2d", turnrush_core::logger::V::U(cameras.iter().count() as u64)),
            ("cam_ui_default", turnrush_core::logger::V::S(&format!("{:?}", default_ui_camera.get()))),
        ],
    );
}

/// Ciclo do modo `--shot`: navega menu → draft → batalha, captura um PNG de
/// cada tela e sai. Exercita `draft_enter`/`battle_enter` em runtime.
#[allow(clippy::too_many_arguments)]
fn shot_tick(
    time: Res<Time>,
    mut cmds: Commands,
    mut exit: MessageWriter<AppExit>,
    mut st: Option<ResMut<ShotState>>,
    mut next: ResMut<NextState<GameState>>,
    mut run: ResMut<RunRes>,
    mut pb: ResMut<PlaybackRes>,
    roots: Query<(&StateRoot, &Visibility, &InheritedVisibility, &ViewVisibility)>,
    ui_nodes: Query<(), With<Node>>,
    cameras: Query<(), With<Camera2d>>,
    default_ui_camera: DefaultUiCamera,
) {
    let Some(st) = st.as_deref_mut() else { return };
    st.frames += 1;
    st.timer -= time.delta_secs();

    let menu_png = variante(&st.base, "_menu");
    let ajuda_png = variante(&st.base, "_como-jogar");
    let elenco_png = variante(&st.base, "_elenco");
    let draft_png = variante(&st.base, "_draft");
    let batalha_png = variante(&st.base, "_battle");
    let logger = turnrush_core::logger::kv;
    let (info, s, u) = (
        turnrush_core::logger::Level::Info,
        turnrush_core::logger::V::S,
        turnrush_core::logger::V::U,
    );

    match st.phase {
        ShotPhase::Menu if st.timer <= 0.0 => {
            logar_ui(&roots, &ui_nodes, &cameras, &default_ui_camera);
            cmds
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(menu_png.clone()));
            logger(info, "screenshot", "captura disparada", &[("path", s(&menu_png))]);
            st.timer = 5.0;
            st.phase = ShotPhase::MenuSalvo;
        }
        ShotPhase::MenuSalvo if gravado(&menu_png) || st.timer <= 0.0 => {
            next.set(GameState::ComoJogar);
            st.timer = 1.2;
            st.phase = ShotPhase::Ajuda;
        }
        ShotPhase::Ajuda if st.timer <= 0.0 => {
            cmds
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(ajuda_png.clone()));
            logger(info, "screenshot", "captura disparada", &[("path", s(&ajuda_png))]);
            st.timer = 5.0;
            st.phase = ShotPhase::AjudaSalvo;
        }
        ShotPhase::AjudaSalvo if gravado(&ajuda_png) || st.timer <= 0.0 => {
            next.set(GameState::Elenco);
            st.timer = 1.2;
            st.phase = ShotPhase::Elenco;
        }
        ShotPhase::Elenco if st.timer <= 0.0 => {
            cmds
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(elenco_png.clone()));
            logger(info, "screenshot", "captura disparada", &[("path", s(&elenco_png))]);
            st.timer = 5.0;
            st.phase = ShotPhase::ElencoSalvo;
        }
        ShotPhase::ElencoSalvo if gravado(&elenco_png) || st.timer <= 0.0 => {
            next.set(GameState::Draft);
            st.timer = 1.2;
            st.phase = ShotPhase::Draft;
        }
        ShotPhase::Draft if st.timer <= 0.0 => {
            cmds
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(draft_png.clone()));
            logger(info, "screenshot", "captura disparada", &[("path", s(&draft_png))]);
            st.timer = 5.0;
            st.phase = ShotPhase::DraftSalvo;
        }
        ShotPhase::DraftSalvo if gravado(&draft_png) || st.timer <= 0.0 => {
            // formação fixa: exercita build_battle + battle_enter sem interação
            run.team = vec![0, 1, 2, 3];
            run.enemy = vec![4, 5, 6, 7];
            build_battle(&mut run, &mut pb, &mut cmds);
            next.set(GameState::Battle);
            st.timer = 2.6;
            st.phase = ShotPhase::Batalha;
        }
        ShotPhase::Batalha if st.timer <= 0.0 => {
            cmds
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(batalha_png.clone()));
            logger(info, "screenshot", "captura disparada", &[("path", s(&batalha_png))]);
            st.timer = 5.0;
            st.phase = ShotPhase::BatalhaSalvo;
        }
        ShotPhase::BatalhaSalvo if gravado(&batalha_png) || st.timer <= 0.0 => {
            logger(info, "shot_exit", "modo --shot encerrando", &[("frames", u(st.frames))]);
            turnrush_core::logger::shutdown();
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

fn new_run() -> RunRes {
    let seed = turnrush_core::logger::unix_nanos() as u64;
    let roster = load_roster_embedded();
    turnrush_core::logger::kv(
        turnrush_core::logger::Level::Info,
        "roster_load",
        "roster carregado (embutido no binário)",
        &[("personagens", turnrush_core::logger::V::U(roster.len() as u64))],
    );
    RunRes {
        roster,
        rng: Rng::new(seed),
        team: Vec::new(),
        enemy: Vec::new(),
        mods: FormationMods::default(),
        wins: 0,
        rounds_total: 0,
        chains_total: 0,
    }
}

/// Roster embutido no binário (sem depender de arquivos externos).
pub fn load_roster_embedded() -> Roster {
    match Roster::from_json(include_str!("../../../data/roster/roster-v2.json")) {
        Ok(r) => r,
        Err(_) => Roster::builtin(),
    }
}

fn new_playback_placeholder() -> PlaybackRes {
    PlaybackRes {
        events: Vec::new(),
        idx: 0,
        wait: 0.0,
        finished: false,
        outcome: Outcome::Draw,
        chains: 0,
        rounds: 0,
        map: HashMap::new(),
        max_hp: [1; 8],
        taken: [0; 8],
        healed: [0; 8],
        alive: [true; 8],
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Raízes de UI (uma por estado, visibilidade sincronizada)
// ─────────────────────────────────────────────────────────────────────────────

fn sync_root_visibility(state: Res<State<GameState>>, mut roots: Query<(&StateRoot, &mut Visibility)>) {
    if !state.is_changed() {
        return;
    }
    for (root, mut vis) in roots.iter_mut() {
        *vis = if root.0 == **state { Visibility::Visible } else { Visibility::Hidden };
    }
}

fn node_full() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    }
}

/// Botão padrão com marcador; devolve a entidade raiz.
fn btn<M: Component>(p: &mut ChildSpawnerCommands, label: &str, w: f32, marker: M) -> Entity {
    let text = p
        .spawn((Text::new(label), TextFont { font_size: FontSize::Px(22.0), ..default() }, TextColor(TXT)))
        .id();
    p.spawn((
        Interaction::default(),
        marker,
        Node {
            width: Val::Px(w),
            height: Val::Px(52.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(AMBER),
    ))
    .add_child(text)
    .id()
}

fn setup_roots(tex: Res<SpriteAssets>, mut cmds: Commands) {
    // ── Menu ──
    cmds.spawn((node_full(), StateRoot(GameState::Menu), Visibility::Visible))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(28.0),
                    ..default()
                },
            ))
            .with_children(|p| {
                p.spawn((Text::new("TURNRUSH"), TextFont { font_size: px(72.0), ..default() }, TextColor(AMBER)));
                p.spawn((
                    Text::new("rodadas rápidas · cadeias de combo · draft simétrico"),
                    TextFont { font_size: px(20.0), ..default() },
                    TextColor(TXT_DIM),
                ));
                // faixa de identidade dos 4 clãs
                p.spawn((Node {
                    column_gap: Val::Px(8.0),
                    padding: UiRect::vertical(Val::Px(2.0)),
                    ..default()
                },))
                .with_children(|strip| {
                    for f in ["ember", "tide", "verdant", "storm"] {
                        strip.spawn((
                            Node { width: Val::Px(42.0), height: Val::Px(7.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
                            BackgroundColor(faction_color(f)),
                        ));
                    }
                });
                btn(p, "Jogar", 340.0, MenuBtn(MenuAction::Play));
                btn(p, "Como Jogar", 340.0, MenuBtn(MenuAction::Help));
                btn(p, "Elenco", 340.0, MenuBtn(MenuAction::Elenco));
                btn(p, "Sair", 340.0, MenuBtn(MenuAction::Exit));
                p.spawn((
                    Text::new(format!(
                        "v{} · engine determinística em Rust · logs em ~/TurnRush/logs",
                        env!("CARGO_PKG_VERSION")
                    )),
                    TextFont { font_size: FontSize::Px(14.0), ..default() },
                    TextColor(TXT_DIM),
                ));
                p.spawn((
                    Text::new("modo offline — você comanda cada ação · adversário = IA do engine"),
                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                    TextColor(TXT_DIM),
                ));
            });
        });

    // ── Como Jogar ──
    cmds.spawn((
        node_full(),
        StateRoot(GameState::ComoJogar),
        Visibility::Hidden,
    ))
    .with_children(|p| {
        p.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(22.0)),
                row_gap: Val::Px(12.0),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("COMO JOGAR"),
                TextFont { font_size: FontSize::Px(32.0), ..default() },
                TextColor(AMBER),
            ));
            p.spawn((
                Node {
                    width: Val::Px(1040.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(9.0),
                    ..default()
                },
            ))
            .with_children(|p| {
                for (titulo, corpo) in AJUDA {
                    p.spawn((
                        Text::new(titulo),
                        TextFont { font_size: FontSize::Px(16.0), ..default() },
                        TextColor(TXT),
                    ));
                    p.spawn((
                        Text::new(corpo),
                        TextFont { font_size: FontSize::Px(14.0), ..default() },
                        TextColor(TXT_DIM),
                    ));
                }
            });
            btn(p, "Voltar", 240.0, BackBtn);
        });
    });

    // ── Elenco (esboço do roster completo — a arte final vem depois) ──
    let elenco = load_roster_embedded();
    cmds.spawn((
        node_full(),
        StateRoot(GameState::Elenco),
        Visibility::Hidden,
    ))
    .with_children(|p| {
        p.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(format!(
                    "ELENCO — {} cartas (esboço: sprites genéricos e números provisórios)",
                    elenco.len()
                )),
                TextFont { font_size: px(24.0), ..default() },
                TextColor(TXT),
            ));
            p.spawn((
                Node {
                    display: Display::Grid,
                    grid_template_columns: RepeatedGridTrack::px(4, 292.0),
                    grid_auto_rows: GridTrack::px(150.0),
                    column_gap: Val::Px(10.0),
                    row_gap: Val::Px(10.0),
                    ..default()
                },
            ))
            .with_children(|grid| {
                for (i, c) in elenco.characters.iter().enumerate() {
                    spawn_elenco_card(grid, &tex, i, c);
                }
            });
            btn(p, "Voltar", 240.0, BackBtn);
        });
    });

    // ── Draft ──
    let tex_draft = tex.clone();
    cmds.spawn((node_full(), StateRoot(GameState::Draft), Visibility::Hidden))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(24.0)),
                    row_gap: Val::Px(16.0),
                    ..default()
                },
            ))
            .with_children(|p| {
                p.spawn((Text::new("MONTE SUA FORMAÇÃO — escolha 4"), TextFont { font_size: px(30.0), ..default() }, TextColor(TXT)));
                p.spawn((DraftCounter, Text::new("0/4"), TextFont { font_size: px(22.0), ..default() }, TextColor(AMBER)));
                p.spawn((
                    Node {
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::px(4, 270.0),
                        grid_auto_rows: GridTrack::px(126.0),
                        column_gap: Val::Px(14.0),
                        row_gap: Val::Px(14.0),
                        ..default()
                    },
                ))
                .with_children(|grid| {
                    for _ in 0..8 {
                        spawn_draft_card(grid, &tex_draft);
                    }
                });
                btn(p, "LUTAR", 300.0, DraftConfirm);
            });
        });

    // ── Batalha ──
    // coluna: header em cima, painéis no meio, log embaixo
    // (Node::default() é ROW — sem isso tudo empilha lado a lado!)
    cmds.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..node_full()
        },
        StateRoot(GameState::Battle),
        Visibility::Hidden,
    ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(44.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ))
            .with_children(|p| {
                p.spawn((BattleHeader, Text::new("Batalha"), TextFont { font_size: FontSize::Px(26.0), ..default() }, TextColor(TXT)));
                // botão Acelerar (canto direito do header)
                let lbl = p
                    .spawn((
                        SpeedLabel,
                        Text::new("Acelerar ▶"),
                        TextFont { font_size: FontSize::Px(14.0), ..default() },
                        TextColor(TXT_DIM),
                    ))
                    .id();
                p.spawn((
                    Interaction::default(),
                    SpeedBtn,
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(24.0),
                        top: Val::Px(4.0),
                        width: Val::Px(160.0),
                        height: Val::Px(34.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(PANEL),
                    BorderColor::all(TXT_DIM),
                ))
                .add_child(lbl);
                // rótulos laterais do header (vitórias da run e cadeias ao vivo)
                p.spawn((
                    WinsText,
                    Text::new(""),
                    TextFont { font_size: px(15.0), ..default() },
                    TextColor(AMBER),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(24.0),
                        top: Val::Px(10.0),
                        ..default()
                    },
                ));
                p.spawn((
                    ChainsText,
                    Text::new("Chains: 0"),
                    TextFont { font_size: px(15.0), ..default() },
                    TextColor(TXT_DIM),
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(210.0),
                        top: Val::Px(10.0),
                        ..default()
                    },
                ));
            });
            // campo aberto — a cena 2D (scene.rs) desenha a batalha no mundo
            p.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(72.0), ..default() },));
            p.spawn((
                Node {
                    width: Val::Percent(92.0),
                    height: Val::Percent(20.0),
                    margin: UiRect::horizontal(Val::Percent(4.0)),
                    padding: UiRect::all(Val::Px(10.0)),
                    overflow: Overflow::clip(),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(PANEL_DARK),
            ))
            .with_children(|lp| {
                lp.spawn((BattleLogText, Text::new(""), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(TXT_DIM)));
            });

            // ── Painel de decisão manual (v0.6 — modo Jogar) ──
            // Overlay absoluto na base: aparece só quando uma decisão do
            // jogador está pendente (cobre parte do log durante a escolha).
            p.spawn((
                DecPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(4.0),
                    bottom: Val::Px(10.0),
                    width: Val::Percent(92.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(PANEL_DARK),
                BorderColor::all(AMBER),
                Visibility::Hidden,
            ))
            .with_children(|dp| {
                dp.spawn((
                    DecTitleText,
                    Text::new("Sua vez"),
                    TextFont { font_size: px(16.0), ..default() },
                    TextColor(AMBER),
                ));
                dp.spawn((Node { column_gap: Val::Px(8.0), ..default() },)).with_children(|row| {
                    let ml = row
                        .spawn((
                            DecMysteryLabel,
                            Text::new("Mystery"),
                            TextFont { font_size: px(14.0), ..default() },
                            TextColor(TXT),
                        ))
                        .id();
                    row.spawn((
                        Interaction::default(),
                        DecBtn(DecKind::Mystery),
                        Node {
                            width: Val::Px(330.0),
                            height: Val::Px(40.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(PANEL),
                        BorderColor::all(AMBER),
                    ))
                    .add_child(ml);
                    let sl = row
                        .spawn((
                            DecStandardLabel,
                            Text::new("Padrão"),
                            TextFont { font_size: px(14.0), ..default() },
                            TextColor(TXT),
                        ))
                        .id();
                    row.spawn((
                        Interaction::default(),
                        DecBtn(DecKind::Standard),
                        Node {
                            width: Val::Px(280.0),
                            height: Val::Px(40.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(PANEL),
                        BorderColor::all(TXT_DIM),
                    ))
                    .add_child(sl);
                    let al = row
                        .spawn((Text::new("Auto ▶"), TextFont { font_size: px(14.0), ..default() }, TextColor(TXT_DIM)))
                        .id();
                    row.spawn((
                        Interaction::default(),
                        DecBtn(DecKind::Auto),
                        Node {
                            width: Val::Px(90.0),
                            height: Val::Px(40.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(PANEL),
                        BorderColor::all(TXT_DIM),
                    ))
                    .add_child(al);
                });
                dp.spawn((
                    DecTargetRow,
                    Node { column_gap: Val::Px(8.0), ..default() },
                    Visibility::Hidden,
                ))
                .with_children(|tr| {
                    for slot in 0..4usize {
                        let tl = tr
                            .spawn((
                                DecTargetLabel(slot),
                                Text::new("—"),
                                TextFont { font_size: px(12.0), ..default() },
                                TextColor(TXT),
                            ))
                            .id();
                        tr.spawn((
                            Interaction::default(),
                            DecTargetBtn(slot),
                            Node {
                                width: Val::Px(185.0),
                                height: Val::Px(34.0),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(6.0)),
                                ..default()
                            },
                            BackgroundColor(PANEL),
                            BorderColor::all(TXT_DIM),
                        ))
                        .add_child(tl);
                    }
                    let ta = tr
                        .spawn((Text::new("Auto-alvo"), TextFont { font_size: px(12.0), ..default() }, TextColor(TXT_DIM)))
                        .id();
                    tr.spawn((
                        Interaction::default(),
                        DecTargetAutoBtn,
                        Node {
                            width: Val::Px(110.0),
                            height: Val::Px(34.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(PANEL),
                        BorderColor::all(TXT_DIM),
                    ))
                    .add_child(ta);
                });
            });
        });

    // ── Recompensa ──
    cmds.spawn((node_full(), StateRoot(GameState::Reward), Visibility::Hidden))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(26.0),
                    ..default()
                },
            ))
            .with_children(|p| {
                p.spawn((Text::new("VITÓRIA"), TextFont { font_size: FontSize::Px(48.0), ..default() }, TextColor(GOOD)));
                p.spawn((
                    Text::new("Escolha um aprimoramento para a próxima batalha:"),
                    TextFont { font_size: FontSize::Px(20.0), ..default() },
                    TextColor(TXT),
                ));
                btn(p, "Ataque +10%", 340.0, RewardBtn(RewardKind::Dano));
                btn(p, "Vida +10%", 340.0, RewardBtn(RewardKind::Vida));
                btn(p, "Equilíbrio +5%/+5%", 340.0, RewardBtn(RewardKind::Equilibrio));
            });
        });

    // ── Fim de run ──
    cmds.spawn((node_full(), StateRoot(GameState::GameOver), Visibility::Hidden))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(24.0),
                    ..default()
                },
            ))
            .with_children(|p| {
                p.spawn((GameOverText, Text::new("Fim"), TextFont { font_size: FontSize::Px(40.0), ..default() }, TextColor(TXT)));
                btn(p, "Jogar de novo", 320.0, MenuBtn(MenuAction::Again));
                btn(p, "Sair", 320.0, MenuBtn(MenuAction::Exit));
            });
        });
}

/// Carta do draft: sprite genérico + nome + resumo (imagem trocada no draft_enter).
fn spawn_draft_card(grid: &mut ChildSpawnerCommands, tex: &SpriteAssets) {
    let sprite = grid
        .spawn((
            Node { width: Val::Px(52.0), height: Val::Px(52.0), margin: UiRect::right(Val::Px(10.0)), ..default() },
            ImageNode::new(tex.units[0].clone()),
        ))
        .id();
    let name = grid.spawn((Text::new("—"), TextFont { font_size: px(18.0), ..default() }, TextColor(TXT))).id();
    let stat = grid.spawn((Text::new(""), TextFont { font_size: px(12.0), ..default() }, TextColor(TXT_DIM))).id();
    let col = grid.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() },))
        .add_child(name)
        .add_child(stat)
        .id();
    grid.spawn((
        Interaction::default(),
        DraftCard { roster_idx: None, name_text: name, stat_text: stat, sprite_node: sprite },
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(TXT_DIM),
    ))
    .add_child(sprite)
    .add_child(col);
}

// ─────────────────────────────────────────────────────────────────────────────
// Menu
// ─────────────────────────────────────────────────────────────────────────────

fn menu_interactions(
    mut cmds: Commands,
    mut state: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
    q: Query<(&Interaction, &MenuBtn), Changed<Interaction>>,
) {
    for (inter, b) in q.iter() {
        if *inter != Interaction::Pressed {
            continue;
        }
        match b.0 {
            MenuAction::Play => state.set(GameState::Draft),
            MenuAction::Help => state.set(GameState::ComoJogar),
            MenuAction::Elenco => state.set(GameState::Elenco),
            MenuAction::Again => {
                cmds.insert_resource(new_run());
                state.set(GameState::Draft);
            }
            MenuAction::Exit => {
                turnrush_core::logger::shutdown();
                exit.write(AppExit::Success);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Draft
// ─────────────────────────────────────────────────────────────────────────────

fn draft_enter(
    mut run: ResMut<RunRes>,
    mut draft: ResMut<DraftRes>,
    mut cmds: Commands,
    tex: Res<SpriteAssets>,
    mut cards: Query<(Entity, &mut DraftCard, &mut BorderColor)>,
    mut texts: Query<&mut Text>,
    mut sprites: Query<&mut ImageNode>,
) {
    let n = run.roster.len();
    let mut idxs: Vec<usize> = (0..n).collect();
    run.rng.shuffle(&mut idxs);
    let mut offers = [None; 8];
    for (i, o) in idxs.iter().take(8).enumerate() {
        offers[i] = Some(*o);
    }
    draft.offers = offers;

    for (i, (e, mut card, mut border)) in cards.iter_mut().enumerate() {
        card.roster_idx = offers[i];
        *border = BorderColor::all(TXT_DIM);
        cmds.entity(e).remove::<CardSelected>();
        if let Some(ri) = offers[i] {
            let c = &run.roster.characters[ri];
            if let Ok(mut t) = texts.get_mut(card.name_text) {
                *t = Text::new(c.name.clone());
            }
            if let Ok(mut t) = texts.get_mut(card.stat_text) {
                *t = Text::new(format!(
                    "{} · {}\nHP {} · ATQ {} · DEF {}",
                    faction_display(c.faction.as_str()),
                    role_display(c.role.as_str()),
                    c.hp,
                    c.attack,
                    c.defense
                ));
            }
            if let Ok(mut img) = sprites.get_mut(card.sprite_node) {
                img.image = tex.units[ri].clone();
            }
        }
    }
}

type DraftClickQuery<'a> = (
    Entity,
    &'a Interaction,
    &'a mut BorderColor,
);

fn draft_card_click(
    mut cmds: Commands,
    mut cards: Query<DraftClickQuery, (With<DraftCard>, Changed<Interaction>)>,
    selected: Query<(), (With<CardSelected>, With<DraftCard>)>,
    mut texts: Query<&mut Text, With<DraftCounter>>,
) {
    let mut mudou = false;
    let mut sel_count = selected.iter().count();
    for (e, inter, mut border) in cards.iter_mut() {
        if *inter != Interaction::Pressed {
            continue;
        }
        if selected.get(e).is_ok() {
            cmds.entity(e).remove::<CardSelected>();
            *border = BorderColor::all(TXT_DIM);
            sel_count = sel_count.saturating_sub(1);
            mudou = true;
        } else if sel_count < 4 {
            cmds.entity(e).insert(CardSelected);
            *border = BorderColor::all(AMBER);
            sel_count += 1;
            mudou = true;
        }
    }
    if mudou {
        if let Ok(mut t) = texts.single_mut() {
            *t = Text::new(format!("{sel_count}/4"));
        }
    }
}

fn draft_confirm(
    mut cmds: Commands,
    mut run: ResMut<RunRes>,
    mut playback: ResMut<PlaybackRes>,
    mut manual: ResMut<ManualRes>,
    mut pending: ResMut<PendingUi>,
    cards: Query<&DraftCard, With<CardSelected>>,
    mut state: ResMut<NextState<GameState>>,
) {
    let selected: Vec<usize> = cards.iter().filter_map(|c| c.roster_idx).collect();
    if selected.len() != 4 {
        return;
    }
    // inimigo: distinto do time e entre si (garante nomes únicos no playback)
    let mut rest: Vec<usize> = (0..run.roster.len()).filter(|i| !selected.contains(i)).collect();
    run.rng.shuffle(&mut rest);
    run.team = selected;
    run.enemy = rest.into_iter().take(4).collect();
    pending.active = false;
    pending.choosing_target = false;
    build_manual(&mut run, &mut playback, &mut manual, &mut cmds);
    state.set(GameState::Battle);
}

// ─────────────────────────────────────────────────────────────────────────────
// Batalha
// ─────────────────────────────────────────────────────────────────────────────

fn build_battle(run: &mut RunRes, pb: &mut PlaybackRes, cmds: &mut Commands) {
    let (a, b) = times_da_run(run);
    let (outcome, chains, rounds, events, seed) = rodar_batalha_auto(run, &a, &b);

    let nomes = |comp: &[usize; 4]| -> String {
        comp.iter().map(|&i| run.roster.characters[i].name.clone()).collect::<Vec<_>>().join(", ")
    };
    turnrush_core::logger::kv(
        turnrush_core::logger::Level::Info,
        "battle_start",
        "batalha 4v4 gerada (playback)",
        &[
            ("time_a", turnrush_core::logger::V::S(&nomes(&a))),
            ("time_b", turnrush_core::logger::V::S(&nomes(&b))),
            ("seed", turnrush_core::logger::V::U(seed)),
            ("mods_a", turnrush_core::logger::V::S(&format!("{}/{}", run.mods.dmg_permille, run.mods.hp_permille))),
        ],
    );

    preencher_playback_base(run, pb, &a, &b);
    pb.events = events;
    pb.outcome = outcome;
    pb.chains = chains;
    pb.rounds = rounds;
    cmds.insert_resource(BattleMode::Playback);
}

/// Times (a, b) a partir do estado da run.
fn times_da_run(run: &RunRes) -> ([usize; 4], [usize; 4]) {
    (
        [run.team[0], run.team[1], run.team[2], run.team[3]],
        [run.enemy[0], run.enemy[1], run.enemy[2], run.enemy[3]],
    )
}

/// Roda a batalha até o fim (modo automático) e devolve (outcome, cadeias,
/// rodadas, eventos parseados, seed).
fn rodar_batalha_auto(
    run: &mut RunRes,
    a: &[usize; 4],
    b: &[usize; 4],
) -> (Outcome, u64, u64, Vec<ParsedEvent>, u64) {
    let seed = run.rng.next_u64();
    let enemy_mods = FormationMods {
        dmg_permille: 1000 + (run.wins as i32) * 110,
        hp_permille: 1000 + (run.wins as i32) * 70,
    };
    let mut f = Formation4v4::new_with_mods(&run.roster, *a, *b, seed, run.mods, enemy_mods);
    // IA adversária: ordem de movimento sorteada por batalha — cada confronto
    // ganha variância tática real (a ordem do jogador continua sendo a do
    // draft; UI de ordem configurável fica para a v0.7).
    let mut ordem_ia = [0usize, 1, 2, 3];
    run.rng.shuffle(&mut ordem_ia);
    f.set_orders([0, 1, 2, 3], ordem_ia);
    f.set_log(true);
    let outcome = f.run();
    let chains = f.chain_links();
    let rounds = f.rounds() as u64;
    let events = parse_all(f.events());
    (outcome, chains, rounds, events, seed)
}

/// Batalha MANUAL: engine vivo por passos, lado 0 = jogador.
fn build_manual(run: &mut RunRes, pb: &mut PlaybackRes, manual: &mut ManualRes, cmds: &mut Commands) {
    let (a, b) = times_da_run(run);
    let seed = run.rng.next_u64();
    let enemy_mods = FormationMods {
        dmg_permille: 1000 + (run.wins as i32) * 110,
        hp_permille: 1000 + (run.wins as i32) * 70,
    };
    let mut f = Formation4v4::new_with_mods(&run.roster, a, b, seed, run.mods, enemy_mods);
    // IA adversária: ordem sorteada; a do jogador segue a do draft (slots).
    let mut ordem_ia = [0usize, 1, 2, 3];
    run.rng.shuffle(&mut ordem_ia);
    f.set_orders([0, 1, 2, 3], ordem_ia);
    f.set_manual_side(Some(0));
    f.set_log(true);
    *manual = ManualRes { f: Some(f), ev_cursor: 0, parsed: Vec::new(), auto: false };
    cmds.insert_resource(BattleMode::Manual);

    let nomes = |comp: &[usize; 4]| -> String {
        comp.iter().map(|&i| run.roster.characters[i].name.clone()).collect::<Vec<_>>().join(", ")
    };
    turnrush_core::logger::kv(
        turnrush_core::logger::Level::Info,
        "battle_start",
        "batalha 4v4 gerada (manual)",
        &[
            ("time_a", turnrush_core::logger::V::S(&nomes(&a))),
            ("time_b", turnrush_core::logger::V::S(&nomes(&b))),
            ("seed", turnrush_core::logger::V::U(seed)),
            ("mods_a", turnrush_core::logger::V::S(&format!("{}/{}", run.mods.dmg_permille, run.mods.hp_permille))),
        ],
    );

    preencher_playback_base(run, pb, &a, &b);
}

/// Base do PlaybackRes (mapa nome→índice, HP máx) comum aos dois modos.
fn preencher_playback_base(run: &RunRes, pb: &mut PlaybackRes, a: &[usize; 4], b: &[usize; 4]) {
    let mut map = HashMap::new();
    let mut max_hp = [1i64; 8];
    for (s, comp) in [(0usize, a), (1usize, b)] {
        for (k, &ci) in comp.iter().enumerate() {
            let c = &run.roster.characters[ci];
            map.insert(c.name.clone(), (s, k));
            max_hp[s * 4 + k] = (c.hp as i64).max(1);
        }
    }
    *pb = PlaybackRes {
        events: Vec::new(),
        idx: 0,
        wait: 0.6,
        finished: false,
        outcome: Outcome::Draw,
        chains: 0,
        rounds: 0,
        map,
        max_hp,
        taken: [0; 8],
        healed: [0; 8],
        alive: [true; 8],
    };
}

/// Query do log de batalha — alias para satisfazer o clippy (type_complexity).
type BattleLogTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (With<BattleLogText>, Without<BattleHeader>),
>;

/// Alias de query do battle_enter (legibilidade + clippy).
type BattleHdrQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (With<BattleHeader>, Without<BattleLogText>, Without<WinsText>),
>;
/// Alias de query do battle_enter (legibilidade + clippy).
type BattleWinsQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (With<WinsText>, Without<BattleHeader>, Without<BattleLogText>),
>;

fn battle_enter(run: Res<RunRes>, mut header: BattleHdrQuery, mut log: BattleLogTextQuery, mut wins: BattleWinsQuery) {
    if let Ok(mut t) = header.single_mut() {
        *t = Text::new("Batalha — rodada 1");
    }
    if let Ok(mut t) = log.single_mut() {
        *t = Text::new("");
    }
    if let Ok(mut t) = wins.single_mut() {
        *t = Text::new(format!(
            "Vitórias {}/{} · inimigo +{}% dano +{}% vida",
            run.wins,
            VICTORIES_TO_WIN,
            run.wins * 11,
            run.wins * 7
        ));
    }
}

/// Aplica um evento ao estado visual (HP, mortes) e anexa no log.
fn apply_event(pb: &mut PlaybackRes, ev: &ParsedEvent, log: &mut String) {
    match ev.kind {
        EvKind::Attack | EvKind::Damage | EvKind::State | EvKind::Reflect | EvKind::Chase => {
            if let Some(t) = ev.target.as_deref().and_then(|n| pb.unit(n)) {
                pb.taken[t] += ev.amount.max(0);
            }
        }
        EvKind::Heal => {
            if let Some(t) = ev.target.as_deref().and_then(|n| pb.unit(n)) {
                pb.healed[t] += ev.amount.max(0);
            }
        }
        EvKind::Death => {
            if let Some(t) = ev.target.as_deref().and_then(|n| pb.unit(n)) {
                pb.alive[t] = false;
                pb.taken[t] = pb.max_hp[t] * 2; // barra a zero
            }
        }
        _ => {}
    }
    if log.is_empty() {
        log.push_str(&ev.text);
    } else {
        log.push('\n');
        log.push_str(&ev.text);
    }
    // mantém as últimas 12 linhas
    let lines: Vec<&str> = log.lines().collect();
    if lines.len() > 12 {
        *log = lines[lines.len() - 12..].join("\n");
    }
}

/// Playback dos eventos da batalha: HP/log (estado) + fila de eventos
/// visuais para a cena (sprites, animações, banners).
fn battle_playback(
    time: Res<Time>,
    ff: Res<FastForward>,
    mut pb: ResMut<PlaybackRes>,
    mut log: BattleLogTextQuery,
    mut header: BattleHdrQuery,
    mut queue: ResMut<FxQueue>,
) {
    if pb.finished {
        pb.wait -= time.delta_secs();
        return;
    }
    pb.wait -= time.delta_secs();
    if pb.wait > 0.0 {
        return;
    }
    if pb.idx >= pb.events.len() {
        pb.finished = true;
        pb.wait = 1.1;
        return;
    }
    let ev = pb.events[pb.idx].clone();
    pb.idx += 1;
    process_event(&ev, &mut pb, &mut log, &mut header, &mut queue);
    let (passo, rodada) = if ff.0 {
        (STEP_SECS / 6.0, ROUND_SECS / 6.0)
    } else {
        (STEP_SECS, ROUND_SECS)
    };
    pb.wait = if ev.kind == EvKind::Round { rodada } else { passo };
}

/// Aplica um evento parseado: HP/log/cabeçalho + tradução para a cena.
/// Comum ao playback automático e ao modo manual (mesma fidelidade visual).
fn process_event(
    ev: &ParsedEvent,
    pb: &mut PlaybackRes,
    log: &mut BattleLogTextQuery,
    header: &mut BattleHdrQuery,
    queue: &mut FxQueue,
) {
    let mut log_str = if let Ok(t) = log.single_mut() { t.to_string() } else { String::new() };
    apply_event(pb, ev, &mut log_str);
    if let Ok(mut t) = log.single_mut() {
        *t = Text::new(log_str);
    }

    // cabeçalho na troca de rodada
    if ev.kind == EvKind::Round {
        if let Ok(mut h) = header.single_mut() {
            *h = Text::new(format!("Batalha — rodada {}", ev.amount));
        }
    }

    // tradução log → eventos visuais da cena (scene.rs consome no mesmo frame)
    match ev.kind {
        EvKind::Cast | EvKind::Attack | EvKind::Chase => {
            if let Some(u) = ev.source.as_deref().and_then(|n| pb.unit(n)) {
                queue.0.push(VisEvent::Lunge(u));
            }
            if let Some(t) = ev.target.as_deref().and_then(|n| pb.unit(n)) {
                if ev.amount > 0 {
                    queue.0.push(VisEvent::Hit(t, ev.amount));
                }
            }
            if ev.kind == EvKind::Chase {
                queue.0.push(VisEvent::Chain);
            }
        }
        EvKind::Damage | EvKind::Reflect => {
            if let Some(t) = ev.target.as_deref().and_then(|n| pb.unit(n)) {
                if ev.amount > 0 {
                    queue.0.push(VisEvent::Hit(t, ev.amount));
                }
            }
        }
        EvKind::State => {
            if let Some(t) = ev.target.as_deref().and_then(|n| pb.unit(n)) {
                if ev.amount > 0 {
                    queue.0.push(VisEvent::Hit(t, ev.amount));
                }
                let k = estado_icone(&ev.text);
                queue.0.push(VisEvent::Dot(t, k));
            }
        }
        EvKind::Heal => {
            if let Some(t) = ev.target.as_deref().and_then(|n| pb.unit(n)) {
                if ev.amount > 0 {
                    queue.0.push(VisEvent::Heal(t, ev.amount));
                }
            }
        }
        EvKind::Death => {
            if let Some(t) = ev.target.as_deref().and_then(|n| pb.unit(n)) {
                queue.0.push(VisEvent::Death(t));
            }
        }
        EvKind::Round => {
            queue.0.push(VisEvent::Round(ev.amount.max(1) as u64));
        }
        EvKind::End => {
            queue.0.push(VisEvent::End(pb.outcome == Outcome::TeamAWin));
        }
        _ => {}
    }
}

/// Tick da batalha MANUAL: drena eventos com o mesmo pacing do playback,
/// pausa quando uma decisão do jogador é necessária e avança a engine um
/// passo atômico por vez ([`Formation4v4::advance_one`]).
#[allow(clippy::too_many_arguments)]
fn manual_tick(
    time: Res<Time>,
    ff: Res<FastForward>,
    run: Res<RunRes>,
    mut manual: ResMut<ManualRes>,
    mut pb: ResMut<PlaybackRes>,
    mut pending: ResMut<PendingUi>,
    mut log: BattleLogTextQuery,
    mut header: BattleHdrQuery,
    mut queue: ResMut<FxQueue>,
) {
    // desestrutura para satisfazer o borrowck (ResMut deref é opaco)
    let ManualRes { f, ev_cursor, parsed, auto } = &mut *manual;
    let Some(f) = f.as_mut() else { return };
    if pb.finished {
        return; // battle_finish conduz a transição
    }
    pb.wait -= time.delta_secs();
    if pb.wait > 0.0 {
        return;
    }

    // 1) drena um evento por tick (mesmo ritmo do playback automático)
    if *ev_cursor < parsed.len() {
        let ev = parsed[*ev_cursor].clone();
        *ev_cursor += 1;
        process_event(&ev, &mut pb, &mut log, &mut header, &mut queue);
        let (passo, rodada) = if ff.0 {
            (STEP_SECS / 6.0, ROUND_SECS / 6.0)
        } else {
            (STEP_SECS, ROUND_SECS)
        };
        pb.wait = if ev.kind == EvKind::Round { rodada } else { passo };
        return;
    }

    // 2) batalha concluída → entrega o resultado para o battle_finish
    if let Some(outcome) = f.outcome_opt() {
        pb.finished = true;
        pb.wait = 1.1;
        pb.outcome = outcome;
        pb.chains = f.chain_links();
        pb.rounds = f.rounds() as u64;
        return;
    }

    // 3) decisão pendente → painel do jogador (ou resolução automática)
    if f.pending_decision().is_some() {
        if *auto {
            f.auto_resolve_pending();
            pb.wait = 0.12;
            return;
        }
        if !pending.active {
            let p = f.pending_decision().expect("decisão verificada acima");
            let ci = run.team[p.slot.min(run.team.len().saturating_sub(1))];
            let c = &run.roster.characters[ci.min(run.roster.len().saturating_sub(1))];
            pending.active = true;
            pending.choosing_target = false;
            pending.name = p.name.clone();
            pending.chakra = p.chakra;
            pending.mystery_ready = p.mystery_ready;
            pending.mystery_cost = p.mystery_cost;
            pending.mystery_name = c.mystery.name.clone();
            pending.mystery_desc = c.mystery.effect.clone();
            pending.standard_name = c.standard.name.clone();
            for (i, b2) in f.side_brief(1).iter().enumerate() {
                pending.targets[i] = (
                    b2.name.clone(),
                    if b2.hp_max > 0 { b2.hp * 100 / b2.hp_max } else { 0 },
                    b2.alive,
                );
            }
        }
        return; // aguarda o clique do jogador
    }

    // 4) avança um passo atômico e enfileira os eventos novos
    let before = f.events().len();
    f.advance_one();
    for linha in &f.events()[before..] {
        parsed.push(parse_line(linha));
    }
    pb.wait = 0.05;
}

/// Sincroniza visibilidade e textos do painel de decisão manual.
#[allow(clippy::type_complexity)] // queries com Without explícito (B0001)
fn sync_decision_panel(
    pending: Res<PendingUi>,
    mut panel: Query<&mut Visibility, (With<DecPanel>, Without<DecTargetRow>)>,
    mut trow: Query<&mut Visibility, (With<DecTargetRow>, Without<DecPanel>)>,
    // `Without` obrigatório: Bevy exige disjunção PROVÁVEL entre &mut Text
    // do mesmo sistema (With<> sozinho não basta — erro B0001 em runtime).
    mut title: Query<
        &mut Text,
        (With<DecTitleText>, Without<DecMysteryLabel>, Without<DecStandardLabel>, Without<DecTargetLabel>),
    >,
    mut mlabel: Query<
        &mut Text,
        (With<DecMysteryLabel>, Without<DecTitleText>, Without<DecStandardLabel>, Without<DecTargetLabel>),
    >,
    mut slabel: Query<
        &mut Text,
        (With<DecStandardLabel>, Without<DecTitleText>, Without<DecMysteryLabel>, Without<DecTargetLabel>),
    >,
    mut tlabels: Query<
        (&DecTargetLabel, &mut Text),
        (Without<DecTitleText>, Without<DecMysteryLabel>, Without<DecStandardLabel>),
    >,
) {
    if !pending.is_changed() {
        return;
    }
    if let Ok(mut v) = panel.single_mut() {
        *v = if pending.active { Visibility::Visible } else { Visibility::Hidden };
    }
    if let Ok(mut v) = trow.single_mut() {
        *v = if pending.active && pending.choosing_target {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mut t) = title.single_mut() {
        *t = if pending.active {
            Text::new(format!(
                "Sua vez — {} · chakra {}\n{}",
                pending.name, pending.chakra, pending.mystery_desc
            ))
        } else {
            Text::new("Sua vez")
        };
    }
    if let Ok(mut t) = mlabel.single_mut() {
        *t = if pending.mystery_ready {
            Text::new(format!(
                "Mystery «{}» · -{} chakra",
                pending.mystery_name, pending.mystery_cost
            ))
        } else {
            Text::new(format!("Mystery «{}» · indisponível", pending.mystery_name))
        };
    }
    if let Ok(mut t) = slabel.single_mut() {
        *t = Text::new(format!("Padrão «{}» → alvo…", pending.standard_name));
    }
    for (lbl, mut t) in tlabels.iter_mut() {
        let (nome, hp, vivo) = &pending.targets[lbl.0.min(3)];
        *t = if *vivo {
            Text::new(format!("{nome} · {hp}%"))
        } else {
            Text::new(format!("{nome} · caído"))
        };
    }
}

/// Cliques do painel de decisão: Mystery / Padrão (→ escolha de alvo) / Auto.
fn decision_click(
    mut manual: ResMut<ManualRes>,
    mut pending: ResMut<PendingUi>,
    q: Query<(&Interaction, &DecBtn), Changed<Interaction>>,
) {
    if !pending.active {
        return;
    }
    for (inter, b) in q.iter() {
        if *inter != Interaction::Pressed {
            continue;
        }
        match b.0 {
            DecKind::Mystery => {
                if let Some(f) = manual.f.as_mut() {
                    f.submit(ActionChoice::Mystery);
                }
                pending.active = false;
            }
            DecKind::Standard => {
                pending.choosing_target = true;
            }
            DecKind::Auto => {
                manual.auto = true;
                pending.active = false;
            }
        }
        if !pending.active {
            pending.choosing_target = false;
            turnrush_core::logger::kv(
                turnrush_core::logger::Level::Info,
                "manual_decision",
                "decisão manual submetida",
                &[("tipo", turnrush_core::logger::V::S(match b.0 {
                    DecKind::Mystery => "mystery",
                    DecKind::Auto => "auto",
                    DecKind::Standard => "standard",
                }))],
            );
        }
    }
}

/// Clique em alvo inimigo (ataque padrão com alvo escolhido) ou Auto-alvo.
#[allow(clippy::type_complexity)] // query de Interaction com filtro de estado
fn target_click(
    mut manual: ResMut<ManualRes>,
    mut pending: ResMut<PendingUi>,
    targets: Query<(&Interaction, &DecTargetBtn), (Changed<Interaction>, Without<DecTargetAutoBtn>)>,
    autos: Query<&Interaction, (With<DecTargetAutoBtn>, Changed<Interaction>)>,
) {
    if !pending.active || !pending.choosing_target {
        return;
    }
    for (inter, tb) in targets.iter() {
        if *inter != Interaction::Pressed {
            continue;
        }
        let slot = tb.0.min(3);
        if !pending.targets[slot].2 {
            continue; // alvo caído — ignora
        }
        if let Some(f) = manual.f.as_mut() {
            f.submit(ActionChoice::Standard { target: Some(slot) });
            turnrush_core::logger::kv(
                turnrush_core::logger::Level::Info,
                "manual_target",
                "alvo do padrão escolhido",
                &[
                    ("slot", turnrush_core::logger::V::U(slot as u64)),
                    ("nome", turnrush_core::logger::V::S(&pending.targets[slot].0)),
                ],
            );
        }
        pending.active = false;
        pending.choosing_target = false;
        return;
    }
    for inter in autos.iter() {
        if *inter == Interaction::Pressed {
            if let Some(f) = manual.f.as_mut() {
                f.submit(ActionChoice::Standard { target: None });
            }
            pending.active = false;
            pending.choosing_target = false;
            return;
        }
    }
}

/// Ícone de estado pelo texto do log (queimadura/veneno/controle/escudo).
fn estado_icone(txt: &str) -> usize {
    let t = txt.to_lowercase();
    if t.contains("queimadura") {
        0
    } else if t.contains("veneno") {
        1
    } else if t.contains("escudo") {
        3
    } else {
        2 // derrubada/arremessado/empurrão/outros controles
    }
}

/// Botão "Voltar" — volta ao menu a partir de Como Jogar ou Elenco.
fn back_click(
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    q: Query<&Interaction, (With<BackBtn>, Changed<Interaction>)>,
) {
    if !matches!(state.get(), GameState::ComoJogar | GameState::Elenco) {
        return;
    }
    for inter in q.iter() {
        if *inter == Interaction::Pressed {
            next.set(GameState::Menu);
        }
    }
}

/// Botão "Acelerar" — alterna o playback entre 1× e 6×.
fn speed_click(
    state: Res<State<GameState>>,
    mut ff: ResMut<FastForward>,
    q: Query<&Interaction, (With<SpeedBtn>, Changed<Interaction>)>,
    mut label: Query<&mut Text, With<SpeedLabel>>,
) {
    if *state.get() != GameState::Battle {
        return;
    }
    for inter in q.iter() {
        if *inter == Interaction::Pressed {
            ff.0 = !ff.0;
            if let Ok(mut t) = label.single_mut() {
                *t = if ff.0 {
                    Text::new("Acelerando 6× ▶▶")
                } else {
                    Text::new("Acelerar ▶")
                };
            }
            turnrush_core::logger::kv(
                turnrush_core::logger::Level::Info,
                "playback_speed",
                "velocidade do playback alterada",
                &[(
                    "acelerado",
                    turnrush_core::logger::V::S(if ff.0 { "6x" } else { "1x" }),
                )],
            );
        }
    }
}

fn battle_finish(
    mut run: ResMut<RunRes>,
    mut pb: ResMut<PlaybackRes>,
    mut state: ResMut<NextState<GameState>>,
) {
    if !pb.finished || pb.wait > 0.0 {
        return;
    }
    run.rounds_total += pb.rounds;
    run.chains_total += pb.chains;
    turnrush_core::logger::kv(
        turnrush_core::logger::Level::Info,
        "battle_end",
        "batalha concluída",
        &[
            ("resultado", turnrush_core::logger::V::S(&format!("{:?}", pb.outcome))),
            ("cadeias", turnrush_core::logger::V::U(pb.chains)),
            ("rodadas", turnrush_core::logger::V::U(pb.rounds)),
        ],
    );
    let venceu = pb.outcome == Outcome::TeamAWin;
    if venceu {
        run.wins += 1;
    }
    // consome o playback para não reprocessar
    pb.finished = false;
    pb.events.clear();
    pb.idx = 0;
    if venceu && run.wins >= VICTORIES_TO_WIN {
        state.set(GameState::GameOver);
    } else if venceu {
        state.set(GameState::Reward);
    } else {
        state.set(GameState::GameOver);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Recompensa
// ─────────────────────────────────────────────────────────────────────────────

fn reward_click(
    mut cmds: Commands,
    mut run: ResMut<RunRes>,
    mut pb: ResMut<PlaybackRes>,
    mut manual: ResMut<ManualRes>,
    mut pending: ResMut<PendingUi>,
    mut state: ResMut<NextState<GameState>>,
    q: Query<(&Interaction, &RewardBtn), Changed<Interaction>>,
) {
    for (inter, r) in q.iter() {
        if *inter != Interaction::Pressed {
            continue;
        }
        let tipo = match r.0 {
            RewardKind::Dano => {
                run.mods.dmg_permille += 100;
                "dano"
            }
            RewardKind::Vida => {
                run.mods.hp_permille += 100;
                "vida"
            }
            RewardKind::Equilibrio => {
                run.mods.dmg_permille += 50;
                run.mods.hp_permille += 50;
                "equilibrio"
            }
        };
        turnrush_core::logger::kv(
            turnrush_core::logger::Level::Info,
            "reward_picked",
            "aprimoramento escolhido",
            &[("tipo", turnrush_core::logger::V::S(tipo))],
        );
        pending.active = false;
        pending.choosing_target = false;
        build_manual(&mut run, &mut pb, &mut manual, &mut cmds);
        state.set(GameState::Battle);
        break;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fim de run
// ─────────────────────────────────────────────────────────────────────────────

fn gameover_enter(run: Res<RunRes>, mut texts: Query<&mut Text, With<GameOverText>>) {
    let venceu = run.wins >= VICTORIES_TO_WIN;
    let titulo = if venceu { "VITÓRIA COMPLETA!" } else { "DERROTA" };
    if let Ok(mut t) = texts.single_mut() {
        *t = Text::new(format!(
            "{titulo}\nvitórias: {}/{} · rodadas: {} · cadeias de combo: {}",
            run.wins, VICTORIES_TO_WIN, run.rounds_total, run.chains_total
        ));
    }
    turnrush_core::logger::kv(
        turnrush_core::logger::Level::Info,
        "game_end",
        "run encerrada",
        &[
            ("vitorias", turnrush_core::logger::V::U(run.wins as u64)),
            ("rodadas", turnrush_core::logger::V::U(run.rounds_total)),
            ("cadeias", turnrush_core::logger::V::U(run.chains_total)),
        ],
    );
}

fn gameover_click(
    mut cmds: Commands,
    mut state: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
    q: Query<(&Interaction, &MenuBtn), Changed<Interaction>>,
) {
    for (inter, b) in q.iter() {
        if *inter != Interaction::Pressed {
            continue;
        }
        match b.0 {
            MenuAction::Again => {
                cmds.insert_resource(new_run());
                state.set(GameState::Draft);
            }
            MenuAction::Play | MenuAction::Help | MenuAction::Elenco | MenuAction::Exit => {
                turnrush_core::logger::shutdown();
                exit.write(AppExit::Success);
            }
        }
    }
}
