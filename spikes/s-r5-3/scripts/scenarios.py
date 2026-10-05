"""Cenários de falha reais do spike S-R5-3.

Cada cenário monta uma instância própria (`instances/s-<id>`) só com os mods dele,
abre o cliente com o protótipo e grava o resumo em `<dados>/scenarios.jsonl`.
Os mods vêm do Modrinth (id da versão) ou do mod de teste `wardenfalhas`
(compilado por `build_testmod.py` em `<dados>/testmod`).

Uso: python scenarios.py [filtro...]
"""

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

from entrada import InputWatch

HERE = Path(__file__).resolve().parent
DATA = Path(os.environ.get("WARDEN_SPIKE_DATA", r"C:\wt\s-r5-3\data"))
BIN = Path(os.environ.get("BISECT_PROTO", r"C:\wt\s-r5-3\target\release\bisect-proto.exe"))
JARS = DATA / "jars"
RT = DATA / "shared" / "runtimes"

# Versões do Modrinth usadas (id da versão -> o que é).
M = {
    "fapi": "rvI2dfzR",           # Fabric API 0.92.12+1.20.1
    "fapi-velha": "rSrmGeeJ",     # Fabric API 0.83.0+1.20.1
    "comforts-fab": "pMr60Kkq",   # Comforts 6.4.0 (Fabric 1.20.1), exige fabric-api
    "comforts-fab-2": "D2F8i5te", # Comforts 6.3.5 (Fabric 1.20.1)
    "comforts-forge": "gBDsc134", # Comforts 6.4.0 (Forge 1.20.1)
    "comforts-neo": "3kpPjcTc",   # Comforts 9.0.5 (NeoForge 1.21.1)
    "comforts-neo-2": "f0FgfKAZ", # Comforts 9.0.4 (NeoForge 1.21.1)
    "comforts-fab-121": "LUPOTXbk",  # Comforts 9.0.5 (Fabric 1.21.1)
    "supp-fab": "i7ejA878",       # Supplementaries 3.1.43 (Fabric 1.20.1): fabric >=0.92.0, moonlight
    "moon-fab": "THA4nqpC",       # Moonlight 2.16.35 (Fabric 1.20.1)
    "supp-forge": "S0TIJ1hU",     # Supplementaries 3.1.43 (Forge 1.20.1), exige moonlight
    "moon-forge": "W0ZWjZib",     # Moonlight 2.16.35 (Forge 1.20.1)
    "supp-neo": "WrZWfRjP",       # Supplementaries 3.9.9 (NeoForge 1.21.1), moonlight >= 1.21-3.6.4
    "moon-neo": "t6iFh4M3",       # Moonlight 3.7.0 (NeoForge 1.21.1)
    "moon-neo-velho": "uPJlV3eS", # Moonlight 3.6.3 (NeoForge 1.21.1), abaixo do exigido
    "quark-112": "MPJKDJmI",      # Quark r1.6-179 (Forge 1.12.2), exige AutoRegLib
    "comforts-112": "dHrSKL4J",   # Comforts 1.4.1.3 (Forge 1.12.2)
}

F120 = ("1.20.1", "fabric:0.19.5")  # versão fixa: sem consulta ao meta.fabricmc.net a cada abertura
G120 = ("1.20.1", "forge:1.20.1-47.4.10")
N121 = ("1.21.1", "neoforge:21.1.252")
G112 = ("1.12.2", "forge:1.12.2-14.23.5.2860")

# id, (mc, loader), mods, objetivo, argumentos extras, tempo limite
SCEN = [
    ("fab-dep-faltando", F120, ["comforts-fab"], "menu", [], 60),
    ("fab-dep-faltando-nogui", F120, ["comforts-fab"], "menu", ["--prop", "fabric.noGui=true"], 60),
    ("fab-dep-versao", F120, ["supp-fab", "moon-fab", "fapi-velha"], "menu", ["--prop", "fabric.noGui=true"], 60),
    ("fab-outro-loader", F120, ["fapi", "comforts-forge"], "menu", ["--prop", "fabric.noGui=true"], 120),
    ("fab-outra-versao-mc", F120, ["fapi", "comforts-fab-121"], "menu", ["--prop", "fabric.noGui=true"], 60),
    ("fab-duplicado", F120, ["fapi", "comforts-fab", "comforts-fab-2"], "menu", ["--prop", "fabric.noGui=true"], 60),
    ("fab-mixin", F120, ["@fabric"], "menu", ["--prop", "fabric.noGui=true", "--prop", "warden.falha=mixin"], 120),
    ("fab-iniciar", F120, ["@fabric"], "menu", ["--prop", "fabric.noGui=true", "--prop", "warden.falha=iniciar"], 120),
    ("fab-entrar", F120, ["@fabric"], "world", ["--prop", "fabric.noGui=true", "--prop", "warden.falha=entrar"], 180),
    ("fab-travar", F120, ["@fabric"], "world", ["--prop", "fabric.noGui=true", "--prop", "warden.falha=travar"], 240),
    ("fab-memoria", F120, ["fapi"], "world", ["--prop", "fabric.noGui=true", "--jvm-arg", "-Xmx160M"], 240),
    ("fab-java8", F120, [], "menu", ["--java", str(RT / "jre-legacy" / "bin" / "javaw.exe")], 60),
    ("neo-dep-faltando", N121, ["supp-neo"], "menu", [], 120),
    ("neo-dep-versao", N121, ["supp-neo", "moon-neo-velho"], "menu", [], 120),
    ("neo-outro-loader", N121, ["comforts-fab-121"], "menu", [], 120),
    ("neo-duplicado", N121, ["comforts-neo", "comforts-neo-2"], "menu", [], 120),
    ("neo-mixin", N121, ["@neoforge"], "menu", ["--prop", "warden.falha=mixin"], 120),
    ("neo-iniciar", N121, ["@neoforge"], "menu", ["--prop", "warden.falha=iniciar"], 120),
    ("neo-entrar", N121, ["@neoforge"], "world", ["--prop", "warden.falha=entrar"], 180),
    ("neo-travar", N121, ["@neoforge"], "world", ["--prop", "warden.falha=travar"], 240),
    ("neo-memoria", N121, ["supp-neo", "moon-neo"], "world", ["--jvm-arg", "-Xmx256M"], 240),
    ("neo-java17", N121, [], "menu", ["--java", str(RT / "java-runtime-gamma" / "bin" / "javaw.exe")], 60),
    ("forge-dep-faltando", G120, ["supp-forge"], "menu", [], 120),
    ("forge-outro-loader", G120, ["comforts-fab"], "menu", [], 120),
    ("forge-duplicado", G120, ["comforts-forge", "comforts-forge"], "menu", [], 120),
    ("forge-mixin", G120, ["@forge"], "menu", ["--prop", "warden.falha=mixin"], 120),
    ("forge-entrar", G120, ["@forge"], "world", ["--prop", "warden.falha=entrar"], 180),
    ("f112-dep-faltando", G112, ["quark-112"], "menu", [], 120),
    ("f112-duplicado", G112, ["comforts-112", "comforts-112"], "menu", [], 120),
    ("f112-java21", G112, [], "menu", ["--java", str(RT / "java-runtime-delta" / "bin" / "javaw.exe")], 60),
]


def fetch(key: str) -> list[Path]:
    """Devolve o(s) jar(s) de uma chave; baixa do Modrinth se faltar."""
    if key.startswith("@"):
        return [DATA / "testmod" / f"wardenfalhas-{key[1:]}.jar"]
    d = JARS / key
    if not d.exists() or not any(d.glob("*.jar")):
        subprocess.run([sys.executable, str(HERE / "modrinth.py"), "version", M[key], str(d)], check=True)
    return sorted(d.glob("*.jar"))


def main() -> None:
    filters = sys.argv[1:]
    for sid, (mc, loader), mods, goal, extra, timeout in SCEN:
        if filters and not any(f in sid for f in filters):
            continue
        inst = DATA / "instances" / f"s-{sid}"
        if inst.exists():
            shutil.rmtree(inst)
        (inst / "mods").mkdir(parents=True)
        for i, key in enumerate(mods):
            for jar in fetch(key):
                # Duplicado do mesmo arquivo: segunda cópia com outro nome.
                name = jar.name if (inst / "mods" / jar.name).exists() is False else f"copia-{i}-{jar.name}"
                shutil.copy2(jar, inst / "mods" / name)
        args = ["run", mc, loader, "--instance", inst.name, "--goal", goal, "--timeout", str(timeout), "--hold", "3",
                "--tag", f"sc-{sid}", "--option", "onboardAccessibility:false", *extra]
        if goal == "world":
            world = DATA / "servers" / f"vanilla-{mc}" / "world"
            shutil.copytree(world, inst / "saves" / "wardentest", ignore=shutil.ignore_patterns("session.lock"))
            args += ["--world", "wardentest"]
        env = dict(os.environ, ALSOFT_DRIVERS="null")
        with InputWatch() as w:
            p = subprocess.run([str(BIN), *args], capture_output=True, text=True, errors="replace", env=env)
        out = p.stdout + p.stderr
        (DATA / "runs" / f"sc-{sid}.out.txt").write_text(out, encoding="utf-8")
        m = re.search(r"== resumo (\{.*\})", out)
        s = json.loads(m.group(1)) if m else {"outcome": "sem-resumo"}
        s["id"] = sid
        s["mods"] = mods
        s.update(w.summary())
        print(f"{sid}: {s.get('outcome')} fatal={s.get('fatal')} pronto={s.get('ready')} mundo={s.get('world')} "
              f"ultima_linha={s.get('last_line')} procs={s.get('procs_in_job')}", flush=True)
        with open(DATA / "scenarios.jsonl", "a", encoding="utf-8") as f:
            f.write(json.dumps(s, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
