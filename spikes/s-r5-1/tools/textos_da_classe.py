"""Lista os textos (CONSTANT_String) e as classes citadas de classes dentro de um jar.

Uso: python textos_da_classe.py <jar> <filtro-de-caminho> [<texto-procurado> ...]

Desce um nível em jars aninhados (META-INF/jars, META-INF/jarjar). Serve para
conferir à mão se o plugin de mixin de um mod cita outro mod (spike S-R5-1).
"""
import io
import struct
import sys
import zipfile


def strings_of(data):
    if data[:4] != b"\xca\xfe\xba\xbe":
        return [], []
    count = struct.unpack(">H", data[8:10])[0]
    i, idx = 10, 1
    utf8, strs, classes = {}, [], []
    while idx < count:
        tag = data[i]
        if tag == 1:
            n = struct.unpack(">H", data[i + 1:i + 3])[0]
            utf8[idx] = data[i + 3:i + 3 + n].decode("utf-8", "replace")
            i += 3 + n
        elif tag in (3, 4):
            i += 5
        elif tag in (5, 6):
            i += 9
            idx += 1
        elif tag == 7:
            classes.append(struct.unpack(">H", data[i + 1:i + 3])[0])
            i += 3
        elif tag == 8:
            strs.append(struct.unpack(">H", data[i + 1:i + 3])[0])
            i += 3
        elif tag in (9, 10, 11, 12, 17, 18):
            i += 5
        elif tag == 15:
            i += 4
        elif tag in (16, 19, 20):
            i += 3
        else:
            break
        idx += 1
    return [utf8.get(s, "?") for s in strs], [utf8.get(c, "?") for c in classes]


def walk(z, prefix=""):
    for n in z.namelist():
        if n.endswith(".jar") and (n.startswith("META-INF/jars/") or n.startswith("META-INF/jarjar/")):
            yield from walk(zipfile.ZipFile(io.BytesIO(z.read(n))), prefix + n + "!/")
        elif n.endswith(".class"):
            yield prefix + n, z, n


def main():
    jar, filt, *wanted = sys.argv[1:]
    for full, z, n in walk(zipfile.ZipFile(jar)):
        if filt not in full:
            continue
        strs, classes = strings_of(z.read(n))
        if wanted:
            hits = [w for w in wanted if any(w in s for s in strs + classes)]
            if hits:
                print(full, "->", hits)
        else:
            print(full)
            for s in strs:
                print("   ", repr(s))


if __name__ == "__main__":
    main()
