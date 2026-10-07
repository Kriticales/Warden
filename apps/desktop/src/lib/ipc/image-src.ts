/**
 * Endereço de imagem para o WebView. As imagens da CurseForge chegam como
 * `warden-img://localhost/<token>` (o Rust as baixa e guarda só em memória, ARCHITECTURE
 * §17.1); o Tauri serve esse protocolo em `http://warden-img.localhost/<token>` no Windows.
 * Qualquer outro endereço (Modrinth) passa direto.
 */
import { convertFileSrc } from '@tauri-apps/api/core';

const PREFIX = 'warden-img://localhost/';

export function imageSrc(url: string | null): string | null {
  if (!url?.startsWith(PREFIX)) return url;
  try {
    return convertFileSrc(url.slice(PREFIX.length), 'warden-img');
  } catch {
    // Fora do app (testes, navegador): o endereço original.
    return url;
  }
}
