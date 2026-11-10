//! Abbildung der Würfel-Augen auf die Spielfaktoren.
//!
//! Regeln laut Aufgabenstellung:
//!   * Der Würfel hat 6 Seiten (Augenzahlen 1 bis 6).
//!   * Die Augenzahlen werden auf **exponentiell steigende** Werte im
//!     Intervall `[1, sqrt(2)]` abgebildet.
//!
//! Umsetzung: geometrische Folge (konstantes Verhältnis zwischen
//! benachbarten Faktoren). Für Augenzahl `a` gilt:
//!
//! ```text
//! factor(a) = (sqrt(2)) ^ ((a - 1) / 5)
//! ```
//!
//! Damit ist factor(1) = 1 und factor(6) = sqrt(2); dieZwischenwerte
//! wachsen mit konstantem Faktor `sqrt(2)^(1/5)` – also exponentiell.

use std::f64::consts::SQRT_2;

/// Anzahl der Würfelseiten.
pub const FACES: u8 = 6;
/// Kleinste Augenzahl.
pub const MIN_FACE: u8 = 1;
/// Größte Augenzahl.
pub const MAX_FACE: u8 = FACES;
/// Untere Intervallgrenze der Abbildung.
pub const LOWER_BOUND: f64 = 1.0;
/// Obere Intervallgrenze der Abbildung: Quadratwurzel aus 2.
pub const UPPER_BOUND: f64 = SQRT_2;

/// Bildet eine Augenzahl (`MIN_FACE..=MAX_FACE`) auf ihren Spielfaktor ab.
///
/// Ergebnis liegt immer in `[1, sqrt(2)]`.
///
/// # Panics
/// Panikt, wenn `face` nicht in `1..=6` liegt.
pub fn face_value(face: u8) -> f64 {
    assert!(
        (MIN_FACE..=MAX_FACE).contains(&face),
        "Augenzahl {face} liegt nicht im gültigen Bereich {MIN_FACE}..={MAX_FACE}"
    );
    // Exponent läuft linear von 0 (Augenzahl 1) bis 1 (Augenzahl 6).
    let exponent = (face - MIN_FACE) as f64 / (MAX_FACE - MIN_FACE) as f64;
    UPPER_BOUND.powf(exponent)
}

/// Logarithmus zur Basis 2 des Spielfaktors einer Augenzahl.
///
/// Praktisch, weil das Spiel multiplikativ ist: Eine Multiplikation des
/// Spielstands entspricht einer Addition im log2-Raum.
pub fn face_log2(face: u8) -> f64 {
    face_value(face).log2()
}

/// Theoretischer Erwartungswert von `log2(factor)` über einen Wurf.
///
/// Wegen `log2(sqrt(2)) = 0.5` und mittlerer Augenzahl 3.5 ergibt sich
/// `(3.5 - 1) / 5 * 0.5 = 0.25`: Pro Runde wächst der typische Spielstand
/// im Mittel um den Faktor `2^0.25`.
pub fn expected_log2_per_roll() -> f64 {
    (MIN_FACE..=MAX_FACE)
        .map(face_log2)
        .sum::<f64>()
        / FACES as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grenzwerte_der_abbildung() {
        // Untere Grenze: Augenzahl 1 => Faktor exakt 1.
        assert_eq!(face_value(1), 1.0);
        // Obere Grenze: Augenzahl 6 => Faktor sqrt(2).
        assert!((face_value(6) - SQRT_2).abs() < 1e-15);
    }

    #[test]
    fn abbildung_ist_streng_steigend() {
        for face in MIN_FACE..MAX_FACE {
            assert!(
                face_value(face + 1) > face_value(face),
                "Faktor von Auge {} ist nicht größer als der von Auge {}",
                face + 1,
                face
            );
        }
    }

    #[test]
    fn abbildung_ist_geometrisch_konstantes_verhältnis() {
        // „Exponentiell steigend“ => konstantes Verhältnis benachbarter Werte.
        let ratio = face_value(2) / face_value(1);
        for face in 2..MAX_FACE {
            let r = face_value(face + 1) / face_value(face);
            assert!(
                (r - ratio).abs() < 1e-12,
                "Verhältnis bei Auge {face} weicht ab: {r} vs. {ratio}"
            );
        }
        // Das konstante Verhältnis ist sqrt(2)^(1/5).
        assert!((ratio - SQRT_2.powf(1.0 / 5.0)).abs() < 1e-12);
    }

    #[test]
    fn alle_werte_liegen_im_intervall() {
        for face in MIN_FACE..=MAX_FACE {
            let v = face_value(face);
            assert!(v >= LOWER_BOUND && v <= UPPER_BOUND + 1e-15);
        }
    }

    #[test]
    fn log2_ist_konsistent_zu_value() {
        for face in MIN_FACE..=MAX_FACE {
            let v = face_value(face);
            assert!((face_log2(face) - v.log2()).abs() < 1e-15);
            // log2(sqrt(2)) = 0.5 => log2-Faktor skaliert linear: (a-1)/10.
            assert!((face_log2(face) - (face - 1) as f64 / 10.0).abs() < 1e-12);
        }
    }

    #[test]
    fn erwartungswert_log2_ist_ein_viertel() {
        assert!((expected_log2_per_roll() - 0.25).abs() < 1e-12);
    }

    #[test]
    #[should_panic(expected = "gültigen Bereich")]
    fn augenzahl_null_panikt() {
        let _ = face_value(0);
    }

    #[test]
    #[should_panic(expected = "gültigen Bereich")]
    fn augenzahl_sieben_panikt() {
        let _ = face_value(7);
    }
}
