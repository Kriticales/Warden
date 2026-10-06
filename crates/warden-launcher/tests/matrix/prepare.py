"""Prepara servidor ou mundo descartável para o smoke-game (S-R5-3 §3/§10).

Uso: python prepare.py <legacy|world> <minecraft> <loader> <versao-loader>
       <pasta> <java> <porta>

Só toca a pasta recebida, que é criada pelo teste sob WARDEN_LAUNCHER_DADOS.
O servidor é local, offline e de teste; seu aceite da EULA não é reutilizado pelo app.
"""

import hashlib
import json
import subprocess
import sys
import urllib.request
from pathlib import Path

HEADERS = {"User-Agent": "Kriticales/Warden-L05 (https://github.com/Kriticales/Warden)"}


def fetch(url: str) -> bytes:
    request = urllib.request.Request(url, headers=HEADERS)
    with urllib.request.urlopen(request, timeout=120) as response:
        return response.read()


def download(url: str, dest: Path, sha1: str | None = None) -> None:
    if dest.is_file():
        data = dest.read_bytes()
        if sha1 is None or hashlib.sha1(data).hexdigest() == sha1:
            return
    data = fetch(url)
    if sha1 is not None and hashlib.sha1(data).hexdigest() != sha1:
        raise RuntimeError(f"SHA-1 incorreto em {url}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(data)


def vanilla(mc: str, directory: Path) -> list[str]:
    manifest = json.loads(fetch("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"))
    version = next(item for item in manifest["versions"] if item["id"] == mc)
    metadata = json.loads(fetch(version["url"]))
    server = metadata["downloads"]["server"]
    download(server["url"], directory / "server.jar", server["sha1"])
    return ["-Xmx1536M", "-jar", "server.jar", "nogui"]


def fabric(mc: str, version: str, directory: Path) -> list[str]:
    installers = json.loads(fetch("https://meta.fabricmc.net/v2/versions/installer"))
    installer = next(item["version"] for item in installers if item.get("stable"))
    url = f"https://meta.fabricmc.net/v2/versions/loader/{mc}/{version}/{installer}/server/jar"
    download(url, directory / "fabric-server-launch.jar")
    return ["-Xmx1536M", "-jar", "fabric-server-launch.jar", "nogui"]


def forge(mc: str, version: str, directory: Path, java: str) -> list[str]:
    artifact = f"{mc}-{version}"
    if mc == "1.7.10":
        artifact += "-1.7.10"
    name = f"forge-{artifact}-installer.jar"
    base = f"https://maven.minecraftforge.net/net/minecraftforge/forge/{artifact}/{name}"
    sha1 = fetch(base + ".sha1").decode("ascii").strip().split()[0]
    download(base, directory / name, sha1)
    marker = directory / "installer-ok"
    if not marker.is_file():
        result = subprocess.run([java, "-jar", name, "--installServer"], cwd=directory,
                                capture_output=True, text=True, errors="replace", timeout=600)
        (directory / "installer-saida.txt").write_text(result.stdout + result.stderr, encoding="utf-8")
        if result.returncode:
            raise RuntimeError(f"Instalador Forge falhou ({result.returncode})")
        marker.write_text("ok\n", encoding="ascii")
    unix_args = sorted(directory.glob("libraries/net/**/unix_args.txt"))
    win_args = sorted(directory.glob("libraries/net/**/win_args.txt"))
    args_files = win_args if sys.platform == "win32" else unix_args
    if args_files:
        return ["-Xmx2G", "@" + args_files[0].relative_to(directory).as_posix(), "nogui"]
    jars = sorted(path.name for path in directory.glob("forge-*.jar") if "installer" not in path.name)
    if not jars:
        raise RuntimeError("Instalador Forge não produziu jar de servidor")
    return ["-Xmx1536M", "-jar", jars[0], "nogui"]


def main() -> None:
    mode, mc, loader, version, folder, java, port = sys.argv[1:]
    directory = Path(folder)
    directory.mkdir(parents=True, exist_ok=True)
    if mode == "world":
        args = vanilla(mc, directory)
    elif loader == "forge":
        args = forge(mc, version, directory, java)
    elif loader == "fabric":
        args = fabric(mc, version, directory)
    else:
        raise RuntimeError(f"Servidor local não previsto para {loader} {mc}")
    (directory / "eula.txt").write_text("# Servidor descartável da matriz L-05\neula=true\n", encoding="utf-8")
    properties = {
        "online-mode": "false", "server-ip": "127.0.0.1", "server-port": port,
        "level-seed": "warden", "level-name": "world", "view-distance": "4",
        "simulation-distance": "4", "spawn-protection": "0", "motd": "Warden L-05",
        "enable-rcon": "false", "enable-query": "false",
    }
    (directory / "server.properties").write_text(
        "".join(f"{key}={value}\n" for key, value in properties.items()), encoding="ascii")
    (directory / "warden-args.json").write_text(json.dumps(args), encoding="utf-8")
    if mode == "world" and not (directory / "world" / "level.dat").is_file():
        process = subprocess.Popen([java, *args], cwd=directory, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                   text=True, errors="replace", bufsize=1)
        try:
            for line in process.stdout:
                if "Done (" in line and "For help, type" in line:
                    break
            else:
                raise RuntimeError("Servidor vanilla saiu antes de gerar o mundo")
            process.stdin.write("stop\n")
            process.stdin.flush()
            process.wait(timeout=60)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
        if not (directory / "world" / "level.dat").is_file():
            raise RuntimeError("Servidor vanilla não gravou world/level.dat")


if __name__ == "__main__":
    main()
