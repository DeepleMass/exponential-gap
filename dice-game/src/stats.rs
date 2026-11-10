//! Statistische Auswertung der Spielstände.
//!
//! Alle Kennzahlen werden auf den **log2-Werten** der Spielstände berechnet.
//! Das ist bei einem multiplikativen Spiel der natürliche Raum: Der Mittelwert
//! der log2-Werte ist der Logarithmus des geometrischen Mittels der Spielstände,
//! und die Verteilung ist auch bei sehr vielen Runden noch beherrschbar
//! (keine Überläufe, symmetrischere Verteilung).

/// Zusammenfassende Statistik über die log2-Spielstände aller Spieler.
#[derive(Debug, Clone, PartialEq)]
pub struct Statistics {
    /// Anzahl der Spieler.
    pub count: usize,
    /// Bereits gespielte Runden.
    pub rounds: u64,
    /// Kleinster log2-Spielstand.
    pub min_log2: f64,
    /// Größter log2-Spielstand.
    pub max_log2: f64,
    /// Arithmetisches Mittel der log2-Spielstände.
    pub mean_log2: f64,
    /// Median der log2-Spielstände.
    pub median_log2: f64,
    /// Standardabweichung (Grundgesamtheit) der log2-Spielstände.
    pub std_dev_log2: f64,
}

impl Statistics {
    /// Kleinster Spielstand (lineare Skala; kann bei sehr vielen Runden `inf` sein).
    pub fn min_score(&self) -> f64 {
        2f64.powf(self.min_log2)
    }
    /// Größter Spielstand (lineare Skala; kann `inf` sein).
    pub fn max_score(&self) -> f64 {
        2f64.powf(self.max_log2)
    }
    /// Geometrisches Mittel der Spielstände = `2^mean_log2`.
    pub fn geometric_mean_score(&self) -> f64 {
        2f64.powf(self.mean_log2)
    }
    /// Median der Spielstände.
    pub fn median_score(&self) -> f64 {
        2f64.powf(self.median_log2)
    }
}

/// Berechnet die Statistik aus den log2-Spielständen.
///
/// # Panics
/// Panikt, wenn `log2_scores` leer ist.
pub fn summarize(log2_scores: &[f64], rounds: u64) -> Statistics {
    assert!(!log2_scores.is_empty(), "mindestens ein Spieler nötig");
    let n = log2_scores.len();
    let sum: f64 = log2_scores.iter().sum();
    let mean = sum / n as f64;

    let mut sorted = log2_scores.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    };

    let variance: f64 = log2_scores.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n as f64;

    Statistics {
        count: n,
        rounds,
        min_log2: sorted[0],
        max_log2: sorted[n - 1],
        mean_log2: mean,
        median_log2: median,
        std_dev_log2: variance.sqrt(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kennzahlen_bekannter_werte() {
        // log2-Werte 0, 1, 2, 3, 4
        let s = summarize(&[0.0, 1.0, 2.0, 3.0, 4.0], 10);
        assert_eq!(s.count, 5);
        assert_eq!(s.rounds, 10);
        assert_eq!(s.min_log2, 0.0);
        assert_eq!(s.max_log2, 4.0);
        assert_eq!(s.mean_log2, 2.0);
        assert_eq!(s.median_log2, 2.0);
        // Varianz = (4+1+0+1+4)/5 = 2
        assert!((s.std_dev_log2 - 2f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn median_bei_gerader_anzahl_ist_mittelwert() {
        let s = summarize(&[1.0, 2.0, 3.0, 10.0], 0);
        assert_eq!(s.median_log2, 2.5);
    }

    #[test]
    fn score_konvertiert_zurueck() {
        let s = summarize(&[0.0, 1.0, 2.0], 0);
        assert_eq!(s.min_score(), 1.0);
        assert_eq!(s.max_score(), 4.0);
        assert!((s.geometric_mean_score() - 2f64.powf(1.0)).abs() < 1e-12);
        assert_eq!(s.median_score(), 2.0);
    }

    #[test]
    fn ein_wert_funktioniert() {
        let s = summarize(&[3.5], 7);
        assert_eq!(s.count, 1);
        assert_eq!(s.mean_log2, 3.5);
        assert_eq!(s.median_log2, 3.5);
        assert_eq!(s.std_dev_log2, 0.0);
    }

    #[test]
    #[should_panic(expected = "mindestens ein Spieler")]
    fn leere_eingabe_panikt() {
        let _ = summarize(&[], 0);
    }
}
