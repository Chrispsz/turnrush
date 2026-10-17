//! turnrush-core — motor determinístico do TurnRush.
//!
//! Princípios:
//! - **Determinismo total**: RNG xoshiro256** semeado + matemática 100% inteira
//!   (per-mille) → mesma seed = mesma partida em qualquer plataforma.
//! - **Equilíbrio por construção**: draft simétrico (os dois veem as mesmas cartas),
//!   LCE compensa sorte em tempo real, DWC calibra composições por winrate medido.
//! - **Estilo Naruto Online**: iniciativa, energia (chakra), cooldowns, lançamentos
//!   e perseguições (chase) encadeadas — no 1v1 e no **modo Formação 4v4**.
//! - **Zero dependências externas** (`std` apenas): build pequeno, CI rápido,
//!   superfície de auditoria mínima (sem unsafe, sem leaks por terceiros).
//! - **Logs de sessão completos**: "script de tudo" barato em memória +
//!   JSONL estruturado + relatório de crash em panics.

pub mod balance;
pub mod cards;
pub mod combat;
pub mod data;
pub mod formation;
pub mod json;
pub mod logger;
pub mod rng;
pub mod run;
pub mod types;

/// Versão da engine (workspace `game/Cargo.toml`).
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use rng::Rng;
