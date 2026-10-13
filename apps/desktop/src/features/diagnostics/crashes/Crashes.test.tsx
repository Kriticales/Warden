/**
 * Travamentos na seção Problemas (SPEC T14; ROADMAP D-06) no app inteiro, com o backend
 * simulado: uma linha por causa ("Travou 2 vezes", com as versões do pack de cada vez), a
 * situação, "Ver o que causou", cada vez que travou, a retenção (aviso perto do limite e
 * "Apagar os mais antigos" com confirmação) e os dados sempre atuais depois de um teste ou de
 * apagar. Os critérios estão no nome de cada teste.
 */
import { QueryClient } from '@tanstack/react-query';
import { createMemoryHistory } from '@tanstack/react-router';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { App, createQueryClient } from '../../../app/App';
import { axePage } from '../../../test/axe';
import { ipcError, type Handler } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { storageKeys } from '../../settings/storage/api';
import { invalidateAfterTest, testKeys } from '../../test/api';
import { diagnosticsKeys } from '../api';
import { makeReport } from '../diagnostics.fixtures';
import { crashesKeys } from './api';
import { BALM_GROUP, makeCrashList, makeDetail, makeGroup } from './crashes.fixtures';
import { crashTitle, versionsText } from './text';

function setup(handlers: Record<string, Handler> = {}) {
  return editorBackend({
    handlers: {
      diagnostics_report_get: () => makeReport({ findings: [] }),
      diagnostics_run: () => makeReport({ findings: [] }),
      crashes_list: () => makeCrashList({ groups: [makeGroup(), BALM_GROUP] }),
      ...handlers,
    },
  });
}

async function openCrashes(url: string) {
  renderApp(`${url}/problemas`);
  const block = await screen.findByRole('region', { name: /^Travamentos/ }, { timeout: 10_000 });
  await waitFor(() => {
    expect(within(block).queryByText('Lendo os travamentos…')).toBeNull();
  });
  return block;
}

function rowOf(block: HTMLElement, text: string | RegExp): HTMLElement {
  const row = within(block).getByText(text).closest('tr');
  if (!row) throw new Error(`linha de ${String(text)}`);
  return row;
}

describe('Travamentos em Problemas (T14)', () => {
  it('CA-T14-08: mesma causa vira uma linha "Travou 2 vezes" com as versões de cada vez; causas diferentes, linhas diferentes; axe', async () => {
    const { url } = setup();
    const block = await openCrashes(url);
    const rows = await within(block).findAllByRole('row');
    // Cabeçalho + uma linha por causa.
    expect(rows).toHaveLength(3);

    const npe = rowOf(block, 'O jogo travou com NullPointerException');
    expect(within(npe).getByText('Travou 2 vezes')).toBeDefined();
    expect(within(npe).getByText('versões 1.4.1 e 1.4.2')).toBeDefined();
    expect(within(npe).getByText('Causa não encontrada')).toBeDefined();

    // A outra causa: o título vem do "Por que travou" gravado.
    const balm = rowOf(block, 'Waystones precisa de balm, que não está no pack');
    expect(within(balm).getByText('Travou 1 vez')).toBeDefined();
    expect(within(balm).getByText('versão 1.4.2 com alterações')).toBeDefined();
    expect(within(balm).getByText('Causa encontrada')).toBeDefined();
    // Só uma vez: nada a abrir.
    expect(within(balm).queryByRole('button', { name: /^Ver a/ })).toBeNull();

    // O "Último travamento" da D-03 saiu.
    expect(screen.queryByRole('region', { name: 'Último travamento' })).toBeNull();
    expect(
      within(block).getByText(/Travamentos com a mesma causa ficam numa linha só.*500 MB\.$/),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('"Ver o que causou" abre o resultado do teste mais novo do grupo', async () => {
    const { url } = setup();
    const block = await openCrashes(url);
    const npe = rowOf(block, 'O jogo travou com NullPointerException');
    const link = within(npe).getByRole('link', { name: /^Ver o que causou o travamento de / });
    expect(link.textContent).toBe('Ver o que causou');
    expect(link.getAttribute('href')).toContain('sessao=2026-10-07_14-20-00');
  });

  it('"Ver as 2 vezes" lista cada vez com a versão e o link para o teste; "Ocultar" fecha', async () => {
    const { backend, url } = setup({ crash_get: () => makeDetail() });
    const user = userEvent.setup();
    const block = await openCrashes(url);
    const npe = rowOf(block, 'O jogo travou com NullPointerException');
    const toggle = within(npe).getByRole('button', { name: 'Ver as 2 vezes' });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    await user.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    const list = await within(block).findByRole('list');
    expect(document.getElementById(toggle.getAttribute('aria-controls') ?? '')).not.toBeNull();
    const links = within(list).getAllByRole('link');
    expect(links).toHaveLength(2);
    expect(links[0]?.textContent).toMatch(/^Teste de .* · 1\.4\.2$/);
    expect(links[1]?.textContent).toMatch(/^Teste de .* · 1\.4\.1$/);
    expect(links[1]?.getAttribute('href')).toContain('sessao=2026-10-07_12-20-00');
    expect(backend.callsOf('crash_get')[0]?.args).toMatchObject({ signature: '0a1b2c3d4e5f6071' });

    await user.click(within(npe).getByRole('button', { name: 'Ocultar' }));
    expect(within(block).queryByRole('list')).toBeNull();
  });

  it('situação "Não voltou a acontecer desde a versão" quando um teste depois fechou normalmente', async () => {
    const { url } = setup({
      crashes_list: () =>
        makeCrashList({
          groups: [
            makeGroup({
              count: 1,
              situation: { kind: 'resolved', since: { version: '1.4.1', unsaved: false } },
            }),
          ],
        }),
    });
    const block = await openCrashes(url);
    expect(within(block).getByText('Não voltou a acontecer desde a 1.4.1').className).toContain(
      'status--muted',
    );
  });

  it('nenhum teste travou: a frase de vazio, sem tabela', async () => {
    const { url } = setup({ crashes_list: () => makeCrashList({ groups: [] }) });
    const block = await openCrashes(url);
    expect(within(block).getByText('Nenhum teste deste pack travou.')).toBeDefined();
    expect(within(block).queryByRole('table')).toBeNull();
  });

  it('erro ao ler: o painel de erro com "Tentar de novo"', async () => {
    let calls = 0;
    const { url } = setup({
      crashes_list: () => {
        calls += 1;
        if (calls === 1) ipcError(makeAppError());
        return makeCrashList();
      },
    });
    const block = await openCrashes(url);
    expect(await within(block).findByText('Não foi possível ler os travamentos.')).toBeDefined();
    await userEvent.setup().click(within(block).getByRole('button', { name: 'Tentar de novo' }));
    expect(await within(block).findByText('Travou 2 vezes')).toBeDefined();
  });
});

describe('retenção dos travamentos guardados', () => {
  it('avisa antes de passar de 500 MB', async () => {
    const { url } = setup({
      crashes_list: () =>
        makeCrashList({
          retention: { budgetBytes: 500 * 1024 * 1024, nearLimit: true, overBudget: 0 },
        }),
    });
    const block = await openCrashes(url);
    expect(
      within(block).getByText('Os travamentos guardados estão perto do limite de 500 MB'),
    ).toBeDefined();
    expect(within(block).queryByRole('button', { name: 'Apagar os mais antigos' })).toBeNull();
  });

  it('passou do limite: "Apagar os mais antigos" confirma, apaga e relê tudo o que mostra testes; Cancelar devolve o foco', async () => {
    let over = 3;
    const { backend, url } = setup({
      crashes_list: () =>
        makeCrashList({
          retention: { budgetBytes: 500 * 1024 * 1024, nearLimit: true, overBudget: over },
        }),
      crashes_prune: () => {
        over = 0;
        return 3;
      },
      storage_usage: () => ({ total: 0 }),
    });
    const packId = url.split('/')[2] ?? '';
    const queryClient = createQueryClient();
    // O que outras telas mostram dessas sessões, já em cache.
    queryClient.setQueryData(testKeys.sessions(packId), []);
    queryClient.setQueryData(testKeys.session(packId, '2026-10-01_10-00-00'), {});
    queryClient.setQueryData(diagnosticsKeys.crash(packId, '2026-10-01_10-00-00'), {});
    queryClient.setQueryData(storageKeys.usage, {});
    render(
      <App
        history={createMemoryHistory({ initialEntries: [`${url}/problemas`] })}
        queryClient={queryClient}
      />,
    );
    const user = userEvent.setup();
    const block = await screen.findByRole('region', { name: /^Travamentos/ }, { timeout: 10_000 });
    expect(
      await within(block).findByText('Os travamentos guardados passaram do limite de 500 MB'),
    ).toBeDefined();
    expect(
      within(block).getByText(
        'Os 3 travamentos mais antigos podem ser apagados para liberar espaço.',
      ),
    ).toBeDefined();

    const trigger = within(block).getByRole('button', { name: 'Apagar os mais antigos' });
    await user.click(trigger);
    let dialog = await screen.findByRole('alertdialog', {
      name: 'Apagar os 3 travamentos mais antigos?',
    });
    await user.click(within(dialog).getByRole('button', { name: 'Cancelar' }));
    await waitFor(() => {
      expect(document.activeElement).toBe(trigger);
    });
    expect(backend.callsOf('crashes_prune')).toHaveLength(0);

    await user.click(trigger);
    dialog = await screen.findByRole('alertdialog');
    await user.click(within(dialog).getByRole('button', { name: 'Apagar 3 travamentos' }));
    await waitFor(() => {
      expect(backend.callsOf('crashes_prune')).toHaveLength(1);
    });
    await waitFor(() => {
      expect(within(block).queryByText(/passaram do limite/)).toBeNull();
    });
    expect(queryClient.getQueryState(testKeys.sessions(packId))?.isInvalidated).toBe(true);
    expect(
      queryClient.getQueryState(testKeys.session(packId, '2026-10-01_10-00-00'))?.isInvalidated,
    ).toBe(true);
    expect(
      queryClient.getQueryState(diagnosticsKeys.crash(packId, '2026-10-01_10-00-00'))
        ?.isInvalidated,
    ).toBe(true);
    expect(queryClient.getQueryState(storageKeys.usage)?.isInvalidated).toBe(true);
  });
});

describe('dados sempre atuais', () => {
  it('o fim de um teste relê os Travamentos (a lista e cada grupo ficam sob as sessões)', async () => {
    const queryClient = new QueryClient();
    const packId = 'pack-1';
    queryClient.setQueryData(crashesKeys.list(packId), makeCrashList());
    queryClient.setQueryData(crashesKeys.detail(packId, 'abc'), makeDetail());
    await invalidateAfterTest(queryClient, packId);
    expect(queryClient.getQueryState(crashesKeys.list(packId))?.isInvalidated).toBe(true);
    expect(queryClient.getQueryState(crashesKeys.detail(packId, 'abc'))?.isInvalidated).toBe(true);
  });
});

describe('textos da linha', () => {
  it('versões: uma, duas, com alterações, sem versão e a faixa quando são muitas', () => {
    expect(versionsText([{ version: '1.4.0', unsaved: false }])).toBe('versão 1.4.0');
    expect(
      versionsText([
        { version: null, unsaved: false },
        { version: '1.0', unsaved: true },
        { version: '1.1', unsaved: false },
      ]),
    ).toBe('versões sem versão, 1.0 com alterações e 1.1');
    expect(
      versionsText(['1.0', '1.1', '1.2', '1.3'].map((version) => ({ version, unsaved: false }))),
    ).toBe('versões 1.0 a 1.3');
    // A mesma versão com árvores diferentes aparece uma vez.
    expect(
      versionsText([
        { version: '2.0', unsaved: false },
        { version: '2.0', unsaved: false },
      ]),
    ).toBe('versão 2.0');
  });

  it('título: o achado gravado, a exceção ou a frase sem detalhe', () => {
    expect(crashTitle(BALM_GROUP)).toBe('Waystones precisa de balm, que não está no pack');
    expect(crashTitle(makeGroup())).toBe('O jogo travou com NullPointerException');
    const signature = { ...makeGroup().signature, exception: null };
    expect(crashTitle(makeGroup({ signature }))).toBe('O jogo travou sem deixar a causa no log');
  });
});
