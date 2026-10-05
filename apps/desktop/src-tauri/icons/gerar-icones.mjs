#!/usr/bin/env node
// Gera os ícones do app e a imagem lateral do instalador a partir de `warden.svg` (A-02).
//
// A marca é pixel art 16×16: cada tamanho é ampliado por vizinho mais próximo, sem
// suavização, para os pixels continuarem nítidos (o `cargo tauri icon` reamostra e borra).
// Sem dependências: o PNG, o ICO e o BMP são escritos aqui, com o zlib do Node.
//
// Uso (na pasta `apps/desktop/src-tauri/icons`):
//   node gerar-icones.mjs           grava os arquivos
//   node gerar-icones.mjs --check   só confere se os arquivos versionados estão atualizados
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { deflateSync } from 'node:zlib';

const HERE = dirname(fileURLToPath(import.meta.url));
const GRID = 16;

/** Fundo da imagem lateral do instalador: `--color-bg` de design/system/tokens.json. */
const SIDEBAR_BG = [0x05, 0x0d, 0x12];

// ---------------------------------------------------------------------------------------------
// Desenho

/** Lê os `<rect>` do SVG (só o subconjunto que a marca usa: inteiros, cor #rrggbb, opacidade). */
export function parseRects(svg) {
  const rects = [];
  for (const [tag] of svg.matchAll(/<rect\b[^>]*>/g)) {
    const attr = (name) => tag.match(new RegExp(`\\s${name}="([^"]*)"`))?.[1];
    const [x, y, width, height] = ['x', 'y', 'width', 'height'].map((name) => {
      const value = Number(attr(name));
      if (!Number.isInteger(value)) throw new Error(`<rect> com ${name} inválido: ${tag}`);
      return value;
    });
    const fill = attr('fill');
    if (!/^#[0-9a-f]{6}$/i.test(fill ?? '')) throw new Error(`<rect> com cor inválida: ${tag}`);
    const opacity = Number(attr('fill-opacity') ?? '1');
    rects.push({ x, y, width, height, rgb: hexToRgb(fill), opacity });
  }
  if (rects.length === 0) throw new Error('o SVG não tem nenhum <rect>');
  return rects;
}

function hexToRgb(hex) {
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
}

/** Pinta os retângulos numa grade RGBA 16×16 (composição "source-over", alfa linear). */
export function rasterize(rects) {
  const px = new Float64Array(GRID * GRID * 4);
  for (const { x, y, width, height, rgb, opacity } of rects) {
    for (let row = y; row < y + height; row++) {
      for (let col = x; col < x + width; col++) {
        if (row < 0 || col < 0 || row >= GRID || col >= GRID) {
          throw new Error(`retângulo fora da grade ${GRID}×${GRID}`);
        }
        const i = (row * GRID + col) * 4;
        const under = px[i + 3];
        const alpha = opacity + under * (1 - opacity);
        for (let c = 0; c < 3; c++) {
          px[i + c] = alpha === 0 ? 0 : (rgb[c] * opacity + px[i + c] * under * (1 - opacity)) / alpha;
        }
        px[i + 3] = alpha;
      }
    }
  }
  return Uint8Array.from(px, (value, i) => Math.round(i % 4 === 3 ? value * 255 : value));
}

/** Amplia a grade para `size`×`size` por vizinho mais próximo. */
export function scale(grid, size) {
  const out = new Uint8Array(size * size * 4);
  for (let row = 0; row < size; row++) {
    const src = Math.floor((row * GRID) / size);
    for (let col = 0; col < size; col++) {
      const from = (src * GRID + Math.floor((col * GRID) / size)) * 4;
      out.set(grid.subarray(from, from + 4), (row * size + col) * 4);
    }
  }
  return out;
}

/** Imagem lateral do instalador (164×314): fundo do app e a marca ampliada 8×, centralizada. */
export function sidebar(grid) {
  const width = 164;
  const height = 314;
  const factor = 8;
  const mark = scale(grid, GRID * factor);
  const size = GRID * factor;
  const left = (width - size) / 2;
  const top = 72;
  const out = new Uint8Array(width * height * 4);
  for (let row = 0; row < height; row++) {
    for (let col = 0; col < width; col++) {
      const o = (row * width + col) * 4;
      let rgb = SIDEBAR_BG;
      const mr = row - top;
      const mc = col - left;
      if (mr >= 0 && mc >= 0 && mr < size && mc < size) {
        const m = (mr * size + mc) * 4;
        const a = mark[m + 3] / 255;
        rgb = SIDEBAR_BG.map((bg, c) => Math.round(mark[m + c] * a + bg * (1 - a)));
      }
      out.set([...rgb, 255], o);
    }
  }
  return { width, height, rgba: out };
}

// ---------------------------------------------------------------------------------------------
// Formatos

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(bytes) {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(data.length, 0);
  head.write(type, 4, 'latin1');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])), 0);
  return Buffer.concat([head, data, crc]);
}

/** PNG RGBA de 8 bits, sem filtro, compressão máxima (saída determinística). */
export function png(width, height, rgba) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 6, 0, 0, 0], 8);
  const raw = Buffer.alloc(height * (width * 4 + 1));
  for (let row = 0; row < height; row++) {
    raw.set(rgba.subarray(row * width * 4, (row + 1) * width * 4), row * (width * 4 + 1) + 1);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/** Imagem de ícone em DIB de 32 bits (BGRA, de baixo para cima) com a máscara AND. */
function iconDib(size, rgba) {
  const header = Buffer.alloc(40);
  header.writeUInt32LE(40, 0);
  header.writeInt32LE(size, 4);
  header.writeInt32LE(size * 2, 8); // XOR + AND
  header.writeUInt16LE(1, 12);
  header.writeUInt16LE(32, 14);
  const xor = Buffer.alloc(size * size * 4);
  const maskStride = Math.ceil(size / 32) * 4;
  const and = Buffer.alloc(maskStride * size);
  for (let row = 0; row < size; row++) {
    const dst = size - 1 - row;
    for (let col = 0; col < size; col++) {
      const s = (row * size + col) * 4;
      const d = (dst * size + col) * 4;
      xor.set([rgba[s + 2], rgba[s + 1], rgba[s], rgba[s + 3]], d);
      if (rgba[s + 3] === 0) and[dst * maskStride + (col >> 3)] |= 0x80 >> (col & 7);
    }
  }
  return Buffer.concat([header, xor, and]);
}

/** ICO com as imagens dadas: DIB abaixo de 256 px (compatível com tudo), PNG em 256 px. */
export function ico(images) {
  const dir = Buffer.alloc(6);
  dir.writeUInt16LE(1, 2);
  dir.writeUInt16LE(images.length, 4);
  const entries = [];
  const blobs = [];
  let offset = 6 + 16 * images.length;
  for (const { size, rgba } of images) {
    const blob = size >= 256 ? png(size, size, rgba) : iconDib(size, rgba);
    const entry = Buffer.alloc(16);
    entry.writeUInt8(size >= 256 ? 0 : size, 0);
    entry.writeUInt8(size >= 256 ? 0 : size, 1);
    entry.writeUInt16LE(1, 4);
    entry.writeUInt16LE(32, 6);
    entry.writeUInt32LE(blob.length, 8);
    entry.writeUInt32LE(offset, 12);
    offset += blob.length;
    entries.push(entry);
    blobs.push(blob);
  }
  return Buffer.concat([dir, ...entries, ...blobs]);
}

/** BMP de 24 bits (o NSIS exige 24 bits nas imagens do instalador). */
export function bmp24(width, height, rgba) {
  const stride = Math.ceil((width * 3) / 4) * 4;
  const pixels = Buffer.alloc(stride * height);
  for (let row = 0; row < height; row++) {
    const dst = (height - 1 - row) * stride;
    for (let col = 0; col < width; col++) {
      const s = (row * width + col) * 4;
      pixels.set([rgba[s + 2], rgba[s + 1], rgba[s]], dst + col * 3);
    }
  }
  const head = Buffer.alloc(54);
  head.write('BM', 0, 'latin1');
  head.writeUInt32LE(54 + pixels.length, 2);
  head.writeUInt32LE(54, 10);
  head.writeUInt32LE(40, 14);
  head.writeInt32LE(width, 18);
  head.writeInt32LE(height, 22);
  head.writeUInt16LE(1, 26);
  head.writeUInt16LE(24, 28);
  head.writeUInt32LE(pixels.length, 34);
  head.writeInt32LE(2835, 38); // 72 dpi
  head.writeInt32LE(2835, 42);
  return Buffer.concat([head, pixels]);
}

// ---------------------------------------------------------------------------------------------
// Saídas

/** Tamanhos do `icon.ico` (Windows: 100 %, 150 %, 200 %, 300 %, 400 % e o grande do Explorer). */
export const ICO_SIZES = [16, 24, 32, 48, 64, 256];

/** Arquivos gerados: nome → bytes. Os nomes são os de `bundle.icon` no `tauri.conf.json`. */
export function outputs(svg) {
  const grid = rasterize(parseRects(svg));
  const square = (size) => png(size, size, scale(grid, size));
  const side = sidebar(grid);
  return {
    '32x32.png': square(32),
    '128x128.png': square(128),
    '128x128@2x.png': square(256),
    'icon.png': square(512),
    'icon.ico': ico(ICO_SIZES.map((size) => ({ size, rgba: scale(grid, size) }))),
    'instalador-lateral.bmp': bmp24(side.width, side.height, side.rgba),
  };
}

function main() {
  const check = process.argv.includes('--check');
  const files = outputs(readFileSync(join(HERE, 'warden.svg'), 'utf8'));
  const stale = [];
  for (const [name, bytes] of Object.entries(files)) {
    const path = join(HERE, name);
    if (check) {
      let current;
      try {
        current = readFileSync(path);
      } catch {
        current = Buffer.alloc(0);
      }
      if (!current.equals(bytes)) stale.push(name);
    } else {
      writeFileSync(path, bytes);
      console.log(`gerado: ${name} (${bytes.length} bytes)`);
    }
  }
  if (stale.length > 0) {
    console.error(`desatualizados: ${stale.join(', ')}. Rode: node gerar-icones.mjs`);
    process.exit(1);
  }
  if (check) console.log('ícones atualizados.');
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
