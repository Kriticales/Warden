import { createMemoryHistory } from '@tanstack/react-router';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { App, createQueryClient } from '../../../app/App';
import type { ImportAnalysis, ImportSummary, PackId } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { axeComponent, axePage } from '../../../test/axe';
import { deferred, ipcError, mockBackend, type Handler } from '../../../test/backend';
import { makeAppError, makePackRow } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { packsKeys } from '../api';

const ORIGIN = 'C:/Users/ana/Downloads/Create+ 6.0.0.mrpack';
const PACK_ID = '0b0f5e8a-6d7c-4c7e-9a1e-0a2b3c4d5e6f' as PackId;

function summary(overrides: Partial<ImportSummary> = {}): ImportSummary {
  return {
    kind: 'mrpack',
    fileName: 'Create+ 6.0.0.mrpack',
    name: 'Create+',
    version: '6.0.0',
    minecraft: '1.21.1',
    loader: 'neoforge',
    loaderVersion: '21.1.228',
    license: { id: 'LicenseRef-All-Rights-Reserved', name: 'All Rights Reserved', free: false },
    references: [
      { path: 'mods/create.pw.toml', name: 'Create', source: 'modrinth', byHash: false },
      { path: 'mods/jei.pw.toml', name: 'JEI', source: 'curseforge', byHash: true },
      {
        path: 'mods/status.pw.toml',
        name: 'Status Effect Bars',
        source: 'modrinth',
        byHash: false,
      },
    ],
    localFiles: [{ path: 'mods/caseiro.jar', name: 'caseiro.jar', reason: 'noCurseforgeKey' }],
    review: [{ path: 'mods/status.pw.toml', name: 'Status Effect Bars', reason: 'unknownSide' }],
    excluded: [
      { path: '.mixin.out', isDir: true, files: 812, reason: 'runtime', canInclude: false },
      { path: 'xmcl.json', isDir: false, files: 1, reason: 'otherLauncher', canInclude: false },
      {
        path: 'configureddefaults',
        isDir: true,
        files: 2,
        reason: 'unknownRoot',
        canInclude: true,
      },
    ],
    files: 40,
    serverFiles: 0,
    preferences: [],
    profile: null,
    unavailableSources: [],
    ...overrides,
  };
}

function analysis(overrides: Partial<ImportSummary> = {}): ImportAnalysis {
  return { token: 'plano-1', summary: summary(overrides) };
}

function importBackend(extra: Record<string, Handler> = {}) {
  return mockBackend({
    import_analyze: () => analysis(),
    pack_create_check: ({ name }) => `D:/Warden/packs/${String(name).toLowerCase()}`,
    import_discard: () => null,
    ...extra,
  });
}

function route(origin = ORIGIN) {
  return `/packs/importar?origem=${encodeURIComponent(origin)}`;
}

describe('Importar modpack (T24)', () => {
  it('lê o modpack com progresso e mostra a revisão sem gravar nada; passa no axe', async () => {
    const reading = deferred<ImportAnalysis>();
    const backend = importBackend({ import_analyze: () => reading.promise });
    const { container } = renderApp(route());
    expect(await screen.findByText('Lendo Create+ 6.0.0.mrpack')).toBeDefined();
    // A pasta do usuário não aparece na tela.
    expect(container.textContent).not.toContain('Users/ana');
    reading.resolve(analysis());

    expect(
      await screen.findByText('Create+ 6.0.0.mrpack · Minecraft 1.21.1 · NeoForge 21.1.228'),
    ).toBeDefined();
    expect(screen.getByText('Licença do modpack: All Rights Reserved.')).toBeDefined();
    expect(
      screen.getByText(/Para uso pessoal, tudo bem; para redistribuir, peça permissão/),
    ).toBeDefined();
    const resumo = screen.getByRole('region', { name: 'O que acontece com cada arquivo' });
    expect(within(resumo).getByText('2 viram referência ao Modrinth')).toBeDefined();
    expect(within(resumo).getByText('1 vira referência à CurseForge')).toBeDefined();
    expect(within(resumo).getByText('1 jar solto identificado pelo hash')).toBeDefined();
    expect(within(resumo).getByText('1 jar fica como arquivo local')).toBeDefined();
    expect(within(resumo).getByText('1 item para revisar')).toBeDefined();
    expect(
      within(resumo).getByText(
        'caseiro.jar · não está no Modrinth; a CurseForge não foi consultada (sem chave)',
      ),
    ).toBeDefined();
    expect(
      within(resumo).getByText(
        'Status Effect Bars · lado desconhecido no modpack: entra como Cliente e servidor',
      ),
    ).toBeDefined();
    const lixo = screen.getByRole('region', { name: 'O que não entra no pack' });
    expect(within(lixo).getByText('.mixin.out/')).toBeDefined();
    expect(within(lixo).getByText('xmcl.json')).toBeDefined();
    expect(
      within(lixo).getByRole('checkbox', { name: 'Levar configureddefaults/ para o pack' }),
    ).toBeDefined();
    expect(within(lixo).queryByRole('checkbox', { name: /xmcl/ })).toBeNull();
    expect(backend.callsOf('import_analyze')[0]?.args).toEqual({ path: ORIGIN });
    expect(backend.callsOf('import_apply')).toHaveLength(0);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Importar como pack novo' })).toHaveProperty(
        'disabled',
        false,
      );
    });
    expect(await axePage(container)).toHaveNoViolations();
  });

  it('importa com nome, itens marcados e abre o pack em Mods; invalida a lista e o pack', async () => {
    const user = userEvent.setup();
    const backend = importBackend({
      import_apply: () => ({ packId: PACK_ID, review: 2, profileCreated: false }),
      pack_get: () => makePackRow({ id: PACK_ID, name: 'Meu Create' }),
    });
    const queryClient = createQueryClient();
    // Dados que mostram o pack em outras telas, já em cache.
    queryClient.setQueryData(packsKeys.list, []);
    queryClient.setQueryData([...queryKeys.pack(PACK_ID), 'inventory'], { items: [] });
    render(
      <App
        history={createMemoryHistory({ initialEntries: [route()] })}
        queryClient={queryClient}
      />,
    );
    const name = await screen.findByRole('textbox', { name: 'Nome do pack' });
    await user.clear(name);
    await user.type(name, 'Meu Create');
    await user.click(
      screen.getByRole('checkbox', { name: 'Levar configureddefaults/ para o pack' }),
    );
    const importar = screen.getByRole('button', { name: 'Importar como pack novo' });
    await waitFor(() => {
      expect(importar).toHaveProperty('disabled', false);
    });
    await user.click(importar);

    await waitFor(() => {
      expect(backend.callsOf('import_apply')).toHaveLength(1);
    });
    expect(backend.callsOf('import_apply')[0]?.args).toEqual({
      token: 'plano-1',
      request: {
        name: 'Meu Create',
        destination: null,
        include: ['configureddefaults'],
        takePreferences: false,
      },
    });
    expect(await screen.findByText('“Meu Create” importado como pack novo.')).toBeDefined();
    expect(screen.getByText('2 itens marcados para revisar aparecem em Problemas.')).toBeDefined();
    // Abre o pack novo.
    await waitFor(() => {
      expect(backend.callsOf('pack_get').some((call) => call.args.packId === PACK_ID)).toBe(true);
    });
    // Tudo o que mostra packs fica velho: Meus packs e os dados do pack.
    expect(queryClient.getQueryState(packsKeys.list)?.isInvalidated).toBe(true);
    expect(
      queryClient.getQueryState([...queryKeys.pack(PACK_ID), 'inventory'])?.isInvalidated,
    ).toBe(true);
    // O plano usado não é descartado de novo à toa: só ao sair, uma vez.
    expect(backend.callsOf('import_discard').length).toBeLessThanOrEqual(1);
  });

  it('instância do Prism: perfil do teste e a pergunta das preferências', async () => {
    const user = userEvent.setup();
    const backend = importBackend({
      import_analyze: () =>
        analysis({
          kind: 'prism',
          license: null,
          preferences: ['options.txt', 'servers.dat'],
          profile: { memoryMb: 6144, jvmArgs: '-XX:+UseG1GC' },
        }),
      import_apply: () => ({ packId: PACK_ID, review: 0, profileCreated: true }),
      pack_get: () => makePackRow({ id: PACK_ID }),
    });
    renderApp(route('D:/Prism/instances/Vale Sereno'));
    const perfil = await screen.findByRole('region', { name: 'Perfil do teste' });
    expect(within(perfil).getByText('Memória: 6144 MB')).toBeDefined();
    expect(within(perfil).getByText('Argumentos: -XX:+UseG1GC')).toBeDefined();
    expect(screen.getByText('instância do Prism ou do MultiMC')).toBeDefined();
    const levar = screen.getByRole('checkbox', {
      name: 'Levar options.txt, servers.dat para o pack',
    });
    expect(levar).toHaveProperty('checked', false);
    await user.click(levar);
    const importar = screen.getByRole('button', { name: 'Importar como pack novo' });
    await waitFor(() => {
      expect(importar).toHaveProperty('disabled', false);
    });
    await user.click(importar);
    await waitFor(() => {
      expect(backend.callsOf('import_apply')).toHaveLength(1);
    });
    const request = backend.callsOf('import_apply')[0]?.args.request as {
      takePreferences: boolean;
    };
    expect(request.takePreferences).toBe(true);
  });

  it('zip da CurseForge sem chave: explica, leva para Configurações e não importa', async () => {
    const user = userEvent.setup();
    const backend = importBackend({
      import_analyze: () =>
        ipcError(makeAppError({ domain: 'import', code: 'CURSEFORGE_KEY_MISSING' })),
    });
    renderApp(route('C:/Users/ana/Downloads/Cozy Create 2.3.0.zip'));
    expect(
      await screen.findByText(
        'Para importar um modpack da CurseForge, o Warden precisa da sua chave da CurseForge.',
      ),
    ).toBeDefined();
    expect(screen.getByText(/Nada foi gravado/)).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Importar como pack novo' })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Abrir Configurações' }));
    expect(await screen.findByRole('heading', { name: 'Configurações', level: 1 })).toBeDefined();
    expect(backend.callsOf('import_apply')).toHaveLength(0);
  });

  it('loader não suportado e caminho perigoso: recusa com o motivo', async () => {
    importBackend({
      import_analyze: () =>
        ipcError(
          makeAppError(
            { domain: 'import', code: 'UNSUPPORTED_LOADER' },
            { params: { loader: 'quilt' } },
          ),
        ),
    });
    const first = renderApp(route());
    expect(
      await screen.findByText('Este modpack usa Quilt, que o Warden ainda não suporta.'),
    ).toBeDefined();
    first.unmount();

    importBackend({
      import_analyze: () =>
        ipcError(
          makeAppError(
            { domain: 'import', code: 'UNSAFE_PATH' },
            { params: { path: '../../fora.jar', reason: 'parent' } },
          ),
        ),
    });
    renderApp(route());
    expect(await screen.findByText(/caminho perigoso \(\.\.\/\.\.\/fora\.jar\)/)).toBeDefined();
    expect(screen.getByRole('button', { name: 'Escolher outro arquivo' })).toBeDefined();
  });

  it('pasta com arquivos: o erro aparece no campo e o botão fica desligado', async () => {
    importBackend({
      pack_create_check: () =>
        ipcError(
          makeAppError(
            { domain: 'project', code: 'DESTINATION_NOT_EMPTY' },
            { params: { path: 'D:/Warden/packs/create' } },
          ),
        ),
    });
    renderApp(route());
    expect(
      await screen.findByText(
        'Esta pasta já tem arquivos. Escolha uma pasta vazia ou mude o nome do pack.',
      ),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Importar como pack novo' })).toHaveProperty(
      'disabled',
      true,
    );
  });

  it('Cancelar volta para Meus packs e esquece o plano', async () => {
    const user = userEvent.setup();
    const backend = importBackend();
    renderApp(route());
    await screen.findByRole('region', { name: 'O que acontece com cada arquivo' });
    await user.click(screen.getByRole('button', { name: 'Cancelar' }));
    expect(await screen.findByRole('heading', { name: 'Você ainda não tem packs' })).toBeDefined();
    await waitFor(() => {
      expect(backend.callsOf('import_discard')[0]?.args).toEqual({ token: 'plano-1' });
    });
  });

  it('Cancelar durante a leitura cancela a operação e volta para Meus packs', async () => {
    const user = userEvent.setup();
    const reading = deferred<ImportAnalysis>();
    const backend = importBackend({
      import_analyze: () => reading.promise,
      operations_list: () => [
        {
          id: 'op-1',
          kind: 'import.analyze',
          packId: null,
          state: 'running',
          stage: null,
          progress: { current: 3, total: 10, unit: 'items' },
          cancellable: true,
          startedAtMs: 1,
          finishedAtMs: null,
          error: null,
        },
      ],
      operation_cancel: () => null,
    });
    renderApp(route());
    await screen.findByText('Lendo Create+ 6.0.0.mrpack');
    await waitFor(() => {
      expect(backend.callsOf('operations_list').length).toBeGreaterThan(0);
    });
    await user.click(screen.getByRole('button', { name: 'Cancelar' }));
    expect(await screen.findByRole('heading', { name: 'Você ainda não tem packs' })).toBeDefined();
    expect(backend.callsOf('operation_cancel')[0]?.args).toEqual({ operationId: 'op-1' });
  });

  it('lojas fora do ar: avisa e oferece ler de novo', async () => {
    const user = userEvent.setup();
    let reads = 0;
    const backend = importBackend({
      import_analyze: () => {
        reads += 1;
        return {
          ...analysis({ unavailableSources: ['Modrinth'] }),
          token: `plano-${String(reads)}`,
        };
      },
    });
    renderApp(route());
    expect(await screen.findByText('Não foi possível consultar Modrinth.')).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Ler de novo' }));
    await waitFor(() => {
      expect(backend.callsOf('import_analyze')).toHaveLength(2);
    });
    await waitFor(() => {
      expect(backend.callsOf('import_discard')[0]?.args).toEqual({ token: 'plano-1' });
    });
  });
});

describe('Entrada da importação em Meus packs: um botão só (protótipo `packs`)', () => {
  it('não há botão separado de importar; o menu tem pasta e arquivo; passa no axe', async () => {
    const user = userEvent.setup();
    importBackend();
    renderApp('/packs');
    await screen.findByRole('heading', { name: 'Você ainda não tem packs' });
    expect(screen.queryByRole('button', { name: /Importar arquivo/ })).toBeNull();
    expect(screen.getAllByRole('button', { name: /Abrir ou importar/ })).toHaveLength(1);
    await user.click(screen.getByRole('button', { name: 'Abrir ou importar…' }));
    const items = await screen.findAllByRole('menuitem');
    expect(items.map((item) => item.textContent)).toEqual([
      'Escolher pasta…Pack packwiz ou instância do Prism, do MultiMC ou do app da CurseForge',
      'Escolher arquivo….mrpack, zip da CurseForge ou zip de instância do Prism',
    ]);
    expect(await axeComponent(screen.getByRole('menu'))).toHaveNoViolations();
  });

  it('"Escolher arquivo…" abre o diálogo de arquivo e leva a Importar modpack', async () => {
    const user = userEvent.setup();
    const backend = importBackend({ import_choose_file: () => ORIGIN });
    renderApp('/packs');
    await screen.findByRole('heading', { name: 'Você ainda não tem packs' });
    await user.click(screen.getByRole('button', { name: 'Abrir ou importar…' }));
    await user.click(await screen.findByRole('menuitem', { name: /Escolher arquivo…/ }));
    expect(
      await screen.findByRole('heading', { name: 'Importar modpack', level: 1 }),
    ).toBeDefined();
    await waitFor(() => {
      expect(backend.callsOf('import_analyze')[0]?.args).toEqual({ path: ORIGIN });
    });
    expect(backend.callsOf('pack_choose_folder')).toHaveLength(0);
  });

  it('desistir do diálogo de arquivo não faz nada', async () => {
    const user = userEvent.setup();
    const backend = importBackend({ import_choose_file: () => null });
    renderApp('/packs');
    await screen.findByRole('heading', { name: 'Você ainda não tem packs' });
    await user.click(screen.getByRole('button', { name: 'Abrir ou importar…' }));
    await user.click(await screen.findByRole('menuitem', { name: /Escolher arquivo…/ }));
    await waitFor(() => {
      expect(backend.callsOf('import_choose_file')).toHaveLength(1);
    });
    expect(screen.getByRole('heading', { name: 'Meus packs', level: 1 })).toBeDefined();
    expect(backend.callsOf('import_analyze')).toHaveLength(0);
  });

  it('"Escolher pasta…" com pasta de instância vai para Importar modpack', async () => {
    const user = userEvent.setup();
    const backend = importBackend({
      pack_choose_folder: () => 'D:/Prism/instances/Vale Sereno',
      import_detect: () => 'prism',
    });
    renderApp('/packs');
    await screen.findByRole('heading', { name: 'Você ainda não tem packs' });
    await user.click(screen.getByRole('button', { name: 'Abrir ou importar…' }));
    await user.click(await screen.findByRole('menuitem', { name: /Escolher pasta…/ }));
    expect(
      await screen.findByRole('heading', { name: 'Importar modpack', level: 1 }),
    ).toBeDefined();
    expect(backend.callsOf('import_detect')[0]?.args).toEqual({
      path: 'D:/Prism/instances/Vale Sereno',
    });
    expect(backend.callsOf('pack_import_preview')).toHaveLength(0);
  });
});
