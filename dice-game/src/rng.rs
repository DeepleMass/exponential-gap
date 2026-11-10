//! Deterministischer Pseudo-Zufallszahlengenerator (SplitMix64).
//!
//! Bewusst ohne externe Abhängigkeit (`rand` etc.): SplitMix64 ist klein,
//! schnell, portabel und für Simulationen dieser Art völlig ausreichend.
//! Gleicher Seed => gleiche Zahlenfolge (wichtig für reproduzierbare Tests
//! und für das GUI, das Spiele über mehrere Aufrufe hinweg fortsetzen will).

/// Pseudo-Zufallszahlengenerator auf Basis des SplitMix64-Algorithmus.
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    /// Interner Zustand; wird bei jedem Schritt weitergeschaltet.
    state: u64,
}

impl SplitMix64 {
    /// Erzeugt einen Generator mit gegebenem Seed.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Nächste pseudo-zufällige `u64`-Zahl (SplitMix64-Kernschritt).
    pub fn next_u64(&mut self) -> u64 {
        // Golden-Gamma-Konstante weiterschalten ...
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        // ... und zweimal mixen (Feistel-artige Shifts + multiplikative Mischung).
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Nächste pseudo-zufällige Fließkommazahl, gleichverteilt in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        // Obere 53 Bit als Mantisse verwenden => exakt gleichverteilt in [0, 1).
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// „Würfelt“ eine Augenzahl in `1..=faces`.
    ///
    /// Verwendet Rejection Sampling, damit `v % faces` keine messbare
    /// Verzerrung zugunsten kleiner Augenzahlen erzeugt.
    pub fn roll_die(&mut self, faces: u8) -> u8 {
        assert!(faces > 0, "ein Würfel braucht mindestens eine Seite");
        let faces = faces as u64;
        // Obere Randzone abschneiden, in der die Modulo-Abbildung ungleichmäßig wäre.
        let zone = u64::MAX - (u64::MAX % faces);
        loop {
            let v = self.next_u64();
            if v < zone {
                return (v % faces) as u8 + 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gleicher_seed_gleiche_folge() {
        let mut a = SplitMix64::new(42);
        let mut b = SplitMix64::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn verschiedene_seeds_verschiedene_folgen() {
        let mut a = SplitMix64::new(1);
        let mut b = SplitMix64::new(2);
        let seq_a: Vec<u64> = (0..10).map(|_| a.next_u64()).collect();
        let seq_b: Vec<u64> = (0..10).map(|_| b.next_u64()).collect();
        assert_ne!(seq_a, seq_b);
    }

    #[test]
    fn wuerfel_liegt_immer_im_bereich() {
        let mut rng = SplitMix64::new(7);
        for _ in 0..10_000 {
            let face = rng.roll_die(6);
            assert!((1..=6).contains(&face));
        }
    }

    #[test]
    fn alle_augen_kommen_vor_und_ungefähr_gleich_häufig() {
        let mut rng = SplitMix64::new(123);
        let n = 60_000;
        let mut counts = [0usize; 7]; // Index 0 bleibt 0
        for _ in 0..n {
            counts[rng.roll_die(6) as usize] += 1;
        }
        let expected = n as f64 / 6.0;
        for face in 1..=6 {
            let c = counts[face] as f64;
            // Grobe Schranke (ca. +/-10 %) – bei festem Seed deterministisch.
            assert!(
                (c - expected).abs() < expected * 0.10,
                "Augenzahl {face} kam {c}-mal (erwartet ca. {expected})"
            );
        }
    }

    #[test]
    fn next_f64_in_einhalb_offenem_intervall() {
        let mut rng = SplitMix64::new(99);
        for _ in 0..10_000 {
            let x = rng.next_f64();
            assert!((0.0..1.0).contains(&x));
        }
    }
}
