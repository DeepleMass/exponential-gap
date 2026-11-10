//! Persistenz des Spielzustands in einer einfachen Textdatei.
//!
//! Damit kann das GUI (oder die CLI) ein Spiel über mehrere Aufrufe hinweg
//! fortsetzen: `init` legt die Datei an, `play` liest, spielt und schreibt
//! zurück, `status` liest nur.
//!
//! Dateiformat (menschen- und maschinenlesbar):
//!
//! ```text
//! dice-game-state v1
//! seed=42
//! rounds=10
//! players=100
//! log2=0.0 0.1 0.2 ...
//! ```
//!
//! Gespeichert wird nur der log2-Spielstand (numerisch stabil); der lineare
//! Spielstand wird beim Laden als `2^log2` rekonstruiert.

use std::fs;
use std::io;
use std::path::Path;

use crate::game::Game;

/// Erste Zeile jeder Zustandsdatei (Versionskennung).
pub const STATE_MAGIC: &str = "dice-game-state v1";

/// Speichert den Zustand des Spiels in `path`.
pub fn save(game: &Game, path: impl AsRef<Path>) -> io::Result<()> {
    let mut text = String::new();
    text.push_str(STATE_MAGIC);
    text.push('\n');
    text.push_str(&format!("seed={}\n", game.seed()));
    text.push_str(&format!("rounds={}\n", game.rounds_played));
    text.push_str(&format!("players={}\n", game.num_players()));
    text.push_str("log2=");
    for (i, p) in game.players().iter().enumerate() {
        if i > 0 {
            text.push(' ');
        }
        // `{}` für f64 druckt die kürzeste Darstellung, die exakt zurückliefert.
        text.push_str(&format!("{}", p.log2_score));
    }
    text.push('\n');
    fs::write(path, text)
}

/// Lädt ein Spiel aus einer Zustandsdatei.
///
/// Liefert `io::Error` mit `ErrorKind::InvalidData`, wenn die Datei nicht
/// dem erwarteten Format entspricht.
pub fn load(path: impl AsRef<Path>) -> io::Result<Game> {
    let text = fs::read_to_string(path)?;
    let mut seed: Option<u64> = None;
    let mut rounds: Option<u64> = None;
    let mut log2: Option<Vec<f64>> = None;

    for (lineno, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if lineno == 0 {
            if line != STATE_MAGIC {
                return Err(invalid("erste Zeile muss 'dice-game-state v1' sein"));
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("seed=") {
            seed = rest
                .trim()
                .parse::<u64>()
                .map(Some)
                .map_err(|_| invalid("seed ist keine Zahl"))?;
        } else if let Some(rest) = line.strip_prefix("rounds=") {
            rounds = rest
                .trim()
                .parse::<u64>()
                .map(Some)
                .map_err(|_| invalid("rounds ist keine Zahl"))?;
        } else if let Some(rest) = line.strip_prefix("players=") {
            // redundant zu log2, wird nur geprüft
            let _: usize = rest
                .trim()
                .parse()
                .map_err(|_| invalid("players ist keine Zahl"))?;
        } else if let Some(rest) = line.strip_prefix("log2=") {
            let values: Result<Vec<f64>, _> = rest
                .split_whitespace()
                .map(|tok| tok.parse::<f64>().map_err(|_| invalid("log2-Wert ist keine Zahl")))
                .collect();
            log2 = Some(values?);
        }
    }

    let seed = seed.ok_or_else(|| invalid("seed= fehlt"))?;
    let rounds = rounds.ok_or_else(|| invalid("rounds= fehlt"))?;
    let log2 = log2.ok_or_else(|| invalid("log2= fehlt"))?;
    if log2.is_empty() {
        return Err(invalid("log2= enthält keine Werte"));
    }
    Ok(Game::from_state(seed, rounds, log2))
}

/// Hilfsfunktion für Formatfehler.
fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    /// Eindeutiger Pfad im Temp-Verzeichnis für jeden Test.
    fn temp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let unique = format!(
            "dice-game-state-test-{}-{}-{}.txt",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        p.push(unique);
        p
    }

    #[test]
    fn save_load_roundtrip() {
        let path = temp_path("roundtrip");
        let mut g = Game::new(25, 42);
        g.play_rounds(13);
        save(&g, &path).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.seed(), 42);
        assert_eq!(loaded.rounds_played, 13);
        assert_eq!(loaded.num_players(), 25);
        for (a, b) in g.players().iter().zip(loaded.players().iter()) {
            assert_eq!(a.log2_score, b.log2_score);
        }
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn geladenes_spiel_spielt_weiter_und_bleibt_reproduzierbar() {
        let path = temp_path("continue");
        let mut g = Game::new(10, 7);
        g.play_rounds(3);
        save(&g, &path).unwrap();

        // Zweimal unabhängig laden und je 2 Runden spielen => gleiche Ergebnisse.
        let mut a = load(&path).unwrap();
        let mut b = load(&path).unwrap();
        a.play_rounds(2);
        b.play_rounds(2);
        assert_eq!(a.log2_scores(), b.log2_scores());
        assert_eq!(a.rounds_played, 5);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn kaputte_datei_wird_abgelehnt() {
        let path = temp_path("broken");
        fs::write(&path, "irgendwas\nseed=x\n").unwrap();
        let err = load(&path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn fehlende_datei_gibt_io_fehler() {
        let path = temp_path("missing");
        let err = load(&path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }
}
