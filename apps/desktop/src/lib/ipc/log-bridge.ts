/**
 * Ponte para o comando do `tauri-plugin-log` (`plugin:log|log`), sem o pacote
 * `@tauri-apps/plugin-log`: o npm não tem a versão do pacote JS que acompanha a crate do Rust
 * (2.10), e o `tauri build` recusa versões diferentes. O contrato do comando é o do pacote
 * oficial: `level` (1 trace a 5 error) e `message`; o resto é opcional.
 */
import { invoke } from '@tauri-apps/api/core';

/** Níveis aceitos pelo plugin (`log::Level` do Rust). */
export const LOG_LEVELS = { trace: 1, debug: 2, info: 3, warn: 4, error: 5 } as const;

export type LogLevelName = keyof typeof LOG_LEVELS;

/** Grava uma linha nos registros do Warden (alvo `webview`). */
export async function writeLog(level: LogLevelName, message: string): Promise<void> {
  await invoke('plugin:log|log', { level: LOG_LEVELS[level], message });
}
