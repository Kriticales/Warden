/**
 * Pack aberto (T05), Mods (T06) e detalhes do item (T07), no app inteiro com o backend
 * simulado. Os critérios de aceite estão no nome de cada teste.
 */
import { focusManager } from '@tanstack/react-query';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { InventoryItem } from '../../lib/ipc/bindings';
import { axePage } from '../../test/axe';
import { renderApp } from '../../test/render';
import {
  AE2,
  INVALID,
  makeDetails,
  makeInventory,
  makeItem,
  OUTSIDE,
  RESOURCE_PACK,
  SHADER,
  SODIUM,
} from './editor.fixtures';
import { editorBackend } from './testing';

const SECTIONS = [
  'Mods',
  'Configs',
  'Problemas',
  'Diagnóstico com IA',
  'Histórico',
  'Exportar',
] as const;

/**
 * O jsdom não mede elementos (`offsetHeight` é 0). A lista virtualizada precisa da altura da área
 * que rola (o `<main>` do pack): nos testes com muitos itens ela tem 900 px.
 */
function giveMainAHeight() {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return this.id === 'conteudo' ? 900 : 0;
  });
}

afterEach(() => {
  vi.restoreAllMocks();
});

/** O menu lateral do pack. */
function sectionMenu() {
  return screen.getByRole('navigation', { name: 'Seções do pack' });
}

async function openMods(url: string) {
  renderApp(url);
  // Primeiro teste do arquivo: a carga fria dos módulos pode passar de 1 s na CI. A lista
  // está pronta quando a busca aparece (o título vem antes, enquanto o índice é lido).
  await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
  return screen.findByRole('searchbox', { name: 'Buscar no pack' });
}

describe('pack aberto (T05)', () => {
  it('CA-T05-03: abre em Mods, com o cabeçalho (nome, ← Meus packs) e exatamente as 6 seções; axe', async () => {
    const { url } = editorBackend({ pack: { unsavedFiles: 5, version: '1.4.2' } });
    await openMods(url);

    const header = screen.getByRole('banner', { name: 'Pack aberto' });
    expect(within(header).getByRole('heading', { level: 1, name: 'Vale Sereno' })).toBeDefined();
    expect(within(header).getByRole('link', { name: 'Meus packs' }).getAttribute('href')).toBe(
      '/packs',
    );
    expect(header.textContent).toContain('Minecraft 1.20.1');
    expect(header.textContent).toContain('Forge 47.3.0');
    expect(header.textContent).toContain('versão 1.4.2');
    expect(
      within(header).getByRole('button', { name: 'Salvar versão, 5 alterações não salvas' }),
    ).toBeDefined();
    expect(within(header).getByRole('button', { name: 'Editar informações' })).toBeDefined();

    const items = within(sectionMenu()).getAllByRole('listitem');
    expect(items.map((item) => item.querySelector('.secmenu__name')?.textContent)).toEqual([
      ...SECTIONS,
    ]);
    const mods = within(sectionMenu()).getByRole('link', { name: /Mods/ });
    expect(mods.getAttribute('aria-current')).toBe('page');
    // Mods tem o contador de itens; as seções ainda sem página aparecem indisponíveis.
    expect(within(mods).getByLabelText('4 itens')).toBeDefined();
    expect(sectionMenu().querySelectorAll('[aria-disabled="true"]')).toHaveLength(5);

    const { container } = { container: document.body };
    expect(await axePage(container)).toHaveNoViolations();
  });

  it('CA-T05-03: fora de um pack (Meus packs) não aparece nada do pack', async () => {
    editorBackend();
    renderApp('/packs');
    await screen.findByRole('heading', { level: 1, name: 'Meus packs' }, { timeout: 10_000 });
    expect(screen.queryByRole('navigation', { name: 'Seções do pack' })).toBeNull();
    expect(screen.queryByRole('banner', { name: 'Pack aberto' })).toBeNull();
    expect(screen.queryByText('Salvar versão')).toBeNull();
  });

  it('pasta sumida: diz por quê e o caminho, sem o menu do pack', async () => {
    const { url } = editorBackend({ pack: { status: 'folderMissing', path: 'C:/Packs/sumiu' } });
    renderApp(url);
    expect(
      await screen.findByText('A pasta deste pack não foi encontrada', {}, { timeout: 10_000 }),
    ).toBeDefined();
    expect(screen.getByText('C:/Packs/sumiu')).toBeDefined();
    expect(screen.queryByRole('navigation', { name: 'Seções do pack' })).toBeNull();
  });

  it('CA-T05-01: o que mudou no disco aparece ao voltar para a janela e com pack-changed', async () => {
    let inventory = makeInventory([SODIUM]);
    const { backend, url, row } = editorBackend({ handlers: { inventory_list: () => inventory } });
    await openMods(url);
    expect(screen.getByRole('button', { name: 'Sodium' })).toBeDefined();

    // Alguém mudou o pack por fora; voltar ao Warden relê o disco.
    inventory = makeInventory([SODIUM, AE2]);
    act(() => {
      focusManager.setFocused(false);
      focusManager.setFocused(true);
    });
    expect(await screen.findByRole('button', { name: 'Applied Energistics 2' })).toBeDefined();
    act(() => {
      focusManager.setFocused(undefined);
    });

    // E o evento pack-changed (escrita do Warden) também.
    inventory = makeInventory([AE2]);
    await act(() => backend.emit('pack-changed', { packId: row.id, areas: ['inventory'] }));
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: 'Sodium' })).toBeNull();
    });
  });
});

describe('Mods (T06)', () => {
  it('uma lista agrupada por tipo, com versão legível e fonte; nunca um ID (CA-T06-02); axe', async () => {
    const { url } = editorBackend();
    await openMods(url);
    expect(screen.getByText('4 itens: 2 mods, 1 resource pack e 1 shader.')).toBeDefined();
    for (const group of ['Mods', 'Resource packs', 'Shaders']) {
      expect(screen.getByRole('button', { name: group, expanded: true })).toBeDefined();
    }
    const modsTable = screen.getByRole('table', { name: 'Itens do pack: Mods' });
    const sodium = within(modsTable).getByRole('button', { name: 'Sodium' }).closest('tr');
    expect(sodium?.textContent).toContain('0.8.13+mc1.21.1');
    expect(sodium?.textContent).toContain('Modrinth');
    expect(sodium?.textContent).toContain('Fixado');
    const ae2 = within(modsTable)
      .getByRole('button', { name: 'Applied Energistics 2' })
      .closest('tr');
    expect(ae2?.textContent).toContain('appliedenergistics2-forge-15.4.10');
    expect(ae2?.textContent).toContain('CurseForge');

    // Dados reais: ID da versão do Modrinth, projeto e file-id da CurseForge não aparecem.
    const text = document.body.textContent;
    for (const id of ['SMxNOGZ6', 'AANobbMI', '7148487', '223794']) {
      expect(text).not.toContain(id);
    }
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T06-01: 1 arquivo inválido entre 100 vira uma linha com erro e o resto aparece', async () => {
    const good = Array.from({ length: 99 }, (_, index) =>
      makeItem({ name: `Mod ${String(index).padStart(2, '0')}` }),
    );
    giveMainAHeight();
    const { url } = editorBackend({ inventory: makeInventory([...good, INVALID]) });
    await openMods(url);
    expect(screen.getByText('100 itens: 100 mods.')).toBeDefined();
    expect(screen.getByText('1 arquivo do pack não pôde ser lido.')).toBeDefined();
    const table = screen.getByRole('table', { name: 'Itens do pack: Mods' });
    expect(table.getAttribute('aria-rowcount')).toBe('101');
    expect(within(table).getByRole('button', { name: 'Mod 00' })).toBeDefined();

    // Só os inválidos: o arquivo com o erro e as ações Ver erro e Abrir no editor de texto.
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Com problemas (1)' }));
    const row = screen.getByRole('button', { name: 'mods/sodium-extra.pw.toml' }).closest('tr');
    expect(row?.textContent).toContain('Arquivo inválido');
    await user.click(within(row as HTMLElement).getByRole('button', { name: 'Ver erro' }));
    const dialog = await screen.findByRole('dialog', { name: 'Erro em mods/sodium-extra.pw.toml' });
    expect(dialog.textContent).toContain('falta fechar as aspas');
  });

  it('com 500 itens só as linhas visíveis vão para o DOM (lista virtualizada)', async () => {
    const many = Array.from({ length: 500 }, (_, index) =>
      makeItem({ name: `Item ${String(index).padStart(3, '0')}` }),
    );
    giveMainAHeight();
    const { url } = editorBackend({ inventory: makeInventory(many) });
    await openMods(url);
    const table = screen.getByRole('table', { name: 'Itens do pack: Mods' });
    expect(table.getAttribute('aria-rowcount')).toBe('501');
    const rendered = table.querySelectorAll('tbody tr.modrow').length;
    expect(rendered).toBeGreaterThan(10);
    expect(rendered).toBeLessThan(80);
  });

  it('busca e filtros (caixas de seleção); sem resultado oferece limpar', async () => {
    const { url } = editorBackend({
      inventory: makeInventory([SODIUM, AE2, RESOURCE_PACK, SHADER, OUTSIDE]),
    });
    await openMods(url);
    const user = userEvent.setup();
    await user.type(screen.getByRole('searchbox', { name: 'Buscar no pack' }), 'fresh');
    expect(screen.getByRole('button', { name: 'Fresh Animations' })).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Sodium' })).toBeNull();
    await user.clear(screen.getByRole('searchbox', { name: 'Buscar no pack' }));

    await user.selectOptions(screen.getByRole('combobox', { name: 'Fonte' }), 'curseforge');
    expect(screen.getByRole('button', { name: 'Applied Energistics 2' })).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Sodium' })).toBeNull();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Lado' }), 'server');
    expect(screen.getByText('Nenhum item com estes filtros')).toBeDefined();
    const [clear] = screen.getAllByRole('button', { name: 'Limpar filtros' });
    if (!clear) throw new Error('sem Limpar filtros');
    await user.click(clear);
    expect(screen.getByRole('button', { name: 'Sodium' })).toBeDefined();
  });

  it('lado na linha grava só aquele item', async () => {
    const { backend, url } = editorBackend({
      handlers: { items_set_side: () => ['mods/sodium.pw.toml'] },
    });
    await openMods(url);
    const user = userEvent.setup();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Lado de Sodium' }), 'both');
    await waitFor(() => {
      expect(backend.callsOf('items_set_side')).toHaveLength(1);
    });
    expect(backend.callsOf('items_set_side')[0]?.args).toMatchObject({
      paths: ['mods/sodium.pw.toml'],
      side: 'both',
    });
    expect(await screen.findByText('Lado de 1 item alterado.')).toBeDefined();
  });

  it('CA-T06-03 (interface): Alterar lado de vários manda todos numa chamada só', async () => {
    const { backend, url } = editorBackend({
      handlers: {
        items_set_side: ({ paths }) => [...(paths as string[]), '.packwizignore'],
      },
    });
    await openMods(url);
    const user = userEvent.setup();
    await user.click(screen.getByRole('checkbox', { name: 'Selecionar todos de Mods' }));
    const bar = screen.getByRole('region', { name: 'Ações para os selecionados' });
    expect(bar.textContent).toContain('2 selecionados');
    await user.click(within(bar).getByRole('button', { name: 'Alterar lado' }));
    await user.click(await screen.findByRole('menuitem', { name: 'Mudar para: Só servidor' }));
    await waitFor(() => {
      expect(backend.callsOf('items_set_side')).toHaveLength(1);
    });
    expect(backend.callsOf('items_set_side')[0]?.args).toMatchObject({
      paths: ['mods/sodium.pw.toml', 'mods/applied-energistics-2.pw.toml'],
      side: 'server',
    });
    // O .packwizignore que o Warden completou não conta como item.
    expect(await screen.findByText('Lado de 2 itens alterado.')).toBeDefined();
  });

  it('Remover mostra quem depende do item e só remove ao confirmar', async () => {
    const { backend, url } = editorBackend({
      handlers: {
        items_remove_plan: () => ({
          targets: [
            {
              key: SODIUM.key,
              path: SODIUM.path,
              name: 'Sodium',
              files: ['mods/sodium.pw.toml'],
            },
          ],
          dependents: [
            { key: 'modrinth:indium', name: 'Indium', needs: ['Sodium'] },
            { key: 'modrinth:extra', name: 'Sodium Extra', needs: ['Sodium'] },
          ],
        }),
        items_remove: () => ['mods/sodium.pw.toml'],
      },
    });
    await openMods(url);
    const user = userEvent.setup();
    await user.click(screen.getByRole('checkbox', { name: 'Selecionar Sodium' }));
    await user.click(screen.getByRole('button', { name: 'Remover' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Remover Sodium?' });
    expect(
      await within(dialog).findByText(
        'Estes mods dependem dele: Indium, Sodium Extra. Sem ele, podem não funcionar.',
      ),
    ).toBeDefined();
    expect(backend.callsOf('items_remove')).toHaveLength(0);
    await user.click(within(dialog).getByRole('button', { name: 'Remover Sodium' }));
    await waitFor(() => {
      expect(backend.callsOf('items_remove')).toHaveLength(1);
    });
    expect(backend.callsOf('items_remove')[0]?.args).toMatchObject({
      paths: ['mods/sodium.pw.toml'],
    });
    expect(await screen.findByText('1 item removido do pack.')).toBeDefined();
  });

  it('arquivos fora do índice: aviso com "Incluir no pack"', async () => {
    const { backend, url } = editorBackend({
      inventory: makeInventory([SODIUM, OUTSIDE]),
      handlers: { inventory_include_outside: () => null },
    });
    await openMods(url);
    expect(screen.getByText('1 arquivo está na pasta, mas fora do índice do pack.')).toBeDefined();
    const row = screen.getByRole('button', { name: OUTSIDE.name }).closest('tr');
    expect(row?.textContent).toContain('Fora do índice');
    await userEvent.setup().click(screen.getByRole('button', { name: 'Incluir no pack' }));
    await waitFor(() => {
      expect(backend.callsOf('inventory_include_outside')).toHaveLength(1);
    });
  });

  it('vazio: "Nenhum mod ainda" com o Minecraft e o loader do pack; axe', async () => {
    const { url } = editorBackend({ inventory: makeInventory([]) });
    renderApp(url);
    expect(
      await screen.findByRole('heading', { name: 'Nenhum mod ainda' }, { timeout: 10_000 }),
    ).toBeDefined();
    expect(
      screen.getByText(/O pack Vale Sereno é para Minecraft 1\.20\.1 com Forge 47\.3\.0\./),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });
});

describe('detalhes do item (T07)', () => {
  async function openDetails(item: InventoryItem, handlers: Parameters<typeof editorBackend>[0]) {
    const setup = editorBackend(handlers);
    await openMods(setup.url);
    await userEvent.setup().click(screen.getByRole('button', { name: item.name }));
    const drawer = await screen.findByRole('dialog', { name: 'Detalhes do mod' });
    return { ...setup, drawer };
  }

  it('abre num painel lateral sem sair da lista, com versão, arquivo, hash e lado; axe', async () => {
    const { drawer } = await openDetails(SODIUM, {
      handlers: {
        item_details: () =>
          makeDetails(SODIUM, {
            authors: ['jellysquid3', 'IMS'],
            pageUrl: 'https://modrinth.com/mod/sodium',
            description: '# Sodium\n\nDeixa o jogo **mais leve**.',
            version: {
              number: '0.8.13+mc1.21.1',
              gameVersions: ['1.21.1'],
              loaders: ['fabric'],
              published: '2026-09-12T10:00:00Z',
              fileName: 'sodium-fabric-0.8.13+mc1.21.1.jar',
              sizeBytes: 1_258_291,
            },
            changelog: '- Corrige um travamento',
          }),
      },
    });
    // A lista continua atrás do painel (escondida do leitor de tela enquanto ele está aberto).
    expect(screen.getByRole('heading', { level: 1, name: 'Mods', hidden: true })).toBeDefined();
    expect(drawer.textContent).toContain('por jellysquid3, IMS');
    expect(within(drawer).getByRole('button', { name: 'Abrir página' })).toBeDefined();
    expect(await within(drawer).findByText('mais leve')).toBeDefined();
    expect(drawer.textContent).toContain('Minecraft 1.21.1 · fabric');
    expect(drawer.textContent).toContain('sodium-fabric-0.8.13+mc1.21.1.jar · 1,2 MB');
    expect(drawer.textContent).toContain(
      '8614a374593698069a80ac74de8cd9c28f3928d16819a0e0d6320b538e823f7e',
    );
    expect(within(drawer).getByRole<HTMLSelectElement>('combobox', { name: 'Lado' }).value).toBe(
      'client',
    );
    expect(within(drawer).getByText('Novidades da versão 0.8.13+mc1.21.1')).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T07-01: descrição com <script> e onerror= não executa código', async () => {
    const flag = { ran: false };
    Object.assign(window, { wardenXss: flag });
    const html =
      '<p>Mod de teste</p><script>window.wardenXss.ran = true</script>' +
      '<img src="x" onerror="window.wardenXss.ran = true"><a href="javascript:alert(1)">link</a>';
    const { drawer } = await openDetails(AE2, {
      handlers: {
        item_details: () =>
          makeDetails(AE2, { description: html, descriptionFormat: 'html', source: 'live' }),
      },
    });
    expect(await within(drawer).findByText('Mod de teste')).toBeDefined();
    expect(drawer.querySelector('script')).toBeNull();
    expect(drawer.querySelector('[onerror]')).toBeNull();
    expect(drawer.querySelector('a[href^="javascript"]')).toBeNull();
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(flag.ran).toBe(false);
  });

  it('CA-T07-02: CurseForge sem internet avisa e ainda mostra nome, arquivo, lado e hash', async () => {
    const { drawer } = await openDetails(AE2, {
      handlers: {
        item_details: () =>
          makeDetails(AE2, {
            source: 'offline',
            file: {
              fileName: 'appliedenergistics2-forge-15.4.10.jar',
              hashFormat: 'sha1',
              hash: 'ac255a120499f79f8deade474da09e3c00ff9bad',
              url: null,
            },
          }),
      },
    });
    expect(await within(drawer).findByText('Detalhes indisponíveis sem internet')).toBeDefined();
    expect(drawer.textContent).toContain('Applied Energistics 2');
    expect(drawer.textContent).toContain('appliedenergistics2-forge-15.4.10.jar');
    expect(drawer.textContent).toContain('ac255a120499f79f8deade474da09e3c00ff9bad');
    expect(within(drawer).getByRole<HTMLSelectElement>('combobox', { name: 'Lado' }).value).toBe(
      'both',
    );
  });

  it('CA-T07-02: Modrinth sem internet abre do cache e diz de onde veio', async () => {
    const { drawer } = await openDetails(SODIUM, {
      handlers: {
        item_details: () => makeDetails(SODIUM, { source: 'cache', description: 'Do cache.' }),
      },
    });
    expect(
      await within(drawer).findByText(
        'Mostrando os dados guardados neste computador (sem internet agora).',
      ),
    ).toBeDefined();
    expect(await within(drawer).findByText('Do cache.')).toBeDefined();
  });

  it('Remover a partir dos detalhes abre a mesma confirmação', async () => {
    await openDetails(SODIUM, {
      handlers: {
        item_details: () => makeDetails(SODIUM),
        items_remove_plan: () => ({ targets: [], dependents: [] }),
      },
    });
    await userEvent.setup().click(screen.getByRole('button', { name: 'Remover' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Remover Sodium?' });
    expect(
      await within(dialog).findByText('Nenhum outro item do pack depende disso.'),
    ).toBeDefined();
  });
});
