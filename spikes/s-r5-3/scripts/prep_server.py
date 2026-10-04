"""Prepara um servidor dedicado descartável para o spike S-R5-3.

Uso:
  python prep_server.py vanilla  <mc>             <pasta> [--port N]
  python prep_server.py fabric   <mc>             <pasta> [--port N]
  python prep_server.py forge    <versão maven>   <pasta> --java <java.exe> [--port N]
  python prep_server.py neoforge <versão>         <pasta> --java <java.exe> [--port N]

Grava `warden-args.txt` (argumentos depois do `java`, um por linha), `server.properties`
com `online-mode=false` e `eula.txt`. O aceite da EULA é feito aqui só porque é um
servidor de teste descartável do spike, na máquina do dono, apagado no fim; no Warden
o aceite é do usuário (R5A §3.4).
"""

import hashlib
import json
import os
import subprocess
import sys
import urllib.request
from pathlib import Path

UA = {"User-Agent": "Kriticales/Warden-spike (kriticalees@gmail.com)"}


def get(url: str) -> bytes:
    return urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=120).read()


def download(url: str, dest: Path, sha1: str | None = None) -> None:
    data = get(url)
    if sha1 and hashlib.sha1(data).hexdigest() != sha1:
        raise SystemExit(f"hash diferente: {url}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(data)


def vanilla_server(mc: str, d: Path) -> list[str]:
    manifest = json.loads(get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"))
    v = next(x for x in manifest["versions"] if x["id"] == mc)
    meta = json.loads(get(v["url"]))
    s = meta["downloads"]["server"]
    download(s["url"], d / "server.jar", s["sha1"])
    return ["-Xmx2G", "-jar", "server.jar", "nogui"]


def fabric_server(mc: str, d: Path) -> list[str]:
    loader = json.loads(get(f"https://meta.fabricmc.net/v2/versions/loader/{mc}"))[0]["loader"]["version"]
    installer = json.loads(get("https://meta.fabricmc.net/v2/versions/installer"))[0]["version"]
    download(f"https://meta.fabricmc.net/v2/versions/loader/{mc}/{loader}/{installer}/server/jar", d / "fabric-server-launch.jar")
    print(f"fabric loader {loader}, installer {installer}")
    return ["-Xmx2G", "-jar", "fabric-server-launch.jar", "nogui"]


def installer_server(kind: str, ver: str, d: Path, java: str) -> list[str]:
    if kind == "forge":
        name = f"forge-{ver}-installer.jar"
        url = f"https://maven.minecraftforge.net/net/minecraftforge/forge/{ver}/{name}"
    else:
        name = f"neoforge-{ver}-installer.jar"
        url = f"https://maven.neoforged.net/releases/net/neoforged/neoforge/{ver}/{name}"
    download(url, d / name)
    r = subprocess.run([java, "-jar", name, "--installServer"], cwd=d, capture_output=True, text=True, errors="replace")
    (d / "installer-saida.txt").write_text(r.stdout + r.stderr, encoding="utf-8")
    if r.returncode != 0:
        raise SystemExit(f"instalador falhou ({r.returncode}); ver {d / 'installer-saida.txt'}")
    # 1.17+: o instalador grava run.bat com @libraries/.../win_args.txt.
    win_args = sorted(d.glob("libraries/net/**/win_args.txt"))
    if win_args:
        rel = win_args[0].relative_to(d).as_posix()
        return ["-Xmx3G", f"@{rel}", "nogui"]
    jars = [p.name for p in d.glob("forge-*.jar") if "installer" not in p.name]
    if not jars:
        raise SystemExit("jar do servidor Forge não encontrado")
    return ["-Xmx2G", "-jar", sorted(jars)[0], "nogui"]


def main() -> None:
    kind, ver, folder = sys.argv[1], sys.argv[2], Path(sys.argv[3])
    opts = sys.argv[4:]
    opt = lambda n, default=None: opts[opts.index(n) + 1] if n in opts else default
    port = opt("--port", "25599")
    folder.mkdir(parents=True, exist_ok=True)
    if kind == "vanilla":
        args = vanilla_server(ver, folder)
    elif kind == "fabric":
        args = fabric_server(ver, folder)
    elif kind in ("forge", "neoforge"):
        args = installer_server(kind, ver, folder, opt("--java"))
    else:
        raise SystemExit(f"tipo desconhecido: {kind}")
    (folder / "warden-args.txt").write_text("\n".join(args) + "\n", encoding="utf-8")
    (folder / "eula.txt").write_text("# servidor de teste descartável do spike S-R5-3\neula=true\n", encoding="utf-8")
    props = {
        "online-mode": "false",
        "server-port": port,
        "server-ip": "127.0.0.1",
        "level-seed": "warden",
        "level-name": "world",
        "view-distance": "6",
        "spawn-protection": "0",
        "motd": "Warden S-R5-3",
        "enable-command-block": "false",
        "snooper-enabled": "false",
    }
    (folder / "server.properties").write_text("".join(f"{k}={v}\n" for k, v in props.items()), encoding="utf-8")
    print(f"pronto: {folder} -> java {' '.join(args)}")


if __name__ == "__main__":
    main()
