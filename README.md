# exponential-gap

Experimente mit exponentieller Entwicklung.

## Inhalt

| Pfad | Inhalt |
|------|--------|
| `dice-game/` | Rust-Projekt: Würfel-Experiment (N Spieler starten bei 1.0, Augenzahlen 1..=6 werden auf exponentiell steigende Faktoren in [1, sqrt(2)] abgebildet und mit dem Spielstand multipliziert). Vollständig kommentiert, dokumentiert, mit Unit-/Integrations-/Simulationstests. Siehe `dice-game/README.md`. |
| `gui/` | Python-GUI (tkinter, nur Standardbibliothek) zur Steuerung des Rust-Binaries: 1-Runde-Button, N-Runden-Button mit Dropdown (1/10/100/1k/1M), Histogramm der Spielstände, Statistik, Top 10, Log. Siehe `gui/README.md`. |
| `.vscode/` (liegt **eine Ebene über dem Repo**, im Ordner `exponential-gap/`) | VS-Code-Tasks (Build/Test/Start) und Debug-Konfigurationen (Rust via CodeLLDB, Python via debugpy) mit Variablen aus `settings.json`. Siehe `../.vscode/`. |

## Quickstart

```bash
cd dice-game && cargo test --release && cargo build --release   # 40 Tests, Binary bauen
cd ../gui && python3 dice_game_gui.py                           # GUI starten
```
