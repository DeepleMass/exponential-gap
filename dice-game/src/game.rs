//! Kern des Spiels: Spieler, Runden und die eigentliche Spielregel.
//!
//! Ablauf pro Runde (laut Aufgabenstellung):
//!   1. Alle N Spieler würfeln **reihum** (in Spielerreihenfolge).
//!   2. Die gewürfelte Augenzahl wird über [`crate::die::face_value`] auf
//!      einen Faktor in `[1, sqrt(2)]` abgebildet.
//!   3. Der Faktor wird mit dem aktuellen Spielstand des Spielers
//!      **multipliziert**.
//!
//! Startspielstand jedes Spielers ist `1.0`.

use crate::die::{self, FACES};
use crate::rng::SplitMix64;
use crate::stats::{self, Statistics};

/// Ein einzelner Spieler mit seinem aktuellen Spielstand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Player {
    /// Laufende Nummer des Spielers (0-basiert).
    pub index: usize,
    /// Spielstand in linearer Skala (Startwert 1.0, grows multiplikativ).
    ///
    /// Achtung: Nach sehr vielen Runden (ab ca. 4096) kann der wahre Wert
    /// den f64-Wertebereich sprengen; `score` läuft dann auf `inf`. Für
    /// Berechnungen und Anzeigen ist deshalb `log2_score` maßgeblich.
    pub score: f64,
    /// Spielstand im log2-Raum: `log2(score)`. Startwert 0.0.
    ///
    /// Multiplikation im linearen Raum == Addition im log2-Raum; dadurch
    /// bleibt der Wert auch nach Millionen von Runden endlich und genau.
    pub log2_score: f64,
}

impl Player {
    /// Neuer Spieler mit Anfangsspielstand 1.0 (log2: 0.0).
    fn new(index: usize) -> Self {
        Self {
            index,
            score: 1.0,
            log2_score: 0.0,
        }
    }

    /// Wendet einen Wurf auf den Spieler an: `score *= factor`.
    fn apply_roll(&mut self, face: u8) {
        let factor = die::face_value(face);
        // Kernregel des Spiels: Multiplikation mit dem gewürfelten Wert.
        self.score *= factor;
        // Äquivalenter, numerisch stabiler Schritt im log2-Raum.
        self.log2_score += factor.log2();
    }
}

/// Ein komplettes Spiel mit allen Spielern und der Rundenanzahl.
#[derive(Debug)]
pub struct Game {
    /// Alle Spieler in fester Reihenfolge.
    players: Vec<Player>,
    /// Bereits gespielte Runden.
    pub rounds_played: u64,
    /// Seed des Zufallsgenerators (für Reproduzierbarkeit und Persistenz).
    seed: u64,
    /// Zufallsgenerator; bewusst `SplitMix64` (deterministisch, ohne Crates).
    rng: SplitMix64,
}

impl Game {
    /// Neues Spiel: `num_players` Spieler starten beim Anfangsspielstand 1.0.
    ///
    /// # Panics
    /// Panikt, wenn `num_players` 0 ist.
    pub fn new(num_players: usize, seed: u64) -> Self {
        assert!(num_players > 0, "mindestens ein Spieler nötig");
        Self {
            players: (0..num_players).map(Player::new).collect(),
            rounds_played: 0,
            seed,
            rng: SplitMix64::new(seed),
        }
    }

    /// Rekonstruiert ein Spiel aus persistiertem Zustand (siehe [`crate::state`]).
    ///
    /// Der Zufallsgenerator wird aus Seed und Rundenanzahl neu abgeleitet,
    /// damit ein geladenes und weitergespieltes Spiel reproduzierbar bleibt.
    pub fn from_state(seed: u64, rounds_played: u64, log2_scores: Vec<f64>) -> Self {
        let players = log2_scores
            .into_iter()
            .enumerate()
            .map(|(i, l)| Player {
                index: i,
                score: 2f64.powf(l),
                log2_score: l,
            })
            .collect();
        let rng = SplitMix64::new(seed ^ rounds_played.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        Self {
            players,
            rounds_played,
            seed,
            rng,
        }
    }

    /// Anzahl der Spieler.
    pub fn num_players(&self) -> usize {
        self.players.len()
    }

    /// Verwendeter Seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Alle Spieler (in Spielreihenfolge).
    pub fn players(&self) -> &[Player] {
        &self.players
    }

    /// Eine Runde spielen: Jeder Spieler würfelt genau einmal, reihum.
    pub fn play_round(&mut self) {
        // Getrennte Feld-Borrowings erlauben gleichzeitigen Zugriff auf
        // RNG und Spielerliste.
        let rng = &mut self.rng;
        for player in &mut self.players {
            let face = rng.roll_die(FACES);
            player.apply_roll(face);
        }
        self.rounds_played += 1;
    }

    /// `rounds` Runden hintereinander spielen.
    pub fn play_rounds(&mut self, rounds: u64) {
        for _ in 0..rounds {
            self.play_round();
        }
    }

    /// Alle log2-Spielstände (in Spielerreihenfolge).
    pub fn log2_scores(&self) -> Vec<f64> {
        self.players.iter().map(|p| p.log2_score).collect()
    }

    /// Statistische Auswertung des aktuellen Stands.
    pub fn statistics(&self) -> Statistics {
        stats::summarize(&self.log2_scores(), self.rounds_played)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neues_spiel_startet_bei_eins() {
        let g = Game::new(100, 42);
        assert_eq!(g.num_players(), 100);
        assert_eq!(g.rounds_played, 0);
        for p in g.players() {
            assert_eq!(p.score, 1.0);
            assert_eq!(p.log2_score, 0.0);
        }
    }

    #[test]
    fn eine_runde_multipliziert_mit_faktor_aus_dem_intervall() {
        let mut g = Game::new(50, 7);
        g.play_round();
        assert_eq!(g.rounds_played, 1);
        for p in g.players() {
            // Faktor liegt in [1, sqrt(2)] => log2 in [0, 0.5].
            assert!(p.log2_score >= 0.0 && p.log2_score <= 0.5 + 1e-12);
            assert!(p.score >= 1.0);
            // score und log2_score müssen zusammenpassen.
            assert!((p.score - 2f64.powf(p.log2_score)).abs() < 1e-12);
        }
    }

    #[test]
    fn null_runden_ist_kein_schritt() {
        let mut g = Game::new(10, 1);
        g.play_rounds(0);
        assert_eq!(g.rounds_played, 0);
        assert!(g.players().iter().all(|p| p.score == 1.0));
    }

    #[test]
    fn spielstand_ist_produkt_der_gewuerfelten_faktoren() {
        // Gleichen Seed manuell "nachspielen", um die Regel zu verifizieren.
        let seed = 42;
        let rounds = 5;
        let mut g = Game::new(8, seed);
        g.play_rounds(rounds);

        // Würfelfolge: erst alle Spieler einer Runde (reihum), dann nächste Runde.
        let mut rng = SplitMix64::new(seed);
        let mut expected_scores = vec![1.0f64; g.num_players()];
        let mut expected_log2s = vec![0.0f64; g.num_players()];
        for _ in 0..rounds {
            for i in 0..g.num_players() {
                let face = rng.roll_die(FACES);
                let v = die::face_value(face);
                expected_scores[i] *= v;
                expected_log2s[i] += v.log2();
            }
        }
        for (i, p) in g.players().iter().enumerate() {
            let expected_score = expected_scores[i];
            let expected_log2 = expected_log2s[i];
            assert!(
                (p.score - expected_score).abs() < 1e-9,
                "score {} != erwartet {}",
                p.score,
                expected_score
            );
            assert!((p.log2_score - expected_log2).abs() < 1e-9);
        }
    }

    #[test]
    fn log2_bleibt_auch_nach_vielen_runden_endlich() {
        // 20_000 Runden: lineare Skala wäre längst bei inf, log2 nicht.
        let mut g = Game::new(20, 5);
        g.play_rounds(20_000);
        assert!(g.players().iter().all(|p| p.log2_score.is_finite()));
        // Maximal erreichbarer log2-Wert: 20_000 * log2(sqrt(2)) = 10_000.
        assert!(g.players().iter().all(|p| p.log2_score <= 10_000.0 + 1e-6));
    }

    #[test]
    #[should_panic(expected = "mindestens ein Spieler")]
    fn null_spieler_panikt() {
        let _ = Game::new(0, 1);
    }
}
