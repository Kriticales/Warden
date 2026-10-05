// @vitest-environment node
/**
 * Guarda de contrato (QUALITY §4.1): todo comando registrado no Rust
 * (`src-tauri/src/commands/mod.rs`) é usado pela interface. Conta como uso `commands.<nome>`
 * no código das telas (`src/features/`, `src/app/`, `src/routes/`), fora de testes.
 *
 * `PENDING` lista os comandos que ainda não têm tela, com a tarefa dona. A lista só pode
 * diminuir: um comando que passa a ser usado e continua aqui também falha o teste.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

import { describe, expect, it } from 'vitest';

const DESKTOP = join(import.meta.dirname, '..', '..', '..');
const REGISTRY = join(DESKTOP, 'src-tauri', 'src', 'commands', 'mod.rs');
const BINDINGS = join(DESKTOP, 'src', 'lib', 'ipc', 'bindings.ts');
const CONSUMERS = ['src/features', 'src/app', 'src/routes'];

/** Comandos sem tela ainda, com a tarefa que vai usá-los. Só diminui. */
const PENDING: Record<string, string> = {
  settings_get: 'P1-13 (Configurações)',
  settings_update: 'P1-13 (Configurações)',
  secrets_status: 'P1-13 (Configurações → Chaves e contas)',
  secrets_set: 'P1-13 (Configurações → Chaves e contas)',
  secrets_test: 'P1-13 (Configurações → Chaves e contas)',
  secrets_remove: 'P1-13 (Configurações → Chaves e contas)',
  secrets_backend_get: 'P1-13 (Configurações → Chaves e contas)',
  secrets_backend_set: 'P1-13 (Configurações → Chaves e contas)',
  catalog_minecraft_versions: 'P1-07 (Criar pack: versão do Minecraft)',
  catalog_loader_versions: 'P1-07 (Criar pack: loader e versão)',
  java_choice: 'P1-08 (Ajustes do teste: Java automático e o motivo) e L-04 (Testar)',
};

/** Nomes `snake_case` dos comandos em `collect_commands![…]`. */
export function registeredCommands(source: string): string[] {
  const block = /collect_commands!\[([\s\S]*?)\]/.exec(source)?.[1] ?? '';
  return [...block.matchAll(/(?:\w+::)*(\w+)\s*,?/g)]
    .map((match) => match[1] ?? '')
    .filter((name) => name.length > 0);
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
  const registered = registeredCommands(readFileSync(REGISTRY, 'utf8'));
  const bindings = readFileSync(BINDINGS, 'utf8');
  const used = usedCommands();

  it('lê o registro do Rust', () => {
    expect(registered).toContain('app_info');
    expect(registered).toContain('operation_cancel');
    expect(new Set(registered).size).toBe(registered.length);
  });

  it('cada comando registrado está no bindings.ts gerado', () => {
    for (const name of registered) {
      expect(bindings, name).toContain(`${camelCase(name)}: (`);
    }
  });

  it('cada comando registrado é usado por uma tela (ou está pendente, com a tarefa dona)', () => {
    const unused = registered.filter((name) => !used.has(camelCase(name)) && !(name in PENDING));
    expect(unused, `comandos sem uso em ${CONSUMERS.join(', ')}`).toEqual([]);
  });

  it('a lista de pendentes só diminui: nenhum pendente já é usado ou deixou de existir', () => {
    const stale = Object.keys(PENDING).filter(
      (name) => used.has(camelCase(name)) || !registered.includes(name),
    );
    expect(stale, 'tire estes de PENDING').toEqual([]);
  });

  it('o leitor do registro entende a forma do mod.rs', () => {
    expect(
      registeredCommands('collect_commands![\n  app::app_info,\n  secrets::secrets_set,\n]'),
    ).toEqual(['app_info', 'secrets_set']);
    expect(camelCase('secrets_backend_get')).toBe('secretsBackendGet');
    expect(relative(DESKTOP, REGISTRY)).toBe(join('src-tauri', 'src', 'commands', 'mod.rs'));
  });
});
