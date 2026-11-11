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
//!
//! Zusätzlich wird eine **Historie** der Rundenstände geführt (für das
//! Liniendiagramm der GUI): bis Runde [`HISTORY_FULL_ROUNDS`] wird jede Runde
//! aufgenommen, danach mit wachsender Schrittweite [`history_stride`]
//! (bis 999 jede Runde, dann alle 10, 100, 1000, ... Runden – etwa 900
//! Schnappschüsse pro Dekade). So bleibt auch nach 1M Runden der Speicher
//! begrenzt und der „Ausläufer“ (die letzten Runden) hat genug Stützpunkte.
//! Der Endpunkt einer gespielten Sequenz wird immer mitgespeichert.

use crate::die::{self, FACES};
use crate::rng::SplitMix64;
use crate::stats::{self, Statistics};

/// Bis zu dieser Rundenzahl wird **jede** Runde in die Historie aufgenommen.
pub const HISTORY_FULL_ROUNDS: u64 = 100;

/// Schrittweite, in der eine Runde in die Historie aufgenommen wird.
///
/// Bis Runde 100 jede Runde (Stride 1); danach pro Dekade eine Zehnerpotenz:
/// 101..=999 → 1, 1000..=9999 → 10, 10_000..=99_999 → 100, usw.
pub fn history_stride(round: u64) -> u64 {
    if round <= HISTORY_FULL_ROUNDS {
        return 1;
    }
    // floor(log10(round)) über die Ziffernanzahl bestimmen.
    let mut digits = 0u32;
    let mut v = round;
    while v > 0 {
        v /= 10;
        digits += 1;
    }
    // floor(log10(r)) = digits - 1; Stride = 10^(floor(log10(r)) - 2), mind. 1.
    let exp = (digits as i64 - 3).max(0) as u32;
    10u64.pow(exp)
}

/// Ein gespeicherter Zwischenstand: Rundennummer + log2-Spielstände aller Spieler.
#[derive(Debug, Clone, PartialEq)]
pub struct RoundSnapshot {
    /// Rundennummer (1-basiert).
    pub round: u64,
    /// log2-Spielstand jedes Spielers in dieser Runde (in Spielerreihenfolge).
    pub log2_scores: Vec<f64>,
}

/// Ein einzelner Spieler mit seinem aktuellen Spielstand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Player {
    /// Laufende Nummer des Spielers (0-basiert).
    pub index: usize,
    /// Spielstand in linearer Skala (Startwert 1.0, wächst multiplikativ).
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

/// Ein komplettes Spiel mit allen Spielern, der Rundenanzahl und der Historie.
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
    /// Historie der Rundenstände (siehe Modul-Doku).
    history: Vec<RoundSnapshot>,
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
            history: Vec::new(),
        }
    }

    /// Rekonstruiert ein Spiel aus persistiertem Zustand (siehe [`crate::state`]).
    ///
    /// Der Zufallsgenerator wird aus Seed und Rundenanzahl neu abgeleitet,
    /// damit ein geladenes und weitergespieltes Spiel reproduzierbar bleibt.
    pub fn from_state(seed: u64, rounds_played: u64, log2_scores: Vec<f64>, history: Vec<RoundSnapshot>) -> Self {
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
            history,
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

    /// Gespeicherte Historie (aufsteigend nach Rundennummer).
    pub fn history(&self) -> &[RoundSnapshot] {
        &self.history
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
        self.record_history();
    }

    /// `rounds` Runden hintereinander spielen.
    ///
    /// Der Endpunkt der Sequenz wird immer als Schnappschuss mitgespeichert,
    /// damit das Diagramm den aktuellen Stand als letzten Punkt enthält.
    pub fn play_rounds(&mut self, rounds: u64) {
        for _ in 0..rounds {
            self.play_round();
        }
        if self.rounds_played > 0
            && self.history.last().map(|s| s.round) != Some(self.rounds_played)
        {
            let scores = self.log2_scores();
            self.history.push(RoundSnapshot {
                round: self.rounds_played,
                log2_scores: scores,
            });
        }
    }

    /// Speichert einen Schnappschuss, falls diese Runde laut [`history_stride`] dran ist.
    fn record_history(&mut self) {
        let r = self.rounds_played;
        if r % history_stride(r) == 0 {
            let scores = self.log2_scores();
            self.history.push(RoundSnapshot {
                round: r,
                log2_scores: scores,
            });
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
    fn stride_wächst_pro_dekade() {
        assert_eq!(history_stride(1), 1);
        assert_eq!(history_stride(100), 1);
        assert_eq!(history_stride(101), 1);
        assert_eq!(history_stride(999), 1);
        assert_eq!(history_stride(1000), 10);
        assert_eq!(history_stride(5000), 10);
        assert_eq!(history_stride(10_000), 100);
        assert_eq!(history_stride(123_456), 1_000);
        assert_eq!(history_stride(1_000_000), 10_000);
    }

    #[test]
    fn neues_spiel_startet_bei_eins() {
        let g = Game::new(100, 42);
        assert_eq!(g.num_players(), 100);
        assert_eq!(g.rounds_played, 0);
        assert!(g.history().is_empty());
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
        assert!(g.history().is_empty());
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
    fn history_erfasst_jede_der_ersten_100_runden() {
        let mut g = Game::new(5, 1);
        g.play_rounds(10);
        let rounds: Vec<u64> = g.history().iter().map(|s| s.round).collect();
        assert_eq!(rounds, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        // Jeder Schnappschuss enthält alle Spieler.
        assert!(g.history().iter().all(|s| s.log2_scores.len() == 5));
    }

    #[test]
    fn history_erfasst_bis_999_jede_runde_dann_stride() {
        let mut g = Game::new(5, 1);
        g.play_rounds(1050);
        let rounds: Vec<u64> = g.history().iter().map(|s| s.round).collect();
        // 1..=999 lückenlos, dann Stride 10 (1000, 1010, ...), Endpunkt 1050.
        assert_eq!(rounds.len(), 999 + 6); // 1..=999 + 1000,1010,1020,1030,1040,1050
        assert_eq!(rounds[998], 999);
        assert_eq!(rounds[999], 1000);
        assert!(!rounds.contains(&1005));
        assert_eq!(*rounds.last().unwrap(), 1050);
    }

    #[test]
    fn history_bleibt_bei_vielen_runden_begrenzt() {
        let mut g = Game::new(50, 3);
        g.play_rounds(200_000);
        let h = g.history();
        // ca. 100 + 899 + 900 + 900 + 101 ≈ 2900 Schnappschüsse.
        assert!(h.len() <= 3200, "History zu groß: {}", h.len());
        assert_eq!(h.first().unwrap().round, 1);
        assert_eq!(h.last().unwrap().round, 200_000);
        assert!(h.iter().any(|s| s.round == 150_000)); // 150_000 % 1000 == 0
        // aufsteigend sortiert
        assert!(h.windows(2).all(|w| w[0].round < w[1].round));
    }

    #[test]
    fn from_state_nimmt_history_mit() {
        let mut g = Game::new(5, 9);
        g.play_rounds(250);
        let h = g.history().to_vec();
        let mut g2 = Game::from_state(9, 250, g.log2_scores(), h.clone());
        assert_eq!(g2.history(), h.as_slice());
        // Weiterspielen zeichnet weiter (bis 999 jede Runde).
        g2.play_rounds(50); // bis Runde 300
        assert!(g2.history().iter().any(|s| s.round == 300));
    }

    #[test]
    #[should_panic(expected = "mindestens ein Spieler")]
    fn null_spieler_panikt() {
        let _ = Game::new(0, 1);
    }
}
