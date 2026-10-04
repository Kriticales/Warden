"""Downloads do Modrinth sem chave para o spike S-R5-3.

Uso:
  python modrinth.py version <id da versão> <pasta>     baixa o arquivo principal (confere sha512)
  python modrinth.py mrpack <projeto> <versão> <instância>
      baixa o .mrpack, instala os arquivos do lado cliente e os overrides na instância
      e imprime a contagem de mods e o tamanho total.
"""

import concurrent.futures
import hashlib
import io
import json
import sys
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path

UA = {"User-Agent": "Kriticales/Warden-spike (kriticalees@gmail.com)"}
API = "https://api.modrinth.com/v2"


def get(url: str) -> bytes:
    return urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=180).read()


def fetch(url: str, dest: Path, sha512: str | None) -> int:
    if dest.exists() and sha512 and hashlib.sha512(dest.read_bytes()).hexdigest() == sha512:
        return dest.stat().st_size
    data = get(url)
    if sha512 and hashlib.sha512(data).hexdigest() != sha512:
        raise SystemExit(f"sha512 diferente: {url}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(data)
    return len(data)


def version(vid: str, folder: Path) -> None:
    v = json.loads(get(f"{API}/version/{vid}"))
    f = next(x for x in v["files"] if x["primary"])
    fetch(f["url"], folder / f["filename"], f["hashes"]["sha512"])
    print(f"{folder / f['filename']} ({f['size']} bytes)")


def mrpack(project: str, number: str, inst: Path) -> None:
    vs = json.loads(get(f"{API}/project/{project}/version"))
    v = next(x for x in vs if x["version_number"] == number)
    f = next(x for x in v["files"] if x["primary"])
    print(f"{project} {number}: {f['url']}")
    z = zipfile.ZipFile(io.BytesIO(get(f["url"])))
    idx = json.loads(z.read("modrinth.index.json"))
    print("dependências:", idx["dependencies"])
    files = [x for x in idx["files"] if x.get("env", {}).get("client", "required") != "unsupported"]
    with concurrent.futures.ThreadPoolExecutor(8) as ex:
        sizes = list(ex.map(lambda x: fetch(x["downloads"][0], inst / x["path"], x["hashes"].get("sha512")), files))
    n_over = 0
    for name in z.namelist():
        for prefix in ("overrides/", "client-overrides/"):
            if name.startswith(prefix) and not name.endswith("/"):
                rel = name[len(prefix):]
                (inst / rel).parent.mkdir(parents=True, exist_ok=True)
                (inst / rel).write_bytes(z.read(name))
                n_over += 1
    mods = sorted(p.name for p in (inst / "mods").glob("*.jar"))
    total = sum(p.stat().st_size for p in (inst / "mods").glob("*.jar"))
    print(f"arquivos do índice: {len(files)} ({sum(sizes) / 1e6:.1f} MB); overrides: {n_over}")
    print(f"mods/*.jar: {len(mods)} ({total / 1e6:.1f} MB)")
    (inst / "warden-mrpack.json").write_text(json.dumps({"project": project, "version": number, "dependencies": idx["dependencies"], "mods": mods}, indent=1), encoding="utf-8")


if __name__ == "__main__":
    if sys.argv[1] == "version":
        version(sys.argv[2], Path(sys.argv[3]))
    elif sys.argv[1] == "mrpack":
        mrpack(sys.argv[2], sys.argv[3], Path(sys.argv[4]))
    else:
        raise SystemExit(__doc__)
