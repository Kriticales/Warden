"""Baixa um .mrpack do Modrinth e os jars de mods/ listados nele (sem chave).

Uso: python baixar_pack.py <slug> <versao-do-jogo> <loader> <pasta-destino>

Grava em <pasta-destino>/: o .mrpack, mods/*.jar e origem.json (projeto, versão,
URL e sha1 de cada jar). Confere o sha1 de cada arquivo baixado.
"""
import hashlib
import json
import pathlib
import sys
import urllib.parse
import urllib.request
import zipfile
from concurrent.futures import ThreadPoolExecutor

UA = "Kriticales/Warden-spike-s-r5-1 (uso pessoal)"
API = "https://api.modrinth.com/v2"


def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def main():
    slug, game, loader, dest = sys.argv[1:5]
    dest = pathlib.Path(dest)
    (dest / "mods").mkdir(parents=True, exist_ok=True)
    q = urllib.parse.urlencode({"game_versions": json.dumps([game]), "loaders": json.dumps([loader])})
    versions = json.loads(get(f"{API}/project/{slug}/version?{q}"))
    v = versions[0]
    f = next(x for x in v["files"] if x["primary"])
    mrpack = dest / f["filename"]
    if not mrpack.exists():
        mrpack.write_bytes(get(f["url"]))
    with zipfile.ZipFile(mrpack) as z:
        index = json.loads(z.read("modrinth.index.json"))
    files = [x for x in index["files"] if x["path"].startswith("mods/") and x["path"].endswith(".jar")]
    skipped = [x["path"] for x in index["files"] if x not in files]

    def baixa(x):
        alvo = dest / x["path"]
        if x.get("env", {}).get("client") == "unsupported":
            return x["path"], "pulado (só servidor)"
        if alvo.exists() and hashlib.sha1(alvo.read_bytes()).hexdigest() == x["hashes"]["sha1"]:
            return x["path"], "ok (cache)"
        for url in x["downloads"]:
            try:
                dados = get(url)
            except Exception as e:  # noqa: BLE001
                err = str(e)
                continue
            if hashlib.sha1(dados).hexdigest() == x["hashes"]["sha1"]:
                alvo.write_bytes(dados)
                return x["path"], "ok"
            err = "sha1 diferente"
        return x["path"], f"FALHOU: {err}"

    with ThreadPoolExecutor(8) as ex:
        res = list(ex.map(baixa, files))
    origem = {
        "projeto": slug,
        "nome": v["name"],
        "versao": v["version_number"],
        "version_id": v["id"],
        "publicado": v["date_published"],
        "link": f"https://modrinth.com/modpack/{slug}/version/{v['id']}",
        "mrpack": f["url"],
        "mrpack_sha1": f["hashes"]["sha1"],
        "dependencias": index.get("dependencies"),
        "jars": [
            {"path": x["path"], "url": x["downloads"][0], "sha1": x["hashes"]["sha1"], "status": s}
            for x, (_, s) in zip(files, res)
        ],
        "outros_arquivos": skipped,
    }
    (dest / "origem.json").write_text(json.dumps(origem, indent=2, ensure_ascii=False), encoding="utf-8")
    falhas = [p for p, s in res if not s.startswith("ok")]
    print(f"{v['name']} ({v['version_number']}) deps={index.get('dependencies')}")
    print(f"jars: {len(files)}; baixados ok: {len(files) - len(falhas)}; problemas: {falhas}")


if __name__ == "__main__":
    main()
