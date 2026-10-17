//! Cartas ("Contratos") do draft e o **LCE — Luck Compensation Engine**.
//!
//! - **Oferta simétrica**: os dois jogadores veem EXATAMENTE as mesmas cartas
//!   (mesmo stream de RNG) → a sorte não é assimétrica por construção.
//! - **LCE**: rastreia o valor acumulado de cartas de cada jogador; quem está
//!   atrás em valor recebe a próxima carta com tier promovido (+1), compensando
//!   desvios de escolha/sorte em tempo real sem punir ninguém.

use crate::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    /// Bônus de stats para uma unidade da composição (escolhida na pick).
    UnitStats { hp: i32, atk: i32, def: i32, spd: i32 },
    /// Bônus de stats para todo o time.
    TeamStats { hp: i32, atk: i32, def: i32, spd: i32 },
    /// Modificação percentual do time (per-mille aditivo).
    TeamPct { dmg_permille: i32, hp_permille: i32 },
}

#[derive(Clone, Copy, Debug)]
pub struct Card {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub tier: u8,
    /// Valor nominal usado pelo LCE para medir o "valor de sorte" acumulado.
    pub value: i32,
    pub kind: CardKind,
}

const fn card(
    id: &'static str,
    name: &'static str,
    desc: &'static str,
    tier: u8,
    value: i32,
    kind: CardKind,
) -> Card {
    Card { id, name, desc, tier, value, kind }
}

/// Pool de cartas do draft (tier 1..4). Ordem determinística — o LCE usa
/// `tier + 1` para promover.
pub const CARD_POOL: [Card; 16] = [
    // ── Tier 1 — refinamentos ────────────────────────────────────────────────
    card("atk1", "Afiar", "+6 ATK para uma unidade", 1, 10, CardKind::UnitStats { hp: 0, atk: 6, def: 0, spd: 0 }),
    card("hp1", "Vigor", "+22 HP para uma unidade", 1, 10, CardKind::UnitStats { hp: 22, atk: 0, def: 0, spd: 0 }),
    card("def1", "Couraçar", "+6 DEF para uma unidade", 1, 10, CardKind::UnitStats { hp: 0, atk: 0, def: 6, spd: 0 }),
    card("spd1", "Agilizar", "+2 SPD para uma unidade", 1, 10, CardKind::UnitStats { hp: 0, atk: 0, def: 0, spd: 2 }),
    // ── Tier 2 — vantagens ───────────────────────────────────────────────────
    card("atk2", "Açoite", "+13 ATK para uma unidade", 2, 20, CardKind::UnitStats { hp: 0, atk: 13, def: 0, spd: 0 }),
    card("hp2", "Fortalecer", "+50 HP para uma unidade", 2, 20, CardKind::UnitStats { hp: 50, atk: 0, def: 0, spd: 0 }),
    card("def2", "Reforçar", "+13 DEF para uma unidade", 2, 20, CardKind::UnitStats { hp: 0, atk: 0, def: 13, spd: 0 }),
    card("spd2", "Sombra Ligeira", "+4 SPD para uma unidade", 2, 20, CardKind::UnitStats { hp: 0, atk: 0, def: 0, spd: 4 }),
    // ── Tier 3 — poderes ─────────────────────────────────────────────────────
    card("atk3", "Titã", "+22 ATK para uma unidade", 3, 30, CardKind::UnitStats { hp: 0, atk: 22, def: 0, spd: 0 }),
    card("hp3", "Colosso", "+95 HP para uma unidade", 3, 30, CardKind::UnitStats { hp: 95, atk: 0, def: 0, spd: 0 }),
    card("all1", "Bandeira de Guerra", "+4 ATK, +2 DEF e +30 HP para o time", 3, 30, CardKind::TeamStats { hp: 30, atk: 4, def: 2, spd: 0 }),
    card("wall1", "Muralha Viva", "+8 DEF para o time", 3, 30, CardKind::TeamStats { hp: 0, atk: 0, def: 8, spd: 0 }),
    // ── Tier 4 — épicas ──────────────────────────────────────────────────────
    card("dmg8", "Ressonância Ofensiva", "+8% de dano para o time", 4, 40, CardKind::TeamPct { dmg_permille: 80, hp_permille: 0 }),
    card("hp8", "Ressonância Vital", "+8% de vida para o time", 4, 40, CardKind::TeamPct { dmg_permille: 0, hp_permille: 80 }),
    card("hybrid", "Coroa do Estrategista", "+4% de dano e +4% de vida para o time", 4, 40, CardKind::TeamPct { dmg_permille: 40, hp_permille: 40 }),
    card("spd3", "Passo do Vento", "+7 SPD para uma unidade", 4, 40, CardKind::UnitStats { hp: 0, atk: 0, def: 0, spd: 7 }),
];

/// Escolha de um jogador: carta + unidade-alvo (slot na composição).
#[derive(Clone, Copy, Debug)]
pub struct Pick {
    pub card: usize,
    pub target_unit: usize,
}

#[derive(Clone, Debug, Default)]
pub struct DraftState {
    /// Valor nominal total acumulado (LCE).
    pub value: i32,
    /// Cartas escolhidas (aplicadas cumulativamente à composição).
    pub picks: Vec<Pick>,
}

/// Oferta de `n` cartas distintas — determinística pelo RNG compartilhado.
/// Como só é chamada uma vez por rodada, ambos os jogadores veem a mesma oferta.
pub fn offer(rng: &mut Rng, n: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..CARD_POOL.len()).collect();
    rng.shuffle(&mut idx);
    idx.truncate(n);
    idx
}

/// Resolve as escolhas dos dois jogadores aplicando a compensação do LCE.
/// Retorna os índices efetivos concedidos (com promoção para quem está atrás).
pub fn resolve_picks(
    pick_a: usize,
    pick_b: usize,
    a: &mut DraftState,
    b: &mut DraftState,
) -> (usize, usize) {
    let eff_a = effective_card(pick_a, a.value, b.value);
    let eff_b = effective_card(pick_b, b.value, a.value);
    a.value += CARD_POOL[eff_a].value;
    a.picks.push(Pick { card: eff_a, target_unit: 0 });
    b.value += CARD_POOL[eff_b].value;
    b.picks.push(Pick { card: eff_b, target_unit: 0 });
    (eff_a, eff_b)
}

/// LCE: se estou atrás em valor por ≥ 2 tiers (20), minha carta é promovida.
pub fn effective_card(pick: usize, my_value: i32, opp_value: i32) -> usize {
    if my_value + 20 <= opp_value {
        promote(pick)
    } else {
        pick
    }
}

fn promote(idx: usize) -> usize {
    let tier = CARD_POOL[idx].tier;
    CARD_POOL
        .iter()
        .position(|c| c.tier == tier + 1)
        .unwrap_or(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oferta_deterministica_e_distinta() {
        let mut r1 = Rng::new(100);
        let mut r2 = Rng::new(100);
        let o1 = offer(&mut r1, 3);
        let o2 = offer(&mut r2, 3);
        assert_eq!(o1, o2);
        assert_eq!(o1.len(), 3);
        // sem repetições
        for (i, a) in o1.iter().enumerate() {
            for b in o1.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn lce_promove_quem_esta_atras() {
        // a está 20 de valor atrás → promoção
        let a = DraftState { value: 10, picks: vec![] };
        let eff = effective_card(0, a.value, 50);
        assert!(CARD_POOL[eff].tier > CARD_POOL[0].tier);
        // sem gap → sem promoção
        let eff2 = effective_card(0, 50, 50);
        assert_eq!(eff2, 0);
    }

    #[test]
    fn resolve_picks_atualiza_valores() {
        let mut a = DraftState::default();
        let mut b = DraftState::default();
        let (ea, eb) = resolve_picks(0, 5, &mut a, &mut b);
        assert_eq!(a.value, CARD_POOL[ea].value);
        assert_eq!(b.value, CARD_POOL[eb].value);
    }
}
