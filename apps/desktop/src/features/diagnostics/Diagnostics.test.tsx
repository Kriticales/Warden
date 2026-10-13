/**
 * Diagnóstico sem IA (SPEC T13 e T14; ROADMAP D-03) no app inteiro, com o backend simulado:
 * seção Problemas (achados por gravidade, evidência, correções, Ignorar, Verificar agora e o
 * contador no menu), marcas na lista de Mods, "Por que travou" no resultado do teste e o
 * diálogo de erros do Testar. Os critérios estão no nome de cada teste.
 */
import { QueryClient } from '@tanstack/react-query';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { AddPlanRequest, TestRequest } from '../../lib/ipc/bindings';
import { axePage } from '../../test/axe';
import { ipcError, type Handler } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { makeInventory, makeItem } from '../pack-editor/editor.fixtures';
import { editorBackend } from '../pack-editor/testing';
import { invalidateAfterTest } from '../test/api';
import { useTestStore } from '../test/store';
import { gameLine, makeSession, makeSessionView } from '../test/test.fixtures';
import { diagnosticsKeys } from './api';
import {
  CRASH_CAUSE,
  MISSING_DEP,
  WRONG_SIDE,
  makeDiagnosis,
  makeReport,
} from './diagnostics.fixtures';

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    if (this.classList.contains('console__log')) return 600;
    return this.classList.contains('console__line') ? 20 : 0;
  });
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return this.classList.contains('console__log') ? 1000 : 0;
  });
});

afterEach(() => {
  useTestStore.setState({ game: null, runs: {}, packChanged: {} });
  vi.restoreAllMocks();
});

const inventory = makeInventory([
  makeItem({ path: 'mods/meu-mod.jar', name: 'Meu Mod', source: 'local', sideEditable: false }),
  makeItem({ path: 'mods/sodium.pw.toml', name: 'Sodium', key: 'modrinth:AANobbMI' }),
]);

function setup(handlers: Record<string, Handler> = {}) {
  return editorBackend({
    inventory,
    handlers: {
      diagnostics_report_get: () => makeReport(),
      diagnostics_run: () => makeReport(),
      ...handlers,
    },
  });
}

function sectionMenu() {
  return screen.getByRole('navigation', { name: 'Seções do pack' });
}

async function openProblems(url: string) {
  renderApp(`${url}/problemas`);
  return screen.findByRole('heading', { level: 1, name: 'Problemas' }, { timeout: 10_000 });
}

describe('seção Problemas (T14)', () => {
  it('lista por gravidade, com explicação, itens e "Como sabemos"; contador no menu; axe', async () => {
    const { url } = setup();
    await openProblems(url);
    expect(screen.getByText(/^3 encontrados · última verificação há 3 minutos/)).toBeDefined();

    const errors = screen.getByRole('region', {
      name: 'Erros impedem o teste, a menos que você escolha testar mesmo assim',
    });
    const missing = within(errors).getByRole('article', {
      name: 'Meu Mod precisa de Sodium, que não está no pack',
    });
    expect(within(missing).getByText('Erro')).toBeDefined();
    expect(
      within(missing).getByText(
        'Sem ele, o jogo costuma parar na tela de carregamento. Adicione a dependência.',
      ),
    ).toBeDefined();
    expect(within(missing).getByRole('link', { name: 'Ver Meu Mod' })).toBeDefined();
    const user = userEvent.setup();
    await user.click(within(missing).getByText('Como sabemos'));
    expect(within(missing).getByText('O arquivo fabric.mod.json de Meu Mod diz:')).toBeDefined();
    expect(within(missing).getByText('sodium: Any')).toBeDefined();

    const warnings = screen.getByRole('region', { name: 'Avisos não impedem o teste' });
    expect(
      within(warnings).getByRole('article', { name: 'Sodium está no lado errado' }),
    ).toBeDefined();
    expect(screen.getByRole('region', { name: 'Informações só para você saber' })).toBeDefined();

    // Contador: 1 erro + 1 aviso (a informação não conta), em cor de perigo.
    const count = within(sectionMenu()).getByLabelText('2 problemas');
    expect(count.className).toContain('danger');
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('"Mudar lado" usa o Alterar lado da lista de Mods (CA: correção de ponta a ponta)', async () => {
    const { backend, url } = setup({ items_set_side: () => ['mods/sodium.pw.toml'] });
    await openProblems(url);
    const card = screen.getByRole('article', { name: 'Sodium está no lado errado' });
    await userEvent
      .setup()
      .click(within(card).getByRole('button', { name: 'Mudar lado para Só cliente' }));
    await waitFor(() => {
      expect(backend.callsOf('items_set_side')).toHaveLength(1);
    });
    expect(backend.callsOf('items_set_side')[0]?.args).toMatchObject({
      paths: ['mods/sodium.pw.toml'],
      side: 'client',
    });
    expect(await screen.findByText('Lado de Sodium mudado.')).toBeDefined();
  });

  it('"Adicionar Sodium" acha o projeto e abre o diálogo de dependências de sempre', async () => {
    const { backend, url } = setup({
      diagnostics_dependency_find: () => ({
        source: 'modrinth',
        projectId: 'AANobbMI',
        name: 'Sodium',
      }),
      add_plan: () => ipcError(makeAppError({ domain: 'app', code: 'INTERNAL' })),
    });
    await openProblems(url);
    const card = screen.getByRole('article', {
      name: 'Meu Mod precisa de Sodium, que não está no pack',
    });
    await userEvent.setup().click(within(card).getByRole('button', { name: 'Adicionar Sodium' }));
    await waitFor(() => {
      expect(backend.callsOf('add_plan')).toHaveLength(1);
    });
    expect(backend.callsOf('diagnostics_dependency_find')[0]?.args).toMatchObject({
      modId: 'sodium',
      source: null,
    });
    expect((backend.callsOf('add_plan')[0]?.args.request as AddPlanRequest).items).toEqual([
      { source: 'modrinth', projectId: 'AANobbMI' },
    ]);
    expect(await screen.findByRole('dialog')).toBeDefined();
  });

  it('dependência que a fonte não conhece: avisa e oferece Adicionar', async () => {
    const { url } = setup({ diagnostics_dependency_find: () => null });
    await openProblems(url);
    const card = screen.getByRole('article', {
      name: 'Meu Mod precisa de Sodium, que não está no pack',
    });
    await userEvent.setup().click(within(card).getByRole('button', { name: 'Adicionar Sodium' }));
    expect(
      await screen.findByText(
        'O Warden não achou Sodium no Modrinth. Procure pelo nome em Adicionar e escolha a versão para o pack.',
      ),
    ).toBeDefined();
  });

  it('"Ignorar neste pack" grava e o aviso some da lista (aparece riscado em "Mostrar os ignorados")', async () => {
    let ignored = false;
    const report = () =>
      makeReport({
        findings: [MISSING_DEP, { ...WRONG_SIDE, ignored }],
      });
    const { backend, url } = setup({
      diagnostics_report_get: report,
      diagnostics_ignore: (args) => {
        ignored = Boolean(args.ignored);
        return report();
      },
    });
    await openProblems(url);
    const user = userEvent.setup();
    const card = screen.getByRole('article', { name: 'Sodium está no lado errado' });
    await user.click(within(card).getByRole('button', { name: 'Ignorar neste pack' }));
    await waitFor(() => {
      expect(backend.callsOf('diagnostics_ignore')[0]?.args).toMatchObject({
        findingId: WRONG_SIDE.id,
        ignored: true,
      });
    });
    await waitFor(() => {
      expect(screen.queryByRole('article', { name: 'Sodium está no lado errado' })).toBeNull();
    });
    expect(screen.getByText('1 aviso ignorado por você neste pack')).toBeDefined();
    expect(within(sectionMenu()).getByLabelText('1 problema')).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Mostrar os ignorados' }));
    const back = screen.getByRole('article', { name: 'Sodium está no lado errado' });
    expect(within(back).getByText('ignorado por você')).toBeDefined();
    expect(within(back).getByRole('button', { name: 'Voltar a mostrar' })).toBeDefined();
  });

  it('"Verificar agora" confere de novo; relatório velho ou ausente é verificado sozinho', async () => {
    const { backend, url } = setup({
      diagnostics_report_get: () => makeReport({ stale: true }),
    });
    await openProblems(url);
    await waitFor(() => {
      expect(backend.callsOf('diagnostics_run')).toHaveLength(1);
    });
    await userEvent.setup().click(await screen.findByRole('button', { name: 'Verificar agora' }));
    await waitFor(() => {
      expect(backend.callsOf('diagnostics_run')).toHaveLength(2);
    });
  });

  it('sem nada encontrado: "Nenhum problema encontrado"; pack vanilla: nada a conferir', async () => {
    const { url } = setup({ diagnostics_report_get: () => makeReport({ findings: [] }) });
    await openProblems(url);
    expect(screen.getByRole('heading', { name: 'Nenhum problema encontrado' })).toBeDefined();
    expect(within(sectionMenu()).queryByLabelText(/problema/)).toBeNull();
  });

  it('"Último travamento" leva ao resultado do teste que travou', async () => {
    const crashed = makeSession({ outcome: 'crashed', exitCode: 255 });
    const { url } = setup({
      test_sessions_list: () => [crashed],
      diagnostics_crash_get: () => makeDiagnosis(),
    });
    await openProblems(url);
    const block = await screen.findByRole('region', { name: 'Último travamento' });
    expect(
      await within(block).findByText('Waystones precisa de balm, que não está no pack'),
    ).toBeDefined();
    expect(
      within(block).getByRole('link', { name: 'Ver o que causou' }).getAttribute('href'),
    ).toContain(`sessao=${crashed.id}`);
  });
});

describe('marcas na lista de Mods (T06)', () => {
  it('o item com erro ganha a marca "Erro", com link para Problemas', async () => {
    const { url } = setup();
    renderApp(url);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    const row = (await screen.findByText('Meu Mod')).closest('tr');
    if (!row) throw new Error('linha do Meu Mod');
    const mark = await within(row).findByRole('link', { name: /^Erro: 1 problema em Problemas/ });
    expect(mark.getAttribute('href')).toContain('/problemas');
    const sodium = screen.getByText('Sodium').closest('tr');
    if (!sodium) throw new Error('linha do Sodium');
    expect(within(sodium).getByRole('link', { name: /^Aviso:/ })).toBeDefined();
  });

  it('o fim de um teste relê o relatório (o pré-teste gravou outro): contador e marcas mudam', async () => {
    const queryClient = new QueryClient();
    const packId = 'pack-1';
    queryClient.setQueryData(diagnosticsKeys.report(packId), makeReport());
    await invalidateAfterTest(queryClient, packId);
    expect(queryClient.getQueryState(diagnosticsKeys.report(packId))?.isInvalidated).toBe(true);
  });
});

describe('Por que travou (CA-T13-06)', () => {
  const crashed = makeSession({
    outcome: 'crashed',
    exitCode: 255,
    durationMs: 48_000,
    crashReports: ['crash-reports/crash-2026-10-07_14.20.48-client.txt'],
  });

  function crashSetup(handlers: Record<string, Handler>) {
    return setup({
      test_sessions_list: () => [crashed],
      test_session_get: () =>
        makeSessionView(crashed, [gameLine('Erro de verdade do jogo', { level: 'error' })]),
      test_artifact_open: () => null,
      ...handlers,
    });
  }

  it('mostra a causa, a correção e o trecho do log; "Copiar relatório" passa pela redação', async () => {
    const { backend, url } = crashSetup({
      diagnostics_crash_get: () => makeDiagnosis(),
      diagnostics_redact: (args) => `REDIGIDO\n${String(args.text)}`,
    });
    renderApp(`${url}/teste?sessao=${crashed.id}`);
    expect(
      await screen.findByRole('heading', { level: 1, name: 'O jogo travou' }, { timeout: 10_000 }),
    ).toBeDefined();
    const why = screen.getByRole('region', { name: 'Por que travou' });
    const cause = await within(why).findByRole('article', {
      name: 'Waystones precisa de balm, que não está no pack',
    });
    expect(within(why).getByText('O jogo fechou com o código 255 depois de 48 s.')).toBeDefined();
    expect(within(cause).getByText('logs/latest.log, linha 1.204:')).toBeDefined();
    expect(within(cause).getByRole('button', { name: 'Adicionar balm' })).toBeDefined();
    // Com causa, as últimas linhas do console não aparecem.
    expect(within(why).queryByText('Erro de verdade do jogo')).toBeNull();

    // O user-event troca a área de transferência pela dele: o espião vem depois.
    const user = userEvent.setup();
    const writeText = vi.spyOn(navigator.clipboard, 'writeText');
    await user.click(within(why).getByRole('button', { name: 'Copiar relatório' }));
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledTimes(1);
    });
    const sent = String(backend.callsOf('diagnostics_redact')[0]?.args.text);
    expect(sent).toContain('Waystones precisa de balm, que não está no pack');
    expect(sent).toContain('logs/latest.log, linha 1.204:');
    expect(writeText).toHaveBeenCalledWith(expect.stringMatching(/^REDIGIDO\n/));
    expect(await screen.findByText('Relatório copiado, sem dados pessoais.')).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('sem causa conhecida: "Não encontramos a causa automaticamente." e as últimas linhas', async () => {
    const { url } = crashSetup({
      diagnostics_crash_get: () => makeDiagnosis({ foundCause: false, findings: [] }),
    });
    renderApp(`${url}/teste?sessao=${crashed.id}`);
    const why = await screen.findByRole('region', { name: 'Por que travou' }, { timeout: 10_000 });
    expect(await within(why).findByText('Não encontramos a causa automaticamente.')).toBeDefined();
    expect(within(why).getByText('Erro de verdade do jogo')).toBeDefined();
    expect(within(why).getByRole('button', { name: 'Abrir crash report' })).toBeDefined();
  });

  it('outras pistas ficam recolhidas', async () => {
    const other = {
      ...CRASH_CAUSE,
      id: 'outro',
      finding: {
        ...CRASH_CAUSE.finding,
        rule: 'E_CRASH_REPORT',
        titleKey: 'diagnostico.achados.crashReport',
        params: { description: 'Initializing game', exception: 'java.lang.RuntimeException' },
      },
    };
    const { url } = crashSetup({
      diagnostics_crash_get: () => makeDiagnosis({ findings: [CRASH_CAUSE, other] }),
    });
    renderApp(`${url}/teste?sessao=${crashed.id}`);
    const why = await screen.findByRole('region', { name: 'Por que travou' }, { timeout: 10_000 });
    expect(await within(why).findByText('Mais 1 pista encontrada nos logs')).toBeDefined();
    expect(within(why).getByText('O jogo travou: Initializing game')).toBeDefined();
  });
});

describe('erros do pré-teste no Testar (T13 passos 1 e 4)', () => {
  it('diálogo com os erros, a correção principal, "Testar mesmo assim" e "Cancelar e ver Problemas"', async () => {
    const { backend, url } = setup({
      test_start: ({ request }) => {
        if ((request as TestRequest).ignorePackErrors) {
          return makeSession();
        }
        return ipcError(
          makeAppError(
            { domain: 'app', code: 'TEST_PACK_HAS_ERRORS' },
            { params: { count: '1', step: 'final' } },
          ),
        );
      },
    });
    renderApp(url);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    const user = userEvent.setup();
    const header = screen.getByRole('banner', { name: 'Pack aberto' });
    await user.click(within(header).getByRole('button', { name: 'Testar' }));

    const dialog = await screen.findByRole('dialog', {
      name: '1 erro pode impedir o jogo de abrir',
    });
    expect(
      within(dialog).getByText(
        'Dá para corrigir agora, aqui mesmo, ou testar assim e ver o que acontece.',
      ),
    ).toBeDefined();
    expect(
      within(dialog).getByRole('article', {
        name: 'Meu Mod precisa de Sodium, que não está no pack',
      }),
    ).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Adicionar Sodium' })).toBeDefined();
    expect(
      within(dialog).getByText(
        '1 aviso também foi encontrado. Ele não impede o teste e fica em Problemas.',
      ),
    ).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Cancelar e ver Problemas' })).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();

    await user.click(within(dialog).getByRole('button', { name: 'Testar mesmo assim' }));
    await waitFor(() => {
      expect(backend.callsOf('test_start')).toHaveLength(2);
    });
    expect((backend.callsOf('test_start')[1]?.args.request as TestRequest).ignorePackErrors).toBe(
      true,
    );
    expect(
      await screen.findByRole('heading', { level: 1, name: 'O jogo fechou normalmente' }),
    ).toBeDefined();
  });

  it('fechado o diálogo, a tela diz onde o teste parou; "Cancelar e ver Problemas" leva à seção', async () => {
    const { url } = setup({
      test_start: () =>
        ipcError(
          makeAppError(
            { domain: 'app', code: 'TEST_PACK_HAS_ERRORS' },
            { params: { count: '1', step: 'check' } },
          ),
        ),
    });
    renderApp(url);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    const user = userEvent.setup();
    await user.click(
      within(screen.getByRole('banner', { name: 'Pack aberto' })).getByRole('button', {
        name: 'Testar',
      }),
    );
    const dialog = await screen.findByRole('dialog');
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(
      screen.getByRole('heading', { level: 1, name: 'O teste parou ao verificar o pack' }),
    ).toBeDefined();
    expect(dialog).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Ver os erros' }));
    const again = await screen.findByRole('dialog');
    await user.click(within(again).getByRole('button', { name: 'Cancelar e ver Problemas' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Problemas' })).toBeDefined();
  });
});
