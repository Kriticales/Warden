import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { PackRow } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError, mockBackend, type Handler } from '../../../test/backend';
import { makeAppError, makeImportPreview, makePackRow } from '../../../test/factories';
import { renderApp } from '../../../test/render';

/** Backend com uma lista que muda conforme as ações (como o registro real). */
function packsBackend(initial: PackRow[], extra: Record<string, Handler> = {}) {
  let rows = initial;
  const backend = mockBackend({
    packs_list: () => rows,
    pack_forget: ({ packId }) => {
      rows = rows.filter((row) => row.id !== packId);
      return null;
    },
    ...extra,
  });
  return {
    backend,
    setRows: (next: PackRow[]) => {
      rows = next;
    },
  };
}

/** Comandos que mudam algo no disco ou no registro. */
const WRITES = [
  'pack_create',
  'pack_import',
  'pack_relocate',
  'pack_hygiene_fix',
  'pack_trash',
  'pack_forget',
];

describe('Meus packs (T02)', () => {
  it('vazio: "Você ainda não tem packs" com Criar pack e Abrir ou importar…; passa no axe', async () => {
    packsBackend([]);
    const { container } = renderApp('/packs');
    // Primeiro teste do arquivo: o carregamento frio dos módulos pode passar de 1 s na CI.
    expect(
      await screen.findByRole('heading', { name: 'Você ainda não tem packs' }, { timeout: 5000 }),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Criar pack' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Abrir ou importar…' })).toBeDefined();
    expect(await axePage(container)).toHaveNoViolations();
  });

  it('mostra cada pack com versões, último teste, não salvas (CA-T02-04) e data; passa no axe', async () => {
    const today = Date.now();
    packsBackend([
      makePackRow({ name: 'Vale Sereno', unsavedFiles: 5, lastTest: 'ok', modifiedAtMs: today }),
      makePackRow({
        name: 'Cobre Antigo',
        minecraft: '1.7.10',
        loader: 'forge',
        loaderVersion: '10.13.4.1614',
        version: '2.3.0',
        lastTest: 'crashed',
        modifiedAtMs: new Date(2026, 0, 15).getTime(),
      }),
      makePackRow({ name: 'Puro', loader: null, loaderVersion: null, modifiedAtMs: null }),
    ]);
    const { container } = renderApp('/packs');
    const table = await screen.findByRole('table', { name: 'Packs registrados neste computador' });
    expect(screen.getByText('3 packs')).toBeDefined();
    const rows = within(table).getAllByRole('row').slice(1);
    expect(rows).toHaveLength(3);

    const vale = rows[0];
    if (!vale) throw new Error('linha ausente');
    expect(within(vale).getByText('Minecraft 1.20.1 · Forge 47.3.0')).toBeDefined();
    expect(within(vale).getByText('Abriu normalmente')).toBeDefined();
    // O contador é o número de arquivos alterados desde a última versão salva.
    expect(within(vale).getByLabelText('5 alterações não salvas').textContent).toBe('5');
    expect(within(vale).getByText(/^hoje, \d\d:\d\d$/)).toBeDefined();
    expect(within(vale).getByRole('button', { name: 'Abrir Vale Sereno' })).toBeDefined();

    const cobre = screen.getByText('Cobre Antigo').closest('tr');
    if (!cobre) throw new Error('linha ausente');
    expect(within(cobre).getByText('Minecraft 1.7.10 · Forge 10.13.4.1614')).toBeDefined();
    expect(within(cobre).getByText('Travou')).toBeDefined();
    expect(within(cobre).getByText('2.3.0')).toBeDefined();
    expect(within(cobre).getByText('15/01/2026')).toBeDefined();

    const puro = screen.getByText('Puro').closest('tr');
    if (!puro) throw new Error('linha ausente');
    expect(within(puro).getByText('Minecraft 1.20.1 · vanilla')).toBeDefined();
    expect(within(puro).getByText('Nunca testado')).toBeDefined();
    expect(within(puro).getByLabelText('Nenhuma alteração não salva')).toBeDefined();
    expect(await axePage(container)).toHaveNoViolations();
  });

  it('com 50 packs, mostra as 50 linhas (o tempo do CA-T02-01 é medido no E2E, no WebView real)', async () => {
    const rows = Array.from({ length: 50 }, (_, index) =>
      makePackRow({ name: `Pack ${String(index + 1)}`, modifiedAtMs: index }),
    );
    packsBackend(rows);
    renderApp('/packs');
    await screen.findByText('Pack 50', {}, { timeout: 5000 });
    expect(screen.getByText('50 packs')).toBeDefined();
    expect(screen.getAllByRole('button', { name: /^Abrir Pack \d+$/ })).toHaveLength(50);
  });

  it('busca e ordena; sem resultado, oferece limpar a busca', async () => {
    const user = userEvent.setup();
    packsBackend([
      makePackRow({ name: 'Zebra', modifiedAtMs: 300 }),
      makePackRow({ name: 'Árvore', modifiedAtMs: 100 }),
      makePackRow({ name: 'Mesa', modifiedAtMs: 200 }),
    ]);
    renderApp('/packs');
    await screen.findByText('Zebra');
    const names = () =>
      screen
        .getAllByRole('button', { name: /^Abrir (?!ou importar)/ })
        .map((button) => button.getAttribute('aria-label'));
    expect(names()).toEqual(['Abrir Zebra', 'Abrir Mesa', 'Abrir Árvore']);
    await user.selectOptions(screen.getByRole('combobox', { name: 'Ordenar' }), 'Ordenar: nome');
    expect(names()).toEqual(['Abrir Árvore', 'Abrir Mesa', 'Abrir Zebra']);

    await user.type(screen.getByRole('searchbox', { name: 'Buscar pack' }), 'arvo');
    await waitFor(() => {
      expect(names()).toEqual(['Abrir Árvore']);
    });
    await user.clear(screen.getByRole('searchbox', { name: 'Buscar pack' }));
    await user.type(screen.getByRole('searchbox', { name: 'Buscar pack' }), 'xyz');
    expect(await screen.findByRole('heading', { name: 'Nenhum pack com “xyz”' })).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Limpar busca' }));
    await waitFor(() => {
      expect(names()).toHaveLength(3);
    });
  });

  it('Abrir leva ao pack aberto (P1-08: abre em Mods), com os dados de pack_get', async () => {
    const user = userEvent.setup();
    const row = makePackRow({ name: 'Vale Sereno', unsavedFiles: 2 });
    packsBackend([row], {
      pack_get: () => row,
      pack_hygiene_scan: () => [],
      inventory_list: () => ({ items: [], indexError: null }),
    });
    renderApp('/packs');
    await user.click(await screen.findByRole('button', { name: 'Abrir Vale Sereno' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Vale Sereno' })).toBeDefined();
    const header = screen.getByRole('banner', { name: 'Pack aberto' });
    expect(header.textContent).toContain('Minecraft 1.20.1');
    expect(header.textContent).toContain('Forge 47.3.0');
    expect(within(header).getByRole('link', { name: 'Meus packs' })).toBeDefined();
    expect(await screen.findByRole('heading', { level: 1, name: 'Mods' })).toBeDefined();
  });

  it('menu ⋯ pelo teclado: Mostrar na pasta chama pack_reveal_folder', async () => {
    const user = userEvent.setup();
    const row = makePackRow({ name: 'Vale Sereno' });
    const { backend } = packsBackend([row], { pack_reveal_folder: () => null });
    renderApp('/packs');
    const trigger = await screen.findByRole('button', { name: 'Mais ações para Vale Sereno' });
    trigger.focus();
    await user.keyboard('{Enter}');
    const menu = await screen.findByRole('menu', { name: 'Mais ações para Vale Sereno' });
    const items = within(menu).getAllByRole('menuitem');
    expect(items.map((item) => item.textContent)).toEqual([
      'Mostrar na pasta',
      'Remover da listaNão apaga nenhum arquivo',
      'Apagar pack…Vai para a Lixeira, com o histórico junto',
    ]);
    await user.click(within(menu).getByRole('menuitem', { name: /Mostrar na pasta/ }));
    await waitFor(() => {
      expect(backend.callsOf('pack_reveal_folder')).toEqual([
        { command: 'pack_reveal_folder', args: { packId: row.id } },
      ]);
    });
  });

  it('CA-T02-03: "Remover da lista" confirma e só chama pack_forget (nenhuma outra escrita)', async () => {
    const user = userEvent.setup();
    const row = makePackRow({ name: 'Vale Sereno' });
    const { backend } = packsBackend([row, makePackRow({ name: 'Outro' })]);
    renderApp('/packs');
    await user.click(await screen.findByRole('button', { name: 'Mais ações para Vale Sereno' }));
    await user.click(await screen.findByRole('menuitem', { name: /Remover da lista/ }));
    const dialog = await screen.findByRole('alertdialog', {
      name: 'Remover “Vale Sereno” da lista?',
    });
    expect(within(dialog).getByText(/continuam no computador, sem nenhuma mudança/)).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Remover da lista' }));
    expect(
      await screen.findByText('“Vale Sereno” saiu da lista. Nenhum arquivo foi apagado.'),
    ).toBeDefined();
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: 'Abrir Vale Sereno' })).toBeNull();
    });
    // A página continua clicável depois do diálogo aberto pelo menu.
    expect(document.body.style.pointerEvents).toBe('');
    const writes = backend.calls.filter((call) => WRITES.includes(call.command));
    expect(writes).toEqual([{ command: 'pack_forget', args: { packId: row.id } }]);
  });

  it('Apagar pack só habilita depois de digitar o nome; manda para a Lixeira', async () => {
    const user = userEvent.setup();
    const row = makePackRow({ name: 'Vale Sereno' });
    const { backend, setRows } = packsBackend([row], {
      pack_trash: () => {
        setRows([]);
        return null;
      },
    });
    renderApp('/packs');
    await user.click(await screen.findByRole('button', { name: 'Mais ações para Vale Sereno' }));
    await user.click(await screen.findByRole('menuitem', { name: /Apagar pack/ }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Apagar “Vale Sereno”?' });
    const confirm = within(dialog).getByRole('button', { name: 'Apagar pack' });
    expect((confirm as HTMLButtonElement).disabled).toBe(true);
    await user.type(within(dialog).getByRole('textbox'), 'Vale');
    expect((confirm as HTMLButtonElement).disabled).toBe(true);
    await user.type(within(dialog).getByRole('textbox'), ' Sereno');
    expect((confirm as HTMLButtonElement).disabled).toBe(false);
    await user.click(confirm);
    expect(await screen.findByText('“Vale Sereno” foi para a Lixeira.')).toBeDefined();
    expect(backend.callsOf('pack_trash')).toEqual([
      { command: 'pack_trash', args: { packId: row.id, confirmation: 'Vale Sereno' } },
    ]);
    expect(await screen.findByRole('heading', { name: 'Você ainda não tem packs' })).toBeDefined();
  });

  it('CA-T02-02: pasta não encontrada → "Localizar…" com a nova pasta restaura a linha', async () => {
    const user = userEvent.setup();
    const id = makePackRow().id;
    const missing = makePackRow({
      id,
      name: 'Antigo Survival',
      path: 'D:/Packs/antigo-survival',
      status: 'folderMissing',
      minecraft: null,
      loader: null,
      loaderVersion: null,
      version: null,
      lastTest: 'ok',
    });
    const { backend, setRows } = packsBackend([missing], {
      pack_choose_folder: () => 'D:/Packs/antigo-survival-2',
      pack_relocate: ({ path }) => {
        setRows([{ ...missing, status: 'ready', path: path as string, minecraft: '1.20.1' }]);
        return null;
      },
    });
    const { container } = renderApp('/packs');
    expect(await screen.findByText('Pasta não encontrada: D:/Packs/antigo-survival')).toBeDefined();
    expect(screen.getByText('Foi movida ou apagada fora do Warden.')).toBeDefined();
    expect(await axePage(container)).toHaveNoViolations();
    await user.click(screen.getByRole('button', { name: 'Localizar…' }));
    expect(await screen.findByText('Pasta de “Antigo Survival” encontrada.')).toBeDefined();
    expect(backend.callsOf('pack_choose_folder')[0]?.args).toEqual({ purpose: 'relocate' });
    expect(backend.callsOf('pack_relocate')[0]?.args).toEqual({
      packId: id,
      path: 'D:/Packs/antigo-survival-2',
    });
    // A linha volta pronta, com o histórico de testes preservado.
    expect(await screen.findByRole('button', { name: 'Abrir Antigo Survival' })).toBeDefined();
    expect(screen.getByText('Abriu normalmente')).toBeDefined();
  });

  it('Localizar… com a pasta de outro pack explica o motivo; desistir do diálogo não faz nada', async () => {
    const user = userEvent.setup();
    let answer: string | null = null;
    const missing = makePackRow({ name: 'Antigo', status: 'folderMissing' });
    const { backend } = packsBackend([missing], {
      pack_choose_folder: () => answer,
      pack_relocate: () =>
        ipcError(
          makeAppError(
            { domain: 'project', code: 'INVALID_PACK' },
            { params: { reason: 'otherPack' } },
          ),
        ),
    });
    renderApp('/packs');
    await user.click(await screen.findByRole('button', { name: 'Localizar…' }));
    await waitFor(() => {
      expect(backend.callsOf('pack_choose_folder')).toHaveLength(1);
    });
    expect(backend.callsOf('pack_relocate')).toHaveLength(0);
    answer = 'D:/Packs/outro';
    await user.click(screen.getByRole('button', { name: 'Localizar…' }));
    expect(
      await screen.findByText(
        'Esta pasta é de outro pack: o identificador em .warden/project.toml não é o de “Antigo”.',
      ),
    ).toBeDefined();
  });

  it('pack ilegível: aviso no topo, "Ver detalhes" com o erro técnico e Mostrar na pasta', async () => {
    const user = userEvent.setup();
    const broken = makePackRow({
      name: 'Pack do Lucas',
      status: 'invalidPack',
      detail: 'pack.toml: TOML parse error at line 4',
    });
    const { backend } = packsBackend([broken, makePackRow({ name: 'Bom' })], {
      pack_reveal_folder: () => null,
    });
    renderApp('/packs');
    expect(await screen.findByText('Não foi possível ler 1 pack.')).toBeDefined();
    expect(screen.getByText('Os outros packs não são afetados.')).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Ver detalhes' }));
    const dialog = await screen.findByRole('dialog', { name: 'Por que “Pack do Lucas” não abre' });
    expect(within(dialog).getByText('pack.toml: TOML parse error at line 4')).toBeDefined();
    expect(within(dialog).getByText('project.INVALID_PACK')).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Mostrar na pasta' }));
    await waitFor(() => {
      expect(backend.callsOf('pack_reveal_folder')).toHaveLength(1);
    });
  });

  it('pack só para leitura ganha a marca com o motivo', async () => {
    packsBackend([makePackRow({ name: 'Clonado', readOnlyReason: 'merge em andamento' })]);
    renderApp('/packs');
    expect(await screen.findByRole('button', { name: 'Só leitura' })).toBeDefined();
  });

  it('"Abrir ou importar…" abre o diálogo de pasta e leva a pasta para Abrir pack', async () => {
    const user = userEvent.setup();
    const { backend } = packsBackend([], {
      pack_choose_folder: () => 'D:/Packs/meu-pack-antigo',
      pack_import_preview: () => makeImportPreview(),
    });
    renderApp('/packs');
    // Espera a lista chegar: enquanto ela é lida, o botão é o do cabeçalho.
    await screen.findByRole('heading', { name: 'Você ainda não tem packs' });
    await user.click(screen.getByRole('button', { name: 'Abrir ou importar…' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Abrir pack' })).toBeDefined();
    expect(await screen.findByText('Meu pack antigo')).toBeDefined();
    expect(backend.callsOf('pack_choose_folder')[0]?.args).toEqual({ purpose: 'openPack' });
    expect(backend.callsOf('pack_import_preview')[0]?.args).toEqual({
      path: 'D:/Packs/meu-pack-antigo',
    });
  });

  it('erro ao ler a lista: painel de erro com "Tentar de novo"', async () => {
    const user = userEvent.setup();
    let fail = true;
    mockBackend({
      packs_list: () => (fail ? ipcError(makeAppError({ domain: 'app', code: 'INTERNAL' })) : []),
    });
    renderApp('/packs');
    const retry = await screen.findByRole('button', { name: 'Tentar de novo' });
    fail = false;
    await user.click(retry);
    expect(await screen.findByRole('heading', { name: 'Você ainda não tem packs' })).toBeDefined();
  });
});
