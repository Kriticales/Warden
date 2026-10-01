#!/usr/bin/env python3
"""Captura a tela do Xvfb com xwd e converte para PNG (sem dependências externas).

Uso: screenshot.py <saida.png>   (usa $DISPLAY; xwd vem da pasta extraída por xvfb.sh)
Suporta apenas o formato que o Xvfb usado aqui produz: ZPixmap TrueColor, 24 ou 32 bpp.
"""
import os
import struct
import subprocess
import sys
import zlib

base = os.path.join(os.environ.get("WARDEN_SPIKE_DATA_ROOT", os.path.expanduser("~/.local/share/warden-spike")), "xvfb/root")
env = dict(os.environ, LD_LIBRARY_PATH=os.path.join(base, "usr/lib/x86_64-linux-gnu"))
raw = subprocess.run([os.path.join(base, "usr/bin/xwd"), "-root", "-silent"], env=env, check=True, capture_output=True).stdout

fields = struct.unpack(">25I", raw[:100])
header_size, _, _, depth, width, height, _, byte_order, _, _, _, bpp, bpl, _, rmask, gmask, bmask, _, _, ncolors = fields[:20]
data = raw[header_size + ncolors * 12:]
assert bpp in (24, 32), f"bpp não suportado: {bpp}"
step = bpp // 8

def shift(mask):
    return (mask & -mask).bit_length() - 1

rs, gs, bs = shift(rmask), shift(gmask), shift(bmask)
order = "little" if byte_order == 0 else "big"
rows = bytearray()
for y in range(height):
    rows.append(0)
    line = data[y * bpl:(y * bpl) + width * step]
    for x in range(0, len(line), step):
        px = int.from_bytes(line[x:x + step], order)
        rows += bytes(((px & rmask) >> rs, (px & gmask) >> gs, (px & bmask) >> bs))

def chunk(tag, body):
    return struct.pack(">I", len(body)) + tag + body + struct.pack(">I", zlib.crc32(tag + body) & 0xFFFFFFFF)

png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(bytes(rows), 9)) + chunk(b"IEND", b"")
with open(sys.argv[1], "wb") as f:
    f.write(png)
print(f"{sys.argv[1]}: {width}x{height}")
