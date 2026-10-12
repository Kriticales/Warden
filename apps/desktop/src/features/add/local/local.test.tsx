/**
 * Link e arquivo do computador na página Adicionar (P1-11; SPEC T08), com o backend simulado:
 * arquivo achado pelo hash vira referência pelo diálogo de dependências (CA-T08-04, lado da
 * interface), jar de outro loader bloqueado com "Adicionar mesmo assim" (CA-T08-05), `http://`
 * recusado sem nenhum comando (CA-T08-07), links do Modrinth e direto, e arquivos arrastados
 * para a janela (na página e na seção Mods).
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type {
  AddResult,
  FileReport,
  HashMatch,
  LocalFileResult,
  ModrinthLinkTarget,
} from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError, type Handler } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { makeInventory, SODIUM as SODIUM_ITEM } from '../../pack-editor/editor.fixtures';
import { editorBackend } from '../../pack-editor/testing';
import { makeNode, makePage, makePlan, makePreview, makeVersions } from '../add.fixtures';

const FABRIC = { minecraft: '1.21.1', loader: 'fabric', loaderVersion: '0.16.5' };
const FORGE = { minecraft: '1.20.1', loader: 'forge', loaderVersion: '47.3.0' };

const SODIUM_MATCH: HashMatch = {
  source: 'modrinth',
  projectId: 'AANobbMI',
  versionId: 'SMxNOGZ6',
  title: 'Sodium',
  versionNumber: '0.6.0',
  kind: 'mod',
  compatible: true,
};

function report(overrides: Partial<FileReport> = {}): FileReport {
  return {
    fileName: 'sodium.jar',
    size: 1_258_291,
    sha256: 'a'.repeat(64),
    kind: 'mod',
    archive: true,
    modInfo: {
      id: 'sodium',
      name: 'Sodium',
      version: '0.6.0',
      loaders: ['fabric'],
      minecraft: '1.21.1',
    },
    incompatible: [],
    side: 'client',
    sideNote: null,
    matches: [],
    lookupFailed: [],
    inPack: null,
    ...overrides,
  };
}

function inspected(token: string, fileReport: FileReport): LocalFileResult[] {
  return [{ token, report: fileReport, error: null }];
}

const ADDED: AddResult = {
  added: [{ key: 'local:mods/sodium.jar', title: 'Sodium', path: 'mods/sodium.jar' }],
  skipped: [],
  removed: [],
};

function setup(pack = FABRIC, handlers: Record<string, Handler> = {}) {
  return editorBackend({
    pack,
    // Um item no pack, para a lista de Mods (com a busca) aparecer.
    inventory: makeInventory([SODIUM_ITEM]),
    handlers: {
      search_projects: () => makePage([]),
      project_details: () => makePreview({ title: 'Sodium', projectId: 'AANobbMI' }),
      project_versions: () => makeVersions(),
      add_local_discard: () => null,
      ...handlers,
    },
  });
}

async function openAdd(url: string) {
  renderApp(`${url}/adicionar`);
  return screen.findByRole('heading', { level: 1, name: 'Adicionar ao pack' }, { timeout: 10_000 });
}

async function chooseFile() {
  await userEvent.click(screen.getByRole('button', { name: 'Escolher arquivo do computador…' }));
  return screen.findByRole('list', { name: 'Arquivos para adicionar' }, { timeout: 10_000 });
}

describe('arquivo do computador', () => {
  it('achado pelo hash: oferece a referência ao Modrinth e abre o diálogo de dependências (CA-T08-04); axe', async () => {
    const { url, backend } = setup(FABRIC, {
      add_local_choose_files: () => [{ token: 'T1', fileName: 'sodium.jar' }],
      add_local_inspect: () => inspected('T1', report({ matches: [SODIUM_MATCH] })),
      add_plan: () => makePlan({ nodes: [makeNode({ projectId: 'AANobbMI', title: 'Sodium' })] }),
    });
    await openAdd(url);
    const list = await chooseFile();
    expect(within(list).getByText('Este arquivo é o Sodium 0.6.0 do Modrinth.')).toBeDefined();
    expect(
      within(list).getByText(/1,2 MB · Sodium 0\.6\.0 · Para Fabric · Minecraft 1\.21\.1/),
    ).toBeDefined();
    expect(backend.callsOf('add_local_inspect')[0]?.args).toMatchObject({ tokens: ['T1'] });
    expect(await axePage(document.body)).toHaveNoViolations();

    await userEvent.click(
      within(list).getByRole('button', {
        name: 'Adicionar como referência ao Modrinth (recomendado)',
      }),
    );
    expect(await screen.findByRole('dialog', { name: 'Adicionar Sodium' })).toBeDefined();
    expect(backend.callsOf('add_plan')[0]?.args.request).toEqual({
      items: [{ source: 'modrinth', projectId: 'AANobbMI', versionId: 'SMxNOGZ6' }],
    });
    // A referência não copia o jar: nenhum `add_local_apply`.
    expect(backend.callsOf('add_local_apply')).toHaveLength(0);
  });

  it('achado: também pode entrar como arquivo local', async () => {
    const { url, backend } = setup(FABRIC, {
      add_local_choose_files: () => [{ token: 'T1', fileName: 'sodium.jar' }],
      add_local_inspect: () => inspected('T1', report({ matches: [SODIUM_MATCH] })),
      add_local_apply: () => ADDED,
    });
    await openAdd(url);
    const list = await chooseFile();
    await userEvent.click(
      within(list).getByRole('button', { name: 'Adicionar como arquivo local' }),
    );
    expect(await within(list).findByText('Adicionado ao pack (mods/sodium.jar).')).toBeDefined();
    expect(backend.callsOf('add_local_apply')[0]?.args.items).toEqual([
      {
        token: 'T1',
        sha256: 'a'.repeat(64),
        kind: 'mod',
        side: 'client',
        acceptIncompatible: false,
      },
    ]);
    expect(screen.getByRole('link', { name: 'Voltar para Mods · 1 adicionado' })).toBeDefined();
  });

  it('jar Fabric num pack Forge: bloqueio com a explicação e "Adicionar mesmo assim" (CA-T08-05); axe', async () => {
    const { url, backend } = setup(FORGE, {
      add_local_choose_files: () => [{ token: 'T1', fileName: 'sodium.jar' }],
      add_local_inspect: () =>
        inspected(
          'T1',
          report({ incompatible: [{ kind: 'loader', found: ['fabric'], pack: 'forge' }] }),
        ),
      add_local_apply: () => ADDED,
    });
    await openAdd(url);
    const list = await chooseFile();
    const block = within(list).getByRole('alert');
    expect(within(block).getByText('Este arquivo não serve para este pack.')).toBeDefined();
    expect(
      within(block).getByText('Ele é um mod para Fabric, e este pack usa Forge.'),
    ).toBeDefined();
    expect(within(block).getByText(/o arquivo aparece como problema no diagnóstico/)).toBeDefined();
    expect(within(list).queryByRole('button', { name: /Adicionar sodium\.jar/ })).toBeNull();
    expect(await axePage(document.body)).toHaveNoViolations();

    await userEvent.click(within(block).getByRole('button', { name: 'Adicionar mesmo assim' }));
    expect(await within(list).findByText('Adicionado ao pack (mods/sodium.jar).')).toBeDefined();
    expect(backend.callsOf('add_local_apply')[0]?.args.items).toEqual([
      {
        token: 'T1',
        sha256: 'a'.repeat(64),
        kind: 'mod',
        side: 'client',
        acceptIncompatible: true,
      },
    ]);
  });

  it('versão do Minecraft incompatível e tipo que o usuário escolhe', async () => {
    const { url, backend } = setup(FABRIC, {
      add_local_choose_files: () => [{ token: 'T1', fileName: 'pacote.zip' }],
      add_local_inspect: () =>
        inspected(
          'T1',
          report({
            fileName: 'pacote.zip',
            kind: null,
            modInfo: null,
            incompatible: [],
            sideNote: 'unknown',
          }),
        ),
      add_local_apply: () => ADDED,
    });
    await openAdd(url);
    const list = await chooseFile();
    const select = within(list).getByRole('combobox', { name: 'Tipo' });
    await userEvent.selectOptions(select, 'Resource pack');
    expect(within(list).getByText(/copiado para a pasta resourcepacks do pack/)).toBeDefined();
    await userEvent.click(within(list).getByRole('button', { name: 'Adicionar pacote.zip' }));
    await waitFor(() => {
      expect(backend.callsOf('add_local_apply')[0]?.args.items).toMatchObject([
        { kind: 'resourcePack' },
      ]);
    });
  });

  it('o erro da gravação aparece no cartão, com a frase do motivo', async () => {
    const { url } = setup(FABRIC, {
      add_local_choose_files: () => [{ token: 'T1', fileName: 'sodium.jar' }],
      add_local_inspect: () => inspected('T1', report()),
      add_local_apply: () =>
        ipcError(
          makeAppError(
            { domain: 'project', code: 'INVALID_INPUT' },
            { params: { field: 'file', reason: 'changed', name: 'sodium.jar' } },
          ),
        ),
    });
    await openAdd(url);
    const list = await chooseFile();
    await userEvent.click(within(list).getByRole('button', { name: 'Adicionar sodium.jar' }));
    expect(
      await within(list).findByText(
        'O arquivo sodium.jar mudou depois de lido. Escolha o arquivo de novo.',
      ),
    ).toBeDefined();
  });

  it('já no pack, e o diálogo cancelado não abre nada', async () => {
    let picked = [{ token: 'T1', fileName: 'sodium.jar' }];
    const { url, backend } = setup(FABRIC, {
      add_local_choose_files: () => picked,
      add_local_inspect: () =>
        inspected(
          'T1',
          report({ inPack: { path: 'mods/sodium.jar', name: 'Sodium', sameFile: true } }),
        ),
    });
    await openAdd(url);
    const list = await chooseFile();
    expect(within(list).getByText('Este arquivo já está no pack (Sodium).')).toBeDefined();
    await userEvent.click(screen.getByRole('button', { name: 'Voltar para a busca' }));
    expect(screen.queryByRole('list', { name: 'Arquivos para adicionar' })).toBeNull();
    // Voltar esquece o arquivo no backend.
    await waitFor(() => {
      expect(backend.callsOf('add_local_discard').at(-1)?.args).toEqual({ tokens: ['T1'] });
    });
    picked = [];
    await userEvent.click(screen.getByRole('button', { name: 'Escolher arquivo do computador…' }));
    await waitFor(() => {
      expect(backend.callsOf('add_local_choose_files')).toHaveLength(2);
    });
    expect(screen.queryByRole('list', { name: 'Arquivos para adicionar' })).toBeNull();
  });

  it('arquivos soltos na página abrem a lista; os outros tipos ficam de fora com aviso', async () => {
    const { url, backend } = setup(FABRIC, {
      add_local_inspect: () => inspected('D1', report()),
    });
    await openAdd(url);
    await backend.emit('local-files-dropped', {
      files: [{ token: 'D1', fileName: 'sodium.jar' }],
      ignored: 2,
    });
    expect(
      await screen.findByRole('list', { name: 'Arquivos para adicionar' }, { timeout: 10_000 }),
    ).toBeDefined();
    expect(screen.getByText('2 itens não são .jar nem .zip e ficaram de fora.')).toBeDefined();
  });

  it('arquivos soltos na lista de Mods abrem a página Adicionar com eles', async () => {
    const { url, backend } = setup(FABRIC, {
      add_local_inspect: () => inspected('D1', report()),
    });
    renderApp(`${url}/mods`);
    await screen.findByRole('searchbox', { name: 'Buscar no pack' }, { timeout: 10_000 });
    await backend.emit('local-files-dropped', {
      files: [{ token: 'D1', fileName: 'sodium.jar' }],
      ignored: 0,
    });
    expect(
      await screen.findByRole(
        'heading',
        { level: 1, name: 'Adicionar ao pack' },
        { timeout: 10_000 },
      ),
    ).toBeDefined();
    expect(
      await screen.findByRole('list', { name: 'Arquivos para adicionar' }, { timeout: 10_000 }),
    ).toBeDefined();
    expect(backend.callsOf('add_local_inspect')[0]?.args).toMatchObject({ tokens: ['D1'] });
  });
});

describe('links colados no campo único', () => {
  async function paste(text: string) {
    const input = screen.getByRole('searchbox', { name: 'Buscar' });
    await userEvent.clear(input);
    await userEvent.click(input);
    await userEvent.paste(text);
  }

  it('http:// é recusado sem nenhum comando (CA-T08-07); axe', async () => {
    const { url, backend } = setup();
    await openAdd(url);
    await paste('http://example.com/mods/sodium.jar');
    expect(await screen.findByText('Links http:// não são aceitos.')).toBeDefined();
    expect(screen.getByText(/Nada foi baixado/)).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
    const linkCommands = backend.calls.filter((call) =>
      /^(add_link|curseforge_link|add_local)/.test(call.command),
    );
    expect(linkCommands).toEqual([]);
  });

  it('link desconhecido diz quais links funcionam', async () => {
    const { url } = setup();
    await openAdd(url);
    await paste('https://example.com/pagina.html');
    expect(await screen.findByText('O Warden não sabe adicionar este link.')).toBeDefined();
  });

  it('link de projeto do Modrinth abre a pré-visualização', async () => {
    const target: ModrinthLinkTarget = {
      projectId: 'AANobbMI',
      title: 'Sodium',
      kind: 'mod',
      versionId: null,
    };
    const { url, backend } = setup(FABRIC, { add_link_modrinth: () => target });
    await openAdd(url);
    await paste('https://modrinth.com/mod/sodium');
    await waitFor(
      () => {
        expect(backend.callsOf('project_details').at(-1)?.args).toMatchObject({
          source: 'modrinth',
          projectId: 'AANobbMI',
        });
      },
      { timeout: 10_000 },
    );
    expect(backend.callsOf('add_link_modrinth')[0]?.args).toMatchObject({
      url: 'https://modrinth.com/mod/sodium',
    });
    expect(
      backend
        .callsOf('search_projects')
        .every((call) => !JSON.stringify(call.args).includes('modrinth.com')),
    ).toBe(true);
  });

  it('arquivos soltos fecham a pré-visualização aberta', async () => {
    const target: ModrinthLinkTarget = {
      projectId: 'AANobbMI',
      title: 'Sodium',
      kind: 'mod',
      versionId: null,
    };
    const { url, backend } = setup(FABRIC, {
      add_link_modrinth: () => target,
      add_local_inspect: () => inspected('D1', report()),
    });
    await openAdd(url);
    await paste('https://modrinth.com/mod/sodium');
    await waitFor(
      () => {
        expect(backend.callsOf('project_details')).not.toHaveLength(0);
      },
      { timeout: 10_000 },
    );
    expect(screen.queryByText(/Escolha um resultado/)).toBeNull();
    await backend.emit('local-files-dropped', {
      files: [{ token: 'D1', fileName: 'sodium.jar' }],
      ignored: 0,
    });
    expect(
      await screen.findByRole('list', { name: 'Arquivos para adicionar' }, { timeout: 10_000 }),
    ).toBeDefined();
    expect(screen.getByText(/Escolha um resultado/)).toBeDefined();
  });

  it('link de versão do Modrinth abre o diálogo com aquela versão', async () => {
    const target: ModrinthLinkTarget = {
      projectId: 'AANobbMI',
      title: 'Sodium',
      kind: 'mod',
      versionId: 'SMxNOGZ6',
    };
    const { url, backend } = setup(FABRIC, {
      add_link_modrinth: () => target,
      add_plan: () => makePlan({ nodes: [makeNode({ projectId: 'AANobbMI', title: 'Sodium' })] }),
    });
    await openAdd(url);
    await paste('https://modrinth.com/mod/sodium/version/mc1.21.1-0.6.0-fabric');
    expect(
      await screen.findByRole('dialog', { name: 'Adicionar Sodium' }, { timeout: 10_000 }),
    ).toBeDefined();
    expect(backend.callsOf('add_plan')[0]?.args.request).toEqual({
      items: [{ source: 'modrinth', projectId: 'AANobbMI', versionId: 'SMxNOGZ6' }],
    });
  });

  it('link direto: baixa, lê e grava pelo link', async () => {
    const { url, backend } = setup(FORGE, {
      add_link_download: () => ({ token: 'L1', fileName: 'Meu Mod.jar' }),
      add_local_inspect: () =>
        inspected(
          'L1',
          report({
            fileName: 'Meu Mod.jar',
            modInfo: {
              id: 'meumod',
              name: 'Meu Mod',
              version: '1.0',
              loaders: ['forge'],
              minecraft: null,
            },
            side: 'both',
            sideNote: 'unknown',
          }),
        ),
      add_local_apply: () => ({
        added: [
          { key: 'url:mods/meu-mod.pw.toml', title: 'Meu Mod', path: 'mods/meu-mod.pw.toml' },
        ],
        skipped: [],
        removed: [],
      }),
    });
    await openAdd(url);
    await paste('https://files.example.com/mods/Meu%20Mod.jar');
    const list = await screen.findByRole(
      'list',
      { name: 'Arquivos para adicionar' },
      { timeout: 10_000 },
    );
    expect(within(list).getByText(/Baixado de files\.example\.com/)).toBeDefined();
    expect(within(list).getByText('Lado desconhecido — confira se é só de cliente.')).toBeDefined();
    expect(backend.callsOf('add_link_download')[0]?.args).toMatchObject({
      url: 'https://files.example.com/mods/Meu%20Mod.jar',
    });
    await userEvent.click(within(list).getByRole('button', { name: 'Adicionar pelo link' }));
    expect(
      await within(list).findByText('Adicionado ao pack (mods/meu-mod.pw.toml).'),
    ).toBeDefined();
  });

  it('link direto com erro mostra o motivo e baixa de novo', async () => {
    let attempts = 0;
    const { url, backend } = setup(FABRIC, {
      add_link_download: () => {
        attempts += 1;
        return ipcError(makeAppError({ domain: 'http', code: 'NOT_FOUND' }));
      },
    });
    await openAdd(url);
    await paste('https://files.example.com/sumiu.jar');
    expect(
      await screen.findByText('Não foi possível baixar este link.', {}, { timeout: 10_000 }),
    ).toBeDefined();
    await userEvent.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    await waitFor(() => {
      expect(attempts).toBe(2);
    });
    expect(backend.callsOf('add_local_inspect')).toHaveLength(0);
  });
});
