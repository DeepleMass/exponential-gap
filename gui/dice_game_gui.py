#!/usr/bin/env python3
"""
dice_game_gui.py – Grafische Oberfläche (GUI) für das Würfel-Experiment „dice-game“.

Die GUI ist nur die Steuerung: Die eigentliche Spielroutine läuft im
kompilierten Rust-Binary (exponential-gap/dice-game/target/release/dice-game),
das über eine Zustandsdatei persistiert und zwischen den Klicks fortgeführt wird.

Ausgabe: Liniendiagramm – x = Rundennummer, y = Spielstand, eine fortlaufende
Linie pro Spieler. Gezeigt werden nur die „Ausläufer“ (die letzten Runden;
wie viele, hängt von der Gesamtrundenzahl ab). Die y-Achse ist per Button
umschaltbar: **linear → log2 → log-log** (doppelt logarithmisch). Bei
f64-Überlauf in linearer Ansicht wird das sanft abgefangen (inf statt Crash).

Kosmetik: helle Farbpalette (Orange/Grün/Gelb/Blau) und abgerundete Rahmen
(über einen Canvas-Trick, da tkinter keine nativ abgerundeten Frames kennt).

Aufruf:
    python3 dice_game_gui.py

Nur Python-Standardbibliothek (tkinter) – keine externen Pakete nötig.
"""

from __future__ import annotations

import math
import os
import subprocess
import sys
import tempfile
import time
import tkinter as tk
from tkinter import filedialog, messagebox, ttk

# ---------------------------------------------------------------------------
# Konstanten & Farbpalette (helle Töne)
# ---------------------------------------------------------------------------

APP_TITLE = "Dice Game – Exponentielles Würfelexperiment"

#: Dropdown-Werte für „N Runden spielen“ (Anzeige, tatsächliche Rundenzahl).
ROUND_CHOICES = [("1", 1), ("10", 10), ("100", 100), ("1k", 1_000), ("1M", 1_000_000)]

DEFAULT_PLAYERS = 100
DEFAULT_SEED = 42

#: Zustandsdatei, über die das GUI das Rust-Binary fortführt.
STATE_FILE = os.path.join(tempfile.gettempdir(), "dice_game_state.txt")

#: Ab welchem log2-Wert die Anzeige auf „2^x“ umschaltet (f64-Grenze ~2^1024).
SCORE_DISPLAY_LIMIT = 1000.0

# --- Palette ---------------------------------------------------------------
BG = "#f6f4ef"            # App-Hintergrund (warm hell)
CARD = "#fffdf8"          # Karten-/Panel-Hintergrund
CARD_BORDER = "#e7e5e4"   # dezente Rahmenfarbe
INK = "#1f2937"           # Haupttext
MUTED = "#78716c"         # Sekundärtext

ORANGE_BG, ORANGE_FG = "#ffedd5", "#9a3412"
GREEN_BG, GREEN_FG = "#dcfce7", "#166534"
YELLOW_BG, YELLOW_FG = "#fef9c3", "#a16207"
BLUE_BG, BLUE_FG = "#dbeafe", "#1d4ed8"

CHART_BG = "#ffffff"
GRID = "#eef2f7"
AXIS = "#94a3b8"
#: Linienfarben der Spieler (helle Töne, werden zyklisch verwendet).
PLAYER_PALETTE = [
    "#8ecae6", "#90be6d", "#f4a261", "#e9c46a", "#a0c4ff",
    "#f28482", "#84a59d", "#ffb4a2", "#c3aed6", "#b5e48c",
]
LEADER_COLOR = "#e76f51"  # Spitzenreiter (kräftiges Orange)

#: Ab wie vielen Gesamtrunden nur noch der Ausläufer gezeigt wird.
TAIL_FULL_ROUNDS = 200

#: y-Achsen-Modi; der Button klickt zyklisch durch.
Y_MODES = ["log2", "linear", "loglog"]


def next_y_mode(mode: str) -> str:
    """Nächster y-Achsen-Modus im Zyklus (log2 -> linear -> loglog -> log2)."""
    return Y_MODES[(Y_MODES.index(mode) + 1) % len(Y_MODES)]


def y_value(log2_score: float, mode: str) -> float:
    """Rechnet einen log2-Spielstand in den y-Achsen-Wert des Modus um.

    Linear rechnet `2**log2` — das kann in Python OverflowError werfen
    (anders als in Rust, wo es inf wird). Wird hier abgefangen: Ergebnis inf.
    """
    if mode == "log2":
        return log2_score
    if mode == "loglog":
        # doppelt logarithmisch: log2(log2(score)); bei 0 auf kleinen Wert klemmen
        return math.log2(log2_score) if log2_score > 0 else math.log2(1e-12)
    # linear
    try:
        return 2.0 ** log2_score
    except OverflowError:
        return math.inf


def default_binary() -> str:
    """Sucht das Rust-Binary: erst Umgebungsvariable, dann Standardpfade."""
    env = os.environ.get("DICE_GAME_BIN")
    if env:
        return env
    here = os.path.dirname(os.path.abspath(__file__))
    for rel in (
        "../dice-game/target/release/dice-game",
        "../dice-game/target/debug/dice-game",
    ):
        cand = os.path.normpath(os.path.join(here, rel))
        if os.path.isfile(cand) and os.access(cand, os.X_OK):
            return cand
    return ""


def tail_window(total_rounds: int) -> int:
    """Wie viele der letzten Runden im Diagramm gezeigt werden (0 = alle).

    Hängt von der Gesamtrundenzahl ab: bis 200 Runden wird alles gezeigt,
    danach die letzten 10 % (mindestens aber die letzten 100 Runden).
    """
    if total_rounds <= TAIL_FULL_ROUNDS:
        return 0
    return max(100, total_rounds // 10)


# ---------------------------------------------------------------------------
# Engine-Anbindung an das Rust-Binary
# ---------------------------------------------------------------------------


class EngineError(RuntimeError):
    """Wird geworfen, wenn das Rust-Binary fehlt oder einen Fehler meldet."""


class Snapshot:
    """Aktueller Spielstand + Historie, aus der Binary-Ausgabe geparst.

    `history` ist eine Liste von `(round, [log2_score_pro_spieler])`,
    aufsteigend nach Rundennummer — die Daten für das Liniendiagramm.
    """

    def __init__(
        self,
        rounds: int,
        players: int,
        seed: int,
        log2_scores: list[float],
        history: list[tuple[int, list[float]]] | None = None,
    ):
        self.rounds = rounds
        self.players = players
        self.seed = seed
        self.log2_scores = log2_scores  # aktueller Stand, in Spielerreihenfolge
        self.history = history if history is not None else []

    # -- abgeleitete Statistik (im log2-Raum, numerisch stabil) --------------

    @property
    def min_log2(self) -> float:
        return min(self.log2_scores)

    @property
    def max_log2(self) -> float:
        return max(self.log2_scores)

    @property
    def mean_log2(self) -> float:
        return sum(self.log2_scores) / len(self.log2_scores)

    @property
    def median_log2(self) -> float:
        s = sorted(self.log2_scores)
        n = len(s)
        mid = n // 2
        return s[mid] if n % 2 == 1 else (s[mid - 1] + s[mid]) / 2.0

    @property
    def std_dev_log2(self) -> float:
        m = self.mean_log2
        return math.sqrt(sum((x - m) ** 2 for x in self.log2_scores) / len(self.log2_scores))

    @property
    def top_players(self) -> list[tuple[int, float]]:
        """(Spielerindex, log2) absteigend sortiert."""
        return sorted(enumerate(self.log2_scores), key=lambda t: t[1], reverse=True)


def parse_output(text: str) -> Snapshot:
    """Parst die maschinenlesbare Ausgabe des Rust-Binary in einen Snapshot."""
    rounds = players = seed = None
    scores: list[tuple[int, float]] = []
    history: list[tuple[int, list[float]]] = []
    for line in text.splitlines():
        parts = line.strip().split(",")
        if len(parts) == 3 and parts[0] == "meta":
            key, val = parts[1], parts[2]
            if key == "rounds":
                rounds = int(val)
            elif key == "players":
                players = int(val)
            elif key == "seed":
                seed = int(val)
        elif len(parts) >= 4 and parts[0] == "player":
            scores.append((int(parts[1]), float(parts[2])))
        elif len(parts) >= 2 and parts[0] == "hist":
            # hist,<round>,<log2_0>,<log2_1>,...
            history.append((int(parts[1]), [float(x) for x in parts[2:]]))
    if rounds is None or players is None or seed is None:
        raise EngineError(f"Ausgabe enthält keine vollständigen meta-Zeilen:\n{text[:300]}")
    scores.sort(key=lambda t: t[0])
    if len(scores) != players:
        raise EngineError(f"erwartet {players} Spieler, Ausgabe enthält {len(scores)}")
    return Snapshot(rounds, players, seed, [s for _, s in scores], history)


class DiceGameEngine:
    """Kapselt die Aufrufe des Rust-Binary (init / play / status)."""

    def __init__(self, binary: str, state_file: str = STATE_FILE):
        if not binary or not os.path.isfile(binary):
            raise EngineError(
                "Rust-Binary nicht gefunden: "
                f"{binary or '(leer)'!r}\nBitte zuerst 'cargo build --release' in "
                "dice-game/ ausführen oder den Pfad über die Binary-Zeile links oben setzen."
            )
        self.binary = binary
        self.state_file = state_file

    def _run(self, *args: str) -> str:
        """Führt das Binary aus und liefert stdout; wirft EngineError bei Misserfolg."""
        try:
            proc = subprocess.run(
                [self.binary, *args],
                capture_output=True,
                text=True,
                timeout=600,
            )
        except FileNotFoundError as exc:
            raise EngineError(f"Binary nicht ausführbar: {exc}") from exc
        except subprocess.TimeoutExpired as exc:
            raise EngineError("Zeitüberschreitung beim Binary (600 s)") from exc
        if proc.returncode != 0:
            raise EngineError(
                f"Befehl {' '.join(args)} schlug fehl (Exit {proc.returncode}):\n{proc.stderr.strip()}"
            )
        return proc.stdout

    def init(self, players: int, seed: int) -> Snapshot:
        """Neues Spiel anlegen (Anfangsspielstand 1.0) und Stand zurückgeben."""
        self._run(
            "init",
            "--players", str(players),
            "--seed", str(seed),
            "--state", self.state_file,
        )
        return self.status()

    def play(self, rounds: int) -> Snapshot:
        """`rounds` Runden weiterspielen (Zustand bleibt in der Datei erhalten)."""
        self._run("play", "--rounds", str(rounds), "--state", self.state_file)
        return self.status()

    def status(self) -> Snapshot:
        """Aktuellen Stand inkl. Historie lesen (ohne zu spielen)."""
        return parse_output(self._run("status", "--state", self.state_file))


# ---------------------------------------------------------------------------
# Formatierung
# ---------------------------------------------------------------------------


def fmt_score(log2: float) -> str:
    """Spielstand anzeigbar formatieren; ab 2^1000 als „2^x“ (f64-Überlauf)."""
    if math.isnan(log2) or math.isinf(log2):
        return "inf"
    if log2 <= SCORE_DISPLAY_LIMIT:
        return f"{2.0 ** log2:.6g}"
    return f"2^{log2:.1f}"


def fmt_num(x: float) -> str:
    """Kleine log2-Werte hübsch formatieren."""
    return f"{x:.4f}"


def fmt_big(x: float) -> str:
    """Große lineare Spielstände (Achsenbeschriftung) kompakt formatieren."""
    if not math.isfinite(x):
        return "inf"
    if x >= 1e6 or x < 1e-3:
        return f"{x:.3e}"
    return f"{x:.4g}"


# ---------------------------------------------------------------------------
# Abgerundeter Rahmen (tkinter kann das nicht nativ -> Canvas-Trick)
# ---------------------------------------------------------------------------


def _rounded_rect_points(x1: float, y1: float, x2: float, y2: float, r: float) -> list[float]:
    """Eckpunkte eines abgerundeten Rechtecks (Bögen an den vier Ecken)."""
    import math as _m
    pts: list[float] = []
    corners = (
        (x1 + r, y1 + r, 180),  # oben links
        (x2 - r, y1 + r, 270),  # oben rechts
        (x2 - r, y2 - r, 0),    # unten rechts
        (x1 + r, y2 - r, 90),   # unten links
    )
    for cx, cy, a0 in corners:
        for deg in range(a0, a0 + 91, 15):
            a = _m.radians(deg)
            pts.append(cx + r * _m.cos(a))
            pts.append(cy + r * _m.sin(a))
    return pts


class RoundedFrame(tk.Canvas):
    """Abgerundete Karte: Canvas zeichnet das rounded rect, innen liegt ein Frame.

    Kinder werden in `self.card` gepackt (ttk.Frame mit Style „Card.TFrame“).
    """

    def __init__(
        self,
        master,
        title: str = "",
        bg: str = CARD,
        border: str = CARD_BORDER,
        title_fg: str = MUTED,
        radius: int = 14,
        pad: int = 10,
        outer_bg: str = BG,
        **kw,
    ):
        super().__init__(master, highlightthickness=0, bg=outer_bg, **kw)
        self._bg, self._border, self._r, self._pad = bg, border, radius, pad
        self.card = ttk.Frame(self, style="Card.TFrame")
        if title:
            ttk.Label(self.card, text=title, style="CardTitle.TLabel", foreground=title_fg).pack(anchor="w")
        self._win = self.create_window(pad, pad, anchor="nw", window=self.card)
        self.card.bind("<Configure>", lambda _e: self._fit())

    def _fit(self) -> None:
        w = self.card.winfo_reqwidth() + 2 * self._pad
        h = self.card.winfo_reqheight() + 2 * self._pad
        self.configure(width=w, height=h)
        self._draw()

    def _draw(self) -> None:
        self.delete("bg")
        w, h = self.winfo_width(), self.winfo_height()
        if w < 4 or h < 4:
            return
        r = min(self._r, w / 2, h / 2)
        pts = _rounded_rect_points(1, 1, w - 1, h - 1, r)
        self.create_polygon(*pts, fill=self._bg, outline=self._border, width=1, smooth=True, tags="bg")
        self.tag_lower("bg")
        self.coords(self._win, self._pad, self._pad)


# ---------------------------------------------------------------------------
# Liniendiagramm-Leinwand
# ---------------------------------------------------------------------------


class LineChartCanvas(tk.Canvas):
    """Liniendiagramm: x = Rundennummer, y = Spielstand (linear oder log2).

    Pro Spieler eine fortlaufende Linie nach rechts (Palette aus hellen
    Tönen), der aktuelle Spitzenreiter kräftiger hervorgehoben.
    """

    def __init__(self, master, **kw):
        super().__init__(master, bg=CHART_BG, highlightthickness=0, **kw)
        self._history: list[tuple[int, list[float]]] = []
        self._y_mode = "log2"
        self._total_rounds = 0
        # Neu zeichnen, sobald die Größe feststeht.
        self.bind("<Configure>", lambda _e: self.redraw())

    def set_data(self, history: list[tuple[int, list[float]]], y_mode: str, total_rounds: int = 0) -> None:
        self._history = history
        self._y_mode = y_mode
        self._total_rounds = total_rounds
        self.redraw()

    def redraw(self) -> None:
        self.delete("all")
        w = max(self.winfo_width(), 300)
        h = max(self.winfo_height(), 200)
        pad_l, pad_r, pad_t, pad_b = 80, 15, 34, 45
        cw, ch = w - pad_l - pad_r, h - pad_t - pad_b  # Zeichenfläche

        # Achsen
        y_base = pad_t + ch  # unten
        self.create_line(pad_l, pad_t, pad_l, y_base, fill=AXIS)
        self.create_line(pad_l, y_base, pad_l + cw, y_base, fill=AXIS)
        self.create_text(pad_l + cw / 2, h - 12, text="Rundennummer", fill=MUTED, anchor="n")
        y_label = {
            "log2": "log2(Spielstand)",
            "linear": "Spielstand (linear)",
            "loglog": "log2(log2(Spielstand))",
        }[self._y_mode]
        self.create_text(16, pad_t + ch / 2, text=y_label, fill=MUTED, angle=90, anchor="center")

        if not self._history:
            self.create_text(
                pad_l + cw / 2, pad_t + ch / 2,
                text="Noch keine Daten – bitte „Neu starten“ klicken.",
                fill=MUTED,
            )
            return

        rounds = [r for r, _ in self._history]
        n_players = len(self._history[0][1])
        rx0, rx1 = rounds[0], rounds[-1]
        if rx1 <= rx0:
            rx1 = rx0 + 1

        # y-Werte je nach Modus (linear kann inf liefern — wird unten abgefangen).
        def yval(log2: float) -> float:
            return y_value(log2, self._y_mode)

        all_vals = [yval(v) for _, scores in self._history for v in scores]
        finite = [v for v in all_vals if math.isfinite(v)]
        overflow = len(finite) < len(all_vals)
        if not finite:
            self.create_text(pad_l + cw / 2, pad_t + ch / 2, text="keine endlichen Werte", fill="#dc2626")
            return
        ymin, ymax = min(finite), max(finite)
        if ymax <= ymin:
            ymax = ymin + 1.0

        def X(r: float) -> float:
            return pad_l + (r - rx0) / (rx1 - rx0) * cw

        def Y(v: float) -> float:
            return y_base - (v - ymin) / (ymax - ymin) * ch

        # x-Achsen-Beschriftung: 5 Ticks (Rundennummern)
        for k in range(5):
            frac = k / 4
            rv = rx0 + frac * (rx1 - rx0)
            xv = X(rv)
            self.create_line(xv, y_base, xv, y_base + 5, fill=AXIS)
            self.create_text(xv, y_base + 10, text=str(int(rv)), fill=MUTED, anchor="n")

        # y-Achsen-Beschriftung: 5 Ticks + dezentes Gitter
        for k in range(5):
            frac = k / 4
            yv = ymin + frac * (ymax - ymin)
            ypix = Y(yv)
            self.create_line(pad_l, ypix, pad_l + cw, ypix, fill=GRID)
            self.create_line(pad_l - 5, ypix, pad_l, ypix, fill=AXIS)
            label = fmt_big(yv) if self._y_mode == "linear" else fmt_num(yv)
            self.create_text(pad_l - 8, ypix, text=label, fill=MUTED, anchor="e")

        # Linien pro Spieler (x = Runde nach rechts, y = Spielstand)
        leader_idx = max(range(n_players), key=lambda i: self._history[-1][1][i])
        for i in range(n_players):
            pts: list[float] = []
            for r, scores in self._history:
                v = yval(scores[i])
                if not math.isfinite(v):
                    v = ymax  # übergelaufene Werte oben abschneiden
                pts.append(X(r))
                pts.append(Y(v))
            is_leader = i == leader_idx
            self.create_line(
                *pts,
                fill=LEADER_COLOR if is_leader else PLAYER_PALETTE[i % len(PLAYER_PALETTE)],
                width=2 if is_leader else 1,
            )

        # Titel: gezeigter Ausläufer + Modus
        if self._total_rounds > rounds[-1]:
            span = f"Runden {rounds[0]}–{rounds[-1]} (Ausläufer von {self._total_rounds} gesamt)"
        else:
            span = f"Runden {rounds[0]}–{rounds[-1]}"
        self.create_text(
            pad_l + cw / 2, 12,
            text=f"Spielstand je Spieler – {span} | {n_players} Spieler | y: {self._y_mode}",
            fill=INK, anchor="n",
        )
        if overflow and self._y_mode == "linear":
            self.create_text(
                pad_l + cw / 2, 28,
                text="Hinweis: Spielstände über dem f64-Maximum sind unendlich – in linearer Ansicht abgeschnitten. Per Button auf log2 oder log-log wechseln.",
                fill="#dc2626", anchor="n",
            )


# ---------------------------------------------------------------------------
# Theme
# ---------------------------------------------------------------------------


def apply_theme(root: tk.Tk) -> None:
    """Helle Palette auf ttk-Theme „clam“ legen (erlaubt Farb-Anpassung)."""
    style = ttk.Style(root)
    try:
        style.theme_use("clam")
    except tk.TclError:
        pass
    style.configure(".", background=BG, foreground=INK, fieldbackground="#ffffff")
    style.configure("TFrame", background=BG)
    style.configure("TLabel", background=BG, foreground=INK)
    style.configure("TButton", background=ORANGE_BG, foreground=ORANGE_FG,
                    bordercolor="#fdba74", focuscolor="#fb923c", padding=6)
    style.map("TButton",
              background=[("active", "#fed7aa"), ("pressed", "#fdba74")],
              foreground=[("active", ORANGE_FG), ("pressed", ORANGE_FG)])
    for name, bg, fg in (
        ("Orange.TButton", ORANGE_BG, ORANGE_FG),
        ("Green.TButton", GREEN_BG, GREEN_FG),
        ("Yellow.TButton", YELLOW_BG, YELLOW_FG),
        ("Blue.TButton", BLUE_BG, BLUE_FG),
    ):
        style.configure(name, background=bg, foreground=fg, bordercolor=bg, focuscolor=fg, padding=6)
        style.map(name, background=[("active", bg), ("pressed", bg)],
                  foreground=[("active", fg), ("pressed", fg)])
    style.configure("TEntry", fieldbackground="#ffffff", foreground=INK)
    style.configure("TSpinbox", fieldbackground="#ffffff", foreground=INK, arrowcolor=INK)
    style.configure("TCombobox", fieldbackground="#ffffff", background="#ffffff", foreground=INK)
    style.configure("Card.TFrame", background=CARD)
    style.configure("Card.TLabel", background=CARD, foreground=INK)
    style.configure("CardTitle.TLabel", background=CARD, foreground=MUTED,
                    font=("TkDefaultFont", 9, "bold"))
    style.configure("Treeview", background="#ffffff", fieldbackground="#ffffff",
                    foreground=INK, rowheight=22)
    style.configure("Treeview.Heading", background=BLUE_BG, foreground=BLUE_FG,
                    font=("TkDefaultFont", 9, "bold"))
    style.map("Treeview", background=[("selected", "#bfdbfe")],
              foreground=[("selected", "#1e3a8a")])
    style.configure("Horizontal.TSeparator", background=CARD_BORDER)
    root.configure(bg=BG)


# ---------------------------------------------------------------------------
# Hauptfenster
# ---------------------------------------------------------------------------


class App(tk.Tk):
    """Hauptfenster der Anwendung."""

    def __init__(self) -> None:
        super().__init__()
        apply_theme(self)
        self.title(APP_TITLE)
        self.geometry("1150x720")
        self.minsize(900, 580)

        self.engine: DiceGameEngine | None = None
        self.snapshot: Snapshot | None = None
        self._y_mode = "log2"  # Startmodus der y-Achse: logarithmisch (log2)

        self._build_menu()
        self._build_toolbar()
        self._build_main()
        self._build_statusbar()

        # Binary-Pfad vorbelegen und Engine versuchen anzulegen.
        self.binary_var.set(default_binary())
        self._apply_binary()

        self.log("Bereit. Bitte „Neu starten“ klicken, um ein Spiel anzulegen.")
        self.protocol("WM_DELETE_WINDOW", self.destroy)

    # -- Aufbau ---------------------------------------------------------------

    def _build_menu(self) -> None:
        menubar = tk.Menu(self, bg="#ffffff", fg=INK, activebackground=ORANGE_BG, activeforeground=ORANGE_FG)
        menu_file = tk.Menu(menubar, tearoff=0, bg="#ffffff", fg=INK,
                           activebackground=ORANGE_BG, activeforeground=ORANGE_FG)
        menu_file.add_command(label="Neu starten", command=self.on_new, accelerator="Strg+N")
        menu_file.add_command(label="Binary wechseln…", command=self.on_browse_binary)
        menu_file.add_separator()
        menu_file.add_command(label="Beenden", command=self.destroy, accelerator="Strg+Q")
        menubar.add_cascade(label="Datei", menu=menu_file)
        menu_help = tk.Menu(menubar, tearoff=0, bg="#ffffff", fg=INK,
                           activebackground=ORANGE_BG, activeforeground=ORANGE_FG)
        menu_help.add_command(label="Über…", command=self.on_about)
        menubar.add_cascade(label="Hilfe", menu=menu_help)
        self.config(menu=menubar)
        self.bind("<Control-n>", lambda _e: self.on_new())
        self.bind("<Control-q>", lambda _e: self.destroy())

    def _build_toolbar(self) -> None:
        bar = ttk.Frame(self, padding=6)
        bar.pack(side="top", fill="x")

        ttk.Label(bar, text="Spieler:").pack(side="left")
        self.players_var = tk.IntVar(value=DEFAULT_PLAYERS)
        ttk.Spinbox(bar, from_=1, to=1_000_000, textvariable=self.players_var, width=8).pack(side="left", padx=(2, 10))

        ttk.Label(bar, text="Seed:").pack(side="left")
        self.seed_var = tk.StringVar(value=str(DEFAULT_SEED))
        ttk.Entry(bar, textvariable=self.seed_var, width=8).pack(side="left", padx=(2, 10))

        ttk.Button(bar, text="Neu starten", style="Orange.TButton", command=self.on_new).pack(side="left", padx=(0, 10))

        ttk.Separator(bar, orient="vertical").pack(side="left", fill="y", padx=6)

        ttk.Button(bar, text="1 Runde spielen", style="Green.TButton", command=self.on_play_one).pack(side="left")
        ttk.Button(bar, text="N Runden spielen", style="Blue.TButton", command=self.on_play_n).pack(side="left", padx=(6, 2))
        self.rounds_label_var = tk.StringVar(value="100")
        self.rounds_combo = ttk.Combobox(
            bar,
            textvariable=self.rounds_label_var,
            values=[label for label, _ in ROUND_CHOICES],
            state="readonly",
            width=6,
        )
        self.rounds_combo.pack(side="left", padx=(2, 10))
        self.rounds_combo.set("100")

        ttk.Separator(bar, orient="vertical").pack(side="left", fill="y", padx=6)

        ttk.Label(bar, text="Binary:").pack(side="left")
        self.binary_var = tk.StringVar()
        ttk.Entry(bar, textvariable=self.binary_var, width=42).pack(side="left", padx=(2, 2))
        ttk.Button(bar, text="…", width=3, command=self.on_browse_binary).pack(side="left")
        ttk.Button(bar, text="übernehmen", command=self._apply_binary).pack(side="left", padx=(4, 0))

    def _build_main(self) -> None:
        main = ttk.Frame(self, padding=8)
        main.pack(side="top", fill="both", expand=True)

        # Linke Seite: Diagramm in abgerundeter Karte
        chart_card = RoundedFrame(main, title="Verlauf der Spielstände",
                                  border="#fdba74", title_fg=ORANGE_FG)
        chart_card.pack(side="left", fill="both", expand=True)
        self.canvas = LineChartCanvas(chart_card.card, width=680, height=440)
        self.canvas.pack(side="top", fill="both", expand=True, padx=4, pady=4)
        opts = ttk.Frame(chart_card.card, style="Card.TFrame")
        opts.pack(side="bottom", fill="x", padx=6, pady=(0, 8))
        # Button zum Umschalten der y-Achse zwischen logarithmisch und linear.
        self.yaxis_button = ttk.Button(opts, text=self._yaxis_button_text(),
                                       style="Yellow.TButton", command=self._toggle_yaxis)
        self.yaxis_button.pack(side="left")
        ttk.Label(opts, text="(x = Rundennummer, y = Spielstand; eine Linie pro Spieler, nur Ausläufer)",
                  style="Card.TLabel", foreground=MUTED).pack(side="left", padx=8)

        # Rechte Seite: drei abgerundete Karten (Statistik, Top 10, Log)
        right = ttk.Frame(main, width=380)
        right.pack(side="right", fill="y", padx=(8, 0))
        right.pack_propagate(False)

        stats_card = RoundedFrame(right, title="Statistik", border="#a7f3d0", title_fg=GREEN_FG)
        stats_card.pack(side="top", fill="x", pady=(0, 8))
        self.stats_var = tk.StringVar(value="–")
        ttk.Label(stats_card.card, textvariable=self.stats_var, style="Card.TLabel",
                  justify="left", font=("TkFixedFont", 9)).pack(anchor="w", padx=4, pady=4)

        top_card = RoundedFrame(right, title="Top 10 Spieler", border="#bfdbfe", title_fg=BLUE_FG)
        top_card.pack(side="top", fill="both", expand=True, pady=(0, 8))
        columns = ("rang", "spieler", "spielstand")
        self.top_tree = ttk.Treeview(top_card.card, columns=columns, show="headings", height=10)
        self.top_tree.heading("rang", text="Rang")
        self.top_tree.heading("spieler", text="Spieler")
        self.top_tree.heading("spielstand", text="Spielstand")
        self.top_tree.column("rang", width=45, anchor="e")
        self.top_tree.column("spieler", width=60, anchor="e")
        self.top_tree.column("spielstand", width=200, anchor="e")
        vsb = ttk.Scrollbar(top_card.card, orient="vertical", command=self.top_tree.yview)
        self.top_tree.configure(yscrollcommand=vsb.set)
        self.top_tree.pack(side="left", fill="both", expand=True, padx=(4, 0), pady=4)
        vsb.pack(side="right", fill="y", pady=4)

        log_card = RoundedFrame(right, title="Log", border="#fef08a", title_fg=YELLOW_FG)
        log_card.pack(side="top", fill="both", expand=True)
        self.log_text = tk.Text(log_card.card, height=8, wrap="word", state="disabled",
                                bg="#ffffff", fg=INK, relief="flat")
        lsb = ttk.Scrollbar(log_card.card, orient="vertical", command=self.log_text.yview)
        self.log_text.configure(yscrollcommand=lsb.set)
        self.log_text.pack(side="left", fill="both", expand=True, padx=(4, 0), pady=4)
        lsb.pack(side="right", fill="y", pady=4)

    def _build_statusbar(self) -> None:
        self.status_var = tk.StringVar(value="Bereit.")
        tk.Label(self, textvariable=self.status_var, bg=BLUE_BG, fg=BLUE_FG,
                 anchor="w", padx=8, pady=3).pack(side="bottom", fill="x")

    # -- Engine / Aktionen ----------------------------------------------------

    def _apply_binary(self) -> None:
        """Übernimmt den Binary-Pfad aus der Eingabezeile und baut die Engine."""
        path = self.binary_var.get().strip()
        try:
            self.engine = DiceGameEngine(path)
        except EngineError as exc:
            self.engine = None
            self.status_var.set(f"Binary nicht gefunden: {path!r}")
            self.log(f"WARN: {exc}")
            return
        self.status_var.set(f"Binary: {path}")
        self.log(f"Binary übernommen: {path}")

    def on_browse_binary(self) -> None:
        path = filedialog.askopenfilename(
            title="Rust-Binary wählen (dice-game)",
            initialdir=os.path.dirname(os.path.abspath(__file__)),
        )
        if path:
            self.binary_var.set(path)
            self._apply_binary()

    def _require_engine(self) -> DiceGameEngine | None:
        if self.engine is None:
            self._apply_binary()
        if self.engine is None:
            messagebox.showerror("Fehler", "Kein gültiges Rust-Binary konfiguriert.\n"
                                 "Bitte 'cargo build --release' in dice-game/ ausführen.")
        return self.engine

    def _read_players_seed(self) -> tuple[int, int]:
        try:
            players = int(self.players_var.get())
            seed = int(self.seed_var.get())
        except (tk.TclError, ValueError):
            raise EngineError("Spieleranzahl und Seed müssen ganze Zahlen sein.")
        if players < 1:
            raise EngineError("Spieleranzahl muss mindestens 1 sein.")
        return players, seed

    def on_new(self) -> None:
        """Neues Spiel anlegen."""
        engine = self._require_engine()
        if engine is None:
            return
        try:
            players, seed = self._read_players_seed()
            snap = engine.init(players, seed)
        except EngineError as exc:
            messagebox.showerror("Fehler", str(exc))
            self.log(f"FEHLER: {exc}")
            return
        self.snapshot = snap
        self.log(f"Neues Spiel: {players} Spieler, Seed {seed}, Runde 0.")
        self._safe_refresh()

    def on_play_one(self) -> None:
        """Genau eine Runde spielen."""
        self._play_rounds(1, label="1 Runde")

    def on_play_n(self) -> None:
        """Die im Dropdown gewählte Anzahl Runden spielen."""
        label = self.rounds_label_var.get()
        rounds = dict(ROUND_CHOICES).get(label, 100)
        self._play_rounds(rounds, label=f"{label} ({rounds}) Runden")

    def _play_rounds(self, rounds: int, label: str) -> None:
        engine = self._require_engine()
        if engine is None or self.snapshot is None:
            messagebox.showinfo("Hinweis", "Bitte zuerst „Neu starten“.")
            return
        self.config(cursor="watch")
        self.update_idletasks()
        t0 = time.perf_counter()
        try:
            snap = engine.play(rounds)
        except EngineError as exc:
            self.config(cursor="")
            messagebox.showerror("Fehler", str(exc))
            self.log(f"FEHLER: {exc}")
            return
        dt = time.perf_counter() - t0
        self.config(cursor="")
        self.snapshot = snap
        self.log(f"{label} gespielt → jetzt Runde {snap.rounds} ({dt:.2f} s, {len(snap.history)} History-Punkte).")
        self._safe_refresh()

    # -- y-Achse umschalten ------------------------------------------------------

    def _yaxis_button_text(self) -> str:
        return f"Y-Achse: {self._y_mode}"

    def _toggle_yaxis(self) -> None:
        """Klickt die y-Achse zyklisch durch: log2 -> linear -> loglog -> log2."""
        self._y_mode = next_y_mode(self._y_mode)
        self.yaxis_button.config(text=self._yaxis_button_text())
        self.log(f"Y-Achse umgeschaltet: {self._y_mode}.")
        self._redraw_chart()

    # -- Anzeige aktualisieren ------------------------------------------------

    def _safe_refresh(self) -> None:
        """Sanfter Abbruch: Anzeigefehler (z. B. beim Zeichnen) nicht durchreichen."""
        try:
            self._refresh()
        except Exception as exc:  # Tk-Callback darf nicht crashen
            self.log(f"WARN: Anzeige konnte nicht aktualisiert werden: {exc}")
            self.status_var.set(f"Anzeige-Fehler (Spielstand ok): {exc}")

    def _refresh(self) -> None:
        snap = self.snapshot
        if snap is None:
            return
        self.stats_var.set(
            f"Runden:          {snap.rounds}\n"
            f"Spieler:         {snap.players}\n"
            f"Seed:            {snap.seed}\n"
            f"min log2:        {fmt_num(snap.min_log2)}\n"
            f"Median log2:     {fmt_num(snap.median_log2)}\n"
            f"Mittel log2:     {fmt_num(snap.mean_log2)}\n"
            f"max log2:        {fmt_num(snap.max_log2)}\n"
            f"Std.-Abw. log2:  {fmt_num(snap.std_dev_log2)}\n"
            f"min Spielstand:  {fmt_score(snap.min_log2)}\n"
            f"geom. Mittel:    {fmt_score(snap.mean_log2)}\n"
            f"max Spielstand:  {fmt_score(snap.max_log2)}"
        )
        # Top 10
        for item in self.top_tree.get_children():
            self.top_tree.delete(item)
        for rank, (idx, log2) in enumerate(snap.top_players[:10], start=1):
            self.top_tree.insert("", "end", values=(rank, idx, fmt_score(log2)))
        # Liniendiagramm (nur Ausläufer, x = Runde, y = Spielstand je Spieler)
        self._redraw_chart()
        self.status_var.set(
            f"Runde {snap.rounds} | {snap.players} Spieler | "
            f"Spannweite log2: {fmt_num(snap.min_log2)} … {fmt_num(snap.max_log2)} | "
            f"Y-Achse: {self._y_mode}"
        )

    def _redraw_chart(self) -> None:
        snap = self.snapshot
        if snap is None:
            return
        # Nur die Ausläufer zeigen: Fenster hängt von der Gesamtrundenzahl ab.
        window = tail_window(snap.rounds)
        hist = snap.history
        if window:
            hist = [s for s in hist if s[0] >= snap.rounds - window] or hist[-1:]
        try:
            self.canvas.set_data(hist, self._y_mode, total_rounds=snap.rounds)
        except Exception as exc:
            # sanfter Abbruch: Diagramm-Fehler loggen, App bleibt benutzbar
            self.log(f"WARN: Diagramm konnte nicht gezeichnet werden: {exc}")
            self.status_var.set(f"Diagramm-Fehler: {exc} (Spielstand ist korrekt)")

    # -- Log / Hilfe ------------------------------------------------------------

    def log(self, msg: str) -> None:
        stamp = time.strftime("%H:%M:%S")
        self.log_text.configure(state="normal")
        self.log_text.insert("end", f"[{stamp}] {msg}\n")
        self.log_text.see("end")
        self.log_text.configure(state="disabled")

    def on_about(self) -> None:
        messagebox.showinfo(
            "Über",
            f"{APP_TITLE}\n\n"
            "N Spieler starten bei Spielstand 1.0. Pro Runde würfelt jeder Spieler\n"
            "reihum; die Augenzahl 1..6 wird auf einen exponentiell steigenden\n"
            "Faktor in [1, sqrt(2)] abgebildet und mit dem Spielstand multipliziert.\n\n"
            "Diagramm: x = Rundennummer, y = Spielstand, eine Linie pro Spieler;\n"
            "gezeigt werden nur die letzten Runden (Ausläufer). Y-Achse per Button\n"
            "umschaltbar: linear / log2 / log-log (doppelt logarithmisch).\n\n"
            "Motor: Rust-Binary „dice-game“ (Projekt ../dice-game).\n"
            "GUI: Python + tkinter (nur Standardbibliothek).",
        )


def main() -> None:
    app = App()
    app.mainloop()


if __name__ == "__main__":
    sys.exit(main())
