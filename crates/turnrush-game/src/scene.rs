//! Cena de batalha em mundo 2D: sprites com animação, barras de HP, números
//! flutuantes, banners de rodada/cadeia/fim, tremor de tela e brasas ambiente.
//!
//! O engine continua a fonte da verdade: a cena apenas **toca** os eventos do
//! playback com pesos de animação — nada de lógica de combate aqui.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::app::{px, ChainsText, PlaybackRes, RunRes, AMBER, BAD, GOOD, ROSE, TXT, WARN};
use crate::sprites::{palette, SpriteAssets};

/// Durações das animações (segundos).
const LUNGE_SECS: f32 = 0.3;
const HIT_SECS: f32 = 0.38;
const DEATH_SECS: f32 = 0.7;
const FLOAT_SECS: f32 = 0.95;

// ─────────────────────────────────────────────────────────────────────────────
// Fila de eventos visuais (app → cena)
// ─────────────────────────────────────────────────────────────────────────────

/// Evento visual enfileirado pelo playback (app.rs converte log → cena).
pub enum VisEvent {
    /// Unidade avança para atacar/conjurar.
    Lunge(usize),
    /// Unidade sofre dano (flash + shake + número).
    Hit(usize, i64),
    /// Unidade recebe cura (número verde).
    Heal(usize, i64),
    /// Ícone de estado (0 queimadura, 1 veneno, 2 CC, 3 escudo).
    Dot(usize, usize),
    /// Unidade cai.
    Death(usize),
    /// Elo de cadeia (contador ao vivo).
    Chain,
    /// v0.9: elo elemental (Ressonância/Cadeia) — texto flutuante central.
    Elemental(&'static str),
    /// Abertura de rodada.
    Round(u64),
    /// Fim da batalha (true = vitória do jogador).
    End(bool),
}

#[derive(Resource, Default)]
pub struct FxQueue(pub Vec<VisEvent>);

/// Marcador de número flutuante.
#[derive(Component)]
pub struct FloatFx {
    t: f32,
}

/// Marcador de banner central (rodada/cadeia/fim).
#[derive(Component)]
pub struct BannerFx {
    t: f32,
    dur: f32,
    scale0: f32,
}

/// Visual de uma unidade em campo.
pub struct UnitVis {
    sprite: Entity,
    name: Entity,
    barbg: Entity,
    barfill: Entity,
    fluxbg: Entity,
    fluxfill: Entity,
    dots: [Entity; 4],
    base: Vec2,
    bob: f32,
    hp_disp: f32,
    lunge: Option<(f32, f32)>, // (t, dir)
    hit: Option<f32>,
    death: Option<f32>,
    dots_on: [bool; 4],
}

#[derive(Resource, Default)]
pub struct BattleScene {
    pub units: [Option<UnitVis>; 8],
    pub spawned: Vec<Entity>,
    pub shake: f32,
    pub chains: u64,
    /// Unidades com highlight dourado (janela de chase manual, v0.7).
    pub chase_gold: Vec<usize>,
}

/// Brasas ambientes (visíveis em todas as telas — clima de roguelike).
#[derive(Component)]
pub struct Ember {
    pub base_x: f32,
    vy: f32,
    wob: f32,
    phase: f32,
    col: Color,
}

// ─────────────────────────────────────────────────────────────────────────────
// Layout (janela fixa 1280×720; mundo = tela − centro)
// ─────────────────────────────────────────────────────────────────────────────

fn slot_base(side: usize, slot: usize) -> Vec2 {
    let sx = if side == 0 { -1.0 } else { 1.0 };
    // frente (slots 0–1) a 300 px do centro; trás (2–3) a 470 px
    let x = match slot {
        0 | 1 => 300.0,
        _ => 470.0,
    };
    let y = if slot.is_multiple_of(2) { 80.0 } else { -70.0 };
    Vec2::new(sx * x, y)
}

fn dot_color(k: usize) -> Color {
    match k {
        0 => Color::srgb(1.0, 0.48, 0.18),  // queimadura
        1 => Color::srgb(0.48, 0.80, 0.29), // veneno
        2 => Color::srgb(1.0, 0.82, 0.25),  // controle (derrubada/arremesso)
        _ => Color::srgb(0.56, 0.83, 0.91), // escudo
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Construção da cena (OnEnter(Battle))
// ─────────────────────────────────────────────────────────────────────────────

pub fn spawn_battle_scene(
    mut cmds: Commands,
    mut scene: ResMut<BattleScene>,
    mut queue: ResMut<FxQueue>,
    run: Res<RunRes>,
    tex: Res<SpriteAssets>,
) {
    // saneamento (a cena anterior deve ter sido despawnada no OnExit)
    scene.units = Default::default();
    scene.spawned.clear();
    scene.shake = 0.0;
    scene.chains = 0;
    scene.chase_gold.clear();
    queue.0.clear();

    // ── piso da arena + linha central + lavagem de cor por lado ──
    let floor = cmds
        .spawn((
            Sprite {
                image: tex.white.clone(),
                custom_size: Some(Vec2::new(820.0, 520.0)),
                color: Color::srgb(0.105, 0.105, 0.15),
                ..default()
            },
            Transform::from_xyz(0.0, 6.0, -10.0),
        ))
        .id();
    let floor2 = cmds
        .spawn((
            Sprite {
                image: tex.white.clone(),
                custom_size: Some(Vec2::new(790.0, 490.0)),
                color: Color::srgb(0.135, 0.135, 0.185),
                ..default()
            },
            Transform::from_xyz(0.0, 6.0, -9.5),
        ))
        .id();
    let mid = cmds
        .spawn((
            Sprite {
                image: tex.white.clone(),
                custom_size: Some(Vec2::new(2.5, 470.0)),
                color: Color::srgba(0.6, 0.6, 0.7, 0.22),
                ..default()
            },
            Transform::from_xyz(0.0, 6.0, -9.0),
        ))
        .id();
    scene.spawned.extend([floor, floor2, mid]);

    let fac = |i: usize| run.roster.characters[i].faction.as_str().to_string();
    let fac_a = fac(*run.team.first().unwrap_or(&0));
    let fac_b = fac(*run.enemy.first().unwrap_or(&0));
    for (side, f) in [(0usize, fac_a), (1usize, fac_b)] {
        let p = palette(&f);
        let gx = if side == 0 { -330.0 } else { 330.0 };
        let glow = cmds
            .spawn((
                Sprite {
                    image: tex.glow.clone(),
                    custom_size: Some(Vec2::new(520.0, 330.0)),
                    color: Color::srgba(
                        p.main[0] as f32 / 255.0,
                        p.main[1] as f32 / 255.0,
                        p.main[2] as f32 / 255.0,
                        0.30,
                    ),
                    ..default()
                },
                Transform::from_xyz(gx, 0.0, -8.0),
            ))
            .id();
        scene.spawned.push(glow);
    }

    // ── rótulos dos lados ──
    let lab_l = cmds
        .spawn((
            Text2d::new("YOUR FORMATION"),
            TextFont { font_size: px(19.0), ..default() },
            TextColor(AMBER),
            Transform::from_xyz(-455.0, 196.0, 2.0),
        ))
        .id();
    let lab_r = cmds
        .spawn((
            Text2d::new("ENEMY FORMATION"),
            TextFont { font_size: px(19.0), ..default() },
            TextColor(ROSE),
            Transform::from_xyz(455.0, 196.0, 2.0),
        ))
        .id();
    scene.spawned.extend([lab_l, lab_r]);

    // ── unidades (sprite + nome + barra + ícones de estado) ──
    for side in 0..2usize {
        let comp = if side == 0 { run.team.clone() } else { run.enemy.clone() };
        for (slot, &ci) in comp.iter().enumerate() {
            let u = side * 4 + slot;
            let c = &run.roster.characters[ci];
            let base = slot_base(side, slot);

            let sprite = cmds
                .spawn((
                    Sprite {
                        image: tex.units[ci].clone(),
                        custom_size: Some(Vec2::splat(96.0)),
                        flip_x: side == 1,
                        ..default()
                    },
                    Transform::from_xyz(base.x, base.y + 36.0, 0.0),
                ))
                .id();
            let name = cmds
                .spawn((
                    Text2d::new(c.name.clone()),
                    TextFont { font_size: px(15.0), ..default() },
                    TextColor(TXT),
                    Transform::from_xyz(base.x, base.y - 30.0, 1.0),
                ))
                .id();
            let barbg = cmds
                .spawn((
                    Sprite {
                        image: tex.white.clone(),
                        custom_size: Some(Vec2::new(98.0, 10.0)),
                        color: Color::srgb(0.06, 0.06, 0.09),
                        ..default()
                    },
                    Transform::from_xyz(base.x, base.y - 44.0, 1.0),
                ))
                .id();
            let barfill = cmds
                .spawn((
                    Sprite {
                        image: tex.white.clone(),
                        custom_size: Some(Vec2::new(94.0, 6.0)),
                        color: GOOD,
                        ..default()
                    },
                    Anchor(Anchor::CENTER_LEFT.0),
                    Transform::from_xyz(base.x - 47.0, base.y - 44.0, 1.5),
                ))
                .id();
            let fluxbg = cmds
                .spawn((
                    Sprite {
                        image: tex.white.clone(),
                        custom_size: Some(Vec2::new(98.0, 7.0)),
                        color: Color::srgb(0.05, 0.07, 0.10),
                        ..default()
                    },
                    Transform::from_xyz(base.x, base.y - 55.0, 1.0),
                ))
                .id();
            let fluxfill = cmds
                .spawn((
                    Sprite {
                        image: tex.white.clone(),
                        custom_size: Some(Vec2::new(94.0, 4.0)),
                        color: Color::srgb(0.30, 0.75, 0.95),
                        ..default()
                    },
                    Anchor(Anchor::CENTER_LEFT.0),
                    Transform::from_xyz(base.x - 47.0, base.y - 55.0, 1.5),
                ))
                .id();
            let mut dots = [Entity::PLACEHOLDER; 4];
            for (k, dot) in dots.iter_mut().enumerate() {
                let dx = base.x - 18.0 + 12.0 * k as f32;
                *dot = cmds
                    .spawn((
                        Sprite {
                            image: tex.white.clone(),
                            custom_size: Some(Vec2::splat(8.0)),
                            color: dot_color(k),
                            ..default()
                        },
                        Transform::from_xyz(dx, base.y + 62.0, 2.0),
                        Visibility::Hidden,
                    ))
                    .id();
            }
            scene.spawned.extend([sprite, name, barbg, barfill, fluxbg, fluxfill]);
            scene.spawned.extend(dots);
            scene.units[u] = Some(UnitVis {
                sprite,
                name,
                barbg,
                barfill,
                fluxbg,
                fluxfill,
                dots,
                base,
                bob: (u as f32) * 1.31,
                hp_disp: 1.0,
                lunge: None,
                hit: None,
                death: None,
                dots_on: [false; 4],
            });
        }
    }

    // (o banner da rodada 1 vem do próprio playback — evita banner duplicado)
    turnrush_core::logger::kv(
        turnrush_core::logger::Level::Info,
        "scene_spawned",
        "cena de batalha construída",
        &[
            ("entidades", turnrush_core::logger::V::U(scene.spawned.len() as u64)),
            ("units", turnrush_core::logger::V::U(scene.units.iter().filter(|u| u.is_some()).count() as u64)),
        ],
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Remoção (OnExit(Battle))
// ─────────────────────────────────────────────────────────────────────────────

pub fn despawn_battle_scene(mut cmds: Commands, mut scene: ResMut<BattleScene>, mut queue: ResMut<FxQueue>) {
    for e in scene.spawned.drain(..) {
        cmds.entity(e).try_despawn();
    }
    scene.units = Default::default();
    scene.shake = 0.0;
    scene.chains = 0;
    queue.0.clear();
    // flutuantes/banners se autodespawnam pelo timer (marcadores)
}

// ─────────────────────────────────────────────────────────────────────────────
// Consumo da fila → spawn de efeitos
// ─────────────────────────────────────────────────────────────────────────────

pub fn scene_fx(
    mut cmds: Commands,
    tex: Res<SpriteAssets>,
    mut scene: ResMut<BattleScene>,
    mut queue: ResMut<FxQueue>,
    mut chains_lbl: Query<&mut Text, With<ChainsText>>,
) {
    for ev in queue.0.drain(..) {
        match ev {
            VisEvent::Lunge(u) => {
                if let Some(v) = scene.units[u].as_mut() {
                    v.lunge = Some((0.0, if v.base.x < 0.0 { 1.0 } else { -1.0 }));
                }
            }
            VisEvent::Hit(u, dmg) => {
                if let Some(v) = scene.units[u].as_mut() {
                    v.hit = Some(0.0);
                    spawn_float(&mut cmds, v.base + Vec2::new(0.0, 68.0), format!("-{dmg}"), BAD, 21.0);
                }
                scene.shake = scene.shake.max((dmg as f32 / 85.0).min(6.5)).max(1.0);
            }
            VisEvent::Heal(u, amt) => {
                if let Some(v) = scene.units[u].as_mut() {
                    spawn_float(&mut cmds, v.base + Vec2::new(0.0, 68.0), format!("+{amt}"), GOOD, 21.0);
                }
            }
            VisEvent::Dot(u, k) => {
                if let Some(v) = scene.units[u].as_mut() {
                    v.dots_on[k] = true;
                }
            }
            VisEvent::Death(u) => {
                if let Some(v) = scene.units[u].as_mut() {
                    v.death = Some(0.0);
                }
            }
            VisEvent::Chain => {
                scene.chains += 1;
                spawn_float(&mut cmds, Vec2::new(0.0, 176.0), format!("CHAIN ×{}", scene.chains), AMBER, 26.0);
                if let Ok(mut t) = chains_lbl.single_mut() {
                    *t = Text::new(format!("Chains: {}", scene.chains));
                }
            }
            VisEvent::Elemental(label) => {
                // v0.9: Ressonância/Cadeia elemental — texto flutuante âmbar.
                spawn_float(&mut cmds, Vec2::new(0.0, 214.0), label.to_string(), AMBER, 24.0);
            }
            VisEvent::Round(n) => {
                spawn_banner(&mut cmds, format!("ROUND {n}"), TXT, 46.0, 1.5);
                // estados visuais reiniciam a cada rodada
                for v in scene.units.iter_mut().flatten() {
                    v.dots_on = [false; 4];
                }
            }
            VisEvent::End(won) => {
                spawn_banner(
                    &mut cmds,
                    (if won { "VICTORY" } else { "DEFEAT" }).to_string(),
                    if won { GOOD } else { BAD },
                    56.0,
                    2.2,
                );
            }
        }
    }
    let _ = &tex;
}

fn spawn_float(cmds: &mut Commands, pos: Vec2, text: String, col: Color, size: f32) {
    cmds.spawn((
        Text2d::new(text),
        TextFont { font_size: px(size), ..default() },
        TextColor(col),
        Transform::from_xyz(pos.x, pos.y, 3.0),
        FloatFx { t: 0.0 },
    ));
}

fn spawn_banner(cmds: &mut Commands, text: String, col: Color, size: f32, dur: f32) {
    cmds.spawn((
        Text2d::new(text),
        TextFont { font_size: px(size), ..default() },
        TextColor(col),
        Transform::from_xyz(0.0, 40.0, 4.0).with_scale(Vec3::splat(1.35)),
        BannerFx { t: 0.0, dur, scale0: 1.35 },
    ));
}

// ─────────────────────────────────────────────────────────────────────────────
// Animação contínua
// ─────────────────────────────────────────────────────────────────────────────

/// Bob de idle, avanço, tremor de impacto, morte e barras de HP.
pub fn scene_animate(
    time: Res<Time>,
    pb: Res<PlaybackRes>,
    mut scene: ResMut<BattleScene>,
    mut sprites: Query<(&mut Transform, &mut Sprite)>,
    mut vis: Query<&mut Visibility>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let dourados: Vec<usize> = scene.chase_gold.clone();
    for u in 0..8usize {
        let Some(v) = scene.units[u].as_mut() else { continue };
        // barra persegue o HP real (suavizado)
        let target = pb.hp_pct(u);
        v.hp_disp += (target - v.hp_disp) * (dt * 9.0).min(1.0);

        // deslocamentos e tinta
        let mut dx = 0.0;
        let mut dy = 0.0;
        let mut tint = Color::WHITE;
        // highlight dourado da janela de chase (pulsa)
        let dourado = dourados.contains(&u) && v.hit.is_none() && v.death.is_none();
        if dourado {
            let pulso = 0.72 + 0.28 * (t * 6.0).sin().abs();
            tint = Color::srgb(1.0 * pulso, 0.82 * pulso, 0.25 * pulso);
        }
        if let Some((lt, dir)) = v.lunge.as_mut() {
            *lt += dt;
            let k = (*lt / LUNGE_SECS).min(1.0);
            dx += *dir * 30.0 * (k * std::f32::consts::PI).sin();
            if *lt >= LUNGE_SECS {
                v.lunge = None;
            }
        }
        if let Some(ht) = v.hit.as_mut() {
            *ht += dt;
            let k = (*ht / HIT_SECS).min(1.0);
            dx += (*ht * 48.0).sin() * 4.5 * (1.0 - k);
            let f = (1.0 - *ht / 0.16).clamp(0.0, 1.0);
            tint = Color::srgb(1.0, 0.35 + 0.65 * (1.0 - f), 0.35 + 0.65 * (1.0 - f));
            if *ht >= HIT_SECS {
                v.hit = None;
                tint = Color::WHITE;
            }
        }
        let mut dead_hidden = false;
        if let Some(dth) = v.death.as_mut() {
            *dth += dt;
            let k = (*dth / DEATH_SECS).min(1.0);
            dy -= 14.0 * k;
            tint = Color::srgba(0.55, 0.55, 0.55, (1.0 - k).max(0.0));
            if *dth >= DEATH_SECS {
                dead_hidden = true;
            }
        } else if v.hit.is_none() {
            dy += (t * 2.2 + v.bob).sin() * 2.6; // idle bob
        }

        // aplica no sprite
        if let Ok((mut tr, mut sp)) = sprites.get_mut(v.sprite) {
            tr.translation.x = v.base.x + dx;
            tr.translation.y = v.base.y + 36.0 + dy;
            sp.color = tint;
        }
        // barra de fluxo (teto 100 — visível dos DOIS lados: ⚡ simétrico)
        let fluxo_pct = (pb.fluxo[u].clamp(0, 100) as f32) / 100.0;
        if let Ok((_, mut fl)) = sprites.get_mut(v.fluxfill) {
            fl.custom_size = Some(Vec2::new((94.0 * fluxo_pct).max(0.0), 4.0));
        }
        // barra de HP
        if let Ok((_, mut bar)) = sprites.get_mut(v.barfill) {
            bar.custom_size = Some(Vec2::new((94.0 * v.hp_disp).max(0.0), 6.0));
            bar.color = if !pb.alive[u] {
                BAD
            } else if v.hp_disp > 0.5 {
                GOOD
            } else if v.hp_disp > 0.25 {
                WARN
            } else {
                BAD
            };
        }
        // visibilidade na morte
        let hide = if dead_hidden { Visibility::Hidden } else { Visibility::Visible };
        for e in [v.sprite, v.name, v.barbg, v.barfill, v.fluxbg, v.fluxfill] {
            if let Ok(mut viz) = vis.get_mut(e) {
                *viz = hide;
            }
        }
        for (k, de) in v.dots.iter().enumerate() {
            if let Ok(mut viz) = vis.get_mut(*de) {
                *viz = if dead_hidden || !v.dots_on[k] { Visibility::Hidden } else { Visibility::Visible };
            }
        }
    }
}

/// Movimento/fade dos flutuantes e banners + decaimento do shake de tela.
#[allow(clippy::type_complexity)]
pub fn fx_motion(
    time: Res<Time>,
    mut cmds: Commands,
    mut scene: ResMut<BattleScene>,
    mut floats: Query<(Entity, &mut FloatFx, &mut Transform, &mut TextColor), Without<BannerFx>>,
    mut banners: Query<(Entity, &mut BannerFx, &mut Transform, &mut TextColor), Without<FloatFx>>,
) {
    let dt = time.delta_secs();

    for (e, mut f, mut tr, mut col) in floats.iter_mut() {
        f.t += dt;
        tr.translation.y += 42.0 * dt;
        let a = (1.0 - f.t / FLOAT_SECS).clamp(0.0, 1.0);
        col.0.set_alpha(a);
        if f.t >= FLOAT_SECS {
            cmds.entity(e).try_despawn();
        }
    }
    for (e, mut b, mut tr, mut col) in banners.iter_mut() {
        b.t += dt;
        let k = (b.t / (b.dur * 0.3)).min(1.0);
        tr.scale = Vec3::splat(b.scale0 + (1.0 - b.scale0) * k);
        let a = if b.t < b.dur * 0.7 {
            1.0
        } else {
            (1.0 - (b.t - b.dur * 0.7) / (b.dur * 0.3)).clamp(0.0, 1.0)
        };
        col.0.set_alpha(a);
        if b.t >= b.dur {
            cmds.entity(e).try_despawn();
        }
    }
    scene.shake *= (1.0 - dt * 6.5).max(0.0);
}

/// Tremor de tela (câmera) proporcional aos impactos.
pub fn scene_shake(time: Res<Time>, scene: Res<BattleScene>, mut cams: Query<&mut Transform, With<Camera2d>>) {
    let Ok(mut cam) = cams.single_mut() else { return };
    if scene.shake > 0.05 {
        let t = time.elapsed_secs();
        cam.translation.x = (t * 57.0).sin() * scene.shake;
        cam.translation.y = (t * 49.0 + 1.7).sin() * scene.shake * 0.7;
    } else {
        cam.translation.x = 0.0;
        cam.translation.y = 0.0;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Brasas ambiente (todas as telas)
// ─────────────────────────────────────────────────────────────────────────────

/// Gera 26 brasas sobem pela tela inteira (menu, draft e batalha).
pub fn spawn_embers(mut cmds: Commands, tex: Res<SpriteAssets>) {
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f32 / (1u64 << 53) as f32
    };
    let cores = [
        Color::srgba(0.96, 0.62, 0.10, 0.55),
        Color::srgba(0.96, 0.35, 0.30, 0.45),
        Color::srgba(0.90, 0.75, 0.30, 0.40),
    ];
    for i in 0..26 {
        let size = 5.0 + rnd() * 9.0;
        let x = -640.0 + rnd() * 1280.0;
        cmds.spawn((
            Sprite {
                image: tex.glow.clone(),
                custom_size: Some(Vec2::splat(size)),
                color: cores[i % cores.len()],
                ..default()
            },
            Transform::from_xyz(x, -390.0 + rnd() * 780.0, -6.0),
            Ember {
                base_x: x,
                vy: 14.0 + rnd() * 22.0,
                wob: 0.6 + rnd() * 1.4,
                phase: rnd() * std::f32::consts::TAU,
                col: cores[i % cores.len()],
            },
        ));
    }
}

/// Deriva das brasas (todas as telas).
pub fn embers_tick(time: Res<Time>, mut q: Query<(&mut Transform, &mut Sprite, &Ember)>) {
    let t = time.elapsed_secs();
    for (mut tr, mut sp, e) in q.iter_mut() {
        tr.translation.y += e.vy * time.delta_secs();
        if tr.translation.y > 400.0 {
            tr.translation.y = -400.0;
        }
        tr.translation.x = e.base_x + (t * e.wob + e.phase).sin() * 14.0;
        let flicker = 0.75 + 0.25 * (t * 3.0 + e.phase).sin();
        let c = e.col.to_srgba();
        sp.color = Color::srgba(c.red, c.green, c.blue, c.alpha * flicker);
    }
}
