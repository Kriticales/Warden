/**
 * Pacote para servidor em Exportar (SPEC T19; CA-T19-06 a CA-T19-08, a parte da interface):
 * a escolha da variante (pelo link só com o pack publicado), o que vai e o que fica de fora, a
 * confirmação de licença com mods dentro, a validação (que pede a EULA antes), a lista do
 * download manual (T20) durante a geração, o resultado e a releitura no fim. O conteúdo do zip
 * e a validação de verdade são provados no Rust (`warden-export`, `tests/server_pack`, e
 * `warden-app`, `server_pack_tests`).
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type {
  ExportPreview,
  ServerPackAnalysis,
  ServerPackInfo,
  ServerPackRun,
  Settings,
} from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { deferred, ipcError, type Handler } from '../../../test/backend';
import { makeAppError, makeOperation } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { makeSettings } from '../../settings/testing';

const PREVIEW: ExportPreview = {
  files: [],
  folders: [],
  bytes: 0,
  references: 0,
  preflight: { hygiene: [], unsavedChanges: false, diagnosticErrors: null },
};

function analysis(overrides: Partial<ServerPackAnalysis> = {}): ServerPackAnalysis {
  return {
    version: '1.4.2',
    minecraft: '1.20.1',
    loader: 'Forge 47.3.0',
    loaderSupported: true,
    clientOnly: ['Embeddium', "Xaero's Minimap", 'Jade', 'Oculus'],
    clientOnlyCount: 4,
    clientFiles: 3,
    kubejsClientScripts: true,
    serverOverrides: 2,
    link: {
      folders: [
        { path: 'config', files: 86 },
        { path: 'kubejs', files: 5 },
      ],
      serverItems: 120,
      thirdParty: 0,
    },
    embedded: {
      folders: [
        { path: 'config', files: 86 },
        { path: 'kubejs', files: 5 },
      ],
      serverItems: 120,
      thirdParty: 121,
    },
    manualDownloads: [],
    curseforgeKeyMissing: false,
    ...overrides,
  };
}

function info(overrides: Partial<ServerPackInfo> = {}): ServerPackInfo {
  return {
    current: analysis(),
    published: {
      version: '1.4.0',
      link: 'https://raw.githubusercontent.com/dono/vale-sereno/main/pack.toml',
      analysis: analysis({ version: '1.4.0' }),
    },
    linkUnavailable: null,
    recommendedMemoryMb: 4096,
    eulaAcceptedAt: '2026-10-13T12:00:00Z',
    ...overrides,
  };
}

const UNPUBLISHED: Partial<ServerPackInfo> = { published: null, linkUnavailable: 'notPublished' };

function generated(overrides: Partial<ServerPackRun> = {}): ServerPackRun {
  return {
    result: {
      path: 'D:\\Exportados\\vale-sereno-1.4.0-servidor.zip',
      variant: 'link',
      bytes: 2048,
      sha256: 'cd'.repeat(32),
      files: ['start.bat', 'start.sh', 'user_jvm_args.txt'],
      mods: 0,
      memoryMb: 4096,
    },
    check: { kind: 'tested', readyMs: 34_000 },
    ...overrides,
  };
}

async function openServerPack(
  handlers: Record<string, Handler> = {},
  settings: Partial<Settings> = { eulaAcceptedAt: '2026-10-13T12:00:00Z' },
  ready = true,
) {
  const state = { settings: makeSettings(settings) };
  const { backend, url, row } = editorBackend({
    handlers: {
      export_preview: () => PREVIEW,
      server_pack_analyze: () => info(),
      settings_get: () => state.settings,
      server_eula_accept: () => {
        state.settings = { ...state.settings, eulaAcceptedAt: '2026-10-18T09:00:00Z' };
        return state.settings;
      },
      ...handlers,
    },
  });
  renderApp(`${url}/exportar`);
  await screen.findByRole('heading', { level: 1, name: 'Exportar' }, { timeout: 10_000 });
  const user = userEvent.setup();
  await user.click(await screen.findByRole('radio', { name: /^Pacote para servidor/ }));
  if (ready) await screen.findByRole('region', { name: 'Como os mods chegam ao servidor' });
  return { backend, user, state, packId: row.id };
}

function generateButton() {
  return screen.getByRole('button', { name: 'Gerar pacote para servidor…' });
}

describe('Pacote para servidor (T19)', () => {
  it('pelo link por padrão: o que vai, o que fica de fora e o aviso de não testado; axe', async () => {
    await openServerPack();
    const link = screen.getByRole('radio', { name: /^Baixar os mods pelo link do pack/ });
    expect(link).toHaveProperty('checked', true);
    expect(link.closest('label')?.textContent).toContain('(está: versão 1.4.0)');
    const contents = screen.getByRole('region', { name: 'O que vai no pacote' });
    expect(contents.textContent).toContain('start.bat e start.sh');
    expect(contents.textContent).toContain('instalam o Forge 47.3.0 na primeira vez');
    expect(contents.textContent).toContain('user_jvm_args.txt: -Xms4G -Xmx4G');
    expect(contents.textContent).toContain('packwiz-installer-bootstrap.jar');
    expect(contents.textContent).toContain(
      '120 mods e datapacks do lado servidor, baixados pelo link.',
    );
    expect(contents.textContent).toContain('config/: 86 arquivos');
    expect(contents.textContent).toContain('kubejs/: 5 arquivos (sem client_scripts/)');
    expect(contents.textContent).toContain(
      "4 mods só de cliente ficam de fora: Embeddium, Xaero's Minimap, Jade e mais 1.",
    );
    expect(contents.textContent).toContain('options.txt, resource packs e shaders');
    expect(contents.textContent).toContain('o script pergunta a quem roda o servidor');
    expect(screen.getByText('Este pacote ainda não foi testado num servidor.')).toBeDefined();
    expect(
      screen.getByRole('checkbox', { name: /Testar no servidor deste computador/ }),
    ).toHaveProperty('checked', true);
    expect(generateButton()).toHaveProperty('disabled', false);
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('sem publicação, a variante pelo link fica desabilitada com o motivo e Publicar versão', async () => {
    const { backend, user } = await openServerPack({
      server_pack_analyze: () => info(UNPUBLISHED),
      server_pack_run: () => null,
    });
    const link = screen.getByRole('radio', { name: /^Baixar os mods pelo link do pack/ });
    expect(link).toHaveProperty('disabled', true);
    expect(link.closest('label')?.textContent).toContain('ele ainda não foi publicado');
    expect(screen.getByRole('button', { name: 'Publicar versão' })).toBeDefined();
    // Cai em "Mods dentro do zip", que pede a confirmação de licença.
    expect(screen.getByRole('radio', { name: /^Mods dentro do zip/ })).toHaveProperty(
      'checked',
      true,
    );
    expect(screen.getByText('121 arquivos de terceiros vão dentro do zip')).toBeDefined();
    expect(generateButton()).toHaveProperty('disabled', true);
    await user.click(screen.getByRole('checkbox', { name: /Tenho o direito de distribuir/ }));
    expect(generateButton()).toHaveProperty('disabled', false);
    await user.click(generateButton());
    await waitFor(() => {
      expect(backend.callsOf('server_pack_run')[0]?.args).toEqual({
        packId: expect.any(String) as string,
        request: {
          options: { variant: 'embedded', memoryMb: 4096, confirmThirdParty: true },
          validate: true,
        },
      });
    });
  });

  it('repositório privado explica por que o link não serve', async () => {
    await openServerPack({
      server_pack_analyze: () => info({ published: null, linkUnavailable: 'privateRepo' }),
    });
    const link = screen.getByRole('radio', { name: /^Baixar os mods pelo link do pack/ });
    expect(link.closest('label')?.textContent).toContain('o repositório do pack é privado');
  });

  it('memória escolhida vai para o user_jvm_args.txt e para o pedido', async () => {
    const { backend, user } = await openServerPack({ server_pack_run: () => null });
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Memória do servidor (user_jvm_args.txt)' }),
      '6144',
    );
    expect(screen.getByRole('region', { name: 'O que vai no pacote' }).textContent).toContain(
      '-Xms6G -Xmx6G',
    );
    await user.click(screen.getByRole('checkbox', { name: /Testar no servidor deste computador/ }));
    await user.click(generateButton());
    await waitFor(() => {
      expect(backend.callsOf('server_pack_run')[0]?.args.request).toEqual({
        options: { variant: 'link', memoryMb: 6144, confirmThirdParty: false },
        validate: false,
      });
    });
  });

  it('sem a EULA aceita, a validação vem desmarcada; marcar pergunta a EULA e o aceite marca', async () => {
    const { backend, user } = await openServerPack(
      { server_pack_analyze: () => info({ eulaAcceptedAt: null }) },
      {},
    );
    const box = screen.getByRole('checkbox', { name: /Testar no servidor deste computador/ });
    expect(box).toHaveProperty('checked', false);
    await user.click(box);
    const eula = await screen.findByRole('dialog', { name: 'Aceitar a EULA do Minecraft?' });
    // "Agora não" fecha sem marcar e devolve o foco à caixa.
    await user.click(within(eula).getByRole('button', { name: 'Agora não' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(box).toHaveProperty('checked', false);
    await waitFor(() => {
      expect(document.activeElement).toBe(box);
    });
    await user.click(box);
    const again = await screen.findByRole('dialog', { name: 'Aceitar a EULA do Minecraft?' });
    await user.click(within(again).getByRole('button', { name: 'Aceito a EULA' }));
    await waitFor(() => {
      expect(
        screen.getByRole('checkbox', { name: /Testar no servidor deste computador/ }),
      ).toHaveProperty('checked', true);
    });
    expect(backend.callsOf('server_eula_accept')).toHaveLength(1);
  });

  it('mods da CurseForge para baixar à mão e falta da chave aparecem antes de gerar', async () => {
    await openServerPack({
      server_pack_analyze: () =>
        info({
          ...UNPUBLISHED,
          current: analysis({ manualDownloads: ['Entity Culling', 'Jade'] }),
        }),
    });
    expect(screen.getByText('2 mods da CurseForge vão precisar ser baixados à mão')).toBeDefined();
    expect(
      screen.getByText(/O autor não deixa outros apps baixarem: Entity Culling, Jade\./),
    ).toBeDefined();
    expect(
      screen.getByRole('radio', { name: /^Mods dentro do zip/ }).closest('label')?.textContent,
    ).toContain('2 mods da CurseForge precisam ser baixados à mão');
  });

  it('sem a chave da CurseForge, com mods dentro, não deixa gerar', async () => {
    await openServerPack({
      server_pack_analyze: () =>
        info({ ...UNPUBLISHED, current: analysis({ curseforgeKeyMissing: true }) }),
    });
    expect(screen.getByText('Falta a chave da CurseForge')).toBeDefined();
    expect(generateButton()).toHaveProperty('disabled', true);
  });

  it('loader sem scripts de servidor avisa e não deixa gerar', async () => {
    await openServerPack({
      server_pack_analyze: () =>
        info({
          ...UNPUBLISHED,
          current: analysis({ loader: 'Quilt 0.24.0', loaderSupported: false }),
        }),
    });
    expect(
      screen.getAllByText('O pacote para servidor ainda não sai para packs com Quilt 0.24.0.')
        .length,
    ).toBeGreaterThan(0);
    expect(generateButton()).toHaveProperty('disabled', true);
  });

  it('CA-T19-08: resultado testado com o tempo até "Done", o passo a passo e Mostrar arquivo', async () => {
    const { backend, user } = await openServerPack({
      server_pack_run: () => generated(),
      server_pack_reveal: () => null,
    });
    await user.click(generateButton());
    const dialog = await screen.findByRole('dialog', { name: 'Pacote para servidor gerado' });
    expect(dialog.textContent).toContain('Pacote para servidor testado: o servidor abriu em 34 s.');
    expect(dialog.textContent).toContain('D:\\Exportados\\vale-sereno-1.4.0-servidor.zip');
    expect(dialog.textContent).toContain('cd'.repeat(32));
    expect(dialog.textContent).toContain('Rode start.bat (Windows) ou start.sh (Linux e macOS).');
    expect(dialog.textContent).toContain('o script instala o Forge 47.3.0');
    expect(dialog.textContent).toContain('baixa pelo link os mods da última versão publicada');
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Mostrar arquivo' }));
    await waitFor(() => {
      expect(backend.callsOf('server_pack_reveal')[0]?.args).toEqual({
        path: 'D:\\Exportados\\vale-sereno-1.4.0-servidor.zip',
      });
    });
  });

  it('CA-T19-08: validação reprovada mostra o mod apontado e o trecho do log do servidor', async () => {
    const { user } = await openServerPack({
      server_pack_run: () =>
        generated({
          check: {
            kind: 'crashed',
            crash: {
              foundCause: true,
              mods: [{ name: 'Zoom de Cliente', path: 'mods/zoom.pw.toml', side: 'both' }],
              clientClass: 'net.minecraft.client.gui.screens.Screen',
              excerpt: ['[Server thread/ERROR]: Attempted to load class net/minecraft/client'],
              titleKey: null,
            },
          },
        }),
    });
    await user.click(generateButton());
    const dialog = await screen.findByRole('dialog', { name: 'Pacote para servidor gerado' });
    expect(dialog.textContent).toContain('O servidor travou com este pacote.');
    expect(dialog.textContent).toContain('Zoom de Cliente');
    expect(dialog.textContent).toContain('net.minecraft.client.gui.screens.Screen');
    expect(dialog.textContent).toContain('Attempted to load class net/minecraft/client');
    expect(dialog.textContent).not.toContain('Pacote para servidor testado');
  });

  it('validação que não abriu o servidor mostra o erro; sem validação, diz que não foi testado', async () => {
    const { user } = await openServerPack({
      server_pack_run: () =>
        generated({
          check: {
            kind: 'failed',
            error: makeAppError(
              { domain: 'server', code: 'READY_TIMEOUT' },
              { params: { seconds: '900' } },
            ),
          },
        }),
    });
    await user.click(generateButton());
    const dialog = await screen.findByRole('dialog', { name: 'Pacote para servidor gerado' });
    expect(dialog.textContent).toContain('Não foi possível testar o pacote neste computador.');
    const [close] = within(dialog).getAllByRole('button', { name: 'Fechar' });
    if (!close) throw new Error('sem o botão Fechar');
    await user.click(close);
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
  });

  it('durante a geração, a lista do download manual aparece; no fim a leitura é refeita', async () => {
    const pending = deferred<ServerPackRun | null>();
    const manual = [
      {
        metafile: 'mods/entityculling.pw.toml',
        name: 'Entity Culling',
        fileName: 'entityculling-forge-1.7.0.jar',
        hasPage: true,
        status: { state: 'waiting' as const },
      },
    ];
    const state = { running: false };
    let running = makeOperation({ kind: 'export.serverPack', finishedAtMs: null });
    const { backend, user, packId } = await openServerPack({
      server_pack_run: () => {
        state.running = true;
        return pending.promise;
      },
      operations_list: () => (state.running ? [running] : []),
      cf_blocked_list: () => (state.running ? manual : []),
    });
    running = { ...running, packId, cancellable: true };
    const before = backend.callsOf('server_pack_analyze').length;
    await user.click(generateButton());
    await act(async () => {
      await backend.emit('operation-updated', running);
      await backend.emit('manual-downloads-changed', { packId });
    });
    expect(await screen.findByText('Gerando o pacote para servidor')).toBeDefined();
    expect(await screen.findByText('entityculling-forge-1.7.0.jar')).toBeDefined();
    state.running = false;
    await act(async () => {
      pending.resolve(null);
      await backend.emit('operation-updated', { ...running, finishedAtMs: 1 });
    });
    await waitFor(() => {
      expect(backend.callsOf('server_pack_analyze').length).toBeGreaterThan(before);
    });
  });

  it('erro da geração aparece com a frase do domínio export', async () => {
    const { user } = await openServerPack({
      server_pack_run: () =>
        ipcError(makeAppError({ domain: 'export', code: 'SERVER_PACK_NOT_PUBLISHED' })),
    });
    await user.click(generateButton());
    expect(await screen.findByText('Não foi possível gerar o pacote para servidor.')).toBeDefined();
    expect(screen.getByText(/publique uma versão antes/)).toBeDefined();
  });

  it('falha ao ler mostra o erro com Tentar de novo', async () => {
    await openServerPack(
      {
        server_pack_analyze: () =>
          ipcError(makeAppError({ domain: 'export', code: 'SERVER_PACK_LOOKUP_FAILED' })),
      },
      undefined,
      false,
    );
    expect(
      await screen.findByText('Não foi possível ler o que vai para o servidor.'),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: /Tentar de novo/ })).toBeDefined();
  });

  it('Testar como servidor abre o diálogo do servidor local e devolve o foco ao botão', async () => {
    const { user } = await openServerPack({
      server_plan: () => ({
        memory: { serverMb: 4096, gameMb: 6144, totalMb: 10_240, freeMb: 16_000, fits: true },
        clientOnly: [],
        clientOnlyCount: 0,
        eulaAcceptedAt: '2026-10-13T12:00:00Z',
        loaderSupported: true,
      }),
    });
    const button = screen.getByRole('button', { name: 'Testar como servidor' });
    await user.click(button);
    const dialog = await screen.findByRole('dialog', { name: 'Testar como servidor' });
    await user.click(within(dialog).getByRole('button', { name: 'Cancelar' }));
    await waitFor(() => {
      expect(document.activeElement).toBe(button);
    });
  });
});
