# GUI für das Würfel-Experiment

`dice_game_gui.py` – grafische Oberfläche in **Python + tkinter**
( ausschliesslich Standardbibliothek, kein matplotlib/PyPI nötig).

## Start

```bash
# 1. Rust-Motor bauen (falls noch nicht geschehen)
cd ../dice-game && cargo build --release && cd ../gui

# 2. GUI starten
python3 dice_game_gui.py
```

Das Binary wird automatisch unter `../dice-game/target/release/dice-game`
gesucht (sonst `target/debug/...` oder Umgebungsvariable `DICE_GAME_BIN`).
In der GUI kann der Pfad auch manuell eingetragen/durchsucht werden.

## Bedienung

| Element | Funktion |
|---|---|
| Spieler / Seed | Anzahl der Spieler (Startspielstand je 1.0) und Reproduzierbarkeits-Seed |
| **Neu starten** | Legt ein neues Spiel in der Zustandsdatei an |
| **1 Runde spielen** | Spielt genau eine Runde (alle Spieler würfeln reihum) |
| **N Runden spielen** + Dropdown | Spielt 1 / 10 / 100 / 1k / 1M Runden auf einen Klick |
| Liniendiagramm | x = Rundennummer, y = Spielstand, eine fortlaufende Linie pro Spieler (Spitzenreiter hervorgehoben); **Y-Achse per Button zwischen log2 und linear umschaltbar** |
| Y-Achse-Button | schaltet die y-Achse des Diagramms zwischen logarithmisch (log2, default) und linear um |
| Statistik-Panel | Runden, min/median/mean/max/std der log2-Werte, min/geom.Mittel/max Spielstand |
| Top 10 | Die höchsten Spielstände (ab 2^1000 als `2^x` dargestellt) |
| Log + Statusleiste | Letzte Aktionen und aktueller Zustand |
| Menü Datei/Hilfe | Neu starten (Strg+N), Beenden (Strg+Q), Über |

## Technik

Die GUI rechnet nicht selbst, sondern ruft das Rust-Binary als Motor:

```text
init   --players N --seed S --state <datei>   # neues Spiel
play   --rounds R      --state <datei>         # R Runden weiterspielen
status                 --state <datei>         # Stand lesen
```

Die Ausgabe ist maschinenlesbar (`meta,rounds,...` / `player,<i>,<log2>,<score>` /
`hist,<round>,<log2_0>,...`) und wird von `parse_output()` in ein `Snapshot`-Objekt
geparst. Die `hist`-Zeilen sind die Historie für das Liniendiagramm (jede Runde
bis 100, dann geometrische Marker — im Rust-Projekt dokumentiert).
Der Zustand liegt in einer Textdatei (`$TMPDIR/dice_game_state.txt`),
dadurch bleibt das Spiel über GUI-Neustarts und Klicks hinweg erhalten.
