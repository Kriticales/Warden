import { describe, expect, it } from 'vitest';

import type { AiSettingsView, KeyOverview } from '../../../lib/ipc/bindings';
import {
  firstUsable,
  keyState,
  moveItem,
  providerRole,
  refusedCount,
  successorAfterRemoval,
  whenParts,
} from './model';

function key(overrides: Partial<KeyOverview> = {}): KeyOverview {
  return { kind: 'openai', configured: true, hint: 'a91c', check: null, ...overrides };
}

function view(overrides: Partial<AiSettingsView> & { keys?: string[]; refused?: string[] } = {}) {
  const { keys = [], refused = [], ...rest } = overrides;
  const order = rest.order ?? ['anthropic', 'gemini', 'openai'];
  const providers = (['gemini', 'openai', 'anthropic'] as const).map((provider) => ({
    provider,
    keyConfigured: keys.includes(provider),
    keyRefused: refused.includes(provider),
    models: null,
  }));
  const usable = order.find((p) => keys.includes(p) && !refused.includes(p)) ?? order[0];
  return {
    provider: usable ?? 'gemini',
    keyConfigured: true,
    models: null,
    providers,
    order,
    ...rest,
  } satisfies AiSettingsView;
}

describe('situação da chave', () => {
  it('sem chave, testando, recusada, funcionando e não testada', () => {
    expect(keyState(undefined, false)).toBe('none');
    expect(keyState(key({ configured: false }), true)).toBe('none');
    expect(keyState(key(), true)).toBe('testing');
    const at = '2026-10-25T21:40:00Z';
    expect(keyState(key({ check: { outcome: 'refused', at } }), false)).toBe('refused');
    expect(keyState(key({ check: { outcome: 'working', at } }), false)).toBe('ok');
    expect(keyState(key(), false)).toBe('untested');
  });

  it('conta as recusadas no último teste', () => {
    const at = '2026-10-25T21:40:00Z';
    expect(refusedCount(undefined)).toBe(0);
    expect(
      refusedCount({
        keys: [
          key({ kind: 'github', check: { outcome: 'refused', at } }),
          key({ kind: 'openai', check: { outcome: 'refused', at } }),
          key({ kind: 'gemini', check: { outcome: 'working', at } }),
          // Sem chave no cofre, o teste antigo não conta.
          key({ kind: 'curseforge', configured: false, check: { outcome: 'refused', at } }),
        ],
        migration: null,
      }),
    ).toBe(2);
  });
});

describe('ordem dos provedores (ADR-0070)', () => {
  it('o papel de cada provedor: em uso, reserva, sem chave e recusada', () => {
    const current = view({ keys: ['anthropic', 'openai'], refused: ['openai'] });
    expect(providerRole(current, 'anthropic')).toBe('use');
    expect(providerRole(current, 'gemini')).toBe('noKey');
    expect(providerRole(current, 'openai')).toBe('refused');
    const both = view({ keys: ['anthropic', 'openai'] });
    expect(providerRole(both, 'openai')).toBe('reserve');
  });

  it('o primeiro utilizável e quem assume ao remover a chave do provedor em uso', () => {
    const current = view({ keys: ['anthropic', 'openai'] });
    expect(firstUsable(current)).toBe('anthropic');
    expect(successorAfterRemoval(current, 'anthropic')).toBe('openai');
    // Tirar uma reserva não muda nada.
    expect(successorAfterRemoval(current, 'openai')).toBeUndefined();
    const alone = view({ keys: ['anthropic'], refused: [] });
    expect(successorAfterRemoval(alone, 'anthropic')).toBeNull();
    expect(firstUsable(view({ keys: [] }))).toBeNull();
  });

  it('mover um item respeita os limites', () => {
    expect(moveItem(['a', 'b', 'c'], 0, 1)).toEqual(['b', 'a', 'c']);
    expect(moveItem(['a', 'b', 'c'], 2, -2)).toEqual(['c', 'a', 'b']);
    expect(moveItem(['a', 'b', 'c'], 0, -1)).toEqual(['a', 'b', 'c']);
    expect(moveItem(['a', 'b', 'c'], 2, 1)).toEqual(['a', 'b', 'c']);
  });
});

describe('hora do último teste', () => {
  it('hoje, ontem ou outro dia; data inválida não vira texto', () => {
    const now = new Date(2026, 9, 25, 22, 0).getTime();
    expect(whenParts(new Date(2026, 9, 25, 21, 40).toISOString(), now)).toEqual({
      relation: 'today',
      hora: '21:40',
      data: '25/10/2026',
    });
    expect(whenParts(new Date(2026, 9, 24, 9, 5).toISOString(), now)?.relation).toBe('yesterday');
    expect(whenParts(new Date(2026, 9, 3, 14, 0).toISOString(), now)?.relation).toBe('other');
    expect(whenParts('não é data', now)).toBeNull();
  });
});
