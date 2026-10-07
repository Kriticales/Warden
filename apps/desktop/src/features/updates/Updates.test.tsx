/**
 * Atualizações (T10) no app inteiro com o backend simulado. Os critérios de aceite estão no nome
 * de cada teste; o contador de requisições à API (CA-T10-01), o canal (CA-T10-03) e a falha de
 * rede no domínio (CA-T10-04) são testados em Rust (`crates/warden-project/tests/updates.rs`).
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { axePage } from '../../test/axe';
import { ipcError } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { makeDetails, makeInventory } from '../pack-editor/editor.fixtures';
import { editorBackend } from '../pack-editor/testing';
import {
  ALFA,
  available,
  BETA,
  FIXADO,
  GAMA,
  makePlan,
  makeReport,
  planItem,
  updateItem,
} from './testing';

async function openMods(url: string) {
  renderApp(url);
  await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
  return screen.findByRole('searchbox', { name: 'Buscar no pack' });
}

function rowOf(name: string) {
  const row = screen.getByRole('button', { name }).closest('tr');
  if (!row) throw new Error(`linha de ${name} não encontrada`);
  return row;
}

const REPORT = makeReport([
  available(ALFA, '1.1', 'newA'),
  updateItem(BETA),
  available(GAMA, 'gama-2', '600'),
  updateItem(FIXADO),
]);

function backendWith(report = REPORT, handlers: Record<string, () => unknown> = {}) {
  return editorBackend({
    inventory: makeInventory([ALFA, BETA, GAMA, FIXADO]),
    handlers: { updates_report: () => report, ...handlers },
  });
}

describe('atualizações na lista de Mods (T10)', () => {
  it('CA-T10-02: o botão Atualizar da linha só existe com "Atualização disponível"; axe', async () => {
    const { url } = backendWith();
    await openMods(url);
    expect(await screen.findByText('2 atualizações disponíveis.')).toBeDefined();
    expect(
      within(rowOf('Alfa')).getByRole('button', { name: 'Atualizar Alfa para 1.1' }),
    ).toBeDefined();
    expect(
      within(rowOf('Gama')).getByRole('button', { name: 'Atualizar Gama para gama-2' }),
    ).toBeDefined();
    expect(within(rowOf('Beta')).queryByRole('button', { name: /^Atualizar/ })).toBeNull();
    expect(within(rowOf('Fixado')).queryByRole('button', { name: /^Atualizar/ })).toBeNull();
    expect(rowOf('Fixado').textContent).toContain('Fixado');
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T10-02: nos detalhes, Atualizar fica desabilitado fora de "Atualização disponível"', async () => {
    const { url } = backendWith(REPORT, { item_details: () => makeDetails(BETA) });
    await openMods(url);
    await screen.findByText('2 atualizações disponíveis.');
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Beta' }));
    const panel = await screen.findByRole('dialog');
    const button = await within(panel).findByRole('button', { name: 'Atualizar' });
    expect(button.hasAttribute('disabled')).toBe(true);
    expect(within(panel).getByText('Em dia')).toBeDefined();
  });

  it('nos detalhes de um item com atualização, o botão abre a revisão de um item', async () => {
    const { url } = backendWith(REPORT, {
      item_details: () => makeDetails(ALFA),
      updates_plan: () => makePlan([planItem(available(ALFA, '1.1', 'newA'))]),
    });
    await openMods(url);
    await screen.findByText('2 atualizações disponíveis.');
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Alfa' }));
    const panel = await screen.findByRole('dialog');
    expect(await within(panel).findByText('Atualização disponível: 1.1')).toBeDefined();
    await user.click(within(panel).getByRole('button', { name: 'Atualizar para 1.1' }));
    expect(await screen.findByRole('heading', { name: 'Atualizar Alfa' })).toBeDefined();
  });

  it('Verificar atualizações consulta (manual) e mostra a faixa; falha de rede não vira "Em dia" (CA-T10-04)', async () => {
    const failed = makeReport([
      available(ALFA, '1.1'),
      updateItem(BETA, { status: 'failed', reason: 'offline', detail: 'sem rede' }),
    ]);
    const { backend, url } = editorBackend({
      inventory: makeInventory([ALFA, BETA]),
      handlers: { updates_report: () => null, updates_check: () => failed },
    });
    await openMods(url);
    expect(screen.queryByText(/atualizações? disponíve/)).toBeNull();
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Verificar atualizações' }));
    expect(await screen.findByText('1 atualização disponível.')).toBeDefined();
    expect(screen.getByText('1 item não pôde ser verificado.')).toBeDefined();
    const manual = backend
      .callsOf('updates_check')
      .filter((call) => call.args.trigger === 'manual');
    expect(manual).toHaveLength(1);
    // O item que falhou mostra o estado e o motivo, nunca "Em dia".
    const beta = rowOf('Beta');
    expect(beta.textContent).toContain('Não foi possível verificar');
    expect(beta.textContent).not.toContain('Em dia');
  });

  it('ao abrir o pack pede a verificação automática (o Rust decide pelo intervalo)', async () => {
    const { backend, url } = backendWith();
    await openMods(url);
    await waitFor(() => {
      expect(
        backend.callsOf('updates_check').filter((call) => call.args.trigger === 'auto'),
      ).toHaveLength(1);
    });
  });

  it('CurseForge sem chave: aviso com Abrir Configurações, sem marcar como atualizado', async () => {
    const report = makeReport([
      updateItem(GAMA, { status: 'notChecked', reason: 'keyMissing' }),
      updateItem(ALFA),
    ]);
    const { url } = editorBackend({
      inventory: makeInventory([ALFA, GAMA]),
      handlers: { updates_report: () => report },
    });
    await openMods(url);
    expect(await screen.findByText('1 item da CurseForge não foi verificado.')).toBeDefined();
    expect(screen.getByRole('link', { name: 'Abrir Configurações' }).getAttribute('href')).toBe(
      '/configuracoes',
    );
  });

  it('um resultado antigo (item mudou depois) não vale: sem botão Atualizar', async () => {
    const stale = makeReport([{ ...available(ALFA, '1.1'), currentId: 'versao-antiga' }]);
    const { url } = editorBackend({
      inventory: makeInventory([ALFA]),
      handlers: { updates_report: () => stale },
    });
    await openMods(url);
    expect(within(rowOf('Alfa')).queryByRole('button', { name: /^Atualizar/ })).toBeNull();
  });

  it('o filtro "Com atualização" mostra só quem pode ser atualizado', async () => {
    const { url } = backendWith();
    await openMods(url);
    await screen.findByText('2 atualizações disponíveis.');
    const user = userEvent.setup();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Mostrar' }), 'updates');
    expect(screen.getByRole('button', { name: 'Alfa' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Gama' })).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Beta' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Fixado' })).toBeNull();
  });
});

describe('revisão e aplicação (T10)', () => {
  const plan = makePlan(
    [
      planItem(available(ALFA, '1.1', 'newA')),
      planItem(available(GAMA, 'gama-2', '600'), {
        manualDownload: true,
        changelog: [
          {
            version: 'gama-2',
            channel: 'release',
            published: null,
            text: '<p>Corrige o <b>travamento</b></p><script>alert(1)</script>',
            format: 'html',
          },
        ],
      }),
    ],
    {
      newDependencies: [
        { projectId: 'lib', name: 'Biblioteca X', source: 'modrinth', neededBy: ['Alfa'] },
      ],
      conflicts: [{ item: 'Alfa', with: 'Beta' }],
    },
  );

  it('Revisar e atualizar mostra tudo, não grava até confirmar e usa só os itens marcados', async () => {
    const { backend, url } = backendWith(REPORT, {
      updates_plan: () => plan,
      updates_apply: () => ({
        updated: [{ path: GAMA.path, name: 'Gama', from: 'gama-1', to: 'gama-2', toId: '600' }],
        safetyPoint: null,
      }),
    });
    await openMods(url);
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Revisar e atualizar' }));
    const dialog = await screen.findByRole('dialog', { name: 'Atualizar 2 itens' });
    expect(await within(dialog).findByText('Biblioteca X, exigido por Alfa')).toBeDefined();
    expect(within(dialog).getByText('Alfa não funciona com Beta')).toBeDefined();
    expect(within(dialog).getByText(/precisa de download manual/)).toBeDefined();
    expect(within(dialog).getByText(/ponto de segurança/)).toBeDefined();
    expect(backend.callsOf('updates_plan')[0]?.args).toMatchObject({
      paths: ['mods/alfa.pw.toml', 'mods/gama.pw.toml'],
    });
    // O HTML da CurseForge é higienizado: sem script.
    expect(dialog.querySelector('script')).toBeNull();
    expect(backend.callsOf('updates_apply')).toHaveLength(0);

    await user.click(within(dialog).getByRole('checkbox', { name: 'Atualizar Alfa' }));
    await user.click(within(dialog).getByRole('button', { name: 'Atualizar Gama' }));
    await waitFor(() => {
      expect(backend.callsOf('updates_apply')).toHaveLength(1);
    });
    expect(backend.callsOf('updates_apply')[0]?.args).toMatchObject({
      selections: [{ path: 'mods/gama.pw.toml', toVersionId: '600' }],
    });
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Atualizar 2 itens' })).toBeNull();
    });
    expect(await screen.findByText('Gama foi atualizado para gama-2.')).toBeDefined();
  });

  it('Atualizar numa linha abre a revisão de um item e manda a versão que ela mostrou', async () => {
    const one = makePlan([planItem(available(ALFA, '1.1', 'newA'))]);
    const { backend, url } = backendWith(REPORT, {
      updates_plan: () => one,
      updates_apply: () => ({
        updated: [{ path: ALFA.path, name: 'Alfa', from: '1.0', to: '1.1', toId: 'newA' }],
        safetyPoint: null,
      }),
    });
    await openMods(url);
    const user = userEvent.setup();
    await user.click(
      within(await screen.findByRole('row', { name: /Alfa/ })).getByRole('button', {
        name: 'Atualizar Alfa para 1.1',
      }),
    );
    const dialog = await screen.findByRole('dialog', { name: 'Atualizar Alfa' });
    // As novidades vêm recolhidas, mas estão na revisão.
    expect(await within(dialog).findByText('Notas da 1.1')).toBeDefined();
    expect(within(dialog).queryByText(/ponto de segurança/)).toBeNull();
    await user.click(within(dialog).getByRole('button', { name: 'Atualizar Alfa' }));
    await waitFor(() => {
      expect(backend.callsOf('updates_apply')[0]?.args).toMatchObject({
        selections: [{ path: 'mods/alfa.pw.toml', toVersionId: 'newA' }],
      });
    });
  });

  it('a barra de seleção só habilita Atualizar para quem tem atualização (fixado fica de fora)', async () => {
    const { url } = backendWith(REPORT, { updates_plan: () => plan });
    await openMods(url);
    const user = userEvent.setup();
    await user.click(screen.getByRole('checkbox', { name: 'Selecionar Fixado' }));
    const bar = screen.getByRole('region', { name: 'Ações para os selecionados' });
    const button = within(bar).getByRole('button', { name: 'Atualizar' });
    expect(button.hasAttribute('disabled')).toBe(true);
    await user.click(screen.getByRole('checkbox', { name: 'Selecionar Alfa' }));
    expect(within(bar).getByRole('button', { name: 'Atualizar' }).hasAttribute('disabled')).toBe(
      false,
    );
  });

  it('erro ao aplicar fica no diálogo, que continua aberto', async () => {
    const { url } = backendWith(REPORT, {
      updates_plan: () => plan,
      updates_apply: () =>
        ipcError(makeAppError({ domain: 'project', code: 'PACK_CHANGED_EXTERNALLY' })),
    });
    await openMods(url);
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Revisar e atualizar' }));
    const dialog = await screen.findByRole('dialog', { name: 'Atualizar 2 itens' });
    await user.click(await within(dialog).findByRole('button', { name: 'Atualizar 2 itens' }));
    await waitFor(() => {
      expect(within(dialog).getByRole('alert')).toBeDefined();
    });
    expect(screen.getByRole('dialog', { name: 'Atualizar 2 itens' })).toBeDefined();
  });
});
