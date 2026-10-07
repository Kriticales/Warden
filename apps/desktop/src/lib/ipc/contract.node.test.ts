// @vitest-environment node
/**
 * Guarda de contrato (QUALITY §4.1): todo comando do Rust (`#[tauri::command]` em
 * `src-tauri/src/commands/*.rs`, registrados pelo `build.rs`; ARCHITECTURE §4.1) é usado pela
 * interface. Conta como uso `commands.<nome>` no código das telas (`src/features/`, `src/app/`,
 * `src/routes/`), fora de testes.
 *
 * Um comando que ainda não tem tela leva, na linha logo acima de `#[tauri::command]`, a nota
 * `// pendente-na-ui: <TAREFA> <motivo>`. A nota fica junto do comando (nenhum arquivo
 * compartilhado) e só pode sair: um comando que passa a ser usado e mantém a nota também falha.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

import { describe, expect, it } from 'vitest';

const DESKTOP = join(import.meta.dirname, '..', '..', '..');
const COMMANDS_DIR = join(DESKTOP, 'src-tauri', 'src', 'commands');
const BINDINGS = join(DESKTOP, 'src', 'lib', 'ipc', 'bindings.ts');
const CONSUMERS = ['src/features', 'src/app', 'src/routes'];

const PENDING_NOTE = /^\/\/\s*pendente-na-ui:\s*(\S.*)$/;

export interface RustCommand {
  name: string;
  /** Texto da nota `pendente-na-ui`, se houver. */
  pending: string | undefined;
}

/**
 * Comandos de um arquivo Rust: `fn` logo abaixo de `#[tauri::command]` na coluna 0 (o mesmo
 * critério do `build_registry.rs`; atributos indentados, de módulos de teste, não contam).
 */
export function parseRustCommands(source: string): RustCommand[] {
  const lines = source.split(/\r?\n/);
  const found: RustCommand[] = [];
  lines.forEach((line, index) => {
    if (!line.startsWith('#[tauri::command]')) return;
    const note = PENDING_NOTE.exec((lines[index - 1] ?? '').trim());
    const name = lines
      .slice(index + 1)
      .map((text) => text.trim())
      .find((text) => text !== '' && !text.startsWith('#') && !text.startsWith('//'))
      ?.match(/\bfn\s+(\w+)/)?.[1];
    if (name) found.push({ name, pending: note?.[1] });
  });
  return found;
}

function rustFiles(): string[] {
  return readdirSync(COMMANDS_DIR).flatMap((entry) => {
    const path = join(COMMANDS_DIR, entry);
    if (statSync(path).isDirectory()) return [join(path, 'mod.rs')];
    return entry.endsWith('.rs') && entry !== 'mod.rs' ? [path] : [];
  });
}

/** Todos os comandos do Rust, com a nota de pendência. */
export function rustCommands(): RustCommand[] {
  return rustFiles().flatMap((file) => parseRustCommands(readFileSync(file, 'utf8')));
}

/** `app_info` → `appInfo` (o nome no `bindings.ts`). */
export function camelCase(name: string): string {
  return name.replace(/_([a-z0-9])/g, (_, letter: string) => letter.toUpperCase());
}

function sourceFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      return sourceFiles(path);
    }
    return /\.tsx?$/.test(entry) && !/\.test\.tsx?$/.test(entry) ? [path] : [];
  });
}

function usedCommands(): Set<string> {
  const used = new Set<string>();
  for (const folder of CONSUMERS) {
    for (const file of sourceFiles(join(DESKTOP, folder))) {
      const text = readFileSync(file, 'utf8');
      for (const match of text.matchAll(/\bcommands\.(\w+)\b/g)) {
        if (match[1]) used.add(match[1]);
      }
    }
  }
  return used;
}

describe('guarda de contrato', () => {
  const commands = rustCommands();
  const registered = commands.map((command) => command.name);
  const pending = commands.filter((command) => command.pending).map((command) => command.name);
  const bindings = readFileSync(BINDINGS, 'utf8');
  const used = usedCommands();

  it('lê os comandos do Rust', () => {
    expect(registered).toContain('app_info');
    expect(registered).toContain('operation_cancel');
    expect(new Set(registered).size).toBe(registered.length);
  });

  it('cada comando está no bindings.ts gerado', () => {
    for (const name of registered) {
      expect(bindings, name).toContain(`${camelCase(name)}: (`);
    }
  });

  it('cada comando é usado por uma tela (ou está pendente, com a tarefa dona)', () => {
    const unused = registered.filter(
      (name) => !used.has(camelCase(name)) && !pending.includes(name),
    );
    expect(unused, `comandos sem uso em ${CONSUMERS.join(', ')}`).toEqual([]);
  });

  it('a nota de pendente só diminui: nenhum pendente já é usado', () => {
    const stale = pending.filter((name) => used.has(camelCase(name)));
    expect(stale, 'tire a nota pendente-na-ui destes').toEqual([]);
  });

  it('o leitor entende a forma dos arquivos de comandos', () => {
    const sample = [
      '/// Docs.',
      '// pendente-na-ui: X-01 sem tela',
      '#[tauri::command]',
      '#[specta::specta]',
      'pub(crate) async fn um(a: u8) {}',
      '#[tauri::command]',
      '// comentário',
      'pub(crate) fn dois() {}',
      'mod tests {',
      '    #[tauri::command]',
      '    fn de_teste() {}',
      '}',
    ].join('\n');
    expect(parseRustCommands(sample)).toEqual([
      { name: 'um', pending: 'X-01 sem tela' },
      { name: 'dois', pending: undefined },
    ]);
    expect(camelCase('secrets_backend_get')).toBe('secretsBackendGet');
    expect(relative(DESKTOP, COMMANDS_DIR)).toBe(join('src-tauri', 'src', 'commands'));
  });
});
