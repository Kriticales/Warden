/**
 * Downloads locais do fluxo completo (A-03). O Modrinth e a CurseForge simulados respondem com
 * as respostas gravadas, cujos arquivos ficam em `cdn.modrinth.com` e `edge.forgecdn.net`: o
 * Testar e o packwiz-installer do jogador baixariam da internet, e os E2E não saem dela.
 *
 * Ligado (`POST /__downloads-locais/ligar`; desligado por padrão e com
 * `POST /__downloads-locais/desligar`), cada arquivo das respostas passa a ser servido por este
 * servidor em `/jogador/sha1-<sha1 original>.jar` (o "jar" sintético de `player.ts`), com o
 * tamanho e os hashes desse jar no lugar dos originais. O nome deriva do SHA-1 original para o
 * mesmo arquivo nas duas fontes continuar com o mesmo hash, como na API real (a pré-visualização
 * confere o SHA-1 do Modrinth com o da CurseForge). Os outros specs veem as respostas gravadas
 * sem mudança nenhuma.
 */
import { createHash } from 'node:crypto';

import { jogadorJar } from './player';

let enabled = false;

/** Liga ou desliga os downloads locais. */
export function setLocalDownloads(on: boolean): void {
  enabled = on;
}

function digest(algorithm: 'sha1' | 'sha512', data: Buffer): string {
  return createHash(algorithm).update(data).digest('hex');
}

/** O jar local que faz o papel do arquivo de SHA-1 dado. */
function localFile(sha1: string, base: string) {
  const name = `sha1-${sha1.toLowerCase()}.jar`;
  const jar = jogadorJar(name);
  return {
    url: `${base}/jogador/${name}`,
    sha1: digest('sha1', jar),
    sha512: digest('sha512', jar),
    size: jar.length,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Troca, no lugar, os arquivos de um objeto da resposta (e dos que estão dentro dele). */
function localize(value: unknown, base: string): void {
  if (Array.isArray(value)) {
    for (const item of value) localize(item, base);
    return;
  }
  if (!isRecord(value)) return;
  const hashes = value.hashes;
  // Modrinth: `{ hashes: { sha1, sha512 }, url, size }`.
  if (isRecord(hashes) && typeof hashes.sha1 === 'string' && typeof value.url === 'string') {
    const local = localFile(hashes.sha1, base);
    value.url = local.url;
    value.size = local.size;
    value.hashes = { sha1: local.sha1, sha512: local.sha512 };
    return;
  }
  // CurseForge: `{ hashes: [{ value, algo: 1 }], downloadUrl, fileLength }`.
  if (Array.isArray(hashes) && 'downloadUrl' in value) {
    const sha1 = hashes.find(
      (hash): hash is { value: string; algo: number } =>
        isRecord(hash) && hash.algo === 1 && typeof hash.value === 'string',
    );
    if (sha1) {
      const local = localFile(sha1.value, base);
      if (typeof value.downloadUrl === 'string') value.downloadUrl = local.url;
      value.fileLength = local.size;
      value.hashes = [{ value: local.sha1, algo: 1 }];
    }
    return;
  }
  for (const child of Object.values(value)) localize(child, base);
}

/** O corpo JSON do Modrinth com os downloads locais, quando ligados. */
export function localizeModrinth(body: string, base: string): string {
  if (!enabled) return body;
  const parsed = JSON.parse(body) as unknown;
  localize(parsed, base);
  return JSON.stringify(parsed);
}

/** O corpo da CurseForge com os downloads locais, quando ligados. */
export function localizeCurseforge(body: unknown, base: string): unknown {
  if (!enabled || body === undefined) return body;
  const copy = structuredClone(body);
  localize(copy, base);
  return copy;
}
