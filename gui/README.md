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
| Liniendiagramm | x = Rundennummer, y = Spielstand, eine fortlaufende Linie pro Spieler (helle Farbpalette, Spitzenreiter kräftig hervorgehoben); **nur die Ausläufer** (die letzten Runden) werden gezeigt — Fenster = letzte 10 % der Gesamtrunden, mind. 100, bis 200 Runden alles |
| Y-Achse-Button (gelb) | klickt zyklisch durch **log2 (default) → linear → log-log** (doppelt logarithmisch). Linear rechnet `2**log2`; Überlauf wird per try/except als `inf` abgefangen (kein Crash mehr), im Diagramm erscheint ein Hinweis |
| Fehlerbehandlung | Anzeige-/Zeichenfehler werden per try/except sanft abgefangen (Log + Statusleiste), die App bleibt benutzbar |
| Kosmetik | helle Palette (Orange/Grün/Gelb/Blau) via ttk-Theme „clam“; Panels als **abgerundete Karten** (`RoundedFrame`, Canvas-Trick, da tkinter keine nativ abgerundeten Rahmen kennt) |
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
bis 999, dann dekadenweise Strides — im Rust-Projekt dokumentiert). Daraus
wählt die GUI per `tail_window()` nur die letzten Runden (Ausläufer) aus.
Der Zustand liegt in einer Textdatei (`$TMPDIR/dice_game_state.txt`),
dadurch bleibt das Spiel über GUI-Neustarts und Klicks hinweg erhalten.
