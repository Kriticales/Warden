// @vitest-environment node
// O leitor de zip dos E2E (`zip.ts`): sem ele certo, os E2E de exportação e da instância do
// Prism conferem o arquivo errado.
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { crc32, deflateRawSync } from 'node:zlib';

import { afterAll, describe, expect, it } from 'vitest';

import { zipEntries, zipEntry } from './zip';

interface Entry {
  name: string;
  text: string;
  deflate?: boolean;
}

/** Zip mínimo, com entradas `stored` ou `deflate`, e um comentário no fim do arquivo. */
function buildZip(entries: Entry[]): Buffer {
  const locals: Buffer[] = [];
  const centrals: Buffer[] = [];
  let offset = 0;
  for (const { name, text, deflate = false } of entries) {
    const nameBytes = Buffer.from(name, 'utf8');
    const raw = Buffer.from(text, 'utf8');
    const data = deflate ? deflateRawSync(raw) : raw;
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(deflate ? 8 : 0, 8);
    local.writeUInt32LE(crc32(raw), 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(raw.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt16LE(deflate ? 8 : 0, 10);
    central.writeUInt32LE(crc32(raw), 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(raw.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt32LE(offset, 42);
    locals.push(local, nameBytes, data);
    centrals.push(central, nameBytes);
    offset += local.length + nameBytes.length + data.length;
  }
  const directory = Buffer.concat(centrals);
  const comment = Buffer.from('comentário do zip', 'utf8');
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  end.writeUInt16LE(comment.length, 20);
  return Buffer.concat([...locals, directory, end, comment]);
}

const dir = mkdtempSync(join(tmpdir(), 'warden-zip-'));
afterAll(() => {
  rmSync(dir, { recursive: true, force: true });
});

function writeZip(name: string, bytes: Buffer): string {
  const path = join(dir, name);
  writeFileSync(path, bytes);
  return path;
}

describe('leitor de zip dos E2E', () => {
  const index = JSON.stringify({ formatVersion: 1, versionId: '1.2.0' });
  const path = writeZip(
    'pack.mrpack',
    buildZip([
      { name: 'overrides/', text: '' },
      { name: 'overrides/config/ação.txt', text: 'ajuste\n' },
      { name: 'modrinth.index.json', text: index, deflate: true },
    ]),
  );

  it('lista só as entradas de arquivo, em ordem, com nomes UTF-8', () => {
    expect(zipEntries(path)).toEqual(['modrinth.index.json', 'overrides/config/ação.txt']);
  });

  it('lê entradas com deflate e sem compressão', () => {
    expect(zipEntry(path, 'modrinth.index.json')?.toString('utf8')).toBe(index);
    expect(zipEntry(path, 'overrides/config/ação.txt')?.toString('utf8')).toBe('ajuste\n');
  });

  it('entrada ausente devolve null', () => {
    expect(zipEntry(path, 'manifest.json')).toBeNull();
  });

  it('arquivo que não é zip dá erro claro', () => {
    const notZip = writeZip('nao-zip.mrpack', Buffer.from('isto não é um zip, só texto comprido'));
    expect(() => zipEntries(notZip)).toThrow('zip sem diretório central');
  });

  it('método de compressão desconhecido dá erro em vez de devolver lixo', () => {
    const bytes = buildZip([{ name: 'a.txt', text: 'abc' }]);
    // Método 12 (bzip2) no diretório central.
    bytes.writeUInt16LE(12, bytes.indexOf(Buffer.from([0x50, 0x4b, 0x01, 0x02])) + 10);
    const odd = writeZip('bzip2.zip', bytes);
    expect(() => zipEntry(odd, 'a.txt')).toThrow('método de compressão 12');
  });
});
