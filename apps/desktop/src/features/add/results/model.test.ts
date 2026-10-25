import { describe, expect, it } from 'vitest';

import { FABRIC_API, LITHIUM, MODMENU, SODIUM } from '../add.fixtures';
import { nextNameSort, readViewMode, saveViewMode, shownItems, sortByName } from './model';

function memoryStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => {
      data.set(key, value);
    },
    data,
  };
}

describe('resultados: modo, o que aparece e a ordem pelo nome', () => {
  it('o modo lembrado: Lista na primeira vez, o último salvo depois, Lista se o armazenamento falhar', () => {
    const storage = memoryStorage();
    expect(readViewMode(storage)).toBe('lista');
    saveViewMode('cartoes', storage);
    expect(readViewMode(storage)).toBe('cartoes');
    expect(readViewMode(memoryStorage({ 'warden.adicionar.modo': 'outra-coisa' }))).toBe('lista');
    const broken = {
      getItem: () => {
        throw new Error('sem armazenamento');
      },
      setItem: () => {
        throw new Error('sem armazenamento');
      },
    };
    expect(readViewMode(broken)).toBe('lista');
    expect(() => {
      saveViewMode('cartoes', broken);
    }).not.toThrow();
  });

  it('bibliotecas escondidas à parte; o que já está no pack some só quando pedido', () => {
    const items = [SODIUM, FABRIC_API, LITHIUM, MODMENU];
    const inventoryKeys = new Set(['modrinth:AANobbMI']);
    const hidden = shownItems(items, { hideLibraries: true, showInPack: true, inventoryKeys });
    expect(hidden.items.map((item) => item.title)).toEqual(['Sodium', 'Lithium', 'Mod Menu']);
    expect(hidden.hiddenLibraries.map((item) => item.title)).toEqual(['Fabric API']);
    const all = shownItems(items, { hideLibraries: false, showInPack: true, inventoryKeys });
    expect(all.items).toHaveLength(4);
    expect(all.hiddenLibraries).toEqual([]);
    // Sodium pelo inventário, Lithium pela busca: os dois saem.
    const notInPack = shownItems(items, { hideLibraries: true, showInPack: false, inventoryKeys });
    expect(notInPack.items.map((item) => item.title)).toEqual(['Mod Menu']);
  });

  it('a ordem pelo nome alterna entre A→Z, Z→A e a da busca', () => {
    expect(nextNameSort(null)).toBe('asc');
    expect(nextNameSort('asc')).toBe('desc');
    expect(nextNameSort('desc')).toBeNull();
    const items = [SODIUM, MODMENU, LITHIUM];
    expect(sortByName(items, 'asc').map((item) => item.title)).toEqual([
      'Lithium',
      'Mod Menu',
      'Sodium',
    ]);
    expect(sortByName(items, 'desc').map((item) => item.title)).toEqual([
      'Sodium',
      'Mod Menu',
      'Lithium',
    ]);
    expect(sortByName(items, null)).toEqual(items);
  });
});
