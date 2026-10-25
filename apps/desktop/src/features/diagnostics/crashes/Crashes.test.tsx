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
import {
  BALM_GROUP,
  makeCrashList,
  makeDetail,
  makeGroup,
  makeSignature,
} from './crashes.fixtures';
import { makeSummary } from '../ai/ai.fixtures';
import { MAX_WHERE_LENGTH, crashTitle, shorten, versionsText } from './text';

/** Título do grupo padrão dos dados de teste (sem achado gravado). */
const NPE_TITLE = 'O jogo travou com NullPointerException (em EntityTicker.tick)';

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
  const row = within(block).getByText(text).closest('li');
  if (!row) throw new Error(`linha de ${String(text)}`);
  return row;
}

describe('Travamentos em Problemas (T14)', () => {
  it('CA-T14-08: mesma causa vira uma linha "Travou 2 vezes" com as versões de cada vez; causas diferentes, linhas diferentes; axe', async () => {
    const { url } = setup();
    const block = await openCrashes(url);
    const rows = await within(block).findAllByRole('listitem');
    // Uma linha por causa.
    expect(rows).toHaveLength(2);
    expect(within(block).getByText('2 causas')).toBeDefined();

    const npe = rowOf(block, NPE_TITLE);
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

  it('Sculk: "Abrir a conversa com o Sculk" quando já há conversa ligada à causa; "✦ Conversar com o Sculk" quando não há', async () => {
    const { url } = setup({
      ai_conversations_list: () => [
        makeSummary({
          id: '01K7AAAAAAAAAAAAAAAAAAAAAA',
          updatedAtMs: 1,
          origin: {
            kind: 'session',
            sessionId: 'x',
            lastCrash: false,
            signature: '0a1b2c3d4e5f6071',
          },
        }),
        // A mais nova vale.
        makeSummary({
          id: '01K7CCCCCCCCCCCCCCCCCCCCCC',
          updatedAtMs: 2,
          origin: {
            kind: 'session',
            sessionId: 'y',
            lastCrash: true,
            signature: '0a1b2c3d4e5f6071',
          },
        }),
        makeSummary({ id: '01K7DDDDDDDDDDDDDDDDDDDDDD', origin: { kind: 'packOnly' } }),
      ],
    });
    const block = await openCrashes(url);
    const npe = rowOf(block, NPE_TITLE);
    const open = await within(npe).findByRole('link', {
      name: /^Abrir a conversa com o Sculk sobre o travamento de /,
    });
    expect(open.textContent).toBe('Abrir a conversa com o Sculk');
    expect(open.getAttribute('href')).toMatch(/\/ia\/01K7CCCCCCCCCCCCCCCCCCCCCC$/);
    expect(within(npe).queryByRole('link', { name: /^Conversar com o Sculk/ })).toBeNull();

    const balm = rowOf(block, 'Waystones precisa de balm, que não está no pack');
    const talk = within(balm).getByRole('link', {
      name: /^Conversar com o Sculk sobre o travamento de /,
    });
    expect(talk.className).toContain('btn--ai');
    expect(talk.getAttribute('href')).toContain('/ia?sessao=');
  });

  it('"Ver o que causou" abre o resultado do teste mais novo do grupo', async () => {
    const { url } = setup();
    const block = await openCrashes(url);
    const npe = rowOf(block, NPE_TITLE);
    const link = within(npe).getByRole('link', { name: /^Ver o que causou o travamento de / });
    expect(link.textContent).toBe('Ver o que causou');
    expect(link.getAttribute('href')).toContain('sessao=2026-10-07_14-20-00');
  });

  it('"Ver as 2 vezes" lista cada vez com a versão e o link para o teste; "Ocultar" fecha', async () => {
    const { backend, url } = setup({ crash_get: () => makeDetail() });
    const user = userEvent.setup();
    const block = await openCrashes(url);
    const npe = rowOf(block, NPE_TITLE);
    const toggle = within(npe).getByRole('button', { name: 'Ver as 2 vezes' });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    await user.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    await waitFor(() => {
      expect(block.querySelector('ul.diag-crashes__occurrences')).not.toBeNull();
    });
    const list = block.querySelector<HTMLElement>('ul.diag-crashes__occurrences');
    if (!list) throw new Error('lista das vezes');
    expect(document.getElementById(toggle.getAttribute('aria-controls') ?? '')).not.toBeNull();
    const links = within(list).getAllByRole('link');
    expect(links).toHaveLength(2);
    expect(links[0]?.textContent).toMatch(/^Teste de .* · 1\.4\.2$/);
    expect(links[1]?.textContent).toMatch(/^Teste de .* · 1\.4\.1$/);
    expect(links[1]?.getAttribute('href')).toContain('sessao=2026-10-07_12-20-00');
    expect(backend.callsOf('crash_get')[0]?.args).toMatchObject({ signature: '0a1b2c3d4e5f6071' });

    await user.click(within(npe).getByRole('button', { name: 'Ocultar' }));
    expect(block.querySelector('ul.diag-crashes__occurrences')).toBeNull();
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
    expect(
      within(block).getByText('Não voltou a acontecer desde a 1.4.1').closest('.tag')?.className,
    ).toContain('tag--plain');
  });

  it('nenhum teste travou: a frase de vazio, sem lista', async () => {
    const { url } = setup({ crashes_list: () => makeCrashList({ groups: [] }) });
    const block = await openCrashes(url);
    expect(within(block).getByText('Nenhum teste deste pack travou.')).toBeDefined();
    expect(within(block).queryByRole('list')).toBeNull();
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
          retention: {
            budgetBytes: 500 * 1024 * 1024,
            nearLimit: true,
            overLimit: false,
            prunable: [],
          },
        }),
    });
    const block = await openCrashes(url);
    expect(
      within(block).getByText('Os travamentos guardados estão perto do limite de 500 MB'),
    ).toBeDefined();
    expect(within(block).queryByRole('button', { name: 'Apagar os mais antigos' })).toBeNull();
  });

  it('passou do limite: "Apagar os mais antigos" confirma, apaga e relê tudo o que mostra testes; Cancelar devolve o foco', async () => {
    const OLDEST = ['2026-09-01_10-00-00', '2026-09-02_10-00-00', '2026-09-03_10-00-00'];
    let prunable = OLDEST;
    const { backend, url } = setup({
      crashes_list: () =>
        makeCrashList({
          retention: {
            budgetBytes: 500 * 1024 * 1024,
            nearLimit: true,
            overLimit: prunable.length > 0,
            prunable,
          },
        }),
      crashes_prune: () => {
        prunable = [];
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
    const sessionsReads = backend.callsOf('test_sessions_list').length;
    await user.click(within(dialog).getByRole('button', { name: 'Apagar 3 travamentos' }));
    await waitFor(() => {
      expect(backend.callsOf('crashes_prune')).toHaveLength(1);
    });
    // Manda exatamente as sessões que a confirmação prometeu apagar.
    expect(backend.callsOf('crashes_prune')[0]?.args).toEqual({ packId, sessions: OLDEST });
    await waitFor(() => {
      expect(within(block).queryByText(/passaram do limite/)).toBeNull();
    });
    // A lista de sessões tem quem a mostre na própria seção (o bloco de memória da L-11): a
    // invalidação a relê na hora, em vez de só marcá-la.
    await waitFor(() => {
      expect(backend.callsOf('test_sessions_list').length).toBeGreaterThan(sessionsReads);
    });
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

describe('retenção sem nada para apagar', () => {
  it('passou do limite só com o mais novo: avisa sem oferecer apagar', async () => {
    const { url } = setup({
      crashes_list: () =>
        makeCrashList({
          retention: {
            budgetBytes: 500 * 1024 * 1024,
            nearLimit: true,
            overLimit: true,
            prunable: [],
          },
        }),
    });
    const block = await openCrashes(url);
    expect(
      within(block).getByText('Os travamentos guardados passaram do limite de 500 MB'),
    ).toBeDefined();
    expect(
      within(block).getByText(
        'O que passa do limite é o travamento mais novo ou o do teste aberto, que o Warden não apaga.',
      ),
    ).toBeDefined();
    expect(within(block).queryByRole('button', { name: 'Apagar os mais antigos' })).toBeNull();
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
    expect(crashTitle(makeGroup())).toBe(
      'O jogo travou com NullPointerException (em EntityTicker.tick)',
    );
    const bare = makeSignature({ frame: null });
    expect(crashTitle(makeGroup({ signature: bare }))).toBe(
      'O jogo travou com NullPointerException',
    );
    const signature = { ...makeGroup().signature, exception: null };
    expect(crashTitle(makeGroup({ signature }))).toBe('O jogo travou sem deixar a causa no log');
    // Sem o achado gravado, onde quebrou separa causas com a mesma exceção.
    const golem = makeSignature({
      frame: null,
      subject: [
        'entity_type=alpha:golem',
        'description=ticking entity',
        'entity_class=com.alpha.Golem',
      ],
    });
    const overlay = makeSignature({ frame: null, subject: ['description=rendering overlay'] });
    const bau = makeSignature({
      frame: null,
      subject: ['description=ticking block entity', 'block_entity=beta:bau'],
    });
    expect(crashTitle(makeGroup({ signature: golem }))).toBe(
      'O jogo travou com NullPointerException (alpha:golem)',
    );
    expect(crashTitle(makeGroup({ signature: overlay }))).toBe(
      'O jogo travou com NullPointerException (rendering overlay)',
    );
    expect(crashTitle(makeGroup({ signature: bau }))).toBe(
      'O jogo travou com NullPointerException (beta:bau)',
    );
    expect(crashTitle(makeGroup({ signature: { ...golem, frame: 'com.alpha.Golem.tick' } }))).toBe(
      'O jogo travou com NullPointerException (alpha:golem em Golem.tick)',
    );
  });

  it('título: a mesma descrição em frames diferentes dá títulos diferentes (2ª revisão)', () => {
    // NPE "Unexpected error" no render, no tick do servidor e num método injetado pelo Mixin:
    // três linhas, três títulos.
    const where = (frame: string) =>
      crashTitle(
        makeGroup({
          signature: makeSignature({
            rule: 'crash_report.description',
            frame,
            mods: [],
            subject: ['description=unexpected error'],
          }),
        }),
      );
    const titles = [
      where('net.minecraft.client.renderer.GameRenderer.render'),
      where('net.minecraft.server.level.ServerLevel.tick'),
      where('net.minecraft.world.entity.Entity.aoTick'),
    ];
    expect(titles).toEqual([
      'O jogo travou com NullPointerException (unexpected error em GameRenderer.render)',
      'O jogo travou com NullPointerException (unexpected error em ServerLevel.tick)',
      'O jogo travou com NullPointerException (unexpected error em Entity.aoTick)',
    ]);
    // Frame nativo da JVM: sem a letra do tipo.
    expect(where('C ig9icd64.dll')).toBe(
      'O jogo travou com NullPointerException (unexpected error em ig9icd64.dll)',
    );
  });

  it('título: onde quebrou é cortado com reticências sem quebrar nomes', () => {
    const long = makeSignature({
      frame: 'net.exemplo.ClasseComUmNomeMuitoGrandeMesmo.metodoQueTambemTemUmNomeEnorme',
      subject: ['description=exception while ticking a very long description of the world state'],
    });
    const title = crashTitle(makeGroup({ signature: long }));
    const onde = /\((.*)\)$/.exec(title)?.[1] ?? '';
    expect(onde.length).toBeLessThanOrEqual(MAX_WHERE_LENGTH);
    // O frame fica inteiro; a descrição é cortada num espaço, sem quebrar a palavra.
    expect(onde).toBe(
      'exception… em ClasseComUmNomeMuitoGrandeMesmo.metodoQueTambemTemUmNomeEnorme',
    );
    const shorter = makeSignature({
      frame: 'net.exemplo.Mundo.tick',
      subject: [
        'description=exception while ticking the world in a very long and detailed description',
      ],
    });
    expect(crashTitle(makeGroup({ signature: shorter }))).toBe(
      'O jogo travou com NullPointerException (exception while ticking the world in a very long and detailed… em Mundo.tick)',
    );

    // Um nome só, sem espaço onde cortar: corta no meio.
    const single = 'a'.repeat(120);
    expect(shorten(single)).toBe(`${'a'.repeat(MAX_WHERE_LENGTH - 1)}…`);
    // Até o limite, nada muda.
    const exact = 'b'.repeat(MAX_WHERE_LENGTH);
    expect(shorten(exact)).toBe(exact);
  });
});
