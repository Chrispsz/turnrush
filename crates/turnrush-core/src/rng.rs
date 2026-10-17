//! RNG determinístico xoshiro256** — semeado por splitmix64.
//! Mesma seed → mesma sequência, em qualquer plataforma/arquitetura.

#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        Rng {
            s: [
                splitmix64(&mut sm),
                splitmix64(&mut sm),
                splitmix64(&mut sm),
                splitmix64(&mut sm),
            ],
        }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Float em [0, 1) com resolução de 24 bits (determinístico).
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u64 << 24) as f32)
    }

    /// Inteiro em [0, n).
    #[inline]
    pub fn range(&mut self, n: usize) -> usize {
        debug_assert!(n > 0);
        (self.next_u64() % (n as u64)) as usize
    }

    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        if v.len() < 2 {
            return;
        }
        for i in (1..v.len()).rev() {
            let j = self.range(i + 1);
            v.swap(i, j);
        }
    }

    /// Escolhe um índice por roleta ponderada (pesos positivos).
    pub fn weighted(&mut self, weights: &[f32]) -> usize {
        let total: f32 = weights.iter().sum();
        if total <= 0.0 {
            return 0;
        }
        let mut roll = self.next_f32() * total;
        for (i, w) in weights.iter().enumerate() {
            roll -= w;
            if roll <= 0.0 {
                return i;
            }
        }
        weights.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesma_seed_mesma_sequencia() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn seeds_diferentes_divergem() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let diff = (0..100).filter(|_| a.next_u64() != b.next_u64()).count();
        assert!(diff > 95, "sequências de seeds distintas devem divergir");
    }

    #[test]
    fn faixa_valida() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            assert!(r.range(5) < 5);
        }
    }

    #[test]
    fn float_no_intervalo() {
        let mut r = Rng::new(9);
        for _ in 0..10_000 {
            let f = r.next_f32();
            assert!((0.0..1.0).contains(&f));
        }
    }

    #[test]
    fn roleta_respeita_pesos() {
        let mut r = Rng::new(11);
        let mut counts = [0usize; 2];
        for _ in 0..10_000 {
            counts[r.weighted(&[3.0, 1.0])] += 1;
        }
        // ~75% no índice 0
        assert!(counts[0] > 7_000 && counts[0] < 8_500, "{counts:?}");
    }
}
