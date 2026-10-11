/**
 * CurseForge na página Adicionar (P1-10; SPEC T08): "Download manual necessário", conferência do
 * SHA-1 entre as fontes na pré-visualização, avisos de chave e o link da CurseForge colado no
 * campo único (CA-T08-03, lado da interface).
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { LinkTarget, ProjectVersions, SearchPage } from '../../../lib/ipc/bindings';
import { axeComponent, axePage } from '../../../test/axe';
import { ipcError, mockBackend, type Handler } from '../../../test/backend';
import { makeAppError, nextPackId } from '../../../test/factories';
import { renderApp, renderWithProviders } from '../../../test/render';
import { makeInventory, SODIUM as SODIUM_ITEM } from '../../pack-editor/editor.fixtures';
import { editorBackend } from '../../pack-editor/testing';
import {
  makeNode,
  makePage,
  makePlan,
  makePreview,
  makeResult,
  makeVersions,
  MODMENU,
  ref,
} from '../add.fixtures';
import { Preview } from '../common/Preview';

const SHA = '003c114c85ca88ef3362e018deb6aca0c682d6a1';
const TARGET = { minecraft: '1.21.1', loader: 'fabric' };
const BOTH = makeResult({
  title: 'Sodium',
  sources: [ref('AANobbMI', { slug: 'sodium' }), ref('394468', { source: 'curseforge' })],
});

function versionsWith(sha1: string | null, id = 'V1'): ProjectVersions {
  const base = makeVersions();
  return {
    defaultId: id,
    versions: [
      {
        id,
        number: '0.6.0',
        channel: 'release',
        published: '2026-09-20T00:00:00Z',
        fileName: 'sodium.jar',
        sha1,
      },
      ...base.versions.slice(0, 0),
    ],
  };
}

function previewWith(handlers: Record<string, Handler>, result = BOTH) {
  // Títulos da descrição são do autor (podem pular níveis): o corpo daqui é só texto.
  mockBackend({ project_details: () => makePreview({ body: 'Texto simples.' }), ...handlers });
  return renderWithProviders(
    <Preview
      packId={nextPackId()}
      result={result}
      inPack={false}
      kind="mod"
      target={TARGET}
      onAdd={() => undefined}
    />,
  );
}

describe('pré-visualização com a CurseForge', () => {
  it('o mesmo arquivo nas duas fontes: confere pelo SHA-1; axe', async () => {
    const { container } = previewWith({
      project_versions: (args) =>
        args.source === 'modrinth' ? versionsWith(SHA) : versionsWith(SHA.toUpperCase(), 'F1'),
    });
    expect(
      await screen.findByText(
        'O arquivo desta versão é o mesmo no Modrinth e na CurseForge (o SHA-1 confere).',
      ),
    ).toBeDefined();
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('arquivo diferente: avisa e diz a fonte sem o mesmo SHA-1', async () => {
    previewWith({
      project_versions: (args) =>
        args.source === 'modrinth'
          ? versionsWith(SHA)
          : versionsWith('ffffffffffffffffffffffffffffffffffffffff', 'F1'),
    });
    expect(
      await screen.findByText('O arquivo desta versão não é o mesmo nas duas fontes.'),
    ).toBeDefined();
    expect(screen.getByText(/Nenhuma versão no CurseForge tem o mesmo SHA-1\./)).toBeDefined();
  });

  it('trocar para a CurseForge confere a versão dela contra o Modrinth', async () => {
    previewWith({
      project_versions: (args) =>
        args.source === 'modrinth'
          ? versionsWith(SHA)
          : versionsWith('ffffffffffffffffffffffffffffffffffffffff', 'F1'),
    });
    await screen.findByText('O arquivo desta versão não é o mesmo nas duas fontes.');
    await userEvent.selectOptions(screen.getByRole('combobox', { name: 'Fonte' }), 'curseforge');
    expect(
      await screen.findByText('Nenhuma versão no Modrinth tem o mesmo SHA-1.', { exact: false }),
    ).toBeDefined();
  });

  it('sem SHA-1 na versão escolhida, nada a conferir; uma fonte só não confere', async () => {
    previewWith({ project_versions: () => versionsWith(null) });
    await screen.findByRole('combobox', { name: 'Versão' });
    expect(screen.queryByText(/SHA-1/)).toBeNull();
  });

  it('Download manual necessário: aviso só quando a fonte escolhida é a CurseForge', async () => {
    const blocked = makeResult({
      title: 'Mod Bloqueado',
      manualDownload: true,
      sources: [ref('900003', { source: 'curseforge' })],
    });
    previewWith({ project_versions: () => versionsWith(SHA) }, blocked);
    expect(await screen.findByText('Download manual necessário')).toBeDefined();
    expect(screen.getByText(/O autor de Mod Bloqueado não deixa outros apps/)).toBeDefined();
  });
});

const PACK = { minecraft: '1.21.1', loader: 'fabric', loaderVersion: '0.16.5' };

function setup(handlers: Record<string, Handler> = {}) {
  return editorBackend({
    pack: PACK,
    inventory: makeInventory([SODIUM_ITEM]),
    handlers: {
      search_projects: () => makePage([MODMENU]),
      project_details: () =>
        makePreview({ title: 'Just Enough Items (JEI)', body: 'Texto simples.' }),
      project_versions: () => makeVersions(),
      ...handlers,
    },
  });
}

async function openAdd(url: string) {
  renderApp(`${url}/adicionar`);
  await screen.findByRole('heading', { level: 1, name: 'Adicionar ao pack' }, { timeout: 10_000 });
  await screen.findByRole('list', { name: 'Resultados' });
}

const FILE_LINK = 'https://www.curseforge.com/minecraft/mc-mods/jei/files/4712345';
const PROJECT_LINK = 'https://www.curseforge.com/minecraft/mc-mods/jei';
const JEI: LinkTarget = {
  projectId: '238222',
  title: 'Just Enough Items (JEI)',
  kind: 'mod',
  fileId: null,
};

describe('link da CurseForge no campo único', () => {
  it('CA-T08-03: link de arquivo é lido pelo backend e abre o diálogo com o arquivo exato', async () => {
    const plan = makePlan({
      nodes: [
        makeNode({ projectId: '238222', title: 'Just Enough Items (JEI)', source: 'curseforge' }),
      ],
    });
    const { url, backend } = setup({
      curseforge_link_resolve: () => ({ ...JEI, fileId: '4712345' }),
      add_plan: () => plan,
    });
    await openAdd(url);
    const field = screen.getByRole('searchbox', { name: 'Buscar' });
    await userEvent.click(field);
    await userEvent.paste(FILE_LINK);
    const dialog = await screen.findByRole('dialog', { name: 'Adicionar Just Enough Items (JEI)' });
    expect(backend.callsOf('curseforge_link_resolve')).toHaveLength(1);
    expect(backend.callsOf('curseforge_link_resolve')[0]?.args.url).toBe(FILE_LINK);
    expect(backend.callsOf('add_plan')[0]?.args.request).toEqual({
      items: [{ source: 'curseforge', projectId: '238222', versionId: '4712345' }],
    });
    // O link não vira texto de busca.
    expect(
      backend
        .callsOf('search_projects')
        .some((call) => JSON.stringify(call.args).includes('curseforge.com')),
    ).toBe(false);
    expect(within(dialog).getByRole('button', { name: /Adicionar/ })).toBeDefined();
    expect(
      await screen.findByText('Link do arquivo de Just Enough Items (JEI) lido.'),
    ).toBeDefined();
  });

  it('link de projeto abre a pré-visualização com o seletor de versão', async () => {
    const { url, backend } = setup({ curseforge_link_resolve: () => JEI });
    await openAdd(url);
    await userEvent.click(screen.getByRole('searchbox', { name: 'Buscar' }));
    await userEvent.paste(PROJECT_LINK);
    const preview = await screen.findByRole('complementary', {
      name: 'Pré-visualização: Just Enough Items (JEI)',
    });
    expect(await within(preview).findByRole('combobox', { name: 'Versão' })).toBeDefined();
    await waitFor(() => {
      expect(backend.callsOf('project_details')[0]?.args).toMatchObject({
        source: 'curseforge',
        projectId: '238222',
      });
    });
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('sem chave: o erro diz o que fazer e oferece Abrir Configurações', async () => {
    const { url } = setup({
      curseforge_link_resolve: () =>
        ipcError(makeAppError({ domain: 'curseforge', code: 'CURSEFORGE_KEY_MISSING' })),
    });
    await openAdd(url);
    await userEvent.click(screen.getByRole('searchbox', { name: 'Buscar' }));
    await userEvent.paste(PROJECT_LINK);
    expect(
      await screen.findByText(
        'Para usar a CurseForge, informe a sua chave da CurseForge em Configurações.',
      ),
    ).toBeDefined();
    expect(screen.getByText('Não foi possível ler este link da CurseForge.')).toBeDefined();
    expect(screen.getByRole('link', { name: 'Abrir Configurações' })).toBeDefined();
  });

  it('o aviso de chave ausente ou recusada continua sem consultar a CurseForge', async () => {
    const page = (reason: 'keyMissing' | 'keyRejected'): SearchPage =>
      makePage([MODMENU], { warnings: [{ source: 'curseforge', reason, detail: null }] });
    const missing = setup({ search_projects: () => page('keyMissing') });
    await openAdd(missing.url);
    expect(screen.getByText('Mostrando só o Modrinth.')).toBeDefined();
    expect(
      screen.getByText('Para buscar também na CurseForge, informe sua chave em Configurações.'),
    ).toBeDefined();
    expect(screen.getByRole('link', { name: 'Abrir Configurações' })).toBeDefined();
  });
});
