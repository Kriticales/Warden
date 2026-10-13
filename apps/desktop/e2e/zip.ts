/**
 * Ajuda comum dos E2E: ler um zip gerado pelo app (`.mrpack`, zip da CurseForge, exportação
 * packwiz) pelo diretório central, sem programa externo. No Windows o `tar` do PATH é o do Git
 * e lê o `C:` de `C:\…` como um computador remoto; com o Node puro o teste é o mesmo nos dois
 * sistemas. Entradas `stored` ou `deflate`, sem zip64 (os arquivos dos testes são pequenos).
 */
import { readFileSync } from 'node:fs';
import { inflateRawSync } from 'node:zlib';

const END_OF_CENTRAL_DIRECTORY = 0x06054b50;
const CENTRAL_FILE_HEADER = 0x02014b50;
const LOCAL_FILE_HEADER = 0x04034b50;
const STORED = 0;
const DEFLATE = 8;

interface CentralEntry {
  name: string;
  method: number;
  compressedSize: number;
  localOffset: number;
}

function centralDirectory(zip: Buffer): CentralEntry[] {
  let end = zip.length - 22;
  while (end >= 0 && zip.readUInt32LE(end) !== END_OF_CENTRAL_DIRECTORY) end -= 1;
  if (end < 0) throw new Error('zip sem diretório central');
  const count = zip.readUInt16LE(end + 10);
  let offset = zip.readUInt32LE(end + 16);
  const entries: CentralEntry[] = [];
  for (let i = 0; i < count; i += 1) {
    if (zip.readUInt32LE(offset) !== CENTRAL_FILE_HEADER) {
      throw new Error(`zip com diretório central inválido na entrada ${String(i)}`);
    }
    const nameLength = zip.readUInt16LE(offset + 28);
    const extra = zip.readUInt16LE(offset + 30);
    const comment = zip.readUInt16LE(offset + 32);
    entries.push({
      name: zip.toString('utf8', offset + 46, offset + 46 + nameLength),
      method: zip.readUInt16LE(offset + 10),
      compressedSize: zip.readUInt32LE(offset + 20),
      localOffset: zip.readUInt32LE(offset + 42),
    });
    offset += 46 + nameLength + extra + comment;
  }
  return entries;
}

/** Nomes das entradas de arquivo do zip, em ordem (sem as entradas de pasta). */
export function zipEntries(path: string): string[] {
  return centralDirectory(readFileSync(path))
    .map((entry) => entry.name)
    .filter((name) => !name.endsWith('/'))
    .sort();
}

/** O conteúdo de uma entrada do zip; `null` se ela não existe. */
export function zipEntry(path: string, name: string): Buffer | null {
  const zip = readFileSync(path);
  const entry = centralDirectory(zip).find((candidate) => candidate.name === name);
  if (!entry) return null;
  const local = entry.localOffset;
  if (zip.readUInt32LE(local) !== LOCAL_FILE_HEADER) {
    throw new Error(`zip sem cabeçalho local para ${name}`);
  }
  const start = local + 30 + zip.readUInt16LE(local + 26) + zip.readUInt16LE(local + 28);
  const data = zip.subarray(start, start + entry.compressedSize);
  if (entry.method === STORED) return Buffer.from(data);
  if (entry.method === DEFLATE) return inflateRawSync(data);
  throw new Error(`${name}: método de compressão ${String(entry.method)} não suportado`);
}
