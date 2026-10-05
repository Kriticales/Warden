"""Gera os golden logs de `spikes/s-r5-3/fixtures/` a partir das capturas em <dados>/runs.

Para cada fixture: a captura mais recente com o rótulo dado e os padrões dos registros
que interessam (ver `golden.py`). Uso: python make_fixtures.py
"""

import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "fixtures"
RUNS = Path(os.environ.get("WARDEN_SPIKE_DATA", r"C:\wt\s-r5-3\data")) / "runs"

# Linhas de identificação do loader e da versão (entram em todos os recortes do cliente).
IDENT = [r"with Fabric Loader", r"Loading Minecraft .* with Quilt Loader", r"Forge Mod Loader version", r"Launching target '",
         r"(Neo)?Forge mod loading, version", r"Setting user: ", r"Backend library|LWJGL Version"]
READY = [r"Sound engine started", r"blocks-atlas", r"textures-atlas", r"blocks\.png-atlas", r"successfully loaded \d+ mods"]
WORLD = [r"Starting integrated minecraft server", r"Preparing start region", r"logged in with entity id", r"joined the game",
         r"Connecting to ", r"Injecting existing", r"Holder lookups applied", r"Connected to a modded server", r"Loaded \d+ advancements"]
SERVER = [r"Forge Mod Loader version", r"Launching target '", r"(Neo)?Forge mod loading", r"with Fabric Loader", r"Starting minecraft server version",
          r"Preparing start region", r"Done \(", r"logged in with entity id", r"joined the game", r"lost connection", r"Stopping server"]
FAIL = [r"---- Minecraft Crash Report ----", r"#@!@#", r"Exception", r"Error", r"Incompatible mods", r"Mod resolution",
        r"requires", r"missing", r"Missing", r"Duplicate", r"duplicate", r"[Mm]ixin", r"Loading errors", r"crash", r"Crash",
        r"OutOfMemory", r"UnsupportedClassVersion", r"ClassCastException", r"Unable to", r"not supported"]

MATRIX = ["forge-1.7.10", "forge-1.12.2", "forge-1.16.5", "fabric-1.16.5", "forge-1.18.2", "fabric-1.18.2", "forge-1.19.2",
          "fabric-1.19.2", "forge-1.20.1", "neoforge-1.20.1", "fabric-1.20.1", "quilt-1.20.1", "forge-1.21.1", "neoforge-1.21.1",
          "fabric-1.21.1", "forge-26.2", "neoforge-26.2", "fabric-26.2", "vanilla-26.3", "fabric-26.3"]


def latest(tag: str, server: bool = False) -> Path | None:
    pat = f"{tag}-server-1*.log" if server else f"{tag}-1*.log"
    found = sorted(RUNS.glob(pat))
    return found[-1] if found else None


def cut(src: Path | None, dst: str, pats: list[str], max_stack: int = 12) -> None:
    if src is None:
        print(f"!! sem captura para {dst}")
        return
    subprocess.run([sys.executable, str(HERE / "golden.py"), str(src), str(OUT / dst), *pats, "--max-stack", str(max_stack)], check=True)


def main() -> None:
    # Matriz: a rodada válida mais recente de cada combinação (entrou no mundo e, se foi sem
    # onboardAccessibility:false, a tela de boas-vindas não foi fechada por alguém).
    import json
    best = {}
    for line in (RUNS.parent / "matrix.jsonl").read_text(encoding="utf-8").splitlines():
        r = json.loads(line)
        contaminated = not r.get("onboard_off") and r.get("options_onboard_final") == "false"
        if r.get("outcome") == "no-mundo" and not contaminated and r.get("capture"):
            best[r["id"]] = r
    for cid in MATRIX:
        r = best.get(cid)
        if not r:
            print(f"!! sem rodada válida para {cid}")
            continue
        cut(RUNS / r["capture"], f"matriz/{cid}-cliente.log", IDENT + READY + WORLD)
        srv = latest(r["tag"], server=True)
        if srv:
            cut(srv, f"matriz/{cid}-servidor.log", SERVER)
    # Mecanismos
    cut(latest("q-1.12.2-sem-prop", True), "mecanismos/forge-1.12.2-servidor-pergunta-mundo-sem-mod.log",
        [r"Forge Mod Loader version", r"missing registry", r"/fml confirm", r"queryResult", r"Missing", r"comforts"])
    cut(latest("q-1.12.2-confirm", True), "mecanismos/forge-1.12.2-servidor-queryResult-confirm.log",
        [r"Forge Mod Loader version", r"missing registry", r"queryResult", r"Done \("], 40)
    cut(latest("nogui-sem-1.20.1"), "mecanismos/fabric-1.20.1-dependencia-faltando-sem-noGui.log", IDENT + FAIL)
    cut(latest("nogui-com-1.20.1"), "mecanismos/fabric-1.20.1-dependencia-faltando-com-noGui.log", IDENT + FAIL)
    cut(latest("world-missing-neoforge"), "mecanismos/neoforge-1.21.1-mundo-salvo-com-mod-ausente.log",
        IDENT + [r"version differences", r"MISSING", r"Things may not work"] + WORLD)
    cut(latest("qp-neoforge-1.21.1-onboard"), "mecanismos/neoforge-1.21.1-quickplay-mundo.log", IDENT + READY + WORLD)
    cut(latest("mp-legacyargs-1.20.1"), "mecanismos/fabric-1.20.1-server-port-ignorado.log", IDENT + READY + WORLD)
    cut(latest("mp-qpmulti-1.20.1"), "mecanismos/fabric-1.20.1-quickplay-multiplayer.log", IDENT + READY + WORLD)
    cut(latest("1.7.10-forge-1.7.10-10.13.4.1614-1.7.10"), "mecanismos/forge-1.7.10-falso-crash-report-splash.log",
        [r"SplashProgress", r"THIS IS NOT A ERROR", r"Loading screen debug info", r"16x16"], 30)
    # Cenários de falha
    # Rodadas inválidas ou refeitas: forge-entrar(-b) usaram o jar sem pack.mcmeta (a tela de avisos
    # do Forge bloqueou o Quick Play); a válida é forge-entrar-c.
    skip = {"forge-entrar", "forge-entrar-b", "forge-entrar-d", "forge-controle-mundo", "forge-mod-sem-falha", "forge-sem-mcmeta-sem-avisos"}
    rename = {"forge-entrar-c": "forge-entrar", "forge-sem-mcmeta": "forge-avisos-bloqueiam-quickplay"}
    for src in sorted(RUNS.glob("sc-*-1*.log")):
        sid = src.name.rsplit("-", 1)[0][3:]
        if sid in skip or latest(f"sc-{sid}") != src:
            continue
        cut(src, f"falhas/{rename.get(sid, sid)}.log", IDENT + FAIL + WORLD + [r"Missing metadata in pack"], 25)


if __name__ == "__main__":
    main()
