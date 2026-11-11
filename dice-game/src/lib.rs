//! # dice-game
//!
//! Experiment zur **exponentiellen Entwicklung**: N Spieler (z. B. 100)
//! starten mit dem Anfangsspielstand 1.0. Pro Runde würfelt jeder Spieler
//! einmal (reihum); die Augenzahl 1..=6 wird auf einen exponentiell
//! steigenden Faktor in `[1, sqrt(2)]` abgebildet und mit dem aktuellen
//! Spielstand multipliziert.
//!
//! ## Abbildung Würfel -> Faktor
//!
//! ```text
//! factor(a) = (sqrt(2)) ^ ((a - 1) / 5)      für a in 1..=6
//! ```
//!
//! also `1, 2^(1/10), 2^(2/10), ..., sqrt(2)` – eine geometrische Folge.
//!
//! ## Beispiel
//!
//! ```
//! use dice_game::Game;
//!
//! let mut game = Game::new(100, 42);   // 100 Spieler, Seed 42
//! game.play_rounds(10);                // 10 Runden spielen
//! let stats = game.statistics();
//! assert_eq!(stats.count, 100);
//! assert!(stats.min_log2 >= 0.0);      // Faktoren sind nie < 1
//! ```
//!
//! ## Module
//!
//! | Modul    | Inhalt                                             |
//! |----------|----------------------------------------------------|
//! | [`rng`]  | deterministischer Zufallsgenerator (SplitMix64)    |
//! | [`die`]  | Abbildung Augenzahl -> Spielfaktor                 |
//! | [`game`] | Spieler, Runden, Spielregel                        |
//! | [`stats`]| statistische Auswertung                            |
//! | [`state`]| Speichern/Laden des Spielzustands                  |

pub mod die;
pub mod game;
pub mod rng;
pub mod state;
pub mod stats;

pub use die::{expected_log2_per_roll, face_log2, face_value, FACES, MAX_FACE, MIN_FACE};
pub use game::{Game, Player, RoundSnapshot, HISTORY_FULL_ROUNDS, HISTORY_MARKER_START};
pub use rng::SplitMix64;
pub use stats::{summarize, Statistics};
