/**
 * "Testar como o jogador recebe" nos E2E (L-07): a URL de download dos Releases do
 * packwiz-installer simulada e os arquivos dos mods do pack de teste.
 *
 * - `/installer/<arquivo>`: o bootstrap e o packwiz-installer **reais**, lidos do cache do
 *   `cargo xtask installer` (o app baixa daqui com `WARDEN_INSTALLER_BASE` e confere o SHA-256
 *   fixado, como faria com o GitHub). Sem o cache, o E2E desse teste é pulado.
 * - `/jogador/<nome>.jar`: um "jar" sintético (`jogadorJar`), baixado pelo installer do jeito
 *   que o jogador baixaria do Modrinth.
 */
import { existsSync, readFileSync } from 'node:fs';
import type { ServerResponse } from 'node:http';
import { homedir } from 'node:os';
import { join } from 'node:path';

/** Os jars fixados (os mesmos nomes do cache da `cargo xtask installer` e do app). */
export const INSTALLER_JARS = [
  'packwiz-installer-bootstrap-0.0.3.jar',
  'packwiz-installer-0.5.14.jar',
] as const;

/** Pasta do cache da `cargo xtask installer` (mesma regra de `xtask/src/installer.rs`). */
export function installerCacheDir(): string {
  const explicit = process.env.WARDEN_INSTALLER_CACHE;
  if (explicit) return explicit;
  if (process.platform === 'win32') {
    return join(process.env.LOCALAPPDATA ?? '', 'Warden-dev', 'cache', 'installer');
  }
  const xdg = process.env.XDG_CACHE_HOME;
  const cache = xdg !== undefined && xdg !== '' ? xdg : join(homedir(), '.cache');
  return join(cache, 'Warden-dev', 'installer');
}

/** Se os dois jars estão no cache. */
export function installerAvailable(): boolean {
  return INSTALLER_JARS.every((name) => existsSync(join(installerCacheDir(), name)));
}

/** Os bytes do "jar" de um mod do pack de teste. */
export function jogadorJar(name: string): Buffer {
  return Buffer.from(`jar de teste do jogador: ${name}\n`, 'utf8');
}

/** Atende `/installer/…` e `/jogador/…`; devolve `false` se o caminho não for deles. */
export function servePlayer(path: string, response: ServerResponse): boolean {
  const installer = /^\/installer\/([^/?]+)$/.exec(path);
  if (installer) {
    const name = INSTALLER_JARS.find((jar) => jar === installer[1]);
    const file = name ? join(installerCacheDir(), name) : null;
    if (file && existsSync(file)) {
      response
        .writeHead(200, { 'content-type': 'application/java-archive' })
        .end(readFileSync(file));
    } else {
      response.writeHead(404).end();
    }
    return true;
  }
  const jar = /^\/jogador\/([a-z0-9-]+\.jar)$/.exec(path);
  if (jar) {
    response
      .writeHead(200, { 'content-type': 'application/java-archive' })
      .end(jogadorJar(jar[1] ?? ''));
    return true;
  }
  return false;
}
