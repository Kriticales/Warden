import { describe, expect, it } from 'vitest';

import {
  groupChanges,
  hasOnlyControlFiles,
  nextVersions,
  summarizeChangelog,
  utcOffsetMinutes,
  versionDomId,
  versionState,
  whenParts,
} from './model';
import { makeChangeSet, makeItemChange, makeUpdated } from './versioning.fixtures';

describe('estado da versão', () => {
  it.each([
    [{ isFinal: false, isPublished: false }, 'saved'],
    [{ isFinal: true, isPublished: false }, 'final'],
    [{ isFinal: true, isPublished: true }, 'published'],
    // Publicada vale mesmo se a marca de versão final sumiu.
    [{ isFinal: false, isPublished: true }, 'published'],
  ] as const)('%j → %s', (flags, expected) => {
    expect(versionState(flags)).toBe(expected);
  });
});

describe('agrupamento das mudanças', () => {
  it('separa mods por tipo de mudança e põe resource packs e shaders juntos', () => {
    const changes = makeChangeSet({
      items: [
        makeItemChange({ name: 'A' }),
        makeItemChange({ name: 'B', kind: 'removed' }),
        makeUpdated('C', '1', '2'),
        makeItemChange({ name: 'D', kind: 'adjusted' }),
        makeItemChange({ name: 'Pack', category: 'resourcePack' }),
        makeItemChange({ name: 'Luz', category: 'shader', kind: 'removed' }),
      ],
      configs: ['config/x.toml'],
      loaders: [{ loader: 'forge', change: { old: '47.3.0', new: '47.3.1' } }],
    });
    const groups = groupChanges(changes);
    expect(groups.added.map((i) => i.name)).toEqual(['A']);
    expect(groups.removed.map((i) => i.name)).toEqual(['B']);
    expect(groups.updated.map((i) => i.name)).toEqual(['C']);
    expect(groups.adjusted.map((i) => i.name)).toEqual(['D']);
    expect(groups.others.map((i) => i.name)).toEqual(['Pack', 'Luz']);
    expect(groups.configs).toEqual(['config/x.toml']);
    expect(groups.platform).toBe(true);
  });

  it('só arquivos de controle: nenhum item, config nem Minecraft', () => {
    const control = makeChangeSet({
      files: [
        { path: 'pack.toml', kind: 'modified' },
        { path: 'CHANGELOG.md', kind: 'added' },
      ],
    });
    expect(hasOnlyControlFiles(control)).toBe(true);
    expect(hasOnlyControlFiles(makeChangeSet({ configs: ['config/a.toml'] }))).toBe(false);
    expect(hasOnlyControlFiles(makeChangeSet({ files: [] }))).toBe(false);
  });
});

describe('resumo do changelog', () => {
  it('lê as notas e conta os itens de cada seção', () => {
    const message = [
      'Mochilas novas e ajuste do Create',
      '',
      '### Atenção',
      '- Mods removidos podem apagar blocos.',
      '',
      '### Minecraft e loader',
      '- Minecraft 1.20.1 → 1.20.4',
      '',
      '### Mods adicionados',
      '- A 1',
      '- B 2',
      '',
      '### Mods removidos',
      '- C',
      '',
      '### Mods atualizados',
      '- D 1 → 2',
      '',
      '### Resource packs e shaders',
      '- Adicionado: E',
      '',
      '### Configs alteradas',
      '- config/a.toml',
      '- config/b.toml',
    ].join('\n');
    expect(summarizeChangelog(message)).toEqual({
      notes: 'Mochilas novas e ajuste do Create',
      added: 2,
      removed: 1,
      updated: 1,
      others: 1,
      configs: 2,
      platform: true,
    });
  });

  it('sem notas, não inventa texto; texto vazio dá resumo vazio', () => {
    expect(summarizeChangelog('### Configs alteradas\n- a\r\n').notes).toBeNull();
    expect(summarizeChangelog('')).toEqual({
      notes: null,
      added: 0,
      removed: 0,
      updated: 0,
      others: 0,
      configs: 0,
      platform: false,
    });
  });
});

describe('exemplos do número da versão', () => {
  it('calcula a próxima de cada tamanho', () => {
    expect(nextVersions('1.4.2')).toEqual({ patch: '1.4.3', minor: '1.5.0', major: '2.0.0' });
    expect(nextVersions('0.1.0')).toEqual({ patch: '0.1.1', minor: '0.2.0', major: '1.0.0' });
  });
  it('sem versão ou fora de X.Y.Z, sem exemplo', () => {
    expect(nextVersions(null)).toBeNull();
    expect(nextVersions('2.0.0-beta.1')).toBeNull();
    expect(nextVersions('1.2')).toBeNull();
  });
});

describe('fuso e datas', () => {
  it('o fuso é minutos a leste do UTC (o JS conta a oeste)', () => {
    // Brasília: getTimezoneOffset() = 180.
    const brasilia = { getTimezoneOffset: () => 180 } as Date;
    const utc = { getTimezoneOffset: () => 0 } as Date;
    const tokyo = { getTimezoneOffset: () => -540 } as Date;
    expect(utcOffsetMinutes(brasilia)).toBe(-180);
    expect(utcOffsetMinutes(utc)).toBe(0);
    expect(Object.is(utcOffsetMinutes(utc), 0)).toBe(true);
    expect(utcOffsetMinutes(tokyo)).toBe(540);
  });

  it('a data de um instante RFC 3339, em relação a agora', () => {
    const now = Date.parse('2026-10-07T12:00:00-03:00');
    expect(whenParts('2026-10-07T09:02:00-03:00', now)?.relation).toBe('today');
    expect(whenParts('2026-10-06T09:02:00-03:00', now)?.relation).toBe('yesterday');
    expect(whenParts('2026-09-28T09:02:00-03:00', now)?.relation).toBe('other');
    expect(whenParts('não é data', now)).toBeNull();
  });

  it('o id do elemento de uma versão não tem ponto', () => {
    expect(versionDomId('1.4.2')).toBe('versao-1-4-2');
    expect(versionDomId('2.0.0-beta.1')).toBe('versao-2-0-0-beta-1');
  });
});
