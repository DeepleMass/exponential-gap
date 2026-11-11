//! Integrationstests für das CLI-Binary.
//!
//! `env!("CARGO_BIN_EXE_dice-game")` liefert den Pfad zum frisch gebauten
//! Binary – Cargo stellt diese Variable für Integrationstests bereit.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Pfad zum kompilierten Binary.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_dice-game")
}

/// Eindeutige temporäre Zustandsdatei für einen Test.
fn temp_state(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "dice-game-cli-test-{}-{}-{}.state",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    p
}

/// Führt das Binary aus und erwartet Erfolg.
fn run_ok(args: &[&str]) -> String {
    let out = Command::new(bin()).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "Befehl {args:?} schlug fehl: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// Zählt die `player,...`-Zeilen in der maschinenlesbaren Ausgabe.
fn count_player_lines(stdout: &str) -> usize {
    stdout.lines().filter(|l| l.starts_with("player,")).count()
}

/// Zählt die `hist,...`-Zeilen (History-Schnappschüsse) in der Ausgabe.
fn count_hist_lines(stdout: &str) -> usize {
    stdout.lines().filter(|l| l.starts_with("hist,")).count()
}

#[test]
fn init_play_status_ablauf() {
    let state = temp_state("flow");
    let s = state.to_str().unwrap();

    // init legt 50 Spieler an.
    let out = run_ok(&["init", "--players", "50", "--seed", "7", "--state", s]);
    assert!(out.contains("meta,rounds,0"));
    assert!(out.contains("meta,players,50"));
    assert_eq!(count_player_lines(&out), 50);

    // 10 Runden spielen => rounds=10, weiterhin 50 Spieler,
    // Historie enthält jede der 10 Runden (hist-Zeilen 1..=10).
    let out = run_ok(&["play", "--rounds", "10", "--state", s]);
    assert!(out.contains("meta,rounds,10"));
    assert_eq!(count_player_lines(&out), 50);
    assert_eq!(count_hist_lines(&out), 10);
    assert!(out.contains("hist,1,"));
    assert!(out.contains("hist,10,"));

    // weitere 5 Runden => rounds=15, Historie wächst auf 15 Einträge.
    let out = run_ok(&["play", "--rounds", "5", "--state", s]);
    assert!(out.contains("meta,rounds,15"));
    assert_eq!(count_hist_lines(&out), 15);

    // status ohne play ändert nichts.
    let out = run_ok(&["status", "--state", s]);
    assert!(out.contains("meta,rounds,15"));

    let _ = fs::remove_file(&state);
}

#[test]
fn gleiches_seed_gleiches_ergebnis() {
    let a = run_ok(&["demo", "--players", "20", "--rounds", "25", "--seed", "99"]);
    let b = run_ok(&["demo", "--players", "20", "--rounds", "25", "--seed", "99"]);
    assert_eq!(a, b, "gleicher Seed muss identische Ausgabe liefern");

    let c = run_ok(&["demo", "--players", "20", "--rounds", "25", "--seed", "100"]);
    assert_ne!(a, c, "anderer Seed soll andere Ergebnisse liefern");
}

#[test]
fn play_ohne_runden_standard_eins() {
    let state = temp_state("default-rounds");
    let s = state.to_str().unwrap();
    run_ok(&["init", "--players", "5", "--state", s]);
    let out = run_ok(&["play", "--state", s]); // kein --rounds
    assert!(out.contains("meta,rounds,1"));
    assert_eq!(count_hist_lines(&out), 1); // Runde 1 ist History-Runde
    let _ = fs::remove_file(&state);
}

#[test]
fn history_bleibt_ueber_viele_runden_in_der_ausgabe_begrenzt() {
    let state = temp_state("hist-cap");
    let s = state.to_str().unwrap();
    run_ok(&["init", "--players", "20", "--seed", "5", "--state", s]);
    let out = run_ok(&["play", "--rounds", "5000", "--state", s]);
    assert!(out.contains("meta,rounds,5000"));
    // 100 volle Runden + Marker 200..3200 (5 Stück) + Endpunkt 5000 = 106.
    let n = count_hist_lines(&out);
    assert!(n <= 120, "zu viele hist-Zeilen: {n}");
    assert!(out.contains("hist,5000,")); // Endpunkt enthalten
    let _ = fs::remove_file(&state);
}

#[test]
fn unbekanntes_kommando_schlaegt_fehl() {
    let out = Command::new(bin()).arg("gibts-nicht").output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unbekanntes Kommando"));
}

#[test]
fn play_ohne_datei_schlaegt_fehl() {
    let state = temp_state("missing");
    let s = state.to_str().unwrap().to_string();
    let out = Command::new(bin())
        .args(["play", "--rounds", "1", "--state", &s])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("nicht laden"));
}

#[test]
fn hilfe_und_kein_kommando() {
    let out = run_ok(&["help"]);
    assert!(out.contains("Aufruf:"));
    let out = Command::new(bin()).output().unwrap();
    assert!(!out.status.success()); // fehlendes Kommando => Fehler
}
