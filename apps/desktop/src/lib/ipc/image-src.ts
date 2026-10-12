/**
 * Endereço de imagem para o WebView (ARCHITECTURE §17.1). As imagens da CurseForge nunca vão
 * direto: viram `warden-img://localhost/<token>` (o token é o link original em base64 seguro
 * para URL), que o Rust baixa e guarda só em memória, com `no-store`, para o cache de disco do
 * WebView não guardar dados da API. O Tauri serve esse protocolo em
 * `http://warden-img.localhost/<token>` no Windows. Qualquer outro endereço (Modrinth) passa
 * direto.
 */
import { convertFileSrc } from '@tauri-apps/api/core';

const PREFIX = 'warden-img://localhost/';

/** Servidores de imagem da CurseForge (o próprio e qualquer subdomínio). */
const CURSEFORGE_IMAGE_HOSTS = ['forgecdn.net', 'curseforge.com'];

/**
 * O endereço `warden-img://` de uma imagem `https:` da CurseForge; `null` para qualquer outra
 * (o proxy do Rust recusaria).
 */
export function curseforgeImageUrl(src: string): string | null {
  let url: URL;
  try {
    url = new URL(src.trim());
  } catch {
    return null;
  }
  const host = url.hostname.toLowerCase();
  const fromCurseForge = CURSEFORGE_IMAGE_HOSTS.some(
    (known) => host === known || host.endsWith(`.${known}`),
  );
  if (url.protocol !== 'https:' || !fromCurseForge) return null;
  const bytes = new TextEncoder().encode(url.href);
  const token = btoa(String.fromCharCode(...bytes))
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replace(/=+$/, '');
  return `${PREFIX}${token}`;
}

export function imageSrc(url: string | null): string | null {
  const proxied = url === null ? null : (curseforgeImageUrl(url) ?? url);
  if (!proxied?.startsWith(PREFIX)) return proxied;
  try {
    return convertFileSrc(proxied.slice(PREFIX.length), 'warden-img');
  } catch {
    // Fora do app (testes, navegador): o endereço do protocolo.
    return proxied;
  }
}
