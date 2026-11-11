//! Kommandozeilen-Frontend für das Würfel-Experiment `dice-game`.
//!
//! Das Binary wird sowohl direkt benutzbar als auch vom Python-GUI als
//! "Motor" gesteuert (über Zustandsdatei + maschinenlesbare Ausgabe).
//!
//! Aufruf: siehe `USAGE` bzw. `dice-game help`.

use std::env;
use std::process::ExitCode;

use dice_game::state;
use dice_game::Game;

/// Hilfetext / Gebrauchsanweisung.
const USAGE: &str = "\
dice-game – Experiment zur exponentiellen Entwicklung

Aufruf:
  dice-game demo   [--players N] [--rounds R] [--seed S]
  dice-game init   --state DATEI [--players N] [--seed S]
  dice-game play   --state DATEI [--rounds R]
  dice-game status --state DATEI
  dice-game help

Kommandos:
  demo     Einmaliges Spiel ohne Zustandsdatei; menschenlesbare Ausgabe.
           Standardwerte: --players 100 --rounds 1000 --seed 42
  init     Neues Spiel in DATEI anlegen (Anfangsspielstand 1.0).
  play     Runden aus dem gespeicherten Spiel weiterspielen (Standard: 1)
           und Zustand zurückschreiben. Maschinenlesbare Ausgabe.
  status   Aktuellen Stand aus DATEI lesen. Maschinenlesbare Ausgabe.
  help     Dieser Text.

Maschinenlesbares Ausgabeformat (play/status/init):
  meta,rounds,<u64>
  meta,players,<usize>
  meta,seed,<u64>
  player,<index>,<log2_score>,<score>
  hist,<round>,<log2_0>,<log2_1>,...   (ein Schnappschuss je History-Runde)
";

/// Geparste Kommandozeilen-Optionen.
struct Options {
    players: usize,
    rounds: u64,
    seed: u64,
    state: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            players: 100,
            rounds: 1,
            seed: 42,
            state: None,
        }
    }
}

/// Parst `--schlüssel wert`-Paare. Unbekannte Schlüssel => Fehler.
fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut opts = Options::default();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        // Wert folgt als eigenes Argument.
        let value = args
            .get(i + 1)
            .ok_or_else(|| format!("fehlender Wert für '{key}'"))?;
        match key {
            "--players" => {
                opts.players = value
                    .parse()
                    .map_err(|_| format!("--players: '{value}' ist keine Zahl"))?
            }
            "--rounds" => {
                opts.rounds = value
                    .parse()
                    .map_err(|_| format!("--rounds: '{value}' ist keine Zahl"))?
            }
            "--seed" => {
                opts.seed = value
                    .parse()
                    .map_err(|_| format!("--seed: '{value}' ist keine Zahl"))?
            }
            "--state" => opts.state = Some(value.clone()),
            other => return Err(format!("unbekannte Option '{other}'")),
        }
        i += 2;
    }
    Ok(opts)
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("Fehler: {msg}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args
        .first()
        .ok_or_else(|| "fehlendes Kommando".to_string())?
        .as_str();

    match command {
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
        }
        "demo" => {
            let mut opts = parse_options(&args[1..])?;
            opts.rounds = if args[1..].iter().any(|a| a == "--rounds") {
                opts.rounds
            } else {
                1000 // Demo-Standard: 1000 Runden
            };
            let mut game = Game::new(opts.players, opts.seed);
            game.play_rounds(opts.rounds);
            print_human(&game);
        }
        "init" => {
            let opts = parse_options(&args[1..])?;
            let state_path = opts.state.ok_or("init braucht --state DATEI")?;
            let game = Game::new(opts.players, opts.seed);
            state::save(&game, &state_path).map_err(|e| format!("konnte {state_path} nicht schreiben: {e}"))?;
            print_machine(&game);
        }
        "play" => {
            let opts = parse_options(&args[1..])?;
            let state_path = opts.state.ok_or("play braucht --state DATEI")?;
            let mut game = state::load(&state_path)
                .map_err(|e| format!("konnte {state_path} nicht laden: {e}"))?;
            game.play_rounds(opts.rounds);
            state::save(&game, &state_path).map_err(|e| format!("konnte {state_path} nicht schreiben: {e}"))?;
            print_machine(&game);
        }
        "status" => {
            let opts = parse_options(&args[1..])?;
            let state_path = opts.state.ok_or("status braucht --state DATEI")?;
            let game = state::load(&state_path)
                .map_err(|e| format!("konnte {state_path} nicht laden: {e}"))?;
            print_machine(&game);
        }
        other => return Err(format!("unbekanntes Kommando '{other}'")),
    }
    Ok(ExitCode::SUCCESS)
}

/// Maschinenlesbare Ausgabe (wird vom Python-GUI geparst).
fn print_machine(game: &Game) {
    println!("meta,rounds,{}", game.rounds_played);
    println!("meta,players,{}", game.num_players());
    println!("meta,seed,{}", game.seed());
    for p in game.players() {
        println!("player,{},{},{}", p.index, p.log2_score, p.score);
    }
    // Historie für das Liniendiagramm: hist,<round>,<log2_0>,<log2_1>,...
    for snap in game.history() {
        let vals = snap
            .log2_scores
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",");
        println!("hist,{},{}", snap.round, vals);
    }
}

/// Menschenlesbare Ausgabe für `demo`.
fn print_human(game: &Game) {
    let stats = game.statistics();
    println!("=== dice-game ===");
    println!("Spieler:        {}", game.num_players());
    println!("Runden:         {}", game.rounds_played);
    println!("Seed:           {}", game.seed());
    println!();
    println!("--- Statistik (log2-Raum) ---");
    println!("min log2:       {:.4}", stats.min_log2);
    println!("median log2:    {:.4}", stats.median_log2);
    println!("mean log2:      {:.4}", stats.mean_log2);
    println!("max log2:       {:.4}", stats.max_log2);
    println!("stdabw log2:    {:.4}", stats.std_dev_log2);
    println!();
    println!("--- Statistik (lineare Spielstände) ---");
    println!("min:            {:.6}", stats.min_score());
    println!("median:         {:.6}", stats.median_score());
    println!("geom. Mittel:   {:.6}", stats.geometric_mean_score());
    println!("max:            {:.6}", stats.max_score());
    println!();
    // Bei wenigen Spielern alle Zeilen, sonst die 10 höchsten Spielstände.
    let mut ranked: Vec<&dice_game::Player> = game.players().iter().collect();
    ranked.sort_by(|a, b| b.log2_score.partial_cmp(&a.log2_score).unwrap());
    let show_all = game.num_players() <= 20;
    let limit = if show_all { ranked.len() } else { 10 };
    println!("--- Spielstände ({}) ---", if show_all { "alle".to_string() } else { format!("Top {limit}") });
    println!("{:>6} {:>18} {:>18}", "Spieler", "score", "log2(score)");
    for p in ranked.iter().take(limit) {
        println!("{:>6} {:>18.6} {:>18.4}", p.index, p.score, p.log2_score);
    }
}
