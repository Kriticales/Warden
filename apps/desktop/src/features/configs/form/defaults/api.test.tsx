/**
 * "Restaurar padrão" em Problemas (C-06) invalida **todas** as consultas que mostram o arquivo
 * ou o pack: a seção Configs (árvore, arquivo aberto, busca e formulário), a linha do pack, a
 * lista de Meus packs e o relatório de Problemas, mesmo as que não estão na tela agora.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it } from 'vitest';

import type { PackId } from '../../../../lib/ipc/bindings';
import { mockBackend } from '../../../../test/backend';
import { diagnosticsKeys } from '../../../diagnostics/api';
import { packsKeys } from '../../../packs/api';
import { configsKeys } from '../../api';
import { searchKeys } from '../../search/api';
import { usePackRestoreWrite } from './api';

const PACK = 'pack-1' as PackId;
const FILE = 'config/chunks-server.toml';

describe('usePackRestoreWrite', () => {
  it('grava com o hash da leitura e invalida tudo o que mostra o arquivo e o pack', async () => {
    const backend = mockBackend({
      config_write: () => ({ hash: 'h2', size: 1, changed: true }),
    });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const keys = [
      configsKeys.tree(PACK, 'pack'),
      configsKeys.file(PACK, 'pack', FILE),
      searchKeys.query(PACK, 'pack', '', null, true),
      packsKeys.row(PACK),
      packsKeys.list,
      diagnosticsKeys.report(PACK),
    ];
    for (const key of keys) client.setQueryData(key, { visto: true });
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const { result } = renderHook(() => usePackRestoreWrite(PACK), { wrapper });

    await act(async () => {
      await result.current.restore(FILE, { before: 'a = 900\n', after: 'a = 64\n', hash: 'h1' });
    });

    expect(backend.callsOf('config_write')[0]?.args).toMatchObject({
      packId: PACK,
      origin: 'pack',
      path: FILE,
      text: 'a = 64\n',
      expectedHash: 'h1',
    });
    for (const key of keys) {
      expect(client.getQueryState(key)?.isInvalidated, JSON.stringify(key)).toBe(true);
    }
  });
});
