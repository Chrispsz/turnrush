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
use turnrush_core::formation::{Formation4v4, FormationMods, Outcome};
use turnrush_core::interactive::{ActionKind, BattleEvent, ChosenAction, SideEdge};
use turnrush_core::rng::Rng;

use crate::controller::{BattleController, CtlPhase};

use crate::battle_log::{parse_all, EvKind, ParsedEvent};
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
    /// Cartas da run (roguelike v0.7): estado + alvos das cartas de unidade
    /// (slot, índice da carta no pool).
    pub cards: turnrush_core::rewards::RunState,
    pub card_targets: Vec<(usize, usize)>,
}

impl RunRes {
    /// Vantagem acumulada das cartas para o lado do jogador.
    pub fn cards_edge(&self) -> SideEdge {
        self.cards.edge(&self.card_targets)
    }
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
    /// Fluxo corrente por unidade (modo manual v0.7; índice = lado*4+slot).
    pub fluxo: [i32; 8],
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

/// Botão de carta (índice na oferta 0..3).
#[derive(Component)]
pub struct RewardBtn(pub usize);

/// Subtítulo da tela de cartas.
#[derive(Component)]
pub struct RewardSubtitle;

/// Linha de escolha de alvo para cartas de unidade.
#[derive(Component)]
pub struct RewardTargetRow;

/// Rótulo do botão de alvo aliado.
#[derive(Component)]
pub struct RewardTargetLabel(pub usize);

/// Botão de alvo aliado para carta de unidade.
#[derive(Component)]
pub struct RewardTargetBtn(pub usize);

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

/// Preferência CLI: `--playback` mantém o modo legado (v0.6) — default manual.
#[derive(Resource, Default, Clone, Copy)]
pub struct PlaybackPref(pub bool);

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

/// Estado da batalha manual (v0.7): controlador da engine interativa.
#[derive(Resource, Default)]
pub struct ManualRes {
    /// Controlador da batalha corrente (lado 0 = jogador). `None` fora dela.
    pub ctl: Option<BattleController>,
    /// Botão Auto: a IA simétrica resolve as decisões do jogador.
    pub auto: bool,
}

/// Decisão pendente exibida no painel manual (dados já resolvidos p/ UI).
#[derive(Resource, Default)]
pub struct PendingUi {
    /// Painel ativo (vez do jogador).
    pub active: bool,
    /// Janela de perseguição (ENTER confirma / ESC recusa).
    pub chase: bool,
    /// Aguardando alvo (após escolher ação que precisa de um).
    pub choosing_target: bool,
    /// Alvo é inimigo (false = aliado, para Skill/Mystery de cura).
    pub target_enemy: bool,
    /// Ação selecionada aguardando alvo.
    pub selected: Option<ActionKind>,
    /// Nome da unidade na vez.
    pub name: String,
    /// HP atual/máximo e fluxo da unidade na vez.
    pub hp: i32,
    pub hp_max: i32,
    pub fluxo: i32,
    /// Botões [1]..[4]: (rótulo, custo, disponível, cd/unlock text).
    pub actions: [(String, i32, bool, String); 4],
    /// Linha de pré-visualização (dano/cura previstos).
    pub preview: String,
    /// (nome, hp%, vivo) por slot inimigo.
    pub targets: [(String, i32, bool); 4],
    /// (nome, hp%, vivo) por slot aliado.
    pub allies: [(String, i32, bool); 4],
    /// Slots aliados elegíveis na janela de chase.
    pub chasers: Vec<usize>,
    /// Texto da timeline da rodada (ordem visível).
    pub timeline: String,
    /// Chave da vez preenchida: (geração de decisão, chase). Enquanto a
    /// mesma chave persiste, o painel NÃO é reenchido — protege a escolha
    /// de alvo do jogador do re-preenchimento por frame (feedback B1).
    pub key: Option<(u64, bool)>,
    /// R1: janela de tática aberta (botões [1]/[2] escolhem a carta).
    pub tactic: bool,
    /// Descrições das 2 táticas ofertadas (linha de prévia).
    pub tactic_descs: [String; 2],
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

/// Rótulo de texto de um botão de ação (atualizado pelo sync).
#[derive(Component, Clone, Copy, PartialEq)]
pub struct DecBtnLabel(pub DecKind);

#[derive(Clone, Copy, PartialEq)]
pub enum DecKind {
    Standard,
    Skill,
    Mystery,
    Interrupt,
    Auto,
}

/// Botão de alvo (slot 0..4; inimigo ou aliado conforme a escolha).
#[derive(Component, Clone, Copy, PartialEq)]
pub struct DecTargetBtn(pub usize);

/// Botão “Auto-alvo” (regra da IA: primeiro inimigo vivo).
#[derive(Component)]
pub struct DecTargetAutoBtn;

/// Botão de reroll da oferta de cartas (Reward, v0.7).
#[derive(Component)]
pub struct RerollBtn;

/// Oferta corrente da tela de cartas (v0.7): até 3 cartas + alvo pendente.
#[derive(Resource, Default)]
pub struct RewardRes {
    /// Cartas ofertadas (referências ao CARD_POOL).
    pub offers: Vec<&'static turnrush_core::rewards::RewardCard>,
    /// Carta de unidade aguardando escolha de alvo vivo.
    pub pending_unit: Option<usize>,
    /// Filtros já comprados (para não repetir rótulos).
    pub gerada: bool,
}

/// Linha de pré-visualização da ação.
#[derive(Component)]
pub struct DecPreviewText;

/// Faixa de timeline da rodada (ordem visível).
#[derive(Component)]
pub struct TimelineText;

/// Faixa de perseguição (janela ativa).
#[derive(Component)]
pub struct ChaseText;

/// Conteúdo da tela "Como Jogar" — regras FIÉIS ao engine (formation.rs).
const AJUDA: [(&str, &str); 7] = [
    (
        "1 · DRAFT — 8 cartas na mesa, escolha 4",
        "A oferta é a mesma para os dois lados (simetria por construção).\nA IA monta o time inimigo entre os personagens restantes do elenco.",
    ),
    (
        "2 · BATALHA 4v4 (VOCÊ COMANDA TUDO)",
        "Timeline visível (SPD); A-B intercalado; quem abre a 1ª rodada é sorteado.\nPainel de comando: Padrão [1] grátis (⚡+15) · Skill [2] ⚡30/CD 2 ·\nMystery [3] ⚡60/CD 3 (destrava R2/R3 por papel) · Interrupt [4] ⚡35/CD 3.\nO PADRÃO tem alvo automático: mira o inimigo na sua frente (1↔1, 2↔2…;\ncaiu, o primeiro vivo da frente) — dispara direto, sem fila de alvos.\nAlvo manual só em poder focado (cura aliada/interrupt). Fluxo: +10/rodada,\nteto 100. Prévia EXATA antes de confirmar. Máximo de 10 rodadas —\ndesempate por % de vida.",
    ),
    (
        "3 · CHAIN COMBOS — PERSEGUIÇÃO MANUAL",
        "Mysteries e ataques aplicam Arremessado/Empurrado/Derrubado. A janela de\nperseguição PAUSA o jogo: elegíveis brilham dourado. [ENTER] persegue ·\n[ESC] recusa (tática válida). Até 12 elos, 1 chase por unidade por rodada.",
    ),
    (
        "4 · INTERRUPT — LEIA O INIMIGO",
        "Os 4 Tacticians têm Interrupt: atordoam ou silenciam um inimigo que AINDA\nvai agiar nesta rodada. O jogo NÃO avisa — leia a timeline e o ⚡ do inimigo\n(mostrado dos dois lados). Atordoado perde a ação; silenciado não conjura.",
    ),
    (
        "5 · TÁTICAS E CARTAS (ROGUELIKE)",
        "A cada 2 rodadas (2/4/6/8) o jogo pausa: adote 1 de 2 TÁTICAS de\nbatalha [1]/[2] — efeito até o fim da batalha. O inimigo escolhe a dele\nem silêncio (mesma força, zero pista). Vitória → 1 de 3 cartas de RUN\n(Equipe/Unidade/Tática) — 1 reroll por batalha. Teto de poder da run:\n+30%. A 3ª batalha é o BOSS. 3 vitórias completam a run.",
    ),
    (
        "6 · CLÃS E BALANCEAMENTO",
        "Pyreborn, Duskmere, Thornvale e Skyrend dão identidade — NÃO existe\ncontador duro entre clãs. O calibrador DWC v2 mantém todas as composições\nem ~50% de winrate médio: a vantagem vem da sua leitura do confronto.",
    ),
    (
        "7 · CONTROLES DA BATALHA",
        "MOUSE: clique na ação — o Padrão dispara direto (alvo automático);\npoderes focados abrem a fila de alvos. TECLADO: [1]-[4] ações · Q/W/E/R\nalvo inimigo · A/S/D/F alvo aliado · ENTER confirma/auto-alvo/persegue ·\nESC cancela/recusa · ESPAÇO 2× · Z modo Auto (IA joga por você).\nA IA inimiga usa EXATAMENTE as mesmas informações que você.",
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

pub fn run(shot: Option<ShotOpts>, playback: bool) {
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
        .insert_resource(PlaybackPref(playback))
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
                battle_keys,
                scene::scene_animate,
                scene::scene_fx,
                scene::fx_motion,
                scene::scene_shake,
                battle_finish,
            )
                .run_if(in_state(GameState::Battle)),
        )
        .add_systems(OnExit(GameState::Battle), despawn_battle_scene)
        // Recompensa (cartas v0.7): enter roda a cada frame — gera a oferta
        // só quando `gerada == false` (reroll reentra aqui)
        .add_systems(
            Update,
            (reward_enter, reward_click).run_if(in_state(GameState::Reward)),
        )
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
    mut manual: ResMut<ManualRes>,
    pref: Res<PlaybackPref>,
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
            // v0.7: o shot da batalha mostra a UI de comando manual (painel
            // de ação com prévia) — aguarda os eventos abrirem a 1ª decisão.
            construir_batalha(pref.0, &mut run, &mut pb, &mut manual, &mut cmds);
            next.set(GameState::Battle);
            st.timer = 7.0;
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
        cards: turnrush_core::rewards::RunState::new(),
        card_targets: Vec::new(),
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
        fluxo: [0; 8],
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
                        Text::new("1× ▶ [ESPAÇO]"),
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
                p.spawn((
                    TimelineText,
                    Text::new(""),
                    TextFont { font_size: px(12.0), ..default() },
                    TextColor(TXT_DIM),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(24.0),
                        top: Val::Px(30.0),
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

            // ── Painel de comando manual (v0.7) ──
            // Vez do jogador: 4 ações [1]-[4], prévia EXATA, fila de alvos
            // e janela de perseguição (ENTER/ESC).
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
                dp.spawn((
                    DecPreviewText,
                    Text::new(""),
                    TextFont { font_size: px(13.0), ..default() },
                    TextColor(GOOD),
                ));
                dp.spawn((Node { column_gap: Val::Px(8.0), ..default() },)).with_children(|row| {
                    let mkbtn = |row: &mut ChildSpawnerCommands, kind: DecKind, width: f32| {
                        let lbl = row
                            .spawn((
                                DecBtnLabel(kind),
                                Text::new("—"),
                                TextFont { font_size: px(13.0), ..default() },
                                TextColor(TXT),
                            ))
                            .id();
                        let cor = match kind {
                            DecKind::Mystery => AMBER,
                            DecKind::Skill => GOOD,
                            _ => TXT_DIM,
                        };
                        row.spawn((
                            Interaction::default(),
                            DecBtn(kind),
                            Node {
                                width: Val::Px(width),
                                height: Val::Px(40.0),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(1.5)),
                                border_radius: BorderRadius::all(Val::Px(8.0)),
                                ..default()
                            },
                            BackgroundColor(PANEL),
                            BorderColor::all(cor),
                        ))
                        .add_child(lbl);
                    };
                    mkbtn(row, DecKind::Standard, 240.0);
                    mkbtn(row, DecKind::Skill, 320.0);
                    mkbtn(row, DecKind::Mystery, 330.0);
                    mkbtn(row, DecKind::Interrupt, 200.0);
                    let al = row
                        .spawn((Text::new("Auto [Z]"), TextFont { font_size: px(13.0), ..default() }, TextColor(TXT_DIM)))
                        .id();
                    row.spawn((
                        Interaction::default(),
                        DecBtn(DecKind::Auto),
                        Node {
                            width: Val::Px(96.0),
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
                    ChaseText,
                    Text::new(""),
                    TextFont { font_size: px(15.0), ..default() },
                    TextColor(Color::srgb(1.0, 0.82, 0.25)),
                ));
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
                        .spawn((Text::new("Auto-alvo [ENTER]"), TextFont { font_size: px(12.0), ..default() }, TextColor(TXT_DIM)))
                        .id();
                    tr.spawn((
                        Interaction::default(),
                        DecTargetAutoBtn,
                        Node {
                            width: Val::Px(140.0),
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

    // ── Recompensa (cartas roguelike v0.7) ──
    cmds.spawn((node_full(), StateRoot(GameState::Reward), Visibility::Hidden))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(18.0),
                    ..default()
                },
            ))
            .with_children(|p| {
                p.spawn((Text::new("VITÓRIA"), TextFont { font_size: FontSize::Px(44.0), ..default() }, TextColor(GOOD)));
                p.spawn((
                    RewardSubtitle,
                    Text::new("Escolha 1 carta para a próxima batalha:"),
                    TextFont { font_size: FontSize::Px(18.0), ..default() },
                    TextColor(TXT),
                ));
                p.spawn((Node { column_gap: Val::Px(14.0), ..default() },)).with_children(|row| {
                    for i in 0..3usize {
                        let nome = row
                            .spawn((
                                Text::new("—"),
                                TextFont { font_size: px(17.0), ..default() },
                                TextColor(AMBER),
                            ))
                            .id();
                        let desc = row
                            .spawn((
                                Text::new(""),
                                TextFont { font_size: px(12.5), ..default() },
                                TextColor(TXT),
                            ))
                            .id();
                        let meta = row
                            .spawn((
                                Text::new(""),
                                TextFont { font_size: px(11.5), ..default() },
                                TextColor(TXT_DIM),
                            ))
                            .id();
                        let col = row
                            .spawn((
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(6.0),
                                    ..default()
                                },
                            ))
                            .add_child(nome)
                            .add_child(desc)
                            .add_child(meta)
                            .id();
                        row.spawn((
                            Interaction::default(),
                            RewardBtn(i),
                            Node {
                                width: Val::Px(330.0),
                                height: Val::Px(118.0),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(2.0)),
                                border_radius: BorderRadius::all(Val::Px(12.0)),
                                ..default()
                            },
                            BackgroundColor(PANEL),
                            BorderColor::all(TXT_DIM),
                        ))
                        .add_child(col);
                    }
                });
                // alvo de carta de unidade (aparece quando precisa)
                p.spawn((
                    RewardTargetRow,
                    Node { column_gap: Val::Px(10.0), ..default() },
                    Visibility::Hidden,
                ))
                .with_children(|tr| {
                    for slot in 0..4usize {
                        let lbl = tr
                            .spawn((
                                RewardTargetLabel(slot),
                                Text::new("—"),
                                TextFont { font_size: px(12.0), ..default() },
                                TextColor(TXT),
                            ))
                            .id();
                        tr.spawn((
                            Interaction::default(),
                            RewardTargetBtn(slot),
                            Node {
                                width: Val::Px(180.0),
                                height: Val::Px(36.0),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(8.0)),
                                ..default()
                            },
                            BackgroundColor(PANEL),
                            BorderColor::all(TXT_DIM),
                        ))
                        .add_child(lbl);
                    }
                });
                let rl = p
                    .spawn((
                        Text::new("Reroll (1 por batalha)"),
                        TextFont { font_size: px(14.0), ..default() },
                        TextColor(TXT_DIM),
                    ))
                    .id();
                p.spawn((
                    Interaction::default(),
                    RerollBtn,
                    Node {
                        width: Val::Px(260.0),
                        height: Val::Px(38.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(PANEL),
                    BorderColor::all(TXT_DIM),
                ))
                .add_child(rl);
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

#[allow(clippy::too_many_arguments)] // assinatura de sistema Bevy
fn draft_confirm(
    mut cmds: Commands,
    mut run: ResMut<RunRes>,
    mut playback: ResMut<PlaybackRes>,
    pref: Res<PlaybackPref>,
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
    construir_batalha(pref.0, &mut run, &mut playback, &mut manual, &mut cmds);
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

/// Constrói a batalha conforme a preferência CLI: manual = padrão (v0.7);
/// `--playback` = modo legado (v0.6) preservado para curiosos e CI visual.
fn construir_batalha(
    pref: bool,
    run: &mut RunRes,
    pb: &mut PlaybackRes,
    manual: &mut ManualRes,
    cmds: &mut Commands,
) {
    if pref {
        build_battle(run, pb, cmds);
        cmds.insert_resource(BattleMode::Playback);
    } else {
        build_manual(run, pb, manual, cmds);
    }
}

/// Batalha MANUAL (v0.7): engine interativa via controlador, lado 0 = jogador.
fn build_manual(run: &mut RunRes, pb: &mut PlaybackRes, manual: &mut ManualRes, cmds: &mut Commands) {
    let (a, b) = times_da_run(run);
    let seed = run.rng.next_u64();
    // Escala do inimigo por vitória (+110‰ dano / +70‰ hp) — a 3ª luta é BOSS.
    let boss = run.wins >= VICTORIES_TO_WIN - 1;
    let edge_b = SideEdge {
        dmg_scale_permille: 1000 + (run.wins as i32) * 110 + if boss { 150 } else { 0 },
        hp_scale_permille: 1000 + (run.wins as i32) * 70 + if boss { 100 } else { 0 },
        ..SideEdge::default()
    };
    // Cartas da run acumuladas no lado do jogador.
    let edge_a = run.cards_edge();
    let ctl = BattleController::new(&run.roster, a, b, seed, edge_a, edge_b, manual.auto);
    *manual = ManualRes { ctl: Some(ctl), auto: manual.auto };
    cmds.insert_resource(BattleMode::Manual);

    let nomes = |comp: &[usize; 4]| -> String {
        comp.iter().map(|&i| run.roster.characters[i].name.clone()).collect::<Vec<_>>().join(", ")
    };
    turnrush_core::logger::kv(
        turnrush_core::logger::Level::Info,
        "battle_start",
        "batalha 4v4 interativa gerada (manual)",
        &[
            ("time_a", turnrush_core::logger::V::S(&nomes(&a))),
            ("time_b", turnrush_core::logger::V::S(&nomes(&b))),
            ("seed", turnrush_core::logger::V::U(seed)),
            ("boss", turnrush_core::logger::V::S(if boss { "sim" } else { "não" })),
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
        fluxo: [0; 8],
    };
}

/// Query do contador de cadeias (alias anti type_complexity).
type ChainsTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (With<ChainsText>, Without<BattleHeader>, Without<BattleLogText>, Without<SpeedLabel>),
>;

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
        let boss = run.wins >= VICTORIES_TO_WIN - 1;
        *t = Text::new(if boss { "BOSS — rodada 1" } else { "Batalha — rodada 1" });
    }
    if let Ok(mut t) = log.single_mut() {
        *t = Text::new("");
    }
    if let Ok(mut t) = wins.single_mut() {
        *t = Text::new(format!("Vitórias {}/{}", run.wins, VICTORIES_TO_WIN));
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

/// Ritmo de playback por classe de evento (spec E.10: rápido sem ser
/// apressado — uma volta de unidade não passa de ~2s de animação).
const fn pacing(ev: &BattleEvent, ff: bool) -> f32 {
    let base = match ev {
        BattleEvent::RoundStart { .. } => 0.8,
        BattleEvent::RoundEnd { .. } => 0.35,
        BattleEvent::ActionDeclared { kind, .. } => match kind {
            ActionKind::Standard => 0.30,
            ActionKind::Skill => 0.50,
            ActionKind::Mystery => 0.80,
            ActionKind::Interrupt => 0.45,
        },
        BattleEvent::ChaseResolved { .. } => 0.40,
        BattleEvent::ChaseOffered { .. } => 0.0,
        BattleEvent::BattleOver { .. } => 1.1,
        _ => 0.22,
    };
    if ff {
        base / 2.0 // SPACE = 2× (spec E.10)
    } else {
        base
    }
}

/// Tick da batalha MANUAL (v0.7): reproduz 1 [`BattleEvent`] por tick,
/// pausa na vez do jogador (painel de comando) e conduz a IA do adversário.
#[allow(clippy::too_many_arguments)]
fn manual_tick(
    time: Res<Time>,
    ff: Res<FastForward>,
    mut manual: ResMut<ManualRes>,
    mut pb: ResMut<PlaybackRes>,
    mut pending: ResMut<PendingUi>,
    mut log: BattleLogTextQuery,
    mut header: BattleHdrQuery,
    mut chains: ChainsTextQuery,
    mut queue: ResMut<FxQueue>,
) {
    let Some(ctl) = manual.ctl.as_mut() else { return };
    if pb.finished {
        return; // battle_finish conduz a transição
    }
    pb.wait -= time.delta_secs();
    if pb.wait > 0.0 {
        return;
    }

    // 1) evento pendente na fila → reproduz (1 por tick)
    if let Some(ev) = ctl.next_event() {
        ctl.advance_event();
        aplicar_evento(&ev, &mut pb, &mut log, &mut header, &mut chains, &mut queue, &mut ctl.bt);
        pb.wait = pacing(&ev, ff.0);
        return;
    }

    // 2) avança o controlador
    match ctl.pump() {
        CtlPhase::PlayingEvents => { /* eventos novos ficam para o próximo tick */ }
        CtlPhase::AwaitingPlayer { chase } => {
            if ctl.auto_player {
                // modo Auto: a IA simétrica decide pelo jogador
                let p = ctl.prompt().expect("prompt na vez");
                let acao = if chase {
                    match ctl.bt.chase_eligible_slots(p.side).first() {
                        Some(&s) => ChosenAction::Chase { chaser_slot: s },
                        None => ChosenAction::RejectChase,
                    }
                } else {
                    crate::controller::greedy_decide(&ctl.bt, &p)
                };
                let _ = ctl.submit(acao);
                pb.wait = 0.12;
            } else {
                preencher_painel(ctl, chase, &mut pending);
                pb.wait = 0.05;
            }
        }
        CtlPhase::Done(outcome) => {
            pb.finished = true;
            pb.wait = 1.1;
            pb.outcome = outcome;
            pb.chains = ctl.bt.chain_links();
            pb.rounds = ctl.bt.round() as u64;
            pending.active = false;
            pending.chase = false;
        }
    }
}

/// Preenche o painel de decisão com o estado da vez corrente.
fn preencher_painel(ctl: &mut BattleController, chase: bool, pending: &mut PendingUi) {
    let Some(p) = ctl.prompt() else { return };
    // B1 (feedback 2026-10-18): `manual_tick` chama isto todo frame; se a
    // MESMA vez continua (mesma geração + mesma janela), preserva o estado
    // da escolha em andamento (alvo selecionado etc.) em vez de resetar.
    let chave = (ctl.bt.generation(), chase);
    if pending.active && pending.key == Some(chave) {
        return;
    }
    pending.key = Some(chave);
    pending.active = true;
    pending.chase = chase;
    pending.choosing_target = false;
    pending.selected = None;
    pending.target_enemy = true;
    // R1 (feedback): janela de tática — 2 cartas, escolha obrigatória
    // (o inimigo já adotou a dele em silêncio; simetria de poder).
    if p.phase == turnrush_core::interactive::PromptPhase::ChooseTactic {
        pending.tactic = true;
        pending.chase = false;
        pending.choosing_target = false;
        pending.selected = None;
        if let Some([a, b]) = ctl.bt.tactic_offer() {
            let pool = &turnrush_core::rewards::TACTIC_POOL;
            let ca = &pool[a.min(pool.len() - 1)];
            let cb = &pool[b.min(pool.len() - 1)];
            pending.tactic_descs = [ca.desc.to_string(), cb.desc.to_string()];
            pending.name = "Oferta de tática".to_string();
            pending.hp = 0;
            pending.hp_max = 0;
            pending.fluxo = 0;
            pending.actions = [
                (format!("[1] {}", ca.name), 0, true, String::new()),
                (format!("[2] {}", cb.name), 0, true, String::new()),
                ("—".to_string(), 0, false, String::new()),
                ("—".to_string(), 0, false, String::new()),
            ];
            pending.preview = format!("{}  ·  {}", ca.desc, cb.desc);
        }
        // timeline visível (mesma rotina da vez normal)
        let tl = ctl.bt.timeline();
        let cur = ctl.bt.timeline_cursor();
        let mut texto = String::from("Ordem: ");
        for (i, (side, slot)) in tl.iter().enumerate() {
            if i > 0 {
                texto.push(' ');
            }
            let nome = ctl.bt.unit(*side, *slot).name.chars().take(2).collect::<String>();
            if i == cur {
                texto.push_str(&format!("[{nome}]"));
            } else if i < cur {
                texto.push_str(&format!("{nome}·"));
            } else {
                texto.push_str(&nome);
            }
        }
        pending.timeline = texto;
        return;
    }
    pending.tactic = false;
    pending.tactic_descs = [String::new(), String::new()];
    let u = ctl.bt.unit(p.side, p.slot);
    pending.name = u.name.clone();
    pending.hp = u.hp;
    pending.hp_max = u.hp_max;
    pending.fluxo = u.fluxo;
    // botões [1]..[4]
    let legais = ctl.bt.legal_actions(p.side, p.slot);
    let achar = |k: ActionKind| legais.iter().find(|l| l.kind == k).cloned();
    let mk = |k: ActionKind, nome: String| match achar(k) {
        Some(l) => (format!("[{}] {nome}", match k { ActionKind::Standard => "1", ActionKind::Skill => "2", ActionKind::Mystery => "3", _ => "4" }), l.fluxo_cost, true, String::new()),
        None => {
            let motivo = match k {
                ActionKind::Mystery => {
                    if u.mystery_cd > 0 {
                        "CD".to_string()
                    } else if u.silence_turns > 0 {
                        "silenciado".to_string()
                    } else {
                        format!("R{}", u.mystery_unlock_round)
                    }
                }
                ActionKind::Skill => {
                    if u.skill_cd > 0 {
                        "CD".to_string()
                    } else if u.silence_turns > 0 {
                        "silenciado".to_string()
                    } else {
                        String::new()
                    }
                }
                ActionKind::Interrupt => "—".to_string(),
                _ => String::new(),
            };
            (format!("[{}] {nome}", match k { ActionKind::Standard => "1", ActionKind::Skill => "2", ActionKind::Mystery => "3", _ => "4" }), 0, false, motivo)
        }
    };
    pending.actions = [
        mk(ActionKind::Standard, u.standard_name.clone()),
        mk(ActionKind::Skill, format!("Skill «{}»", u.skill_name)),
        mk(ActionKind::Mystery, format!("Mystery «{}»", u.mystery_name)),
        mk(ActionKind::Interrupt, "Interrupt".to_string()),
    ];
    // alvos
    for k in 0..4 {
        let e = ctl.bt.unit(1, k);
        let a = ctl.bt.unit(0, k);
        pending.targets[k] = (e.name.clone(), if e.hp_max > 0 { e.hp * 100 / e.hp_max } else { 0 }, e.alive);
        pending.allies[k] = (a.name.clone(), if a.hp_max > 0 { a.hp * 100 / a.hp_max } else { 0 }, a.alive);
    }
    // chase
    pending.chasers = if chase { ctl.bt.chase_eligible_slots(p.side) } else { Vec::new() };
    // timeline visível (rodada corrente, marcando a posição)
    let tl = ctl.bt.timeline();
    let cur = ctl.bt.timeline_cursor();
    let mut texto = String::from("Ordem: ");
    for (i, (side, slot)) in tl.iter().enumerate() {
        if i > 0 {
            texto.push(' ');
        }
        let nome = ctl.bt.unit(*side, *slot).name.chars().take(2).collect::<String>();
        if i == cur {
            texto.push_str(&format!("[{nome}]"));
        } else if i < cur {
            texto.push_str(&format!("{nome}·"));
        } else {
            texto.push_str(&nome);
        }
    }
    pending.timeline = texto;
    pending.preview = String::new();
}

/// Traduz um [`BattleEvent`] fino para o estado visual + fila de efeitos.
/// (O modo manual NUNCA parseia texto — consome eventos estruturados.)
#[allow(clippy::too_many_arguments)]
fn aplicar_evento(
    ev: &BattleEvent,
    pb: &mut PlaybackRes,
    log: &mut BattleLogTextQuery,
    header: &mut BattleHdrQuery,
    chains: &mut ChainsTextQuery,
    queue: &mut FxQueue,
    bt: &mut turnrush_core::interactive::InteractiveBattle,
) {
    use turnrush_core::interactive::SkipReason;
    let mut linha = String::new();
    match ev {
        BattleEvent::RoundStart { round, starter } => {
            if let Ok(mut h) = header.single_mut() {
                let boss = *round >= 3;
                *h = Text::new(format!(
                    "Batalha — rodada {round}{} (abre: {})",
                    if boss { " · BOSS" } else { "" },
                    if *starter == 0 { "você" } else { "inimigo" }
                ));
            }
            queue.0.push(VisEvent::Round(*round as u64));
            linha = format!("— Rodada {round} —");
        }
        BattleEvent::FluxoGain { side, slot, amount } => {
            pb.fluxo[side * 4 + slot] = bt.unit(*side, *slot).fluxo;
            let _ = amount;
        }
        BattleEvent::ActionDeclared { side, slot, kind, name, target } => {
            let ator = side * 4 + slot;
            queue.0.push(VisEvent::Lunge(ator));
            // U2 (feedback): log enxuto — só cast Especial entra no log;
            // padrão/skill são visíveis pelos números flutuantes.
            linha = if matches!(kind, ActionKind::Mystery | ActionKind::Interrupt) {
                match target {
                    Some((tside, tslot)) => {
                        format!(
                            "{} usa «{name}» → {}",
                            bt.unit(*side, *slot).name,
                            bt.unit(*tside, *tslot).name
                        )
                    }
                    None => format!("{} usa «{name}»", bt.unit(*side, *slot).name),
                }
            } else {
                String::new()
            };
            if *kind == ActionKind::Mystery {
                queue.0.push(VisEvent::Chain); // brilho de mystery reutiliza o efeito de cadeia
            }
        }
        BattleEvent::Damage { side, slot, amount, killed } => {
            let idx = side * 4 + slot;
            pb.taken[idx] += (*amount).max(0) as i64;
            if *killed {
                pb.alive[idx] = false;
                pb.taken[idx] = pb.max_hp[idx] * 2;
                queue.0.push(VisEvent::Death(idx));
                linha = format!("✖ {} caiu", bt.unit(*side, *slot).name);
            } else if *amount > 0 {
                queue.0.push(VisEvent::Hit(idx, *amount as i64));
            }
        }
        BattleEvent::Heal { side, slot, amount } => {
            pb.healed[side * 4 + slot] += (*amount).max(0) as i64;
            if *amount > 0 {
                queue.0.push(VisEvent::Heal(side * 4 + slot, *amount as i64));
            }
            // U2: cura visível pelo número verde — não polui o log.
        }
        BattleEvent::Revive { side, slot, amount } => {
            pb.alive[side * 4 + slot] = true;
            pb.taken[side * 4 + slot] = (pb.max_hp[side * 4 + slot] - *amount as i64).max(0);
            queue.0.push(VisEvent::Heal(side * 4 + slot, *amount as i64));
            linha = format!("✦ {} revive com {amount} de HP!", bt.unit(*side, *slot).name);
        }
        BattleEvent::Shield { side, slot, amount } => {
            queue.0.push(VisEvent::Heal(side * 4 + slot, *amount as i64));
            // U2: escudo visível pelo número verde — não polui o log.
            let _ = (side, slot, amount);
        }
        BattleEvent::StateApplied { side, slot, state } => {
            queue.0.push(VisEvent::Dot(side * 4 + slot, estado_icone(state.label())));
            linha = format!("{} recebe {}", bt.unit(*side, *slot).name, state.label());
        }
        BattleEvent::Debuffed { side, slot, kind } => {
            queue.0.push(VisEvent::Dot(side * 4 + slot, 2));
            linha = match kind {
                turnrush_core::data::InterruptKind::Stun => {
                    format!("{} foi ATORDOADO (perde a próxima ação)", bt.unit(*side, *slot).name)
                }
                turnrush_core::data::InterruptKind::Silence => {
                    format!("{} foi SILENCIADO (sem Skill/Mystery)", bt.unit(*side, *slot).name)
                }
            };
        }
        BattleEvent::Skip { side, slot, reason } => {
            linha = match reason {
                SkipReason::Dead => format!("{} está caído — vez passada", bt.unit(*side, *slot).name),
                SkipReason::Stunned => format!("{} está atordoado — vez passada", bt.unit(*side, *slot).name),
            };
        }
        BattleEvent::ChaseOffered { side, target, state, eligible } => {
            linha = format!(
                "janela de perseguição: {} sob {} — {} pode perseguir",
                bt.unit(target.0, target.1).name,
                state.label(),
                if eligible.is_empty() { "ninguém".to_string() } else { "aliado(s)".to_string() }
            );
            let _ = side;
        }
        BattleEvent::ChaseResolved { chaser, target, amount, killed } => {
            let ci = chaser.0 * 4 + chaser.1;
            let ti = target.0 * 4 + target.1;
            queue.0.push(VisEvent::Lunge(ci));
            if *killed {
                pb.alive[ti] = false;
                pb.taken[ti] = pb.max_hp[ti] * 2;
                pb.taken[ti] += (*amount).max(0) as i64;
                queue.0.push(VisEvent::Death(ti));
            } else {
                pb.taken[ti] += (*amount).max(0) as i64;
                queue.0.push(VisEvent::Hit(ti, *amount as i64));
            }
            queue.0.push(VisEvent::Chain);
            pb.chains += 1;
            if let Ok(mut t) = chains.single_mut() {
                *t = Text::new(format!("Chains: {}", pb.chains));
            }
            linha = format!("chase: {} → {} ({amount} de dano)", bt.unit(chaser.0, chaser.1).name, bt.unit(target.0, target.1).name);
        }
        BattleEvent::ChaseRejected { side } => {
            linha = "cadeia recusada (tática)".to_string();
            let _ = side;
        }
        BattleEvent::TacticPicked { side, tactic } => {
            // R1: só a tática do JOGADOR entra no log — a do inimigo é
            // silenciosa por design (feedback: "ela vai decidir para ela").
            if *side == 0 {
                linha = format!("✦ tática adotada: {tactic}");
            }
        }
        BattleEvent::RoundEnd { .. } => {}
        BattleEvent::BattleOver { outcome } => {
            pb.outcome = *outcome;
            queue.0.push(VisEvent::End(*outcome == Outcome::TeamAWin));
            linha = format!("fim: {outcome:?}");
        }
        BattleEvent::Unmodeled { text } => {
            linha = format!("[sem efeito] {text}");
        }
    }
    if linha.is_empty() {
        return;
    }
    if let Ok(mut t) = log.single_mut() {
        let mut atual = t.to_string();
        if atual.is_empty() {
            atual = linha;
        } else {
            atual.push('\n');
            atual.push_str(&linha);
        }
        let linhas: Vec<&str> = atual.lines().collect();
        if linhas.len() > 8 {
            atual = linhas[linhas.len() - 8..].join("\n");
        }
        *t = Text::new(atual);
    }
}

/// Sincroniza visibilidade e textos do painel de comando manual (v0.7).
#[allow(clippy::type_complexity)] // queries com Without explícito (B0001)
#[allow(clippy::too_many_arguments)] // assinatura de sistema Bevy
fn sync_decision_panel(
    pending: Res<PendingUi>,
    mut panel: Query<&mut Visibility, (With<DecPanel>, Without<DecTargetRow>)>,
    mut trow: Query<&mut Visibility, (With<DecTargetRow>, Without<DecPanel>)>,
    mut scene: ResMut<BattleScene>,
    mut title: Query<
        &mut Text,
        (With<DecTitleText>, Without<DecPreviewText>, Without<DecMysteryLabel>, Without<DecStandardLabel>, Without<DecTargetLabel>, Without<ChaseText>, Without<TimelineText>),
    >,
    mut preview: Query<
        &mut Text,
        (With<DecPreviewText>, Without<DecTitleText>, Without<DecMysteryLabel>, Without<DecStandardLabel>, Without<DecTargetLabel>, Without<ChaseText>, Without<TimelineText>),
    >,
    mut chase_banner: Query<
        &mut Text,
        (With<ChaseText>, Without<DecTitleText>, Without<DecPreviewText>, Without<DecMysteryLabel>, Without<DecStandardLabel>, Without<DecTargetLabel>, Without<TimelineText>),
    >,
    mut timeline: Query<
        &mut Text,
        (With<TimelineText>, Without<DecTitleText>, Without<DecPreviewText>, Without<DecMysteryLabel>, Without<DecStandardLabel>, Without<DecTargetLabel>, Without<ChaseText>),
    >,
    mut labels: Query<
        (&DecBtn, &mut BorderColor),
        (Without<DecTargetBtn>, Without<DecTargetAutoBtn>, Without<SpeedBtn>, Without<BackBtn>, Without<MenuBtn>, Without<DraftCard>, Without<DraftConfirm>, Without<RewardBtn>, Without<RerollBtn>),
    >,
    mut tlabels: Query<
        (&DecTargetLabel, &mut Text),
        (Without<DecTitleText>, Without<DecPreviewText>, Without<DecMysteryLabel>, Without<DecStandardLabel>, Without<ChaseText>, Without<TimelineText>, Without<DecBtnLabel>),
    >,
    mut blabels: Query<
        (&DecBtnLabel, &mut Text),
        (Without<DecTitleText>, Without<DecPreviewText>, Without<DecMysteryLabel>, Without<DecStandardLabel>, Without<ChaseText>, Without<TimelineText>, Without<DecTargetLabel>),
    >,
) {
    if let Ok(mut v) = panel.single_mut() {
        *v = if pending.active { Visibility::Visible } else { Visibility::Hidden };
    }
    if let Ok(mut v) = trow.single_mut() {
        *v = if pending.active && pending.choosing_target { Visibility::Visible } else { Visibility::Hidden };
    }
    if let Ok(mut t) = title.single_mut() {
        *t = if pending.active && pending.tactic {
            Text::new("Escolha uma tática — [1]/[2] adotam")
        } else if pending.active && !pending.chase {
            Text::new(format!(
                "Sua vez — {} · HP {}/{} · Fluxo ⚡{}",
                pending.name, pending.hp, pending.hp_max, pending.fluxo
            ))
        } else if pending.active {
            Text::new(String::new())
        } else {
            Text::new("Sua vez")
        };
    }
    if let Ok(mut t) = preview.single_mut() {
        *t = Text::new(pending.preview.clone());
    }
    if let Ok(mut t) = chase_banner.single_mut() {
        *t = if pending.active && pending.chase {
            Text::new("CADEIA! [ENTER] persegue · [ESC] recusa (tática válida)")
        } else {
            Text::new(String::new())
        };
    }
    if let Ok(mut t) = timeline.single_mut() {
        *t = Text::new(pending.timeline.clone());
    }
    // borda dourada nos botões disponíveis + destaque da ação selecionada
    for (btn, mut borda) in labels.iter_mut() {
        let idx = match btn.0 {
            DecKind::Standard => 0,
            DecKind::Skill => 1,
            DecKind::Mystery => 2,
            DecKind::Interrupt => 3,
            DecKind::Auto => continue,
        };
        let (_, _, disponivel, _) = pending.actions[idx];
        let selecionada = pending.selected == Some(match btn.0 {
            DecKind::Standard => ActionKind::Standard,
            DecKind::Skill => ActionKind::Skill,
            DecKind::Mystery => ActionKind::Mystery,
            _ => ActionKind::Interrupt,
        });
        *borda = BorderColor::all(if !pending.active {
            TXT_DIM
        } else if selecionada {
            AMBER
        } else if disponivel {
            GOOD
        } else {
            Color::srgb(0.28, 0.28, 0.32)
        });
    }
    // rótulos de alvo (inimigos ou aliados)
    for (lbl, mut t) in tlabels.iter_mut() {
        let k = lbl.0.min(3);
        let fonte = if pending.target_enemy { &pending.targets } else { &pending.allies };
        let (nome, hp, vivo) = &fonte[k];
        *t = if *vivo {
            Text::new(format!("{nome} · {hp}%"))
        } else {
            Text::new(format!("{nome} · caído"))
        };
    }
    // rótulos dos botões de ação ([1]-[4] com custo/estado)
    for (lbl, mut t) in blabels.iter_mut() {
        let idx = match lbl.0 {
            DecKind::Standard => 0,
            DecKind::Skill => 1,
            DecKind::Mystery => 2,
            DecKind::Interrupt => 3,
            DecKind::Auto => continue,
        };
        let (nome, custo, disponivel, motivo) = &pending.actions[idx];
        *t = if *disponivel {
            Text::new(if *custo > 0 { format!("{nome} · ⚡{custo}") } else { nome.clone() })
        } else if motivo.is_empty() {
            Text::new(format!("{nome} (sem ⚡)"))
        } else {
            Text::new(format!("{nome} [{motivo}]"))
        };
    }
    // highlight dourado dos perseguidores elegíveis (cena)
    scene.chase_gold = if pending.active && pending.chase {
        pending.chasers.clone()
    } else {
        Vec::new()
    };
}

/// Cliques do painel de comando: ação → alvo (quando preciso) → executa.
fn decision_click(
    mut manual: ResMut<ManualRes>,
    mut pending: ResMut<PendingUi>,
    mut scene: ResMut<BattleScene>,
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
            DecKind::Auto => {
                if let Some(ctl) = manual.ctl.as_mut() {
                    ctl.auto_player = !ctl.auto_player;
                    manual.auto = ctl.auto_player;
                }
                pending.active = false;
                pending.chase = false;
                scene.chase_gold.clear();
            }
            kind_btn => {
                if pending.chase {
                    continue; // na janela de chase só ENTER/ESC decidem
                }
                // R1: na janela de tática, [1]/[2] adotam a carta ofertada.
                if pending.tactic {
                    let idx = match kind_btn {
                        DecKind::Standard => 0usize,
                        DecKind::Skill => 1usize,
                        _ => continue,
                    };
                    if let Some(ctl) = manual.ctl.as_mut() {
                        match ctl.submit(ChosenAction::PickTactic { index: idx }) {
                            Ok(()) => {
                                pending.active = false;
                                pending.tactic = false;
                            }
                            Err(e) => {
                                pending.preview = format!("⚠ tática recusada: {e:?}");
                            }
                        }
                    }
                    continue;
                }
                let kind = match kind_btn {
                    DecKind::Standard => ActionKind::Standard,
                    DecKind::Skill => ActionKind::Skill,
                    DecKind::Mystery => ActionKind::Mystery,
                    _ => continue,
                };
                escolher_acao(manual.ctl.as_mut(), &mut pending, kind);
            }
        }
    }
}

/// Seleciona uma ação: precisa de alvo → abre a fila de alvos; senão executa.
fn escolher_acao(ctl: Option<&mut BattleController>, pending: &mut PendingUi, kind: ActionKind) {
    let Some(ctl) = ctl else { return };
    let Some(p) = ctl.prompt() else { return };
    let Some(legal) = ctl.bt.legal_actions(p.side, p.slot).into_iter().find(|l| l.kind == kind) else {
        return; // indisponível (CD/fluxo/unlock) — botão já aparece desabilitado
    };
    let precisa_alvo = match legal.targeting {
        turnrush_core::interactive::TargetKind::Enemy => true,
        turnrush_core::interactive::TargetKind::Ally => true,
        turnrush_core::interactive::TargetKind::Auto => false,
    };
    if precisa_alvo {
        pending.selected = Some(kind);
        pending.choosing_target = true;
        pending.target_enemy = legal.targeting == turnrush_core::interactive::TargetKind::Enemy;
        // prévia da ação com auto-alvo
        let prev = ctl.bt.preview_action(p.side, p.slot, ChosenAction::Act { kind, target: None }).ok();
        pending.preview = format_previsao(&prev, kind);
    } else {
        let prev = ctl.bt.preview_action(p.side, p.slot, ChosenAction::Act { kind, target: None }).ok();
        pending.preview = format_previsao(&prev, kind);
        match ctl.submit(ChosenAction::Act { kind, target: None }) {
            Ok(()) => pending.active = false,
            Err(e) => {
                // Recusa visível; o painel permanece na mesma vez.
                pending.preview = format!("⚠ ação recusada: {e:?}");
            }
        }
        turnrush_core::logger::kv(
            turnrush_core::logger::Level::Info,
            "manual_decision",
            "decisão manual submetida",
            &[("tipo", turnrush_core::logger::V::S(kind.label()))],
        );
    }
}

/// Formata a linha de previsão para a UI (números EXATOS da engine).
fn format_previsao(prev: &Option<turnrush_core::interactive::ActionPreview>, kind: ActionKind) -> String {
    let Some(p) = prev else { return String::new() };
    let mut partes = Vec::new();
    if p.damage > 0 {
        partes.push(format!("dano {} (total)", p.damage));
    }
    if p.heal > 0 {
        partes.push(format!("cura {}", p.heal));
    }
    if p.shield > 0 {
        partes.push(format!("escudo {}", p.shield));
    }
    if !p.states.is_empty() {
        partes.push(format!("aplica {}", p.states.len()));
    }
    if p.fluxo_delta != 0 {
        partes.push(if p.fluxo_delta > 0 {
            format!("⚡+{}", p.fluxo_delta)
        } else {
            format!("⚡{}", p.fluxo_delta)
        });
    }
    for n in &p.notes {
        partes.push(n.clone());
    }
    if partes.is_empty() {
        return match kind {
            ActionKind::Interrupt => "interrompe o alvo (atordoa/silencia)".to_string(),
            _ => String::new(),
        };
    }
    partes.join(" · ")
}

/// Clique em alvo (inimigo ou aliado) ou Auto-alvo — executa a ação pendente.
#[allow(clippy::type_complexity)] // query de Interaction com filtro de estado
fn target_click(
    mut manual: ResMut<ManualRes>,
    mut pending: ResMut<PendingUi>,
    mut scene: ResMut<BattleScene>,
    targets: Query<(&Interaction, &DecTargetBtn), (Changed<Interaction>, Without<DecTargetAutoBtn>)>,
    autos: Query<&Interaction, (With<DecTargetAutoBtn>, Changed<Interaction>)>,
) {
    if !pending.active || !pending.choosing_target || pending.chase {
        return;
    }
    let Some(kind) = pending.selected else { return };
    for (inter, tb) in targets.iter() {
        if *inter != Interaction::Pressed {
            continue;
        }
        let slot = tb.0.min(3);
        let fonte = if pending.target_enemy { &pending.targets } else { &pending.allies };
        if !fonte[slot].2 {
            continue; // alvo caído — ignora
        }
        if let Some(ctl) = manual.ctl.as_mut() {
            match ctl.submit(ChosenAction::Act { kind, target: Some(slot) }) {
                Ok(()) => {
                    turnrush_core::logger::kv(
                        turnrush_core::logger::Level::Info,
                        "manual_target",
                        "alvo escolhido",
                        &[
                            ("slot", turnrush_core::logger::V::U(slot as u64)),
                            ("acao", turnrush_core::logger::V::S(kind.label())),
                        ],
                    );
                    pending.active = false;
                    pending.choosing_target = false;
                    pending.selected = None;
                    scene.chase_gold.clear();
                }
                Err(e) => {
                    // Recusa visível: a escolha permanece aberta para retry.
                    pending.preview = format!("⚠ ação recusada: {e:?}");
                }
            }
        } else {
            return;
        }
        return;
    }
    for inter in autos.iter() {
        if *inter == Interaction::Pressed {
            if let Some(ctl) = manual.ctl.as_mut() {
                match ctl.submit(ChosenAction::Act { kind, target: None }) {
                    Ok(()) => {
                        pending.active = false;
                        pending.choosing_target = false;
                        pending.selected = None;
                        scene.chase_gold.clear();
                    }
                    Err(e) => {
                        pending.preview = format!("⚠ ação recusada: {e:?}");
                    }
                }
            }
            return;
        }
    }
}

/// Teclado da batalha manual (spec E.6): 1-4 ações · Q/W/E/R alvo inimigo ·
/// A/S/D/F alvo aliado · ENTER confirma/auto-alvo/persegue · ESC cancela/recusa ·
/// TAB alterna alvo inimigo rápido · SPACE 2× · Z modo Auto.
#[allow(clippy::too_many_arguments)]
fn battle_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut manual: ResMut<ManualRes>,
    mut pending: ResMut<PendingUi>,
    mut ff: ResMut<FastForward>,
    mut scene: ResMut<BattleScene>,
) {
    if !pending.active {
        // SPACE fora da decisão também acelera
        if keys.just_pressed(KeyCode::Space) {
            ff.0 = !ff.0;
        }
        return;
    }
    if keys.just_pressed(KeyCode::Space) {
        ff.0 = !ff.0;
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        if let Some(ctl) = manual.ctl.as_mut() {
            ctl.auto_player = !ctl.auto_player;
            manual.auto = ctl.auto_player;
        }
        pending.active = false;
        pending.chase = false;
        scene.chase_gold.clear();
        return;
    }
    // janela de chase: ENTER confirma (1º elegível), ESC recusa
    if pending.chase {
        if keys.just_pressed(KeyCode::Enter) {
            if let Some(ctl) = manual.ctl.as_mut() {
                let acao = match ctl.bt.chase_eligible_slots(0).first() {
                    Some(&s) => ChosenAction::Chase { chaser_slot: s },
                    None => ChosenAction::RejectChase,
                };
                let _ = ctl.submit(acao);
            }
            pending.active = false;
            pending.chase = false;
            scene.chase_gold.clear();
        } else if keys.just_pressed(KeyCode::Escape) {
            if let Some(ctl) = manual.ctl.as_mut() {
                let _ = ctl.submit(ChosenAction::RejectChase);
            }
            pending.active = false;
            pending.chase = false;
            scene.chase_gold.clear();
        }
        return;
    }
    // R1: janela de tática — [1]/[2] adotam; nada mais responde.
    if pending.tactic {
        let idx = if keys.just_pressed(KeyCode::Digit1) {
            Some(0usize)
        } else if keys.just_pressed(KeyCode::Digit2) {
            Some(1usize)
        } else {
            if keys.just_pressed(KeyCode::Space) {
                ff.0 = !ff.0;
            }
            None
        };
        if let Some(index) = idx {
            if let Some(ctl) = manual.ctl.as_mut() {
                match ctl.submit(ChosenAction::PickTactic { index }) {
                    Ok(()) => {
                        pending.active = false;
                        pending.tactic = false;
                    }
                    Err(e) => {
                        pending.preview = format!("⚠ tática recusada: {e:?}");
                    }
                }
            }
        }
        return;
    }
    // ações [1]-[4]
    if keys.just_pressed(KeyCode::Digit1) {
        escolher_acao(manual.ctl.as_mut(), &mut pending, ActionKind::Standard);
    } else if keys.just_pressed(KeyCode::Digit2) {
        escolher_acao(manual.ctl.as_mut(), &mut pending, ActionKind::Skill);
    } else if keys.just_pressed(KeyCode::Digit3) {
        escolher_acao(manual.ctl.as_mut(), &mut pending, ActionKind::Mystery);
    } else if keys.just_pressed(KeyCode::Digit4) {
        escolher_acao(manual.ctl.as_mut(), &mut pending, ActionKind::Interrupt);
    }
    // cancelar escolha de alvo
    if keys.just_pressed(KeyCode::Escape) && pending.choosing_target {
        pending.choosing_target = false;
        pending.selected = None;
        pending.preview = String::new();
        return;
    }
    // alvos
    if pending.choosing_target {
        let teclas: [(KeyCode, usize, bool); 9] = [
            (KeyCode::KeyQ, 0, true),
            (KeyCode::KeyW, 1, true),
            (KeyCode::KeyE, 2, true),
            (KeyCode::KeyR, 3, true),
            (KeyCode::KeyA, 0, false),
            (KeyCode::KeyS, 1, false),
            (KeyCode::KeyD, 2, false),
            (KeyCode::KeyF, 3, false),
            (KeyCode::Enter, usize::MAX, pending.target_enemy),
        ];
        for (tecla, slot, inimigo) in teclas {
            if !keys.just_pressed(tecla) {
                continue;
            }
            let fonte = if inimigo && slot != usize::MAX { &pending.targets } else { &pending.allies };
            if slot != usize::MAX && !fonte[slot].2 {
                continue; // caído
            }
            let kind = pending.selected.expect("seleção verificada");
            let alvo = if slot == usize::MAX { None } else { Some(slot) };
            if let Some(ctl) = manual.ctl.as_mut() {
                match ctl.submit(ChosenAction::Act { kind, target: alvo }) {
                    Ok(()) => {
                        pending.active = false;
                        pending.choosing_target = false;
                        pending.selected = None;
                        scene.chase_gold.clear();
                    }
                    Err(e) => {
                        pending.preview = format!("⚠ ação recusada: {e:?}");
                    }
                }
            }
            break;
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
                    Text::new("2× ▶▶ [ESPAÇO]")
                } else {
                    Text::new("1× ▶ [ESPAÇO]")
                };
            }
            turnrush_core::logger::kv(
                turnrush_core::logger::Level::Info,
                "playback_speed",
                "velocidade do playback alterada",
                &[(
                    "acelerado",
                    turnrush_core::logger::V::S(if ff.0 { "2x" } else { "1x" }),
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

/// Entra na tela de cartas: gera a oferta (run seed) e preenche os botões.
fn reward_enter(
    mut run: ResMut<RunRes>,
    mut rw: ResMut<RewardRes>,
    mut textos: Query<(&RewardBtn, &mut Text), Without<RewardTargetLabel>>,
    mut sub: Query<&mut Text, (With<RewardSubtitle>, Without<RewardBtn>)>,
) {
    if !rw.gerada {
        let factions = turnrush_core::rewards::RunState::team_factions(
            &run.roster,
            [run.team.first().copied().unwrap_or(0), run.team.get(1).copied().unwrap_or(0), run.team.get(2).copied().unwrap_or(0), run.team.get(3).copied().unwrap_or(0)],
        );
        let RunRes { cards, rng, .. } = &mut *run;
        rw.offers = cards.offer(rng, 3, &factions);
        rw.gerada = true;
        rw.pending_unit = None;
    }
    let _ = sub.single_mut().map(|mut t| {
        *t = Text::new(format!(
            "Escolha 1 carta · poder da run {}/300",
            run.cards.poder
        ));
    });
    for (btn, mut t) in textos.iter_mut() {
        let i = (btn.0).min(rw.offers.len().saturating_sub(1));
        if rw.offers.is_empty() {
            *t = Text::new("—");
            continue;
        }
        let c = rw.offers[i];
        let classe = match c.class {
            turnrush_core::rewards::CardClass::Team => "Equipe",
            turnrush_core::rewards::CardClass::Unit => "Unidade",
            turnrush_core::rewards::CardClass::Tactic => "Tática",
        };
        *t = Text::new(format!(
            "{} [{}]\n{}\npoder +{} · {}",
            c.name,
            classe,
            c.desc,
            c.poder,
            c.rarity.label()
        ));
    }
}

/// Cliques da tela de cartas: escolher carta (com alvo, se de unidade),
/// reroll 1×/batalha e avanço para a próxima batalha.
#[allow(clippy::too_many_arguments)] // assinatura de sistema Bevy
fn reward_click(
    mut cmds: Commands,
    mut run: ResMut<RunRes>,
    mut rw: ResMut<RewardRes>,
    mut pb: ResMut<PlaybackRes>,
    pref: Res<PlaybackPref>,
    mut manual: ResMut<ManualRes>,
    mut pending: ResMut<PendingUi>,
    mut scene: ResMut<BattleScene>,
    mut trow: Query<&mut Visibility, (With<RewardTargetRow>, Without<RewardBtn>)>,
    mut labels: Query<(&RewardTargetLabel, &mut Text), Without<RewardBtn>>,
    mut state: ResMut<NextState<GameState>>,
    cards: Query<(&Interaction, &RewardBtn), Changed<Interaction>>,
    alvos: Query<(&Interaction, &RewardTargetBtn), Changed<Interaction>>,
    rerolls: Query<&Interaction, (With<RerollBtn>, Changed<Interaction>)>,
) {
    // escolha de alvo para carta de unidade
    if let Some(card_idx) = rw.pending_unit {
        if let Ok(mut v) = trow.single_mut() {
            *v = Visibility::Visible;
        }
        for (lbl, mut t) in labels.iter_mut() {
            let slot = lbl.0.min(3);
            let ci = run.team.get(slot).copied().unwrap_or(0);
            let c = &run.roster.characters[ci.min(run.roster.len() - 1)];
            *t = Text::new(format!("{} →", c.name));
        }
        for (inter, tb) in alvos.iter() {
            if *inter != Interaction::Pressed {
                continue;
            }
            let slot = tb.0.min(3);
            tomar_carta(&mut run, &mut rw, card_idx, Some(slot));
            iniciar_proxima_batalha(&mut run, &mut pb, &mut manual, &mut pending, &mut scene, &mut cmds, pref.0, &mut state);
            return;
        }
    } else if let Ok(mut v) = trow.single_mut() {
        *v = Visibility::Hidden;
    }

    // reroll (1 por batalha) — a nova oferta é gerada pelo reward_enter
    for inter in rerolls.iter() {
        if *inter == Interaction::Pressed && rw.pending_unit.is_none() && run.cards.reroll() {
            rw.gerada = false;
            rw.offers.clear();
        }
    }

    // escolha de carta
    for (inter, rb) in cards.iter() {
        if *inter != Interaction::Pressed || rw.pending_unit.is_some() {
            continue;
        }
        let Some(&c) = rw.offers.get((rb.0).min(rw.offers.len().saturating_sub(1))) else {
            continue;
        };
        if c.class == turnrush_core::rewards::CardClass::Unit {
            rw.pending_unit = Some(pool_index(c));
            continue;
        }
        tomar_carta(&mut run, &mut rw, pool_index(c), None);
        iniciar_proxima_batalha(&mut run, &mut pb, &mut manual, &mut pending, &mut scene, &mut cmds, pref.0, &mut state);
        return;
    }
}

/// Posição estável de uma carta no CARD_POOL (id para `card_targets`).
fn pool_index(c: &turnrush_core::rewards::RewardCard) -> usize {
    turnrush_core::rewards::CARD_POOL.iter().position(|x| x.id == c.id).unwrap_or(0)
}

/// Registra a carta comprada (alvo para cartas de unidade). `idx` = posição
/// no CARD_POOL (usado como id estável para `card_targets`).
fn tomar_carta(run: &mut RunRes, rw: &mut RewardRes, idx: usize, slot: Option<usize>) {
    let c = &turnrush_core::rewards::CARD_POOL[idx];
    if run.cards.take(c, slot).is_ok() {
        if let Some(slot) = slot {
            run.card_targets.push((slot, idx));
        }
        turnrush_core::logger::kv(
            turnrush_core::logger::Level::Info,
            "card_taken",
            "carta comprada",
            &[
                ("carta", turnrush_core::logger::V::S(c.name)),
                ("poder", turnrush_core::logger::V::U(run.cards.poder as u64)),
            ],
        );
    }
    rw.gerada = false;
    rw.pending_unit = None;
    rw.offers.clear();
}

/// Avança para a próxima batalha (ou fim de run).
#[allow(clippy::too_many_arguments)]
fn iniciar_proxima_batalha(
    run: &mut RunRes,
    pb: &mut PlaybackRes,
    manual: &mut ManualRes,
    pending: &mut PendingUi,
    scene: &mut BattleScene,
    cmds: &mut Commands,
    pref: bool,
    state: &mut ResMut<NextState<GameState>>,
) {
    pending.active = false;
    pending.choosing_target = false;
    scene.chase_gold.clear();
    construir_batalha(pref, run, pb, manual, cmds);
    state.set(GameState::Battle);
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
