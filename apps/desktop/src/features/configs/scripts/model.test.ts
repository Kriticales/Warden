import { describe, expect, it } from 'vitest';

import type { ScriptIds, ScriptProblem } from '../../../lib/ipc/bindings';
import {
  errorLines,
  flattenIds,
  formatLabel,
  idContext,
  insertText,
  matchIds,
  problemsFor,
  reloadTargetPath,
  scriptToolOf,
  toolLabel,
} from './model';

const IDS: ScriptIds = {
  groups: [
    {
      source: 'Minecraft',
      items: ['minecraft:copper_ingot', 'minecraft:stone', 'minecraft:raw_copper'],
      blocks: ['minecraft:copper_block', 'minecraft:stone'],
      itemTags: ['forge:ingots/copper'],
      blockTags: ['minecraft:mineable/pickaxe'],
      fluidTags: ['minecraft:water'],
      recipes: ['minecraft:copper_block'],
    },
    {
      source: 'Create',
      items: ['minecraft:copper_ingot', 'create:copper_sheet'],
      blocks: [],
      itemTags: [],
      blockTags: [],
      fluidTags: [],
      recipes: [],
    },
  ],
  unread: [],
};

describe('modelo do editor de scripts', () => {
  it('reconhece scripts pela pasta e pela extensão', () => {
    expect(scriptToolOf('kubejs/server_scripts/receitas.js')).toBe('kubejs');
    expect(scriptToolOf('kubejs/startup_scripts/sub/a.js')).toBe('kubejs');
    expect(scriptToolOf('KubeJS/Client_Scripts/A.JS')).toBe('kubejs');
    expect(scriptToolOf('scripts/receitas.zs')).toBe('crafttweaker');
    expect(scriptToolOf('kubejs/config/web_server.json')).toBeNull();
    expect(scriptToolOf('kubejs/assets/a.js')).toBeNull();
    expect(scriptToolOf('config/a.js')).toBeNull();
  });

  it('rótulos da barra e do rodapé', () => {
    const six = { tool: 'kubejs', major: 6, version: '2001.6.5' } as const;
    expect(toolLabel('kubejs', six, 'forge', '1.20.1')).toBe('KubeJS 6 · Forge 1.20.1');
    expect(toolLabel('kubejs', { ...six, major: null }, null, '1.20.1')).toBe(
      'KubeJS · Minecraft 1.20.1',
    );
    expect(formatLabel('crafttweaker', { tool: 'crafttweaker', major: 14, version: null })).toBe(
      'ZenScript · CraftTweaker 14',
    );
  });

  it('lista plana de IDs sem repetir e tags com #', () => {
    const flat = flattenIds(IDS);
    expect(flat.filter((option) => option.id === 'minecraft:copper_ingot')).toHaveLength(1);
    expect(flat.find((option) => option.id === 'create:copper_sheet')?.source).toBe('Create');
    expect(flat.find((option) => option.kind === 'itemTag')?.id).toBe('#forge:ingots/copper');
  });

  it('contexto do autocompletar no KubeJS (strings) e no CraftTweaker (colchetes)', () => {
    expect(idContext("  event.add('minecraft:copper_", 'kubejs')).toEqual({
      length: 'minecraft:copper_'.length,
      typed: 'minecraft:copper_',
      kinds: ['item', 'block', 'recipe'],
    });
    expect(idContext("event.add('#forge:ingots/", 'kubejs')?.kinds).toEqual([
      'itemTag',
      'blockTag',
      'fluidTag',
    ]);
    // Fora de uma string, nada.
    expect(idContext("event.add('a', minecraft:co", 'kubejs')).toBeNull();
    expect(idContext("'a' + 'b' + minecraft", 'kubejs')).toBeNull();
    expect(idContext('<item:minecraft:cop', 'crafttweaker')).toEqual({
      length: 'minecraft:cop'.length,
      typed: 'minecraft:cop',
      kinds: ['item'],
    });
    expect(idContext('<tag:items:forge:', 'crafttweaker')?.kinds).toEqual(['itemTag']);
    expect(idContext('<block:minecraft:', 'crafttweaker')?.kinds).toEqual(['block']);
    expect(idContext('<minecraft:sto', 'crafttweaker')?.kinds).toEqual(['item', 'block']);
    expect(idContext('a < b', 'crafttweaker')).toBeNull();
  });

  it('sugestões: começo primeiro, depois o que contém; tags só com #', () => {
    const flat = flattenIds(IDS);
    const copper = matchIds(flat, idContext("'minecraft:copper_", 'kubejs') ?? fail());
    expect(copper.map((option) => option.id)).toEqual([
      'minecraft:copper_ingot',
      'minecraft:copper_block',
      'minecraft:copper_block',
    ]);
    const contains = matchIds(flat, idContext("'copper", 'kubejs') ?? fail());
    expect(contains.map((option) => option.id)).toContain('minecraft:raw_copper');
    const tags = matchIds(flat, idContext("'#forge:", 'kubejs') ?? fail());
    expect(tags.map((option) => option.id)).toEqual(['#forge:ingots/copper']);
    const zs = matchIds(flat, idContext('<tag:items:forge:in', 'crafttweaker') ?? fail());
    expect(zs.map((option) => insertText(option, 'crafttweaker'))).toEqual(['forge:ingots/copper']);
  });

  it('erros do painel: só da ferramenta, os do arquivo primeiro; linhas marcadas', () => {
    const make = (file: string | null, line: number | null, logFile: string | null) =>
      ({
        level: 'error',
        file,
        line,
        column: null,
        message: 'x',
        logLine: 'x',
        logFile,
      }) satisfies ScriptProblem;
    const problems = [
      make('kubejs/server_scripts/b.js', 2, 'logs/kubejs/server.log'),
      make('scripts/a.zs', 1, 'logs/crafttweaker.log'),
      make('kubejs/server_scripts/a.js', 7, 'logs/kubejs/server.log'),
      make(null, null, 'logs/kubejs/server.log'),
      make(null, null, null),
      make(null, null, 'crafttweaker.log'),
    ];
    const kubejs = problemsFor(problems, 'kubejs', 'kubejs/server_scripts/a.js');
    expect(kubejs.map((p) => p.file)).toEqual([
      'kubejs/server_scripts/a.js',
      'kubejs/server_scripts/b.js',
      null,
      null,
    ]);
    const zs = problemsFor(problems, 'crafttweaker', 'scripts/a.zs');
    expect(zs.map((p) => p.logFile)).toEqual(['logs/crafttweaker.log', 'crafttweaker.log']);
    expect(errorLines(problems, 'kubejs/server_scripts/a.js')).toEqual([7]);
  });

  it('caminho da recarga da tela do teste', () => {
    const kubejs = { tool: 'kubejs', major: 6, version: null } as const;
    const ct = { tool: 'crafttweaker', major: 14, version: null } as const;
    expect(reloadTargetPath({ kubejs, crafttweaker: ct })).toBe('kubejs/server_scripts/scripts.js');
    expect(reloadTargetPath({ kubejs: null, crafttweaker: ct })).toBe('scripts/scripts.zs');
    expect(reloadTargetPath({ kubejs: null, crafttweaker: null })).toBeNull();
  });
});

function fail(): never {
  throw new Error('contexto esperado');
}
