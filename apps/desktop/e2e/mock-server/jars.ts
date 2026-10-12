/**
 * Jars sintéticos dos E2E de "Adicionar arquivo local" e do link direto (P1-11), montados na
 * hora (zip sem compressão), sem nenhum jar de terceiros no repositório.
 *
 * - `sodiumJar`: faz o papel do jar do Sodium baixado do Modrinth. O `/version_files` simulado
 *   (em `server.ts`) acha o sha512 dele e devolve a versão **gravada** do Sodium
 *   (`2026-10-04-version-SMxNOGZ6.json`), como o Modrinth faria com o jar real.
 * - `fabricJar`: um mod Fabric que nenhuma fonte conhece (o jar Fabric num pack Forge,
 *   CA-T08-05).
 * - `directJar`: o arquivo do link direto `https://files.example.com/…`, servido pelo
 *   servidor de fixtures no lugar do site (`WARDEN_E2E_DIRECT_VIA`).
 */
import { createHash } from 'node:crypto';
import { crc32 } from 'node:zlib';

/** Zip com as entradas sem compressão (o formato que o Warden lê, como um jar). */
export function storedZip(entries: Record<string, string>): Buffer {
  const locals: Buffer[] = [];
  const centrals: Buffer[] = [];
  let offset = 0;
  for (const [name, text] of Object.entries(entries)) {
    const nameBytes = Buffer.from(name, 'utf8');
    const data = Buffer.from(text, 'utf8');
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(0x0800, 6); // nomes em UTF-8
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt32LE(offset, 42);
    locals.push(local, nameBytes, data);
    centrals.push(central, nameBytes);
    offset += local.length + nameBytes.length + data.length;
  }
  const directory = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(centrals.length / 2, 8);
  end.writeUInt16LE(centrals.length / 2, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, directory, end]);
}

function fabricModJson(fields: Record<string, unknown>): string {
  return JSON.stringify({ schemaVersion: 1, ...fields }, null, 2);
}

export const sodiumJar = storedZip({
  'fabric.mod.json': fabricModJson({
    id: 'sodium',
    version: '0.8.13+mc1.21.1',
    name: 'Sodium',
    environment: 'client',
    depends: { minecraft: '1.21.1', fabricloader: '>=0.16.0' },
  }),
});

export const fabricJar = storedZip({
  'fabric.mod.json': fabricModJson({
    id: 'mod_fabric_do_teste',
    version: '1.0.0',
    name: 'Mod Fabric do Teste',
    depends: { minecraft: '1.20.1' },
  }),
});

export const directJar = storedZip({
  'fabric.mod.json': fabricModJson({
    id: 'mod_do_link',
    version: '2.0.0',
    name: 'Mod do Link',
    environment: '*',
    depends: { minecraft: '1.21.1' },
  }),
});

/** Caminho do link direto no servidor de fixtures (`<base>/direto/<servidor><caminho>`). */
export const DIRECT_LINK = 'https://files.example.com/mods/Mod%20do%20Link%202.0.jar';
export const DIRECT_PATH = '/direto/files.example.com/mods/Mod%20do%20Link%202.0.jar';

export function sha512(bytes: Buffer): string {
  return createHash('sha512').update(bytes).digest('hex');
}
