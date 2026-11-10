#!/usr/bin/env python3
"""
dice_game_gui.py – Grafische Oberfläche (GUI) für das Würfel-Experiment „dice-game“.

Die GUI ist nur die Steuerung: Die eigentliche Spielroutine läuft im
kompilierten Rust-Binary (../dice-game/target/release/dice-game), das über
eine Zustandsdatei persistiert und zwischen den Klicks fortgeführt wird.

Aufruf:
    python3 dice_game_gui.py

Nur Python-Standardbibliothek (tkinter) – keine externen Pakete nötig.

Bedienung:
    * Spieleranzahl + Seed einstellen, dann „Neu starten“.
    * „1 Runde spielen“ spielt genau eine Runde.
    * „N Runden spielen“ spielt die im Dropdown gewählte Anzahl
      (1, 10, 100, 1k, 1M) auf einen Klick.
    * Das Histogramm zeigt die Verteilung der Spielstände (log2-Raum),
      rechts daneben Statistik und Top 10, unten ein Log.
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
# Konstanten
# ---------------------------------------------------------------------------

APP_TITLE = "Dice Game – Exponentielles Würfelexperiment"

#: Dropdown-Werte für „N Runden spielen“ (Anzeige, tatsächliche Rundenzahl).
ROUND_CHOICES = [("1", 1), ("10", 10), ("100", 100), ("1k", 1_000), ("1M", 1_000_000)]

DEFAULT_PLAYERS = 100
DEFAULT_SEED = 42

#: Zustandsdatei, über die das GUI das Rust-Binary fortführt.
STATE_FILE = os.path.join(tempfile.gettempdir(), "dice_game_state.txt")

#: Anzahl der Balken im Histogramm.
HISTOGRAM_BINS = 40

#: Ab welchem log2-Wert die Anzeige auf „2^x“ umschaltet (f64-Grenze ~2^1024).
SCORE_DISPLAY_LIMIT = 1000.0


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


# ---------------------------------------------------------------------------
# Engine-Anbindung an das Rust-Binary
# ---------------------------------------------------------------------------


class EngineError(RuntimeError):
    """Wird geworfen, wenn das Rust-Binary fehlt oder einen Fehler meldet."""


class Snapshot:
    """Aktueller Spielstand, aus der maschinenlesbaren Binary-Ausgabe geparst."""

    def __init__(self, rounds: int, players: int, seed: int, log2_scores: list[float]):
        self.rounds = rounds
        self.players = players
        self.seed = seed
        self.log2_scores = log2_scores  # in Spielerreihenfolge

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
    if rounds is None or players is None or seed is None:
        raise EngineError(f"Ausgabe enthält keine vollständigen meta-Zeilen:\n{text[:300]}")
    scores.sort(key=lambda t: t[0])
    if len(scores) != players:
        raise EngineError(f"erwartet {players} Spieler, Ausgabe enthält {len(scores)}")
    return Snapshot(rounds, players, seed, [s for _, s in scores])


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
        """Aktuellen Stand lesen (ohne zu spielen)."""
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


# ---------------------------------------------------------------------------
# Histogramm-Leinwand
# ---------------------------------------------------------------------------


class HistogramCanvas(tk.Canvas):
    """Zeichnet die Verteilung der log2-Spielstände als Balkendiagramm."""

    def __init__(self, master, **kw):
        super().__init__(master, bg="white", highlightthickness=1, highlightbackground="#999", **kw)
        self._data: list[float] = []
        self._log_y = False
        # Neu zeichnen, sobald die Größe feststeht.
        self.bind("<Configure>", lambda _e: self.redraw())

    def set_data(self, log2_scores: list[float], log_y: bool) -> None:
        self._data = sorted(log2_scores)
        self._log_y = log_y
        self.redraw()

    def redraw(self) -> None:
        self.delete("all")
        w = max(self.winfo_width(), 300)
        h = max(self.winfo_height(), 200)
        pad_l, pad_r, pad_t, pad_b = 70, 15, 30, 45
        cw, ch = w - pad_l - pad_r, h - pad_t - pad_b  # Zeichenfläche

        # Achsen
        x0, y0 = pad_l, pad_t + ch  # Ursprung (links unten)
        self.create_line(pad_l, pad_t, pad_l, y0, fill="black")
        self.create_line(pad_l, y0, pad_l + cw, y0, fill="black")
        self.create_text(pad_l + cw / 2, h - 12, text="log2(Spielstand)", anchor="n")
        self.create_text(14, pad_t + ch / 2, text="Anzahl Spieler", angle=90, anchor="center")

        if not self._data:
            self.create_text(
                pad_l + cw / 2, pad_t + ch / 2,
                text="Noch keine Daten – bitte „Neu starten“ klicken.",
                fill="#666",
            )
            return

        lo, hi = self._data[0], self._data[-1]
        if hi <= lo:
            hi = lo + 1.0  # entarteter Fall (z. B. nach 0 Runden)

        # Bins über [lo, hi]
        bins = min(HISTOGRAM_BINS, max(5, len(self._data) // 2))
        counts = [0] * bins
        for v in self._data:
            idx = int((v - lo) / (hi - lo) * bins)
            counts[min(idx, bins - 1)] += 1

        max_count = max(counts)
        # y-Achse wahlweise logarithmisch (damit kleine Balken sichtbar bleiben).
        def y_scale(c: int) -> float:
            if self._log_y:
                return math.log10(c + 1) / math.log10(max_count + 1)
            return c / max_count if max_count else 0.0

        bar_w = cw / bins
        for i, c in enumerate(counts):
            if c == 0:
                continue
            bh = y_scale(c) * ch
            x1 = pad_l + i * bar_w
            self.create_rectangle(
                x1, y0 - bh, x1 + bar_w - 1, y0,
                fill="#4C78A8", outline="#2f4f73",
            )

        # x-Achsen-Beschriftung: 5 Ticks
        for k in range(5):
            frac = k / 4
            xv = pad_l + frac * cw
            val = lo + frac * (hi - lo)
            self.create_line(xv, y0, xv, y0 + 5, fill="black")
            self.create_text(xv, y0 + 10, text=fmt_num(val), anchor="n")

        # y-Achsen-Beschriftung: 0 und Maximum
        self.create_text(pad_l - 6, y0, text="0", anchor="e")
        self.create_text(pad_l - 6, pad_t, text=str(max_count), anchor="e")
        if self._log_y:
            self.create_text(pad_l - 6, pad_t + 12, text="(log10)", anchor="e", fill="#666")

        # Titel mit Rundenzahl
        self.create_text(
            pad_l + cw / 2, 12,
            text=f"Verteilung der Spielstände nach {self._rounds_text} Runden",
            anchor="n",
        )

    # Die Rundenzahl für den Titel wird von außen gesetzt.
    _rounds_text = "?"

    def set_rounds_text(self, text: str) -> None:
        self._rounds_text = text
        self.redraw()


# ---------------------------------------------------------------------------
# Hauptfenster
# ---------------------------------------------------------------------------


class App(tk.Tk):
    """Hauptfenster der Anwendung."""

    def __init__(self) -> None:
        super().__init__()
        self.title(APP_TITLE)
        self.geometry("1100x700")
        self.minsize(850, 550)

        self.engine: DiceGameEngine | None = None
        self.snapshot: Snapshot | None = None

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
        menubar = tk.Menu(self)
        menu_file = tk.Menu(menubar, tearoff=0)
        menu_file.add_command(label="Neu starten", command=self.on_new, accelerator="Strg+N")
        menu_file.add_command(label="Binary wechseln…", command=self.on_browse_binary)
        menu_file.add_separator()
        menu_file.add_command(label="Beenden", command=self.destroy, accelerator="Strg+Q")
        menubar.add_cascade(label="Datei", menu=menu_file)
        menu_help = tk.Menu(menubar, tearoff=0)
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

        ttk.Button(bar, text="Neu starten", command=self.on_new).pack(side="left", padx=(0, 10))

        ttk.Separator(bar, orient="vertical").pack(side="left", fill="y", padx=6)

        ttk.Button(bar, text="1 Runde spielen", command=self.on_play_one).pack(side="left")
        ttk.Button(bar, text="N Runden spielen", command=self.on_play_n).pack(side="left", padx=(6, 2))
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
        main = ttk.Frame(self, padding=6)
        main.pack(side="top", fill="both", expand=True)

        # Linke Seite: Histogramm + Optionen darunter
        left = ttk.Frame(main)
        left.pack(side="left", fill="both", expand=True)
        self.canvas = HistogramCanvas(left)
        self.canvas.pack(side="top", fill="both", expand=True)
        opts = ttk.Frame(left)
        opts.pack(side="bottom", fill="x", pady=(4, 0))
        self.log_y_var = tk.BooleanVar(value=False)
        ttk.Checkbutton(
            opts, text="logarithmische y-Achse", variable=self.log_y_var,
            command=self._redraw_chart,
        ).pack(side="left")
        ttk.Label(opts, text="(Bins im log2-Raum;Balken = Anzahl Spieler)").pack(side="left", padx=8)

        # Rechte Seite: Statistik + Top 10 + Log
        right = ttk.Frame(main, width=360)
        right.pack(side="right", fill="y", padx=(8, 0))
        right.pack_propagate(False)

        stats_box = ttk.LabelFrame(right, text="Statistik", padding=6)
        stats_box.pack(side="top", fill="x")
        self.stats_var = tk.StringVar(value="–")
        ttk.Label(stats_box, textvariable=self.stats_var, justify="left", font=("TkFixedFont", 9)).pack(anchor="w")

        top_box = ttk.LabelFrame(right, text="Top 10 Spieler", padding=6)
        top_box.pack(side="top", fill="both", expand=True, pady=(6, 0))
        columns = ("rang", "spieler", "spielstand")
        self.top_tree = ttk.Treeview(top_box, columns=columns, show="headings", height=10)
        self.top_tree.heading("rang", text="Rang")
        self.top_tree.heading("spieler", text="Spieler")
        self.top_tree.heading("spielstand", text="Spielstand")
        self.top_tree.column("rang", width=45, anchor="e")
        self.top_tree.column("spieler", width=60, anchor="e")
        self.top_tree.column("spielstand", width=170, anchor="e")
        vsb = ttk.Scrollbar(top_box, orient="vertical", command=self.top_tree.yview)
        self.top_tree.configure(yscrollcommand=vsb.set)
        self.top_tree.pack(side="left", fill="both", expand=True)
        vsb.pack(side="right", fill="y")

        log_box = ttk.LabelFrame(right, text="Log", padding=6)
        log_box.pack(side="top", fill="both", expand=True, pady=(6, 0))
        self.log_text = tk.Text(log_box, height=8, wrap="word", state="disabled")
        lsb = ttk.Scrollbar(log_box, orient="vertical", command=self.log_text.yview)
        self.log_text.configure(yscrollcommand=lsb.set)
        self.log_text.pack(side="left", fill="both", expand=True)
        lsb.pack(side="right", fill="y")

    def _build_statusbar(self) -> None:
        self.status_var = tk.StringVar(value="Bereit.")
        ttk.Label(self, textvariable=self.status_var, relief="sunken", anchor="w", padding=(6, 2)).pack(
            side="bottom", fill="x"
        )

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
        self._refresh()

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
        self.log(f"{label} gespielt → jetzt Runde {snap.rounds} ({dt:.2f} s).")
        self._refresh()

    # -- Anzeige aktualisieren ------------------------------------------------

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
        # Histogramm
        self.canvas.set_rounds_text(str(snap.rounds))
        self._redraw_chart()
        self.status_var.set(
            f"Runde {snap.rounds} | {snap.players} Spieler | "
            f"Spannweite log2: {fmt_num(snap.min_log2)} … {fmt_num(snap.max_log2)}"
        )

    def _redraw_chart(self) -> None:
        if self.snapshot is not None:
            self.canvas.set_data(self.snapshot.log2_scores, self.log_y_var.get())

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
            "Motor: Rust-Binary „dice-game“ (Projekt ../dice-game).\n"
            "GUI: Python + tkinter (nur Standardbibliothek).",
        )


def main() -> None:
    app = App()
    app.mainloop()


if __name__ == "__main__":
    sys.exit(main())
