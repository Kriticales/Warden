import { describe, expect, it } from 'vitest';

import { OPTIONS, SESSION, fileView, makeCapture } from './capture.fixtures';
import { buildRequest, initialSelection, selectedCount } from './model';

describe('seleção da captura', () => {
  it('pré-seleção da SPEC T15 e o pedido só com o escolhido', () => {
    const view = makeCapture();
    const selection = initialSelection(view);
    expect(selection.files).toEqual({
      'config/sodium-options.json': true,
      'saves/Mundo de teste/serverconfig/create-server.toml': false,
      'config/jade/plugins.json': false,
    });
    expect(selection.options).toEqual({ renderDistance: true, 'key_key.zoom': false });
    expect(selectedCount(selection)).toBe(2);
    expect(buildRequest(view, selection)).toEqual({
      sessionId: SESSION,
      files: [
        {
          path: 'config/sodium-options.json',
          resolution: { kind: 'take' },
          expectedPackSha256: 'pack-config/sodium-options.json',
        },
      ],
      optionKeys: ['renderDistance'],
      expectedOptionsSha256: 'pack-options.txt',
    });
  });

  it('chave que também mudou no pack começa desmarcada; conflito sem junção começa no pack', () => {
    const view = makeCapture({
      files: [
        fileView({ path: 'config/a.toml' }, { conflict: { packValues: [], mergeable: false } }),
        {
          ...OPTIONS,
          conflict: {
            packValues: [{ key: 'renderDistance', before: '12', after: '10' }],
            mergeable: true,
          },
        },
      ],
    });
    const selection = initialSelection(view);
    expect(selection.options.renderDistance).toBe(false);
    expect(selection.conflicts['config/a.toml']).toEqual({ kind: 'keepPack' });
    expect(selectedCount(selection)).toBe(0);
    expect(buildRequest(view, selection).files).toEqual([]);
  });
});
