//! Sprites procedurais — pixel-art 64×64 gerada em runtime (zero assets).
//!
//! Cada personagem recebe um sprite derivado de **(clã × classe)**: a paleta
//! vem do clã e a silhueta/equipamento vem da classe. Estilo genérico,
//! coerente e legível — pensado para ser substituído por arte final sem
//! mudar uma linha do cliente.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Lado do canvas de personagem (64×64, "pés" em y=58).
const SZ: usize = 64;

// ─────────────────────────────────────────────────────────────────────────────
// Canvas RGBA com primitivas de desenho
// ─────────────────────────────────────────────────────────────────────────────

struct Canvas {
    px: Vec<u8>,
}

impl Canvas {
    fn new() -> Self {
        Canvas { px: vec![0; SZ * SZ * 4] }
    }

    fn set(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if x < 0 || y < 0 || x as usize >= SZ || y as usize >= SZ || c[3] == 0 {
            return;
        }
        let i = (y as usize * SZ + x as usize) * 4;
        if c[3] == 255 {
            self.px[i..i + 4].copy_from_slice(&c);
        } else {
            // mistura alpha simples sobre o que já existe
            let a = c[3] as u32;
            for (k, &src) in c.iter().take(3).enumerate() {
                let d = self.px[i + k] as u32;
                self.px[i + k] = ((src as u32 * a + d * (255 - a)) / 255) as u8;
            }
            let da = self.px[i + 3] as u32;
            self.px[i + 3] = (a + da * (255 - a) / 255).min(255) as u8;
        }
    }

    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: [u8; 4]) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.set(x, y, c);
            }
        }
    }

    fn circle(&mut self, cx: i32, cy: i32, r: i32, c: [u8; 4]) {
        for y in (cy - r)..=(cy + r) {
            for x in (cx - r)..=(cx + r) {
                let dx = (x - cx) * (x - cx);
                let dy = (y - cy) * (y - cy);
                if dx + dy <= r * r {
                    self.set(x, y, c);
                }
            }
        }
    }

    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, t: i32, c: [u8; 4]) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for s in 0..=steps {
            let x = x0 + (x1 - x0) * s / steps;
            let y = y0 + (y1 - y0) * s / steps;
            for dy in 0..t {
                for dx in 0..t {
                    self.set(x + dx, y + dy, c);
                }
            }
        }
    }

    fn diamond(&mut self, cx: i32, cy: i32, r: i32, c: [u8; 4]) {
        for y in (cy - r)..=(cy + r) {
            let w = r - (y - cy).abs();
            self.set(cx - w, y, c);
            self.set(cx + w, y, c);
            for x in (cx - w)..=(cx + w) {
                self.set(x, y, c);
            }
        }
    }

    fn into_image(self) -> Image {
        Image::new(
            Extent3d { width: SZ as u32, height: SZ as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            self.px,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Paletas por clã
// ─────────────────────────────────────────────────────────────────────────────

pub struct Pal {
    pub main: [u8; 4],
    pub dark: [u8; 4],
    pub light: [u8; 4],
    pub accent: [u8; 4],
}

pub fn palette(faction: &str) -> Pal {
    match faction {
        "ember" => Pal {
            main: [225, 95, 32, 255],
            dark: [118, 45, 14, 255],
            light: [250, 140, 66, 255],
            accent: [255, 210, 70, 255],
        },
        "tide" => Pal {
            main: [45, 165, 156, 255],
            dark: [20, 78, 76, 255],
            light: [110, 208, 198, 255],
            accent: [190, 240, 235, 255],
        },
        "verdant" => Pal {
            main: [78, 172, 82, 255],
            dark: [30, 92, 36, 255],
            light: [138, 212, 120, 255],
            accent: [200, 244, 155, 255],
        },
        "storm" => Pal {
            main: [214, 190, 48, 255],
            dark: [106, 92, 18, 255],
            light: [238, 222, 112, 255],
            accent: [255, 242, 178, 255],
        },
        _ => Pal {
            main: [140, 140, 150, 255],
            dark: [70, 70, 80, 255],
            light: [180, 180, 190, 255],
            accent: [220, 220, 230, 255],
        },
    }
}

const STEEL: [u8; 4] = [196, 200, 208, 255];
const STEEL_DARK: [u8; 4] = [96, 100, 112, 255];
const VISOR: [u8; 4] = [22, 20, 28, 255];
const SHADOW: [u8; 4] = [0, 0, 0, 80];

// ─────────────────────────────────────────────────────────────────────────────
// Sprite de personagem (clã × classe)
// ─────────────────────────────────────────────────────────────────────────────

/// Desenha o sprite 64×64 de um personagem genérico.
///
/// Anatomia compartilhada (todos os personagens): sombra elíptica, pernas,
/// tronco, cabeça com viseira. A **classe** define silhueta/equipamento e o
/// **clã** define a paleta — sprites genéricos e coerentes para todo o elenco.
pub fn unit_image(faction: &str, role: &str) -> Image {
    let p = palette(faction);
    let mut c = Canvas::new();

    // sombra no chão
    for (i, y) in [10usize, 6, 2].iter().enumerate() {
        let w = *y as i32;
        let yy = 58 + i as i32;
        c.rect(32 - w, yy, 32 + w, yy, SHADOW);
    }

    match role {
        // REAVER (striker): silhueta esguia + lâmina diagonal
        "striker" => {
            c.rect(28, 44, 30, 57, p.dark);
            c.rect(33, 44, 35, 57, p.dark);
            c.rect(27, 26, 36, 45, p.main);
            c.rect(27, 40, 36, 42, p.accent); // faixa do cinto
            c.rect(24, 27, 28, 32, p.dark); // ombro esq.
            c.circle(32, 15, 7, p.light); // cabeça
            c.rect(28, 13, 36, 15, VISOR); // viseira
            c.line(38, 36, 54, 12, 2, STEEL); // lâmina
            c.rect(36, 35, 40, 38, p.accent); // guarda
        }
        // BASTION (guardian): tronco largo + escudo torre frontal
        "guardian" => {
            c.rect(27, 43, 30, 57, p.dark);
            c.rect(34, 43, 37, 57, p.dark);
            c.rect(24, 22, 39, 45, p.main);
            c.rect(21, 23, 26, 31, p.dark); // pauldron esq.
            c.rect(37, 23, 42, 31, p.dark); // pauldron dir.
            c.rect(28, 38, 35, 40, p.accent);
            c.circle(32, 14, 7, p.light);
            c.rect(28, 12, 36, 14, VISOR);
            c.rect(30, 8, 34, 10, p.accent); // crista do elmo
            c.rect(15, 30, 25, 50, p.dark); // escudo torre
            c.rect(17, 32, 23, 48, STEEL_DARK);
            c.rect(19, 38, 21, 42, p.accent); // brasão
        }
        // MENDER (support): túnica + cajado com orbe luminoso
        "support" => {
            c.rect(27, 26, 36, 42, p.main); // tronco
            c.rect(24, 42, 39, 56, p.main); // saia da túnica
            c.rect(24, 52, 39, 54, p.dark);
            c.circle(32, 15, 7, p.light);
            c.rect(28, 13, 36, 15, VISOR);
            c.rect(44, 18, 46, 52, STEEL_DARK); // cajado
            c.circle(45, 14, 4, p.accent); // orbe
            c.circle(45, 14, 2, [255, 255, 255, 255]);
        }
        // TACTICIAN (controller): capuz + runas flutuantes
        "controller" => {
            c.rect(26, 26, 37, 47, p.dark); // manto
            c.rect(24, 45, 39, 56, p.dark);
            c.circle(32, 16, 7, p.dark); // capuz fechado
            c.rect(28, 15, 36, 17, VISOR);
            c.diamond(47, 24, 4, p.accent); // runa maior
            c.diamond(17, 30, 3, p.accent); // runa menor
            c.rect(27, 40, 36, 42, p.main);
        }
        _ => {
            c.rect(28, 44, 30, 57, p.dark);
            c.rect(33, 44, 35, 57, p.dark);
            c.rect(27, 26, 36, 45, p.main);
            c.circle(32, 15, 7, p.light);
            c.rect(28, 13, 36, 15, VISOR);
        }
    }

    c.into_image()
}

/// Retângulo branco 4×4 — usado esticado para barras/painéis/linhas.
pub fn white_image() -> Image {
    Image::new(
        Extent3d { width: 4, height: 4, depth_or_array_layers: 1 },
        TextureDimension::D2,
        vec![255; 4 * 4 * 4],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Brilho radial branco 64×64 (aura, brasas, lavagem de cor por lado).
pub fn glow_image() -> Image {
    let mut c = Canvas::new();
    let half = SZ as f32 / 2.0;
    for y in 0..SZ {
        for x in 0..SZ {
            let dx = (x as f32 - half + 0.5) / half;
            let dy = (y as f32 - half + 0.5) / half;
            let d = (dx * dx + dy * dy).sqrt().min(1.0);
            let a = ((1.0 - d) * (1.0 - d) * 255.0) as u8;
            c.set(x as i32, y as i32, [255, 255, 255, a]);
        }
    }
    c.into_image()
}

/// Handles dos sprites gerados no `Startup`.
#[derive(Resource, Clone)]
pub struct SpriteAssets {
    /// Um sprite por índice do roster.
    pub units: Vec<Handle<Image>>,
    pub white: Handle<Image>,
    pub glow: Handle<Image>,
}
