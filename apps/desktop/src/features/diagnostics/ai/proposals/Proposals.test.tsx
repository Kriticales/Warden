/**
 * Propostas da IA e consultas externas na conversa (SPEC T14 ✦ passos 2 e 3; ROADMAP D-14), com
 * o backend simulado: os cartões com Aplicar e Descartar (nada é chamado antes do clique), o
 * diálogo com a diferença exata e o token do que foi mostrado (CA-T14-12), os dados do pack
 * relidos depois de Aplicar (lista de Mods, linha do pack, Meus packs), Descartar sem gravar, o
 * pack mudado no meio, a espera enquanto a IA responde, os estados aplicada e descartada, o bloco
 * "Enviado ao GitHub" byte a byte (CA-T14-03) e o número de propostas na lista de Conversas.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { Proposal, ProposalReview, ToolStep } from '../../../../lib/ipc/bindings';
import { axePage } from '../../../../test/axe';
import { ipcError, type Handler } from '../../../../test/backend';
import { makeAppError } from '../../../../test/factories';
import { renderApp } from '../../../../test/render';
import { makePlan } from '../../../bisect/bisect.fixtures';
import { editorBackend } from '../../../pack-editor/testing';
import { CONVERSATION_ID, makeSummary, makeTurn, makeView } from '../ai.fixtures';

const BEFORE = '[trains]\nmaxAssemblyLength = 128\n';
const AFTER = '[trains]\nmaxAssemblyLength = 256\n';

function makeProposal(overrides: Partial<Proposal> = {}): Proposal {
  return {
    id: 'p1',
    turn: 0,
    change: {
      kind: 'editConfig',
      path: 'config/create-common.toml',
      key: ['trains', 'maxAssemblyLength'],
      value: '256',
    },
    reason: 'Trens longos passam do limite e travam o servidor.',
    evidence: [
      {
        id: 'config:config/create-common.toml#L2',
        quote: 'maxAssemblyLength = 128',
        status: 'valid',
        sourceText: 'maxAssemblyLength = 128',
      },
    ],
    summary: {
      target: 'config/create-common.toml: trains.maxAssemblyLength',
      from: '128',
      to: '256',
    },
    state: { state: 'pending' },
    createdAtMs: Date.UTC(2026, 9, 3, 21, 19),
    ...overrides,
  };
}

const BISECT: Proposal = makeProposal({
  id: 'p2',
  change: { kind: 'startBisect' },
  reason: 'As evidências não bastam para apontar um mod.',
  summary: { target: null, from: null, to: null },
});

function makeReview(): ProposalReview {
  return {
    proposalId: 'p1',
    token: 'token-da-previa',
    changesPack: true,
    preview: {
      kind: 'editConfig',
      path: 'config/create-common.toml',
      key: 'trains.maxAssemblyLength',
      before: BEFORE,
      after: AFTER,
    },
  };
}

const GITHUB_STEP: ToolStep = {
  callId: 'call_9',
  name: 'search_mod_issues',
  args: '{"mod_id":"railways","query":"NoSuchMethodError tickBogeys"}',
  sent: '{"functionResponse":{"id":"call_9","name":"search_mod_issues","response":{"itens":[{"id":"issue:Layers-of-Railways/Railway#412","texto":"#412 (fechada) NoSuchMethodError tickBogeys"}]}}}',
  ok: true,
  denied: false,
  external: {
    service: 'github',
    request:
      'GET https://api.github.com/search/issues?q=repo%3ALayers-of-Railways%2FRailway+is%3Aissue+NoSuchMethodError+tickBogeys&per_page=5',
    repository: 'Layers-of-Railways/Railway',
    terms: 'NoSuchMethodError tickBogeys',
    cached: false,
  },
};

function setup(proposals: Proposal[], handlers: Record<string, Handler> = {}) {
  return editorBackend({
    handlers: {
      secrets_status: () => ({
        backend: 'keyring',
        curseforge: false,
        gemini: true,
        github: false,
      }),
      ai_conversations_list: () => [makeSummary({ pendingProposals: 1 })],
      ai_conversation_get: () =>
        makeView({ proposals, turns: [makeTurn({ steps: [GITHUB_STEP] })] }),
      ai_proposal_review: () => makeReview(),
      ai_proposal_apply: () => ({
        proposal: makeProposal({
          state: {
            state: 'applied',
            atMs: Date.now(),
            safetyPoint: '20261003T211900Z-proposta-da-ia',
          },
        }),
        safetyPoint: '20261003T211900Z-proposta-da-ia',
      }),
      ai_proposal_discard: () => makeProposal({ state: { state: 'discarded', atMs: Date.now() } }),
      ...handlers,
    },
  });
}

async function openConversation(url: string) {
  renderApp(`${url}/ia/${CONVERSATION_ID}`);
  await screen.findByRole(
    'heading',
    { level: 1, name: makeSummary().title ?? '' },
    { timeout: 10_000 },
  );
  return screen.findByRole('region', {
    name: 'Mudar config/create-common.toml: trains.maxAssemblyLength de 128 para 256',
  });
}

describe('✦ Propostas da IA (D-14)', () => {
  it('CA-T14-12: nada muda até Aplicar; Aplicar mostra a diferença e grava o que foi mostrado; o pack é relido; axe', async () => {
    const { backend, url } = setup([makeProposal()]);
    const card = await openConversation(url);
    expect(
      within(card).getByText('Trens longos passam do limite e travam o servidor.'),
    ).toBeDefined();
    expect(within(card).getByText('Com base em')).toBeDefined();
    expect(within(card).getByText(/config\/create-common\.toml, linha 2/)).toBeDefined();
    expect(screen.getByText('Nada muda no pack sem você clicar em Aplicar.')).toBeDefined();
    expect(backend.callsOf('ai_proposal_review')).toHaveLength(0);
    expect(backend.callsOf('ai_proposal_apply')).toHaveLength(0);

    const user = userEvent.setup();
    const apply = within(card).getByRole('button', { name: /^Aplicar a proposta/ });
    await user.click(apply);
    const dialog = await screen.findByRole('dialog', { name: 'Aplicar a proposta?' });
    expect(within(dialog).getByText(/cria um ponto de segurança/)).toBeDefined();
    await within(dialog).findByText('config/create-common.toml');
    expect(within(dialog).getByText(/maxAssemblyLength = 256/)).toBeDefined();
    expect(within(dialog).getByText(/maxAssemblyLength = 128/)).toBeDefined();
    expect(backend.callsOf('ai_proposal_apply')).toHaveLength(0);
    expect(await axePage(document.body)).toHaveNoViolations();

    const reads = (command: string) => backend.callsOf(command).length;
    const before = ['pack_get', 'inventory_list', 'ai_conversation_get'].map(reads);
    await user.click(within(dialog).getByRole('button', { name: 'Aplicar' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(backend.callsOf('ai_proposal_apply')[0]?.args).toEqual({
      packId: expect.any(String) as string,
      conversationId: CONVERSATION_ID,
      proposalId: 'p1',
      token: 'token-da-previa',
    });
    // Dados sempre atuais: a linha do pack (cabeçalho), a lista de Mods e a conversa são
    // relidas (Meus packs também é invalidada; aqui não está na tela).
    await waitFor(() => {
      const after = ['pack_get', 'inventory_list', 'ai_conversation_get'].map(reads);
      after.forEach((count, index) => {
        expect(count).toBeGreaterThan(before[index] ?? 0);
      });
    });
    expect(document.activeElement).toBe(apply);
    expect(backend.callsOf('ai_proposal_discard')).toHaveLength(0);
  });

  it('Descartar não abre a diferença nem aplica', async () => {
    const { backend, url } = setup([makeProposal()]);
    const card = await openConversation(url);
    await userEvent
      .setup()
      .click(within(card).getByRole('button', { name: /^Descartar a proposta/ }));
    await waitFor(() => {
      expect(backend.callsOf('ai_proposal_discard')).toHaveLength(1);
    });
    expect(backend.callsOf('ai_proposal_discard')[0]?.args).toMatchObject({ proposalId: 'p1' });
    expect(backend.callsOf('ai_proposal_review')).toHaveLength(0);
    expect(backend.callsOf('ai_proposal_apply')).toHaveLength(0);
  });

  it('o pack mudou depois da diferença: o erro diz para revisar, a diferença é relida e o próximo Aplicar grava a nova', async () => {
    let reviews = 0;
    let applies = 0;
    const { backend, url } = setup([makeProposal()], {
      ai_proposal_review: () => {
        reviews += 1;
        return { ...makeReview(), token: reviews === 1 ? 'token-da-previa' : 'token-relido' };
      },
      ai_proposal_apply: () => {
        applies += 1;
        if (applies === 1) {
          return ipcError(makeAppError({ domain: 'ai', code: 'PROPOSAL_STALE' }));
        }
        return {
          proposal: makeProposal({
            state: { state: 'applied', atMs: Date.now(), safetyPoint: null },
          }),
          safetyPoint: null,
        };
      },
    });
    const card = await openConversation(url);
    const user = userEvent.setup();
    await user.click(within(card).getByRole('button', { name: /^Aplicar a proposta/ }));
    const dialog = await screen.findByRole('dialog', { name: 'Aplicar a proposta?' });
    await within(dialog).findByText('config/create-common.toml');
    await user.click(within(dialog).getByRole('button', { name: 'Aplicar' }));
    expect(
      await within(dialog).findByText(
        /O pack mudou desde que a diferença foi mostrada\. Nada foi gravado/,
      ),
    ).toBeDefined();
    await waitFor(() => {
      expect(backend.callsOf('ai_proposal_review').length).toBeGreaterThanOrEqual(2);
    });
    await waitFor(() => {
      expect(within(dialog).getByRole('button', { name: 'Aplicar' })).toHaveProperty(
        'disabled',
        false,
      );
    });
    await user.click(within(dialog).getByRole('button', { name: 'Aplicar' }));
    await waitFor(() => {
      expect(backend.callsOf('ai_proposal_apply')).toHaveLength(2);
    });
    expect(backend.callsOf('ai_proposal_apply')[1]?.args).toMatchObject({
      token: 'token-relido',
    });
  });

  it('enquanto a IA responde nesta conversa, os botões esperam', async () => {
    const { url } = setup([makeProposal()], {
      ai_conversation_get: () =>
        makeView({ proposals: [makeProposal()], live: { kind: 'thinking' } }),
    });
    const card = await openConversation(url);
    expect(
      within(card).getByRole<HTMLButtonElement>('button', { name: /^Aplicar a proposta/ }).disabled,
    ).toBe(true);
    expect(within(card).getByText('Espere a IA terminar de responder para decidir.')).toBeDefined();
  });

  it('aplicada e descartada mostram o estado, sem os botões de decisão', async () => {
    const applied = makeProposal({
      state: { state: 'applied', atMs: Date.now(), safetyPoint: 'ponto' },
    });
    const discarded = makeProposal({
      id: 'p3',
      change: { kind: 'removeMod', path: 'mods/railways.pw.toml' },
      summary: { target: 'Steam n Rails', from: null, to: null },
      state: { state: 'discarded', atMs: Date.now() },
    });
    const { url } = setup([applied, discarded]);
    const card = await openConversation(url);
    expect(within(card).getByText(/Aplicada hoje, .* · ponto de segurança criado/)).toBeDefined();
    expect(within(card).queryByRole('button', { name: /^Aplicar a proposta/ })).toBeNull();
    expect(within(card).getByRole('button', { name: 'Testar agora' })).toBeDefined();
    expect(within(card).getByRole('button', { name: /^Desfazer/ })).toBeDefined();
    const removed = screen.getByRole('region', { name: 'Remover Steam n Rails' });
    expect(within(removed).getByText('Descartada')).toBeDefined();
    expect(within(removed).getByText('Nada foi gravado no pack.')).toBeDefined();
  });

  it('a busca do culpado proposta abre o diálogo normal; a proposta fica aceita ao começar', async () => {
    const { backend, url } = setup([makeProposal(), BISECT], {
      bisect_plan: () => makePlan(),
      bisect_start: () => new Promise(() => undefined),
      ai_proposal_review: () => ({
        ...makeReview(),
        proposalId: 'p2',
        token: 'token-busca',
        changesPack: false,
        preview: { kind: 'startBisect' },
      }),
    });
    await openConversation(url);
    const card = screen.getByRole('region', { name: 'Encontrar o mod culpado' });
    expect(within(card).getByText(/testa o pack em rodadas/)).toBeDefined();
    const user = userEvent.setup();
    await user.click(
      within(card).getByRole('button', { name: /^Aplicar a proposta: Encontrar o mod culpado/ }),
    );
    const dialog = await screen.findByRole('dialog');
    // Abrir o diálogo não aceita nada; começar a busca aceita.
    expect(backend.callsOf('ai_proposal_apply')).toHaveLength(0);
    await user.click(await within(dialog).findByRole('button', { name: 'Começar a busca' }));
    await waitFor(() => {
      expect(backend.callsOf('ai_proposal_apply')[0]?.args).toMatchObject({
        proposalId: 'p2',
        token: 'token-busca',
      });
    });
    expect(backend.callsOf('bisect_start')).toHaveLength(1);
  });

  it('CA-T14-03: o bloco "Enviado ao GitHub" mostra byte a byte o pedido', async () => {
    const { url } = setup([makeProposal()]);
    await openConversation(url);
    const block = screen.getByTestId('enviado-github');
    expect(
      within(block).getByText(
        'Enviado ao GitHub: busca de issues em Layers-of-Railways/Railway com “NoSuchMethodError tickBogeys”',
      ),
    ).toBeDefined();
    expect(within(block).getByLabelText('Pedido enviado ao GitHub').textContent).toBe(
      GITHUB_STEP.external?.request,
    );
    // O bloco da consulta para a IA continua logo abaixo, com o título da D-14.
    expect(
      screen.getByText(
        /Enviado à IA: busca de issues de railways com “NoSuchMethodError tickBogeys”/,
      ),
    ).toBeDefined();
  });

  it('a lista de Conversas mostra as propostas esperando você', async () => {
    const { url } = setup([makeProposal()]);
    renderApp(`${url}/ia`);
    expect(
      await screen.findByText('1 proposta esperando você', {}, { timeout: 10_000 }),
    ).toBeDefined();
  });
});
