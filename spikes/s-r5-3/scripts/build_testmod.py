"""Compila o mod de teste `wardenfalhas` (spike S-R5-3) para Fabric, NeoForge e Forge.

Uso: python build_testmod.py <pasta de saída> [--modo <modo>] [--sufixo <texto>]

`--modo` grava `/wardenfalhas.txt` no jar (o modo padrão sem `-Dwarden.falha`).
Precisa de um JDK em $WARDEN_SPIKE_JDK (padrão: o baixado em <dados>/jdk) e das
bibliotecas já baixadas pelo motor em <dados>/shared/libraries.
"""

import os
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
MOD = HERE.parent / "testmod"
DATA = Path(os.environ.get("WARDEN_SPIKE_DATA", r"C:\wt\s-r5-3\data"))
LIBS = DATA / "shared" / "libraries"


def jdk_bin(tool: str) -> str:
    root = Path(os.environ.get("WARDEN_SPIKE_JDK", "")) if os.environ.get("WARDEN_SPIKE_JDK") else next((DATA / "jdk").iterdir())
    return str(root / "bin" / (tool + ".exe" if os.name == "nt" else tool))


def lib(pattern: str) -> str:
    found = sorted(LIBS.glob(pattern))
    if not found:
        raise SystemExit(f"biblioteca não encontrada: {pattern}")
    return str(found[-1])


FABRIC_JSON = """{
  "schemaVersion": 1,
  "id": "wardenfalhas",
  "version": "1.0.0",
  "name": "Warden Falhas (teste)",
  "description": "Mod de teste do spike S-R5-3: provoca falhas controladas.",
  "license": "MIT",
  "environment": "*",
  "mixins": ["wardenfalhas.fabric.mixins.json"],
  "depends": { "fabricloader": "*" }
}
"""

NEO_TOML = """modLoader = "javafml"
loaderVersion = "[1,)"
license = "MIT"

[[mods]]
modId = "wardenfalhas"
version = "1.0.0"
displayName = "Warden Falhas (teste)"
description = "Mod de teste do spike S-R5-3: provoca falhas controladas."

[[mixins]]
config = "wardenfalhas.mojmap.mixins.json"
"""

FORGE_TOML = """modLoader = "javafml"
loaderVersion = "[47,)"
license = "MIT"

[[mods]]
modId = "wardenfalhas"
version = "1.0.0"
displayName = "Warden Falhas (teste)"
description = "Mod de teste do spike S-R5-3: provoca falhas controladas."
"""


def build(out: Path, modo: str | None, sufixo: str) -> None:
    out.mkdir(parents=True, exist_ok=True)
    src = MOD / "src" / "dev" / "kriticales" / "wardenfalhas"
    common = [src / "Falhas.java", src / "FalhasPlugin.java"]
    cp_base = [lib("net/fabricmc/sponge-mixin/*/sponge-mixin-*.jar"), lib("org/ow2/asm/asm-tree/9.*/asm-tree-*.jar"), lib("org/ow2/asm/asm/9.*/asm-*.jar")]
    variants = {
        "fabric": (common + sorted((src / "fabric").glob("*.java")), [], "wardenfalhas.fabric.mixins.json"),
        "neoforge": (common + sorted((src / "mojmap").glob("*.java")) + [src / "NeoMod.java"],
                     [lib("net/neoforged/fancymodloader/loader/4.*/loader-4.*.jar")], "wardenfalhas.mojmap.mixins.json"),
        "forge": (common + sorted((src / "mojmap").glob("*.java")) + [src / "ForgeMod.java"],
                  [lib("net/minecraftforge/javafmllanguage/1.20.1-*/javafmllanguage-1.20.1-*.jar")], "wardenfalhas.mojmap.mixins.json"),
    }
    for name, (files, extra_cp, mixins) in variants.items():
        with tempfile.TemporaryDirectory() as tmp:
            classes = Path(tmp)
            cmd = [jdk_bin("javac"), "--release", "17", "-nowarn", "-proc:none", "-d", str(classes),
                   "-cp", os.pathsep.join(cp_base + extra_cp), *map(str, files)]
            subprocess.run(cmd, check=True)
            jar = out / f"wardenfalhas-{name}{sufixo}.jar"
            with zipfile.ZipFile(jar, "w", zipfile.ZIP_DEFLATED) as z:
                manifest = "Manifest-Version: 1.0\n"
                if name == "forge":
                    manifest += f"MixinConfigs: {mixins}\n"
                z.writestr("META-INF/MANIFEST.MF", manifest)
                for p in classes.rglob("*.class"):
                    z.write(p, p.relative_to(classes).as_posix())
                z.write(MOD / "res" / mixins, mixins)
                if name == "fabric":
                    z.writestr("fabric.mod.json", FABRIC_JSON)
                elif name == "neoforge":
                    z.writestr("META-INF/neoforge.mods.toml", NEO_TOML)
                else:
                    z.writestr("META-INF/mods.toml", FORGE_TOML)
                if name != "fabric":
                    # Sem pack.mcmeta o Forge 1.20.1 avisa "Missing metadata in pack mod:wardenfalhas".
                    fmt = 15 if name == "forge" else 34  # 1.20.1 / 1.21.1
                    z.writestr("pack.mcmeta", '{"pack": {"description": "wardenfalhas", "pack_format": %d}}' % fmt)
                if modo:
                    z.writestr("wardenfalhas.txt", modo)
            print(jar)


if __name__ == "__main__":
    args = sys.argv[1:]
    opt = lambda n: args[args.index(n) + 1] if n in args else None
    build(Path(args[0]), opt("--modo"), opt("--sufixo") or "")
