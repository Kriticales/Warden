/**
 * Downloads manuais da CurseForge na tela do teste (SPEC T20; ROADMAP L-06), no app inteiro
 * com o backend simulado: a lista com o arquivo esperado e a situação de cada mod, "Selecionar
 * arquivo baixado…" com o arquivo errado recusado e o certo aceito (CA-T20-01), "Abrir página
 * de download", a lista sumindo quando todos chegam (o teste continua) e o axe.
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';

import type {
  GameState,
  ManualDownloadItem,
  ManualDownloadStatus,
  PackId,
} from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError, type Handler } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { useTestStore } from '../store';
import { makeGame } from '../test.fixtures';

afterEach(() => {
  useTestStore.setState({ game: null, runs: {}, packChanged: {} });
});

function item(
  name: string,
  fileName: string,
  status: ManualDownloadStatus = { state: 'waiting' },
  hasPage = true,
): ManualDownloadItem {
  const slug = fileName.split('-')[0] ?? fileName;
  return { metafile: `mods/${slug}.pw.toml`, name, fileName, hasPage, status };
}

const XAERO = item('Xaero’s World Map', 'xaeros-worldmap-1.39.0.jar');
const EPIC = item('Epic Fight', 'epicfight-forge-20.9.4-1.20.1.jar');

/** O pack com o teste esperando os downloads manuais de `list()`. */
function waiting(list: () => ManualDownloadItem[], handlers: Record<string, Handler> = {}) {
  let packId = '' as PackId;
  const setup = editorBackend({
    handlers: {
      test_game_state: (): GameState => makeGame(packId, 'preparing'),
      cf_blocked_list: () => list(),
      ...handlers,
    },
  });
  packId = setup.row.id;
  return setup;
}

async function openTest(url: string) {
  renderApp(url);
  await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
  const header = screen.getByRole('banner', { name: 'Pack aberto' });
  await userEvent
    .setup()
    .click(await within(header).findByRole('button', { name: 'Testando… ver progresso' }));
  await screen.findByRole('heading', { level: 1, name: 'Testando o pack' });
}

function row(name: string): HTMLElement {
  const cell = within(screen.getByRole('table', { name: 'Mods para baixar à mão' })).getByText(
    name,
  );
  const tr = cell.closest('tr');
  if (!tr) throw new Error(`linha de ${name} não encontrada`);
  return tr;
}

describe('downloads manuais da CurseForge (T20)', () => {
  it('mostra a lista com o arquivo esperado e a situação, esperando você; axe', async () => {
    const found = item('Xaero’s World Map', XAERO.fileName, {
      state: 'found',
      via: 'downloadsFolder',
    });
    const { url } = waiting(() => [found, EPIC]);
    await openTest(url);
    expect(
      await screen.findByText('2 mods da CurseForge precisam ser baixados à mão'),
    ).toBeDefined();
    expect(screen.getByText(/esperando você/)).toBeDefined();
    expect(screen.getByText(/procura sozinho na pasta Downloads/)).toBeDefined();
    expect(within(row('Xaero’s World Map')).getByText('Encontrado em Downloads')).toBeDefined();
    expect(within(row('Xaero’s World Map')).queryByRole('button')).toBeNull();
    const epic = row('Epic Fight');
    expect(within(epic).getByText(EPIC.fileName)).toBeDefined();
    expect(within(epic).getByText('Aguardando')).toBeDefined();
    expect(
      within(epic).getByRole('button', { name: 'Abrir página de download de Epic Fight' }),
    ).toBeDefined();
    expect(
      within(epic).getByRole('button', { name: 'Selecionar arquivo baixado de Epic Fight' }),
    ).toBeDefined();
    expect(screen.getByText(/Quem jogar o pack também vai precisar baixar/)).toBeDefined();
    // A barra de progresso da preparação dá lugar à lista.
    expect(screen.queryByRole('progressbar')).toBeNull();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T20-01: arquivo errado é recusado com o nome do certo; o certo é aceito', async () => {
    let list = [EPIC];
    let answer: ManualDownloadItem | null = null;
    const { backend, url } = waiting(() => list, {
      cf_blocked_provide_file: () => answer,
    });
    await openTest(url);
    const user = userEvent.setup();

    // Fechou o diálogo sem escolher: nada muda.
    await user.click(
      await screen.findByRole('button', { name: 'Selecionar arquivo baixado de Epic Fight' }),
    );
    await waitFor(() => {
      expect(backend.callsOf('cf_blocked_provide_file')).toHaveLength(1);
    });
    expect(within(row('Epic Fight')).getByText('Aguardando')).toBeDefined();

    const wrong = { ...EPIC, status: { state: 'wrongFile', chosenFile: 'epicfight (1).jar' } };
    answer = wrong as ManualDownloadItem;
    list = [answer];
    await user.click(
      screen.getByRole('button', { name: 'Selecionar arquivo baixado de Epic Fight' }),
    );
    expect(await within(row('Epic Fight')).findByText('Este não é o arquivo certo.')).toBeDefined();
    expect(
      within(row('Epic Fight')).getByText(
        `Você escolheu epicfight (1).jar. O certo é ${EPIC.fileName}.`,
      ),
    ).toBeDefined();
    expect(backend.callsOf('cf_blocked_provide_file')[1]?.args).toMatchObject({
      metafile: EPIC.metafile,
    });
    expect(
      screen.getByText(`Epic Fight: este não é o arquivo certo. O certo é ${EPIC.fileName}.`),
    ).toBeDefined();

    answer = { ...EPIC, status: { state: 'found', via: 'selected' } };
    list = [answer];
    await user.click(
      screen.getByRole('button', { name: 'Selecionar arquivo baixado de Epic Fight' }),
    );
    expect(await within(row('Epic Fight')).findByText('Encontrado')).toBeDefined();
    expect(
      screen.getByText('Epic Fight: arquivo certo. Ele foi guardado para os próximos testes.'),
    ).toBeDefined();
  });

  it('quando todos chegam, a lista some e a preparação continua', async () => {
    let list = [EPIC];
    const { backend, url } = waiting(() => list);
    await openTest(url);
    expect(await screen.findByRole('table', { name: 'Mods para baixar à mão' })).toBeDefined();
    list = [];
    const packId = (backend.callsOf('cf_blocked_list')[0]?.args.packId ?? '') as PackId;
    await act(async () => {
      await backend.emit('manual-downloads-changed', { packId });
    });
    await waitFor(() => {
      expect(screen.queryByRole('table', { name: 'Mods para baixar à mão' })).toBeNull();
    });
    expect(screen.getByRole('progressbar')).toBeDefined();
  });

  it('"Abrir página de download" pede ao backend; sem página, diz para procurar no site', async () => {
    const noPage = item('Mod Sem Página', 'sempagina-1.0.jar', { state: 'waiting' }, false);
    const { backend, url } = waiting(() => [EPIC, noPage], {
      cf_blocked_open_page: () => null,
    });
    await openTest(url);
    await userEvent
      .setup()
      .click(await screen.findByRole('button', { name: 'Abrir página de download de Epic Fight' }));
    await waitFor(() => {
      expect(backend.callsOf('cf_blocked_open_page')[0]?.args).toMatchObject({
        metafile: EPIC.metafile,
      });
    });
    const other = row('Mod Sem Página');
    expect(within(other).queryByRole('button', { name: /Abrir página/ })).toBeNull();
    expect(within(other).getByText('Procure Mod Sem Página no site da CurseForge.')).toBeDefined();
  });

  it('falha ao ler o arquivo escolhido vira aviso, e a linha continua esperando', async () => {
    const { url } = waiting(() => [EPIC], {
      cf_blocked_provide_file: () =>
        ipcError(makeAppError({ domain: 'core', code: 'IO' }, { params: { path: 'C:/x/y.jar' } })),
    });
    await openTest(url);
    await userEvent
      .setup()
      .click(
        await screen.findByRole('button', { name: 'Selecionar arquivo baixado de Epic Fight' }),
      );
    expect(
      await screen.findByText(/Não foi possível ler ou gravar um arquivo em C:\/x\/y.jar/),
    ).toBeDefined();
    expect(within(row('Epic Fight')).getByText('Aguardando')).toBeDefined();
  });
});
