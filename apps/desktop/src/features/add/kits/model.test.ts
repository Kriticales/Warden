import { describe, expect, it } from 'vitest';

import { FABRIC_KIT } from '../../packs/create/initial-mods/initial-mods.fixtures';
import { addChoices, defaultSelection, kitChoice, selectedItems, toggled } from './model';

describe('seleção do kit', () => {
  it('vem com os itens marcados nos dados e o C2ME desmarcado', () => {
    const selection = defaultSelection(FABRIC_KIT);
    expect([...selection]).toEqual(['AANobbMI', 'gvQqBUqZ', 'uXXizFIs', '5ZwdcRci']);
    expect(selection.has('VSNURh3q')).toBe(false);
  });

  it('liga e desliga sem mexer na seleção recebida', () => {
    const base = defaultSelection(FABRIC_KIT);
    const withC2me = toggled(base, 'VSNURh3q');
    expect(withC2me.has('VSNURh3q')).toBe(true);
    expect(base.has('VSNURh3q')).toBe(false);
    expect(toggled(withC2me, 'VSNURh3q').has('VSNURh3q')).toBe(false);
  });

  it('o pedido segue a ordem do kit, não a ordem dos cliques', () => {
    const selection = toggled(toggled(new Set<string>(), 'gvQqBUqZ'), 'AANobbMI');
    expect(selectedItems(FABRIC_KIT, selection).map((item) => item.name)).toEqual([
      'Sodium',
      'Lithium',
    ]);
    expect(kitChoice(FABRIC_KIT, selection)).toEqual({
      id: 'fabric-moderno',
      projects: ['AANobbMI', 'gvQqBUqZ'],
    });
    expect(addChoices(FABRIC_KIT, selection)).toEqual([
      { source: 'modrinth', projectId: 'AANobbMI' },
      { source: 'modrinth', projectId: 'gvQqBUqZ' },
    ]);
  });
});
