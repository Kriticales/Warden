"""Detecta entrada humana (teclado ou mouse, em qualquer janela) durante uma rodada.

Usa `GetLastInputInfo` do Windows: o contador muda a cada evento de entrada. Serve para
marcar como contaminada uma medição em que alguém mexeu na máquina (ex.: clicou em
"Continue" na tela de boas-vindas do jogo). Não registra o que foi digitado nem onde.
"""

import ctypes
import os
import threading
import time


class _LASTINPUTINFO(ctypes.Structure):
    _fields_ = [("cbSize", ctypes.c_uint), ("dwTime", ctypes.c_uint)]


def _last_input_tick() -> int | None:
    if os.name != "nt":
        return None
    info = _LASTINPUTINFO()
    info.cbSize = ctypes.sizeof(info)
    if not ctypes.windll.user32.GetLastInputInfo(ctypes.byref(info)):
        return None
    return info.dwTime


class InputWatch:
    """`with InputWatch() as w: ...`; depois `w.events` = segundos (desde o início) com entrada."""

    def __init__(self, interval: float = 0.5) -> None:
        self.interval = interval
        self.events: list[float] = []
        self._stop = threading.Event()

    def __enter__(self) -> "InputWatch":
        self._t0 = time.monotonic()
        self._last = _last_input_tick()
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()
        return self

    def _run(self) -> None:
        while not self._stop.wait(self.interval):
            cur = _last_input_tick()
            if cur is not None and cur != self._last:
                self._last = cur
                self.events.append(round(time.monotonic() - self._t0, 1))

    def __exit__(self, *exc) -> None:
        self._stop.set()
        self._thread.join()

    def summary(self) -> dict:
        ev = self.events
        return {"entrada_humana": bool(ev), "entrada_primeira_s": ev[0] if ev else None, "entrada_eventos": len(ev)}
