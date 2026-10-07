import { describe, expect, it } from 'vitest';

import type { ConfigFile } from '../../lib/ipc/bindings';
import {
  baseName,
  buildTree,
  filterFiles,
  formatKey,
  isGameOptions,
  isModernMinecraft,
  languageOf,
  noticesFor,
  type TreeNode,
} from './model';

function file(path: string, binary = false): ConfigFile {
  return { path, size: 1, modifiedAtMs: null, binary };
}

function names(nodes: readonly TreeNode[]): string[] {
  return nodes.map((node) => (node.kind === 'dir' ? `${node.name}/` : node.name));
}

describe('árvore de arquivos', () => {
  const files = [
    file('options.txt'),
    file('config/b.toml'),
    file('config/A.json'),
    file('config/jei/jei-client.ini'),
    file('config/mod10.toml'),
    file('config/mod2.toml'),
    file('defaultconfigs/x.toml'),
    file('saves/Mundo/serverconfig/create-server.toml'),
  ];

  it('põe pastas antes dos arquivos, em ordem natural, e junta pastas de filho único', () => {
    const tree = buildTree(files);
    expect(names(tree)).toEqual([
      'config/',
      'defaultconfigs/',
      'saves/Mundo/serverconfig/',
      'options.txt',
    ]);
    const config = tree[0];
    expect(config?.kind === 'dir' && names(config.children)).toEqual([
      'jei/',
      'A.json',
      'b.toml',
      'mod2.toml',
      'mod10.toml',
    ]);
    expect(config?.kind === 'dir' && config.count).toBe(5);
  });

  it('o filtro ignora maiúsculas e acentos e exige todas as palavras', () => {
    expect(filterFiles(files, 'JEI').map((f) => f.path)).toEqual(['config/jei/jei-client.ini']);
    expect(filterFiles(files, 'mundo server').map((f) => f.path)).toEqual([
      'saves/Mundo/serverconfig/create-server.toml',
    ]);
    expect(filterFiles(files, 'inexistente')).toEqual([]);
    expect(filterFiles(files, '   ')).toHaveLength(files.length);
  });
});

describe('linguagem do realce', () => {
  it.each([
    ['config/a.toml', 'toml'],
    ['config/a.json5', 'json'],
    ['config/a.jsonc', 'json'],
    ['config/a.yml', 'yaml'],
    ['config/a.cfg', 'properties'],
    ['options.txt', 'properties'],
    ['kubejs/server_scripts/x.js', 'javascript'],
    ['scripts/x.zs', 'plain'],
    ['LEIAME', 'plain'],
  ])('%s → %s', (path, expected) => {
    expect(languageOf(path)).toBe(expected);
  });

  it('nome do formato e do arquivo', () => {
    expect(formatKey('plain')).toBe('texto');
    expect(formatKey('typescript')).toBe('javascript');
    expect(formatKey('toml')).toBe('toml');
    expect(baseName('config/jei/a.ini')).toBe('a.ini');
    expect(isGameOptions('optionsof.txt')).toBe(true);
    expect(isGameOptions('config/options.txt')).toBe(false);
  });
});

describe('avisos contextuais', () => {
  const base = { origin: 'pack', loader: 'forge', minecraft: '1.20.1' } as const;

  it('serverconfig só no pack, em config/, com Forge/NeoForge 1.13+', () => {
    const path = 'config/create-server.toml';
    expect(noticesFor({ ...base, path })).toContain('serverconfig');
    expect(noticesFor({ ...base, path, minecraft: '1.12.2' })).not.toContain('serverconfig');
    expect(noticesFor({ ...base, path, loader: 'fabric' })).not.toContain('serverconfig');
    expect(noticesFor({ ...base, path, origin: 'instance' })).not.toContain('serverconfig');
    expect(noticesFor({ ...base, path: 'defaultconfigs/create-server.toml' })).not.toContain(
      'serverconfig',
    );
  });

  it('reescrita do Forge e, no NeoForge, a correção de valores', () => {
    expect(noticesFor({ ...base, path: 'config/a.toml' })).toEqual(['reescrita']);
    expect(noticesFor({ ...base, path: 'config/a.cfg', minecraft: '1.12.2' })).toEqual([
      'reescrita',
    ]);
    expect(noticesFor({ ...base, loader: 'neoforge', path: 'config/a.toml' })).toEqual([
      'reescrita',
      'neoforge',
    ]);
    expect(noticesFor({ ...base, path: 'config/a.json' })).toEqual([]);
  });

  it('options.txt no pack substitui as preferências', () => {
    expect(noticesFor({ ...base, loader: 'fabric', path: 'options.txt' })).toEqual(['options']);
    expect(noticesFor({ ...base, origin: 'instance', path: 'options.txt' })).toEqual([]);
  });

  it('versões do Minecraft', () => {
    expect(isModernMinecraft('1.13')).toBe(true);
    expect(isModernMinecraft('1.21.1')).toBe(true);
    expect(isModernMinecraft('1.7.10')).toBe(false);
    expect(isModernMinecraft(null)).toBe(false);
    expect(isModernMinecraft('26.1')).toBe(true);
  });
});
