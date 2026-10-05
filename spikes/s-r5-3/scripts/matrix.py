"""Matriz de versões do spike S-R5-3: para cada combinação, abre o cliente e espera
"entrou no mundo" (que inclui "cliente pronto").

- 1.20+: mundo gerado por um servidor vanilla da mesma versão, copiado para `saves/`,
  e `--quickPlaySingleplayer` + `--quickPlayPath`.
- antes de 1.20: servidor dedicado local com o mesmo loader e `--server/--port`.

Cada combinação roda primeiro sem `options.txt` (como uma instância nova). Se não
entrar no mundo, roda de novo com `onboardAccessibility:false`.

Uso: python matrix.py [filtro...]   (filtro = trecho do id, ex.: 1.18.2)
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

# (id, mc, loader do cliente, servidor: (tipo, versão) ou None para Quick Play)
MATRIX = [
    ("forge-1.7.10", "1.7.10", "forge:1.7.10-10.13.4.1614-1.7.10", ("forge", "1.7.10-10.13.4.1614-1.7.10")),
    ("forge-1.12.2", "1.12.2", "forge:1.12.2-14.23.5.2860", ("forge", "1.12.2-14.23.5.2860")),
    ("forge-1.16.5", "1.16.5", "forge:1.16.5-36.2.34", ("forge", "1.16.5-36.2.34")),
    ("fabric-1.16.5", "1.16.5", "fabric", ("fabric", "1.16.5")),
    ("forge-1.18.2", "1.18.2", "forge:1.18.2-40.3.0", ("forge", "1.18.2-40.3.0")),
    ("fabric-1.18.2", "1.18.2", "fabric", ("fabric", "1.18.2")),
    ("forge-1.19.2", "1.19.2", "forge:1.19.2-43.5.0", ("forge", "1.19.2-43.5.0")),
    ("fabric-1.19.2", "1.19.2", "fabric", ("fabric", "1.19.2")),
    ("forge-1.20.1", "1.20.1", "forge:1.20.1-47.4.10", None),
    ("neoforge-1.20.1", "1.20.1", "neoforge:1.20.1-47.1.106", None),
    ("fabric-1.20.1", "1.20.1", "fabric", None),
    ("quilt-1.20.1", "1.20.1", "quilt", None),
    ("forge-1.21.1", "1.21.1", "forge:1.21.1-52.1.0", None),
    ("neoforge-1.21.1", "1.21.1", "neoforge:21.1.252", None),
    ("fabric-1.21.1", "1.21.1", "fabric", None),
    ("forge-26.2", "26.2", "forge:26.2-65.1.0", None),
    ("neoforge-26.2", "26.2", "neoforge:26.2.0.88", None),
    ("fabric-26.2", "26.2", "fabric", None),
    ("vanilla-26.3", "26.3", "vanilla", None),
    ("fabric-26.3", "26.3", "fabric", None),
]

PORTS = {}


def run(args: list[str], timeout: int = 1800) -> tuple[int, str]:
    env = dict(os.environ, ALSOFT_DRIVERS="null")
    p = subprocess.run([str(BIN), *args], capture_output=True, text=True, errors="replace", env=env, timeout=timeout)
    return p.returncode, p.stdout + p.stderr


def java_for(mc: str, loader: str, inst: str) -> str:
    code, out = run(["run", mc, loader, "--instance", inst, "--install-only"])
    m = re.search(r"== java-console (.+)", out)
    if not m:
        raise SystemExit(f"instalação falhou para {mc} {loader}:\n{out[-2000:]}")
    return m.group(1).strip()


def prep_server(kind: str, ver: str, java: str, port: int) -> Path:
    d = DATA / "servers" / f"m-{kind}-{ver}"
    if not (d / "warden-args.txt").exists():
        args = [sys.executable, str(HERE / "prep_server.py"), kind, ver, str(d), "--port", str(port)]
        if kind in ("forge", "neoforge"):
            args += ["--java", java]
        subprocess.run(args, check=True)
    return d


def vanilla_world(mc: str, java: str, port: int) -> Path:
    d = DATA / "servers" / f"vanilla-{mc}"
    if not (d / "world" / "level.dat").exists():
        if not (d / "warden-args.txt").exists():
            subprocess.run([sys.executable, str(HERE / "prep_server.py"), "vanilla", mc, str(d), "--port", str(port)], check=True)
        code, out = run(["server", str(d), "--java", java, "--tag", f"gen-{mc}", "--timeout", "600"])
        if code != 0:
            raise SystemExit(f"servidor vanilla {mc} falhou:\n{out[-2000:]}")
    return d / "world"


def summary(out: str) -> dict:
    m = re.search(r"== resumo (\{.*\})", out)
    return json.loads(m.group(1)) if m else {"outcome": "sem-resumo", "raw": out[-1500:]}


def one(cid: str, mc: str, loader: str, server, onboard_off: bool, port: int) -> dict:
    inst = f"m-{cid}"
    idir = DATA / "instances" / inst
    java = java_for(mc, loader, inst)
    opt = idir / "options.txt"
    if opt.exists():
        opt.unlink()
    # MATRIX_TAG_SUFFIX separa rodadas refeitas (ex.: "-limpa", sem clique humano na janela).
    tag = f"mx-{cid}" + ("-onboard" if onboard_off else "") + os.environ.get("MATRIX_TAG_SUFFIX", "")
    args = ["run", mc, loader, "--instance", inst, "--goal", "world", "--hold", "5", "--timeout", "240", "--tag", tag]
    if onboard_off:
        args += ["--option", "onboardAccessibility:false"]
    if server:
        sdir = prep_server(server[0], server[1], java, port)
        args += ["--server-dir", str(sdir), "--legacy-server", f"127.0.0.1:{port}"]
    else:
        world = vanilla_world(mc, java, port)
        dst = idir / "saves" / "wardentest"
        if dst.exists():
            shutil.rmtree(dst)
        shutil.copytree(world, dst, ignore=shutil.ignore_patterns("session.lock"))
        qp = idir / "quickplay.json"
        if qp.exists():
            qp.unlink()
        args += ["--world", "wardentest", "--qp-path", "quickplay.json"]
    with InputWatch() as w:
        code, out = run(args)
    (DATA / "runs" / f"{tag}.out.txt").write_text(out, encoding="utf-8")
    s = summary(out)
    s["java"] = java
    s.update(w.summary())
    # Ao fechar a tela de boas-vindas, o jogo grava onboardAccessibility:false (indício de clique).
    m2 = re.search(r"onboardAccessibility:(\w+)", opt.read_text(encoding="utf-8")) if opt.exists() else None
    s["options_onboard_final"] = m2.group(1) if m2 else None
    return s


def save(r: dict) -> None:
    if os.environ.get("MATRIX_TAG_SUFFIX"):
        r["serie"] = os.environ["MATRIX_TAG_SUFFIX"].strip("-")
    with open(DATA / "matrix.jsonl", "a", encoding="utf-8") as f:
        f.write(json.dumps(r, ensure_ascii=False) + "\n")


def main() -> None:
    filters = sys.argv[1:]
    for i, (cid, mc, loader, server) in enumerate(MATRIX):
        if filters and not any(f in cid for f in filters):
            continue
        port = 25610 + i
        s = one(cid, mc, loader, server, False, port)
        print(f"{cid}: {s.get('outcome')} pronto={s.get('ready')} mundo={s.get('world')} qp={s.get('qp_file')}", flush=True)
        save({"id": cid, "onboard_off": False, **s})
        if s.get("outcome") != "no-mundo":
            s2 = one(cid, mc, loader, server, True, port)
            print(f"{cid} (onboardAccessibility:false): {s2.get('outcome')} pronto={s2.get('ready')} mundo={s2.get('world')}", flush=True)
            save({"id": cid, "onboard_off": True, **s2})


if __name__ == "__main__":
    main()
