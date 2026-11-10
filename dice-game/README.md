# dice-game

Experiment zur **exponentiellen Entwicklung** in Rust.

## Spielregeln

0. N Spieler (z. B. 100) starten mit dem Anfangsspielstand **1.0**.
1. Pro Runde würfeln alle N Spieler **reihum** (in fester Reihenfolge).
2. Der Würfel hat **6 Seiten** (Augenzahlen 1 bis 6).
3. Die Augenzahlen werden auf **exponentiell steigende** Werte im Intervall
   `[1, sqrt(2)]` abgebildet (geometrische Folge):

   ```text
   factor(a) = (sqrt(2)) ^ ((a - 1) / 5)        für a = 1..=6
   ```

   also `1, 2^(1/10), 2^(2/10), 2^(3/10), 2^(4/10), sqrt(2)`.

4. Der gewürfelte Wert wird mit dem **aktuellen Spielstand multipliziert**:
   `score *= factor(a)`.
5. Wiederhole ab 1.

**Mathematische Einordnung:** Wegen `log2(sqrt(2)) = 0.5` ist der
Erwartungswert von `log2(factor)` pro Wurf `(3.5 - 1) / 5 * 0.5 = 0.25`.
Der typische (mediane/geometrische) Spielstand wächst daher pro Runde um
`2^0.25 ≈ 1.189` – nach `r` Runden ist er `2^(r/4)`. Die Verteilung der
log2-Spielstände ist nach vielen Runden annähernd normal (Zentraler
Grenzwertsatz im Log-Raum), die Spannweite zwischen bestem und schlechtestem
Spieler wächst mit `sqrt(r)` – die namensgebende „exponential gap".

## Design-Hinweis: log2-Raum

Der Spielstand wird zweifach geführt: als `score: f64` (exakt die Regel
`score *= factor`) und als `log2_score: f64` (Addition von `log2(factor)`).
Beides ist mathematisch äquivalent, aber ab ca. 4096 Runden übersteigt der
wahre Wert den f64-Bereich (`2^1024`), sodass `score` auf `inf` liefe –
`log2_score` bleibt exakt. Alle Statistiken und Anzeigen arbeiten daher im
log2-Raum.

## Projektstruktur

```text
dice-game/
├── Cargo.toml            # Paketdefinition (keine externen Abhängigkeiten)
├── README.md             # diese Datei
├── src/
│   ├── lib.rs            # Bibliotheks-Wurzel, Doku, Re-Exporte
│   ├── rng.rs            # SplitMix64-Zufallsgenerator (deterministisch)
│   ├── die.rs            # Abbildung Augenzahl -> Faktor in [1, sqrt(2)]
│   ├── game.rs           # Spieler, Runden, Spielregel
│   ├── stats.rs          # Statistik (min/max/mean/median/std im log2-Raum)
│   ├── state.rs          # Zustandsdatei speichern/laden
│   └── main.rs           # CLI: demo / init / play / status
└── tests/
    ├── cli.rs            # Integrationstests des Binaries (CARGO_BIN_EXE_*)
    └── simulation.rs     # Simulationstests: Regeln + Statistik
```

Alle Quelldateien sind vollständig kommentiert (deutsch); zusätzlich gibt es
Unit-Tests in jedem Modul (`#[cfg(test)]`) und Doc-Tests in `lib.rs`.

## Bauen und Testen

```bash
cd dice-game
cargo build --release   # Binary nach target/release/dice-game
cargo test              # Unit-Tests + Doc-Tests + Integrationstests
cargo test --release    # dasselbe optimiert
cargo doc --open        # rustdoc-Dokumentation
```

Es werden **keine externen Crates** benötigt (der Zufallsgenerator ist
implementiert), das Projekt baut also auch offline.

## Benutzung

### CLI direkt

```bash
# Einmaliges Spiel, menschenlesbare Ausgabe (100 Spieler, 1000 Runden, Seed 42)
./target/release/dice-game demo
./target/release/dice-game demo --players 100 --rounds 100000 --seed 7

# Spiel mit Zustandsdatei (so steuert das Python-GUI das Binary)
./target/release/dice-game init   --state /tmp/spiel.txt --players 100 --seed 42
./target/release/dice-game play   --state /tmp/spiel.txt --rounds 10
./target/release/dice-game status --state /tmp/spiel.txt
```

### Maschinenlesbares Format (für das GUI)

`init`, `play` und `status` drucken:

```text
meta,rounds,<u64>
meta,players,<usize>
meta,seed,<u64>
player,<index>,<log2_score>,<score>
```

### Zustandsdatei

```text
dice-game-state v1
seed=42
rounds=10
players=100
log2=0.0 0.1 0.2 ...
```

### Als Bibliothek

```rust
use dice_game::Game;

let mut game = Game::new(100, 42);
game.play_rounds(1000);
let stats = game.statistics();
println!("geometrisches Mittel: {}", stats.geometric_mean_score());
```

## Tests

| Ebene | Wo | Inhalt |
|-------|-----|--------|
| Unit | `src/*.rs` (`#[cfg(test)]`) | RNG (Determinismus, Verteilung, Bereich), Würfelabbildung (Grenzen, Monotonie, geometrisches Verhältnis), Spielregeln (Start bei 1.0, Produktregel, log2-Konsistenz), Statistik, Zustandsdatei (Roundtrip, Fehlerfälle) |
| Doc-Tests | `src/lib.rs` | Beispiel aus der Doku ist ausführbar |
| Integration (CLI) | `tests/cli.rs` | init/play/status-Ablauf, Determinismus über Seeds, Fehlerfälle, Hilfe |
| Integration (Simulation) | `tests/simulation.rs` | Regeltreue über 20 Runden, Abgleich mit manuellem RNG, statistisches Gesetz (Mittel der log2-Werte ≈ 0.25·Runden), sichtbare „exponential gap", Stabilität bei 200 000 Runden |

## GUI

Die grafische Steuerung liegt in `../gui/dice_game_gui.py` (Python/tkinter)
und ruft dieses Binary als Motor auf.
