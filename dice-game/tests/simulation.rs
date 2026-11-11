//! Integrationstests auf Simulationsebene (Bibliothek, nicht CLI).
//!
//! Prüfen die Spielregeln und statistische Eigenschaften des Experiments.

use dice_game::{die, Game, SplitMix64, FACES};

#[test]
fn spielregeln_gelten_ueber_alle_runden() {
    let mut game = Game::new(30, 2024);
    for round in 1..=20u64 {
        game.play_round();
        assert_eq!(game.rounds_played, round);
        for p in game.players() {
            // Faktor >= 1 => Spielstand nie unter 1.0.
            assert!(p.score >= 1.0);
            assert!(p.log2_score >= 0.0);
            // Nach einer Runde: log2 <= log2(sqrt(2)) = 0.5.
            assert!(p.log2_score <= round as f64 * 0.5 + 1e-9);
            // score und log2_score konsistent.
            assert!((p.score - 2f64.powf(p.log2_score)).abs() <= 1e-9 * p.score.max(1.0));
        }
    }
}

#[test]
fn rundenprodukt_stimmt_mit_manuellem_rng() {
    let seed = 555;
    let rounds = 7;
    let mut game = Game::new(12, seed);
    game.play_rounds(rounds);

    // Dieselbe Würfelfolge unabhängig nachspielen:
    // erst würfeln alle Spieler einer Runde reihum, dann die nächste Runde.
    let mut rng = SplitMix64::new(seed);
    let mut expected = vec![1.0f64; game.num_players()];
    for _ in 0..rounds {
        for e in expected.iter_mut() {
            *e *= die::face_value(rng.roll_die(FACES));
        }
    }
    for (p, exp) in game.players().iter().zip(expected.iter()) {
        assert!(
            (p.score - exp).abs() < 1e-9 * exp,
            "Spieler {}: {} vs. erwartet {}",
            p.index,
            p.score,
            exp
        );
    }
}

#[test]
fn statistisches_gesetz_nach_vielen_runden() {
    // 2000 Spieler, 50 Runden: Mittel der log2-Werte muss nahe am
    // theoretischen Erwartungswert rounds * E[log2 factor] = 50 * 0.25 liegen.
    let mut game = Game::new(2000, 7);
    game.play_rounds(50);
    let s = game.statistics();
    let expected = 50.0 * die::expected_log2_per_roll();
    assert!(
        (s.mean_log2 - expected).abs() < 0.5,
        "mean_log2 {} weicht zu stark von {} ab",
        s.mean_log2,
        expected
    );
    // Median der log-normalen Verteilung liegt ebenfalls beim Erwartungswert.
    assert!((s.median_log2 - expected).abs() < 0.5);
    // Grenzen: 0 <= log2 <= 50 * 0.5.
    assert!(s.min_log2 >= 0.0);
    assert!(s.max_log2 <= 25.0 + 1e-9);
}

#[test]
fn exponential_gap_wird_sichtbar() {
    // Kern des Experiments: Nach genug Runden klafft eine exponentielle
    // Lücke zwischen bestem und schlechtestem Spieler – gemessen im
    // log2-Raum wächst die Spannweite linear mit den Runden.
    let mut game = Game::new(500, 11);
    game.play_rounds(200);
    let s = game.statistics();
    let gap = s.max_log2 - s.min_log2;
    // Theoretisch: Spannweite der Augensummen skaliert mit sqrt(rounds);
    // hier nur die Plausibilität prüfen: Lücke deutlich größer als 0 und
    // kleiner als die theoretische Maximallipse (200 * 0.5 = 100).
    assert!(gap > 1.0, "zu kleine Lücke nach 200 Runden: {gap}");
    assert!(gap <= 100.0);
    // Standardabweichung wächst mit sqrt(Runden): ca. 0.17 * sqrt(200) ≈ 2.4.
    assert!(s.std_dev_log2 > 1.0 && s.std_dev_log2 < 6.0);
}

#[test]
fn history_ist_vollstaendig_und_gekappt() {
    // Bis Runde 100 jede Runde, dann geometrische Marker + Endpunkt.
    let mut game = Game::new(30, 17);
    game.play_rounds(5000);
    let h = game.history();
    let rounds: Vec<u64> = h.iter().map(|s| s.round).collect();
    assert_eq!(&rounds[..10], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    assert_eq!(rounds[100], 200); // erste Marker nach den vollen Runden
    assert!(!rounds.contains(&150));
    assert_eq!(*rounds.last().unwrap(), 5000); // Endpunkt
    assert!(h.len() <= 120);
    // Jeder Schnappschuss hat alle Spieler und aufsteigende Runden.
    assert!(h.iter().all(|s| s.log2_scores.len() == 30));
    assert!(h.windows(2).all(|w| w[0].round < w[1].round));
}

#[test]
fn sehr_viele_runden_bleiben_im_log2_raum_stabil() {
    // 1M Runden wie im GUI-Dropdown: 50 Spieler x 200_000 Runden = 1e7 Würfe.
    let mut game = Game::new(50, 3);
    game.play_rounds(200_000);
    let s = game.statistics();
    assert!(s.min_log2.is_finite() && s.max_log2.is_finite());
    // Erwartungswert: 200_000 * 0.25 = 50_000.
    let expected = 200_000.0 * die::expected_log2_per_roll();
    assert!(
        (s.mean_log2 - expected).abs() < expected * 0.01,
        "mean_log2 {} vs. erwartet ca. {}",
        s.mean_log2,
        expected
    );
}
