/**
 * Links externos: abrem no navegador do sistema pelo plugin `opener`, e só com `https:`
 * (ARCHITECTURE §18 e §20; a capability só permite `https://*`).
 */
import { openUrl } from '@tauri-apps/plugin-opener';

import { log } from './log';

/** O endereço como URL `https:` válida, ou `null`. */
export function safeExternalUrl(raw: string | null | undefined): URL | null {
  if (!raw) {
    return null;
  }
  try {
    const url = new URL(raw);
    return url.protocol === 'https:' ? url : null;
  } catch {
    return null;
  }
}

/** Abre o link no navegador. Endereço que não é `https:` é ignorado (e registrado). */
export async function openExternal(raw: string): Promise<void> {
  const url = safeExternalUrl(raw);
  if (!url) {
    log.warn('link externo recusado (só https é aberto)');
    return;
  }
  try {
    await openUrl(url.href);
  } catch (error) {
    log.error('falha ao abrir um link no navegador', error);
  }
}
