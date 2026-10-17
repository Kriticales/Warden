/**
 * "Testar como o jogador recebe" (SPEC T13; CA-T13-14; ROADMAP L-07) no app inteiro com o
 * backend simulado: o item do menu ▾, o teste começado nesse modo, o resultado na tela do teste
 * e os dados atualizados depois que o teste termina.
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';

import type { PlayerReport, TestRequest, TestSessionSummary } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { deferred } from '../../../test/backend';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { useTestStore } from '../store';
import { makeGame, makeSession, makeSessionView } from '../test.fixtures';

afterEach(() => {
  useTestStore.setState({ game: null, runs: {}, packChanged: {} });
});

function makeReport(overrides: Partial<PlayerReport> = {}): PlayerReport {
  return {
    source: {
      kind: 'published',
      link: 'https://raw.githubusercontent.com/dono/vale/main/pack.toml',
      version: '1.4.2',
    },
    receivedVersion: '1.4.2',
    outdated: false,
    receivedFiles: 42,
    crashAssistant: true,
    items: [],
    ...overrides,
  };
}

const AS_PLAYER: TestSessionSummary = makeSession({ mode: 'asPlayer' });

function header() {
  return screen.getByRole('banner', { name: 'Pack aberto' });
}

async function openPack(url: string) {
  renderApp(url);
  await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
}

/** A tela do teste com a última sessão (como o jogador recebe) e o resultado dado. */
function withPlayerSession(report: PlayerReport | null, session = AS_PLAYER) {
  return editorBackend({
    handlers: {
      test_sessions_list: () => [session],
      test_session_get: () => makeSessionView(session, []),
      test_player_report: () => report,
      test_start: () => deferred<TestSessionSummary>().promise,
    },
  });
}

describe('menu ▾: Testar como o jogador recebe', () => {
  it('começa o teste nesse modo e abre a tela do teste', async () => {
    const { backend, url } = editorBackend({
      handlers: { test_start: () => deferred<TestSessionSummary>().promise },
    });
    await openPack(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByText('Outros testes')).toBeDefined();
    const item = within(menu).getByRole('menuitem', { name: /Testar como o jogador recebe/ });
    expect(item.textContent).toContain('Instala pelo link do pack, como um jogador');
    await user.click(item);
    expect(await screen.findByRole('heading', { level: 1, name: 'Testando o pack' })).toBeDefined();
    expect(backend.callsOf('test_start')[0]?.args.request).toEqual({
      mode: 'asPlayer',
      profile: null,
      replaceInstanceChanges: false,
    } satisfies TestRequest);
  });

  it('com um jogo aberto (deste ou de outro pack), o item fica indisponível e diz por quê', async () => {
    const { backend, url } = editorBackend({
      handlers: {
        test_game_state: () =>
          makeGame('01J9ZQ0000000000000000OUTRO', 'running', {
            packName: 'Outro Pack',
          }),
      },
    });
    await openPack(url);
    await waitFor(() => {
      expect(backend.callsOf('test_game_state').length).toBeGreaterThan(0);
    });
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    const item = await screen.findByRole('menuitem', { name: /Testar como o jogador recebe/ });
    expect(item.getAttribute('aria-disabled')).toBe('true');
    expect(item.textContent).toContain('Feche o jogo antes');
    expect(backend.callsOf('test_start')).toHaveLength(0);
  });
});

describe('resultado: como o jogador recebe', () => {
  it('mostra a origem, os bloqueados, o Crash Assistant e as diferenças em linguagem simples; axe', async () => {
    const { url } = withPlayerSession(
      makeReport({
        items: [
          {
            kind: 'blocked',
            name: 'Entity Culling',
            path: 'mods/entityculling.jar',
            defaultOn: null,
          },
          { kind: 'notPublished', name: 'Sodium', path: 'mods/sodium.jar', defaultOn: null },
          { kind: 'serverOnly', name: 'Lithium', path: 'mods/lithium.jar', defaultOn: null },
          { kind: 'optional', name: 'Zoom', path: 'mods/zoom.jar', defaultOn: false },
          { kind: 'extra', name: 'velho.jar', path: 'mods/velho.jar', defaultOn: null },
        ],
      }),
    );
    renderApp(`${url}/teste`);
    const region = await screen.findByRole('region', { name: 'Como o jogador recebe' });
    expect(
      within(region).getByText(
        'Instalado do zero pelo link do pack (versão 1.4.2 publicada). O jogador recebeu 42 arquivos.',
      ),
    ).toBeDefined();
    expect(within(region).getByText('1 mod não baixa sozinho para os jogadores')).toBeDefined();
    expect(
      within(region).getByText('O Crash Assistant (ferramenta do jogador) chegou ao jogador.'),
    ).toBeDefined();
    const table = within(region).getByRole('table', {
      name: 'Diferenças entre o que o jogador recebeu e o pack',
    });
    const rows = within(table).getAllByRole('row').slice(1);
    expect(rows.map((row) => row.textContent)).toEqual([
      'Entity Cullingmods/entityculling.jarNão baixa sozinho: o jogador precisa baixar pelo site.',
      'Sodiummods/sodium.jarFaltando: ainda não está na versão que os jogadores recebem.',
      'Lithiummods/lithium.jarLado errado: na versão que os jogadores recebem, está marcado como "Só servidor".',
      'Zoommods/zoom.jarOpcional: o jogador escolhe na instalação (vem desmarcado).',
      'velho.jarmods/velho.jarA mais: o jogador recebe, mas não está mais no pack.',
    ]);
    expect(within(region).queryByText(/versão anterior/)).toBeNull();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('o GitHub ainda entregando a versão anterior: avisa e "Testar de novo como o jogador" repete nesse modo', async () => {
    const { backend, url } = withPlayerSession(makeReport({ outdated: true }));
    renderApp(`${url}/teste`);
    const region = await screen.findByRole('region', { name: 'Como o jogador recebe' });
    expect(
      within(region).getByText(
        'O GitHub ainda está entregando a versão anterior. Tente de novo em alguns minutos.',
      ),
    ).toBeDefined();
    expect(within(region).getByText('O jogador recebeu o mesmo que o pack.')).toBeDefined();
    await userEvent
      .setup()
      .click(within(region).getByRole('button', { name: 'Testar de novo como o jogador' }));
    await waitFor(() => {
      expect(backend.callsOf('test_start')).toHaveLength(1);
    });
    expect((backend.callsOf('test_start')[0]?.args.request as TestRequest).mode).toBe('asPlayer');
  });

  it('pack não publicado e sem Crash Assistant marcado: diz que veio do pack exportado agora', async () => {
    const { url } = withPlayerSession(
      makeReport({ source: { kind: 'local' }, crashAssistant: null, receivedFiles: 1 }),
    );
    renderApp(`${url}/teste`);
    const region = await screen.findByRole('region', { name: 'Como o jogador recebe' });
    expect(
      within(region).getByText(
        'O pack ainda não foi publicado: instalado do zero pelo pack exportado agora, servido só neste computador. O jogador recebeu 1 arquivo.',
      ),
    ).toBeDefined();
    expect(within(region).queryByText(/Crash Assistant/)).toBeNull();
  });

  it('o Crash Assistant que não chegou ao jogador é apontado', async () => {
    const { url } = withPlayerSession(makeReport({ crashAssistant: false }));
    renderApp(`${url}/teste`);
    expect(
      await screen.findByText(
        'O Crash Assistant (ferramenta do jogador) não chegou ao jogador. Confira se ele está na versão publicada.',
      ),
    ).toBeDefined();
  });

  it('sessão de um teste normal: nada aparece e o resultado nem é pedido', async () => {
    const normal = makeSession({ mode: 'normal' });
    const { backend, url } = withPlayerSession(null, normal);
    renderApp(`${url}/teste`);
    expect(
      await screen.findByRole('heading', { level: 1, name: 'O jogo fechou normalmente' }),
    ).toBeDefined();
    expect(screen.queryByRole('region', { name: 'Como o jogador recebe' })).toBeNull();
    expect(backend.callsOf('test_player_report')).toHaveLength(0);
  });
});

describe('dados atuais depois do teste', () => {
  it('ao terminar, o resultado novo, a lista de testes e o "último teste" de Meus packs são lidos de novo', async () => {
    const finished = deferred<TestSessionSummary>();
    let done = false;
    const setup = editorBackend({
      handlers: {
        test_start: () => finished.promise,
        test_player_report: () => makeReport(),
        test_sessions_list: () => (done ? [AS_PLAYER] : []),
        packs_list: () => [{ ...setup.row, lastTest: done ? 'ok' : null }],
      },
    });
    const { backend, url } = setup;
    await openPack(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    await user.click(await screen.findByRole('menuitem', { name: /Testar como o jogador recebe/ }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Testando o pack' })).toBeDefined();
    const sessionsBefore = backend.callsOf('test_sessions_list').length;
    done = true;
    await act(async () => {
      finished.resolve(AS_PLAYER);
      await Promise.resolve();
    });
    expect(await screen.findByRole('region', { name: 'Como o jogador recebe' })).toBeDefined();
    expect(backend.callsOf('test_player_report')[0]?.args).toMatchObject({
      sessionId: AS_PLAYER.id,
    });
    // Testes anteriores: a sessão nova aparece.
    await waitFor(() => {
      expect(backend.callsOf('test_sessions_list').length).toBeGreaterThan(sessionsBefore);
    });
    // Meus packs: o "último teste" do pack já mostra o resultado.
    await user.click(within(header()).getByRole('link', { name: 'Meus packs' }));
    await screen.findByRole('heading', { level: 1, name: 'Meus packs' }, { timeout: 10_000 });
    expect(await screen.findByText('Abriu normalmente')).toBeDefined();
  });
});
