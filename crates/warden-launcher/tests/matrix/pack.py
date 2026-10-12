"""Cria packs packwiz mínimos e confere o mod materializado pela L-03."""

import hashlib
import json
import re
import sys
import zipfile
from pathlib import Path


MODS = json.loads(Path(__file__).with_name("mods.json").read_text(encoding="utf-8"))


def fixture(name):
    if name == "vanilla-26.3":
        return None
    return MODS[name]


def create(name, minecraft, loader, version, root):
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    mod = fixture(name)
    index = 'hash-format = "sha256"\n'
    if mod:
        assert re.fullmatch(r"[a-f0-9]{40}", mod["sha1"])
        assert mod["url"].startswith("https://cdn.modrinth.com/")
        assert Path(mod["filename"]).name == mod["filename"]
        metafile = (
            f'name = "{mod["project"]}"\n'
            f'filename = "{mod["filename"]}"\n'
            'side = "client"\n\n'
            '[download]\n'
            f'url = "{mod["url"]}"\n'
            'hash-format = "sha1"\n'
            f'hash = "{mod["sha1"]}"\n\n'
            '[update.modrinth]\n'
            f'mod-id = "{mod["project"]}"\n'
            f'version = "{mod["version"]}"\n'
        )
        mods_dir = root / "mods"
        mods_dir.mkdir(exist_ok=True)
        (mods_dir / "smoke.pw.toml").write_text(metafile, encoding="utf-8")
        hash_value = hashlib.sha256(metafile.encode()).hexdigest()
        index += f'\n[[files]]\nfile = "mods/smoke.pw.toml"\nhash = "{hash_value}"\nmetafile = true\n'
    (root / "index.toml").write_text(index, encoding="utf-8")
    index_hash = hashlib.sha256(index.encode()).hexdigest()
    versions = f'minecraft = "{minecraft}"\n'
    if loader != "vanilla":
        versions += f'{loader} = "{version}"\n'
    pack = (
        'name = "Warden L-05"\nauthor = "Warden"\n'
        'version = "1.0.0"\npack-format = "packwiz:1.1.0"\n\n'
        '[index]\nfile = "index.toml"\nhash-format = "sha256"\n'
        f'hash = "{index_hash}"\n\n[versions]\n{versions}'
    )
    (root / "pack.toml").write_text(pack, encoding="utf-8")


def verify(name, game_dir):
    mod = fixture(name)
    mods_dir = Path(game_dir) / "mods"
    if not mod:
        assert not mods_dir.exists() or not list(mods_dir.iterdir())
        return
    jar = mods_dir / mod["filename"]
    data = jar.read_bytes()
    assert hashlib.sha1(data).hexdigest() == mod["sha1"], jar
    with zipfile.ZipFile(jar) as archive:
        files = archive.namelist()
        assert archive.testzip() is None, jar
        if mod["mixin"]:
            configs = [path for path in files if path.endswith((".mixins.json", ".mixin.json"))]
            if "fabric.mod.json" in files:
                metadata = json.loads(archive.read("fabric.mod.json"))
                for declared in metadata.get("mixins", []):
                    config = declared if isinstance(declared, str) else declared.get("config")
                    if config in files:
                        configs.append(config)
            assert configs, jar
        else:
            assert "mcmod.info" in files, jar
    print(f"mod materializado: {name}: {mod['filename']}")


if __name__ == "__main__":
    action, name, *args = sys.argv[1:]
    if action == "create":
        create(name, *args)
    elif action == "verify":
        verify(name, *args)
    else:
        raise ValueError(action)
