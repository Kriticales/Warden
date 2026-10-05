"""Busca do culpado de ponta a ponta (spike S-R5-3), no desenho da R5A §3.2.

- grafo de dependências obrigatórias lido dos jars (`neoforge.mods.toml`/`mods.toml`,
  `fabric.mod.json`, inclusive jars embutidos em `META-INF/jarjar` e `META-INF/jars`);
- ordem topológica estável (bibliotecas primeiro, depois nome do arquivo);
- rodada 0 (pack inteiro, grava a assinatura), rodada 1 (sem mods), busca binária no
  tamanho do prefixo, minimização para pares e confirmação;
- cada rodada recria `mods/` com links físicos, apaga `config/` e copia o mundo de teste.

Uso:
  python busca_culpado.py grafo <pasta de mods>                 só imprime a ordem e o grafo
  python busca_culpado.py rodada <pasta de mods> <rótulo> [n]   uma rodada com o pack inteiro (ou prefixo n)
  python busca_culpado.py busca <pasta de mods>                 a busca completa
"""

import heapq
import io
import json
import os
import re
import shutil
import subprocess
import sys
import time
import tomllib
import zipfile
from pathlib import Path

from entrada import InputWatch

DATA = Path(os.environ.get("WARDEN_SPIKE_DATA", r"C:\wt\s-r5-3\data"))
BIN = Path(os.environ.get("BISECT_PROTO", r"C:\wt\s-r5-3\target\release\bisect-proto.exe"))
MC, LOADER = "1.21.1", "neoforge:21.1.236"
INST = DATA / "instances" / "pack-bisect"
WORLD = DATA / "servers" / "vanilla-1.21.1" / "world"
LOG = DATA / "bisect-rounds.jsonl"
IGNORED = {"minecraft", "neoforge", "forge", "java", "fabricloader", "fabric", "javafml", "mcp"}
HOLD = int(os.environ.get("BISECT_HOLD", "20"))
TIMEOUT = int(os.environ.get("BISECT_TIMEOUT", "480"))


# ---------------------------------------------------------------- grafo

def _meta(z: zipfile.ZipFile) -> tuple[set[str], set[str]]:
    """(ids fornecidos, ids exigidos) de um jar, recursivo nos jars embutidos."""
    provides, requires = set(), set()
    names = set(z.namelist())
    for toml_name in ("META-INF/neoforge.mods.toml", "META-INF/mods.toml"):
        if toml_name in names:
            try:
                t = tomllib.loads(z.read(toml_name).decode("utf-8", "replace"))
            except Exception:
                t = {}
            for m in t.get("mods", []):
                if "modId" in m:
                    provides.add(m["modId"])
            for _, deps in (t.get("dependencies") or {}).items():
                for d in deps if isinstance(deps, list) else []:
                    # NeoForge: sem "type", a dependência é obrigatória; Forge usa "mandatory".
                    if "type" in d:
                        req = d["type"].lower() == "required"
                    else:
                        req = d.get("mandatory", True) is True
                    if req and "modId" in d:
                        requires.add(d["modId"])
    if "fabric.mod.json" in names:
        try:
            j = json.loads(z.read("fabric.mod.json").decode("utf-8", "replace"), strict=False)
            provides.add(j["id"])
            provides.update(p if isinstance(p, str) else p.get("id", "") for p in j.get("provides", []))
            requires.update((j.get("depends") or {}).keys())
        except Exception:
            pass
    for n in names:
        if (n.startswith("META-INF/jarjar/") or n.startswith("META-INF/jars/")) and n.endswith(".jar"):
            try:
                p, _ = _meta(zipfile.ZipFile(io.BytesIO(z.read(n))))
                provides |= p  # dependências de um embutido são resolvidas por quem o embute
            except Exception:
                pass
    return provides, requires


def graph(mods_dir: Path):
    jars = sorted(p.name for p in mods_dir.glob("*.jar"))
    provides, requires = {}, {}
    for j in jars:
        with zipfile.ZipFile(mods_dir / j) as z:
            provides[j], requires[j] = _meta(z)
    owner = {}
    for j in jars:
        for pid in provides[j]:
            owner.setdefault(pid, j)
    deps, missing = {j: set() for j in jars}, set()
    for j in jars:
        for r in requires[j] - IGNORED:
            o = owner.get(r)
            if o is None:
                missing.add(r)
            elif o != j:
                deps[j].add(o)
    graph.owner = owner
    return jars, deps, missing


def topo(jars, deps):
    dependents = {j: set() for j in jars}
    for j, ds in deps.items():
        for d in ds:
            dependents[d].add(j)
    indeg = {j: len(deps[j]) for j in jars}
    key = lambda j: (0 if dependents[j] else 1, j.lower(), j)
    heap = [key(j) for j in jars if indeg[j] == 0]
    heapq.heapify(heap)
    order = []
    while heap:
        _, _, j = heapq.heappop(heap)
        order.append(j)
        for k in dependents[j]:
            indeg[k] -= 1
            if indeg[k] == 0:
                heapq.heappush(heap, key(k))
    if len(order) != len(jars):
        # Ciclo: os restantes entram juntos no fim (na R5A viram um nó só).
        order += sorted(set(jars) - set(order), key=str.lower)
    return order, dependents


def closure(items, deps):
    out, stack = set(), list(items)
    while stack:
        j = stack.pop()
        if j not in out:
            out.add(j)
            stack.extend(deps[j])
    return out


def up_closure(items, dependents):
    return closure(items, dependents)


# ---------------------------------------------------------------- rodada

ANCHORS = [
    ("entrar", "Couldn't place player in world"),
    ("crash", "---- Minecraft Crash Report ----"),
    ("pre-carregamento", "Error during pre-loading phase"),
    ("dependencias", "Missing or unsupported mandatory dependencies"),
    ("mixin", "Mixin apply"),
    ("fabric-incompativel", "Incompatible mods found!"),
    ("memoria", "java.lang.OutOfMemoryError"),
]
EXC = re.compile(r"((?:[a-z_$][\w$]*\.)+[A-Z][\w$]*(?:Exception|Error)\b[^\r\n\]]*)")


def signature(capture: Path) -> str:
    """Assinatura da falha: a primeira âncora conhecida + a primeira exceção a partir dela, sem números."""
    lines = capture.read_text(encoding="utf-8", errors="replace").splitlines()
    for name, anchor in ANCHORS:
        for i, line in enumerate(lines):
            if anchor in line:
                for nxt in lines[i:i + 40]:
                    m = EXC.search(nxt)
                    if m and "InvalidCredentials" not in m.group(1):
                        return re.sub(r"\d+", "#", f"{name} | {m.group(1).strip()}")[:300]
                return f"{name} | ?"
    return "desconhecida"


def run_round(mods_dir: Path, subset: list[str], label: str) -> dict:
    t0 = time.monotonic()
    md = INST / "mods"
    if md.exists():
        shutil.rmtree(md)
    md.mkdir(parents=True)
    for j in subset:
        try:
            os.link(mods_dir / j, md / j)
        except OSError:
            shutil.copy2(mods_dir / j, md / j)
    for d in ("config", "saves", "crash-reports", "logs", "defaultconfigs"):
        if (INST / d).exists():
            shutil.rmtree(INST / d)
    shutil.copytree(WORLD, INST / "saves" / "wardentest", ignore=shutil.ignore_patterns("session.lock"))
    prep = time.monotonic() - t0
    args = [str(BIN), "run", MC, LOADER, "--instance", INST.name, "--goal", "world", "--world", "wardentest",
            "--hold", str(HOLD), "--timeout", str(TIMEOUT), "--tag", f"b-{label}", "--option", "onboardAccessibility:false"]
    with InputWatch() as w:
        p = subprocess.run(args, capture_output=True, text=True, errors="replace", env=dict(os.environ, ALSOFT_DRIVERS="null"))
    out = p.stdout + p.stderr
    m = re.search(r"== resumo (\{.*\})", out)
    s = json.loads(m.group(1)) if m else {"outcome": "sem-resumo"}
    stable = "instável" not in out
    if s.get("outcome") == "no-mundo" and stable:
        verdict, sig = "passou", None
    elif s.get("outcome") == "tempo-esgotado":
        verdict, sig = "tempo", None
    else:
        verdict, sig = "falhou", signature(DATA / "runs" / s.get("capture", "x"))
    r = {"label": label, "n": len(subset), "verdict": verdict, "sig": sig, "outcome": s.get("outcome"),
         "ready": s.get("ready"), "world": s.get("world"), "prep_s": round(prep, 1),
         "total_s": round(time.monotonic() - t0, 1), "capture": s.get("capture"), **w.summary()}
    with open(LOG, "a", encoding="utf-8") as f:
        f.write(json.dumps(r, ensure_ascii=False) + "\n")
    print(f"[{label}] {len(subset)} mods -> {verdict} ({r['total_s']} s; pronto {r['ready']}, mundo {r['world']}) {sig or ''}", flush=True)
    return r


# ---------------------------------------------------------------- busca

def search(mods_dir: Path) -> None:
    t_start = time.monotonic()
    jars, deps, missing = graph(mods_dir)
    order, dependents = topo(jars, deps)
    print(f"{len(jars)} mods; dependências não resolvidas (ignoradas): {sorted(missing)}")
    rounds = []

    def test(subset, label):
        r = run_round(mods_dir, sorted(closure(subset, deps)), label)
        rounds.append(r)
        return r

    r0 = test(order, "r0-pack-inteiro")
    if r0["verdict"] != "falhou":
        print("o problema não apareceu com o pack inteiro; fim")
        return
    target = r0["sig"]
    r1 = test([], "r1-sem-mods")
    if r1["verdict"] != "passou":
        print("sem mods também falha: não é mod")
        return

    class Restart(Exception):
        pass

    REQ = re.compile(r"Mod (\S+) requires (\S+)")

    def infer_edge(r) -> bool:
        """R5A §3.4: dependência não declarada vira aresta inferida a partir do log."""
        text = (DATA / "runs" / (r.get("capture") or "x")).read_text(encoding="utf-8", errors="replace") if r.get("capture") else ""
        owner = graph.owner
        added = False
        for a, b in set(REQ.findall(text)):
            ja, jb = owner.get(a), owner.get(b)
            if ja and jb and ja != jb and jb not in deps[ja]:
                deps[ja].add(jb)
                inferred.append((ja, jb))
                print(f"   dependência inferida do log: {ja} -> {jb} ({a} requires {b})")
                added = True
        return added

    def fails(subset, label):
        r = test(subset, label)
        if r["verdict"] == "falhou" and r["sig"] == target:
            return True
        if r["verdict"] == "passou":
            return False
        print(f"   rodada inconclusiva ({r['verdict']}: {r['sig']})")
        if infer_edge(r):
            raise Restart()
        return None

    def bisect_prefix(seq, fixed, tag):
        """Menor k tal que seq[:k] + fixed falha (seq[:len] falha, seq[:0] passa)."""
        lo, hi, i = 0, len(seq), 0
        while hi - lo > 1:
            mid = (lo + hi) // 2
            i += 1
            res = fails(list(seq[:mid]) + list(fixed), f"{tag}-{i}-prefixo-{mid}")
            if res is None:  # inconclusiva: tenta o vizinho de cima
                res = fails(list(seq[:mid + 1]) + list(fixed), f"{tag}-{i}b-prefixo-{mid + 1}")
                mid += 1
                if res is None:
                    raise SystemExit("duas rodadas inconclusivas seguidas")
            if res:
                hi = mid
            else:
                lo = mid
        return hi

    inferred = []
    attempt = 0
    while True:
        attempt += 1
        try:
            k = bisect_prefix(order, [], f"b{attempt}")
            break
        except Restart:
            order, dependents = topo(jars, deps)
            if attempt >= 5:
                raise SystemExit("dependências inferidas demais")
    culprits = [order[k - 1]]
    print(f"último necessário: {culprits[0]} (posição {k} de {len(order)})")
    # Minimização para pares: busca entre os mods antes do culpado, com ele sempre ligado.
    before = order[: k - 1]
    while True:
        alone = fails(culprits, f"p{len(culprits)}-sozinho")
        if alone:
            break
        k2 = bisect_prefix(before, culprits, f"p{len(culprits)}")
        partner = before[k2 - 1]
        culprits.append(partner)
        before = before[: k2 - 1]
        print(f"parceiro: {partner}")
        if len(culprits) >= 3:
            break
    minimal = sorted(closure(culprits, deps))
    c1 = fails(culprits, "c1-so-o-conjunto")
    without = [j for j in order if j not in up_closure(culprits, dependents)]
    c2 = fails(without, "c2-pack-sem-o-culpado")
    total = time.monotonic() - t_start
    res = {"culpados": culprits, "conjunto_minimo": minimal, "confirma_falha": c1, "sem_culpado_passa": c2 is False,
           "rodadas": len(rounds), "tempo_total_s": round(total, 1), "dependencias_inferidas": inferred,
           "posicao_culpado": order.index(culprits[0]) + 1, "total_mods": len(order)}
    (DATA / "bisect-result.json").write_text(json.dumps(res, indent=1, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(res, indent=1, ensure_ascii=False))


def main() -> None:
    cmd, mods_dir = sys.argv[1], Path(sys.argv[2])
    if cmd == "grafo":
        jars, deps, missing = graph(mods_dir)
        order, dependents = topo(jars, deps)
        for i, j in enumerate(order, 1):
            print(f"{i:3} {'L' if dependents[j] else ' '} {j}  <- {sorted(deps[j])}")
        print("não resolvidas:", sorted(missing))
    elif cmd == "rodada":
        jars, deps, _ = graph(mods_dir)
        order, _ = topo(jars, deps)
        n = int(sys.argv[4]) if len(sys.argv) > 4 else len(order)
        run_round(mods_dir, sorted(closure(order[:n], deps)), sys.argv[3])
    elif cmd == "busca":
        search(mods_dir)


if __name__ == "__main__":
    main()
