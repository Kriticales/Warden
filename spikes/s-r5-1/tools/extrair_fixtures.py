"""Extrai as fixtures do S-R5-1 (configs de mixin, refmaps e descritores pequenos)
dos jars baixados e grava o resultado resumido de cada pack.

Uso: python extrair_fixtures.py <pasta-packs> <pasta-out> <pasta-fixtures>

Cada arquivo extraído vai para fixtures/<pack>/<jar>/<caminho> e entra em
fixtures/ORIGEM.json com o jar, a URL do Modrinth e o sha1 do jar.
"""
import io
import json
import pathlib
import sys
import zipfile

# (pack, prefixo do jar, arquivos dentro do jar; "!/" desce num aninhado)
WANTED = [
    ("fabric-1.20.1", "lithium-fabric", ["lithium.mixins.json", "fabric.mod.json"]),
    ("fabric-1.20.1", "modernfix-fabric", ["modernfix-fabric.mixins.json", "modernfix-common.mixins.json"]),
    ("fabric-1.20.1", "ferritecore-6.0.1-fabric", ["fabric.mod.json", "ferritecore.threaddetec.mixin.json", "ferritecore.dedupmultipart.mixin.json"]),
    ("fabric-1.20.1", "krypton-", ["fabric.mod.json"]),
    ("fabric-1.20.1", "iris-1.7.6", ["mixins.iris.compat.sodium.json", "mixins.iris.json"]),
    ("fabric-1.20.1", "sodium-fabric", ["sodium.mixins.json"]),
    ("fabric-1.20.1", "redirected-", ["fabric.mod.json", "mixins.redirected_fabric_1.20.1.json"]),
    ("fabric-1.20.1", "sodium-extra-", ["sodium-extra.mixins.json"]),
    ("fabric-1.20.1", "sodiumextras-fabric", ["mixins.sodiumextras.json"]),
    ("fabric-1.20.1", "SubtleEffects-fabric", ["subtle_effects.mixins.json", "subtle_effects.refmap.json"]),
    ("fabric-1.20.1", "particle_core", ["particle_core.mixins.json", "particle_core-refmap.json"]),
    ("fabric-1.20.1", "fabric-api-0.92.7", ["fabric.mod.json"]),
    ("forge-1.20.1", "radium-", ["META-INF/MANIFEST.MF", "META-INF/mods.toml"]),
    ("forge-1.20.1", "supplementaries-1.20-3.1.43-forge", ["META-INF/mods.toml", "META-INF/MANIFEST.MF"]),
    ("forge-1.20.1", "BadOptimizations-", ["fabric.mod.json", "META-INF/mods.toml", "forge-badoptimizations.mixins.json", "fabric-badoptimizations.mixins.json"]),
    ("forge-1.20.1", "packetfixer-", ["META-INF/MANIFEST.MF", "fabric.mod.json", "META-INF/mods.toml"]),
    ("neoforge-1.21.1", "ferritecore-7.0.2-neoforge", ["META-INF/neoforge.mods.toml"]),
    ("neoforge-1.21.1", "XaeroPlus-", ["META-INF/neoforge.mods.toml"]),
    ("neoforge-1.21.1", "mes-", ["mes-neoforge.mixins.json", "mes-common.mixins.json"]),
    ("neoforge-1.21.1", "lithium-neoforge", ["lithium.mixins.json"]),
]
MAX_BYTES = 64 * 1024
PACK_CONFIGS = ["lithium.properties", "modernfix-mixins.properties", "ferritecore.mixin.properties", "ferritecore-mixin.toml"]


def main():
    packs, out, fx = (pathlib.Path(a) for a in sys.argv[1:4])
    fx.mkdir(parents=True, exist_ok=True)
    origem = {"descricao": "Arquivos pequenos tirados de jars públicos do Modrinth (spike S-R5-1). Nada de dado pessoal.", "arquivos": []}
    for pack, prefix, files in WANTED:
        info = json.loads((packs / pack / "origem.json").read_text(encoding="utf-8"))
        jar = next(j for j in info["jars"] if pathlib.Path(j["path"]).name.startswith(prefix))
        z = zipfile.ZipFile(packs / pack / jar["path"])
        names = set(z.namelist())
        for f in files:
            if f not in names:
                print("ausente:", pack, prefix, f)
                continue
            data = z.read(f)
            if len(data) > MAX_BYTES:
                print("grande demais, pulado:", f, len(data))
                continue
            dest = fx / pack / pathlib.Path(jar["path"]).stem / f
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)
            origem["arquivos"].append({
                "fixture": dest.relative_to(fx).as_posix(),
                "jar": pathlib.Path(jar["path"]).name,
                "url": jar["url"],
                "sha1_do_jar": jar["sha1"],
                "pack": f"{info['nome']} ({info['link']})",
            })
    for pack in ["fabric-1.20.1", "forge-1.20.1", "neoforge-1.21.1"]:
        info = json.loads((packs / pack / "origem.json").read_text(encoding="utf-8"))
        cdir = packs / pack / "config"
        for c in PACK_CONFIGS:
            p = cdir / c
            if p.exists():
                dest = fx / pack / "config" / c
                dest.parent.mkdir(parents=True, exist_ok=True)
                dest.write_bytes(p.read_bytes())
                origem["arquivos"].append({"fixture": dest.relative_to(fx).as_posix(), "origem": f"overrides/config/{c} do {info['nome']}", "mrpack": info["mrpack"], "mrpack_sha1": info["mrpack_sha1"]})
        # Resultado resumido (o que a D-10 deve reproduzir, com folga).
        pr = json.loads((out / f"proto-{pack}.json").read_text(encoding="utf-8"))
        pr["resumo"]["pasta"] = f"<packs>/{pack}/mods"
        pr["resumo"]["cache"] = None
        resumo = {
            "pack": info["nome"],
            "link": info["link"],
            "resumo": pr["resumo"],
            "sobreposicoes_r5a": pr["sobreposicoes"],
            "sobreposicoes_refinadas": pr["sobreposicoes_refinadas"],
        }
        (fx / f"resultado-{pack}.json").write_text(json.dumps(resumo, indent=1, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
        # Lista dos jars do pack (origem), sem os próprios jars.
        (fx / f"jars-{pack}.json").write_text(json.dumps({k: info[k] for k in info if k != "outros_arquivos"}, indent=1, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    (fx / "ORIGEM.json").write_text(json.dumps(origem, indent=1, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    print("ok", len(origem["arquivos"]), "arquivos")


if __name__ == "__main__":
    main()
