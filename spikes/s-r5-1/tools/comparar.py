"""Resume as saídas do harness (protótipo e intermed) para o relatório do S-R5-1.

Uso: python comparar.py <pasta-out>

Lê proto-<pack>.json, intermed-<pack>.json e doctor-<pack>-frio.json e imprime:
contagens por regra/categoria, os avisos finais de cada ferramenta e o que um
achou e o outro não.
"""
import collections
import json
import pathlib
import sys

PACKS = ["fabric-1.20.1", "forge-1.20.1", "neoforge-1.21.1"]


def kinds(o):
    return [i.split(":", 1)[1].split("@", 1)[0] if ":" in i else "?" for i in o["involved"]]


def categoria(o):
    k = kinds(o)
    if o["base"] == "Alto":
        return "A: dois substitutos no mesmo ponto"
    if "Overwrite" in k:
        return "B: @Overwrite + injeção no corpo"
    if "Redirect" in k:
        outros = [x for x in k if x != "Redirect"]
        if outros and all(x == "Inject" for x in outros):
            return "C: @Redirect + @Inject"
        return "D: @Redirect + MixinExtras/outro"
    return "E: @ModifyVariable/@ModifyArg(s) no mesmo ponto"


def main():
    out = pathlib.Path(sys.argv[1])
    for p in PACKS:
        pr = json.loads((out / f"proto-{p}.json").read_text(encoding="utf-8"))
        r = pr["resumo"]
        print(f"## {p}")
        print(f"  leitura+cruzamento: {r['tempo_ms']['total']:.0f} ms (1 thread), pico {r['pico_memoria_mb']:.0f} MB")
        for modo, chave in [("R5A como escrita", "sobreposicoes"), ("refinado", "sobreposicoes_refinadas")]:
            ovs = pr[chave]
            avisos = [o for o in ovs if o["final_one"] in ("Alto", "Medio")]
            cat = collections.Counter(categoria(o) for o in avisos)
            base = collections.Counter(o["base"] for o in ovs)
            print(f"  [{modo}] base alto={base['Alto']} médio={base['Medio']}; avisos finais={len(avisos)}"
                  f" (alto={sum(o['final_one']=='Alto' for o in avisos)})")
            for c, n in sorted(cat.items()):
                print(f"      {c}: {n}")
        d = json.loads((out / f"doctor-{p}-frio.json").read_text(encoding="utf-8"))
        warns = [f for f in d["findings"] if f.get("rule_id") == "mixin-risk" and f["severity"] in ("warn", "error")]
        tipos = collections.Counter()
        for f in warns:
            t = f["title"]
            if t.startswith("Mixin may not apply"):
                tipos["pode não aplicar (remap=false)"] += 1
            elif t.startswith("Mixin order/replace conflict"):
                for e in f["explanation"].split("type(s): ")[1].split(". Affected")[0].split(", "):
                    tipos[e] += 1
            else:
                tipos["outro: " + t[:50]] += 1
        print(f"  [intermed doctor] avisos mixin-risk={len(warns)}; tipos de aresta: {dict(tipos)}")
        it = json.loads((out / f"intermed-{p}.json").read_text(encoding="utf-8"))
        nested = sum(1 for c in it["classes"] if "!" in c["archive"])
        print(f"  [intermed lib] classes={len(it['classes'])}, de jars aninhados={nested}")
        print()


if __name__ == "__main__":
    main()
