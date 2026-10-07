import { describe, expect, it } from 'vitest';

import type { ExportPreview, PreviewFile } from '../../lib/ipc/bindings';
import { buildTree, canExclude, collectAlerts, ignoreRule } from './model';

function file(path: string, overrides: Partial<PreviewFile> = {}): PreviewFile {
  const reference = path.endsWith('.pw.toml');
  return {
    path,
    bytes: 100,
    reference,
    local: !reference && path !== 'pack.toml' && path !== 'index.toml',
    config: path.startsWith('config/'),
    alerts: [],
    ...overrides,
  };
}

function preview(files: PreviewFile[], folders: ExportPreview['folders'] = []): ExportPreview {
  return {
    files,
    folders,
    bytes: files.reduce((sum, f) => sum + f.bytes, 0),
    references: files.filter((f) => f.reference).length,
    preflight: { hygiene: [], unsavedChanges: false, diagnosticErrors: null },
  };
}

describe('buildTree', () => {
  it('separa o controle, agrupa por pasta de primeiro nível e deixa a raiz por último', () => {
    const tree = buildTree(
      preview(
        [
          file('config/b/c.toml'),
          file('config/a.toml', { bytes: 50 }),
          file('index.toml'),
          file('mods/sodium.pw.toml'),
          file('mods/jei.pw.toml'),
          file('options.txt'),
          file('pack.toml'),
        ],
        [{ path: 'config', files: 2, bytes: 150 }],
      ),
    );
    expect(tree.control.map((f) => f.path)).toEqual(['pack.toml', 'index.toml']);
    expect(tree.groups.map((g) => g.folder)).toEqual(['config', 'mods', null]);
    const [config, mods, root] = tree.groups;
    expect(config).toMatchObject({ count: 2, bytes: 150, references: 0 });
    expect(mods).toMatchObject({ count: 2, bytes: 200, references: 2 });
    expect(root?.files.map((f) => f.path)).toEqual(['options.txt']);
  });

  it('pack só com o controle não tem grupos', () => {
    expect(buildTree(preview([file('index.toml'), file('pack.toml')])).groups).toEqual([]);
  });
});

describe('collectAlerts', () => {
  it('um item por alerta, com o caminho', () => {
    const alerts = collectAlerts(
      preview([
        file('options.txt', {
          alerts: [{ kind: 'optionsOverridesPreferences' }, { kind: 'looseRootFile' }],
        }),
        file('config/a.toml'),
      ]),
    );
    expect(alerts).toEqual([
      { path: 'options.txt', alert: { kind: 'optionsOverridesPreferences' } },
      { path: 'options.txt', alert: { kind: 'looseRootFile' } },
    ]);
  });
});

describe('Excluir do pack', () => {
  it('a regra é ancorada na raiz, com barra no fim para pasta', () => {
    expect(ignoreRule('options.txt', false)).toBe('/options.txt');
    expect(ignoreRule('config/x', true)).toBe('/config/x/');
  });

  it('nunca o controle; colchetes pedem a regra à mão', () => {
    expect(canExclude('pack.toml')).toBe(false);
    expect(canExclude('index.toml')).toBe(false);
    expect(canExclude('config/[a].txt')).toBe(false);
    expect(canExclude('config/a.txt')).toBe(true);
  });
});
