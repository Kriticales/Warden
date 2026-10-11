import { describe, expect, it } from 'vitest';

import { parseCurseforgeLink, resultOfLink } from './link';

describe('links da CurseForge', () => {
  it('reconhece link de projeto, com variações do endereço', () => {
    for (const text of [
      'https://www.curseforge.com/minecraft/mc-mods/jei',
      '  https://www.curseforge.com/minecraft/mc-mods/jei/  ',
      'https://curseforge.com/minecraft/mc-mods/jei?utm=1#x',
      'https://legacy.curseforge.com/minecraft/mc-mods/jei',
      'https://www.curseforge.com/minecraft/mc-mods/jei/files',
      'https://www.curseforge.com/minecraft/texture-packs/faithful-32x',
      'https://www.curseforge.com/minecraft/shaders/complementary',
    ]) {
      expect(parseCurseforgeLink(text), text).toBe('project');
    }
  });

  it('reconhece link de arquivo', () => {
    expect(
      parseCurseforgeLink('https://www.curseforge.com/minecraft/mc-mods/jei/files/4712345'),
    ).toBe('file');
    expect(
      parseCurseforgeLink('https://www.curseforge.com/minecraft/mc-mods/jei/download/4712345'),
    ).toBe('file');
    expect(
      parseCurseforgeLink('https://www.curseforge.com/minecraft/mc-mods/jei/files/latest'),
    ).toBe(null);
  });

  it('recusa o que não é link da CurseForge (inclusive http)', () => {
    for (const text of [
      'jei',
      '',
      'http://www.curseforge.com/minecraft/mc-mods/jei',
      'https://modrinth.com/mod/jei',
      'https://curseforge.com.evil.example/minecraft/mc-mods/jei',
      'https://www.curseforge.com/minecraft/mc-mods',
      'https://www.curseforge.com/minecraft/nada/jei',
      'https://www.curseforge.com/other-game/mc-mods/jei',
      'https://www.curseforge.com/',
    ]) {
      expect(parseCurseforgeLink(text), text).toBeNull();
    }
  });

  it('o projeto do link vira um resultado para a pré-visualização', () => {
    const result = resultOfLink({
      projectId: '238222',
      title: 'Just Enough Items (JEI)',
      kind: 'mod',
      fileId: null,
    });
    expect(result.key).toBe('curseforge:238222');
    expect(result.sources).toEqual([
      { source: 'curseforge', projectId: '238222', slug: '', downloads: 0 },
    ]);
    expect(result.inPack).toBe(false);
  });
});
