/**
 * "O que mudou durante o teste" (SPEC T15; C-03) no app inteiro com o backend simulado: o bloco
 * no resultado do teste, a pré-seleção, trazer para o pack (só o escolhido, com os dados da tela
 * relidos), descartar, o conflito de três vias, os mods colocados à mão, o aviso do cabeçalho e a
 * pergunta antes de testar de novo.
 */
import { QueryClient } from '@tanstack/react-query';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
  CaptureApplyRequest,
  CaptureView,
  PackRow,
  TestRequest,
} from '../../lib/ipc/bindings';
import { axePage } from '../../test/axe';
import { ipcError, type Handler } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { editorBackend } from '../pack-editor/testing';
import { makeSession, makeSessionView } from '../test/test.fixtures';
import { makeSavePreview } from '../versioning/versioning.fixtures';
import { useTestStore } from '../test/store';
import { JEI_MATCH, SESSION, fileView, makeCapture } from './capture.fixtures';

afterEach(() => {
  useTestStore.setState({ game: null, runs: {}, packChanged: {} });
  vi.restoreAllMocks();
});

const session = makeSession({ id: SESSION });

/** O pack com a sessão gravada e a captura dada (que pode mudar durante o teste). */
function setup(capture: () => CaptureView | null, handlers: Record<string, Handler> = {}) {
  return editorBackend({
    handlers: {
      test_sessions_list: () => [session],
      test_session_get: () => makeSessionView(session, []),
      capture_get: () => capture(),
      capture_identify: () => ({
        items: [
          { path: 'mods/jei-1.20.1-forge-15.20.0.112.jar', found: JEI_MATCH },
          { path: 'mods/desconhecido.jar', found: null },
        ],
        failed: [],
      }),
      ...handlers,
    },
  });
}

async function openResult(url: string) {
  renderApp(`${url}/teste`);
  return screen.findByRole('region', { name: 'O que mudou durante o teste' }, { timeout: 10_000 });
}

function header() {
  return screen.getByRole('banner', { name: 'Pack aberto' });
}

describe('O que mudou durante o teste no resultado (T15)', () => {
  it('mostra os grupos com a pré-seleção da SPEC e o resumo em linguagem simples', async () => {
    const { url } = setup(() => makeCapture());
    const block = await openResult(url);
    expect(within(block).getByText(/^Durante o teste, 6 arquivos mudaram/)).toBeDefined();
    // Config do pack: marcada, com a diferença por valor.
    expect(
      within(block).getByRole('checkbox', {
        name: 'Trazer config/sodium-options.json para o pack',
      }),
    ).toHaveProperty('checked', true);
    expect(within(block).getByText('quality.weather_quality')).toBeDefined();
    // options.txt por chave: a que já existia marcada, a nova desmarcada; as voláteis escondidas.
    expect(
      within(block).getByRole('checkbox', { name: 'Trazer renderDistance para o pack' }),
    ).toHaveProperty('checked', true);
    expect(
      within(block).getByRole('checkbox', { name: 'Trazer key_key.zoom para o pack' }),
    ).toHaveProperty('checked', false);
    expect(
      within(block).getByText(
        'As 3 mudanças automáticas do jogo (versão, janela, último servidor) ficam escondidas.',
      ),
    ).toBeDefined();
    // Config de mundo: desmarcada, com o destino.
    expect(
      within(block).getByRole('checkbox', {
        name: 'Trazer saves/Mundo de teste/serverconfig/create-server.toml para o pack',
      }),
    ).toHaveProperty('checked', false);
    expect(
      within(block).getByText(
        'Se trouxer, vai para defaultconfigs/create-server.toml e vale para todo mundo novo dos jogadores.',
      ),
    ).toBeDefined();
    // Arquivos criados pelos mods: recolhidos e desmarcados.
    const generated = within(block).getByText(/^Arquivo criado pelos mods \(1\)/);
    expect(generated.closest('details')?.open).toBe(false);
    expect(
      within(block).getByText(
        '2 arquivos só foram reformatados pelo jogo (mesmos valores) e não aparecem.',
      ),
    ).toBeDefined();
    expect(
      within(block).getByRole('button', { name: 'Trazer 2 selecionados para o pack' }),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T15-01: trazer manda só as escolhas e relê tudo o que mostra o pack', async () => {
    let capture: CaptureView | null = makeCapture();
    let unsaved = 0;
    let row: PackRow | null = null;
    const result = setup(() => capture, {
      // A linha do pack (alterações não salvas do cabeçalho) vem do disco a cada leitura.
      pack_get: () => ({ ...row, unsavedFiles: unsaved }),
      packs_list: () => [{ ...row, unsavedFiles: unsaved }],
      capture_apply: ({ request }) => {
        capture = { ...makeCapture(), status: { kind: 'applied', count: 1 }, needsReview: false };
        unsaved = 1;
        return {
          count: (request as CaptureApplyRequest).optionKeys.length,
          files: ['options.txt'],
        };
      },
    });
    row = result.row;
    const { backend, url } = result;
    const spy = vi.spyOn(QueryClient.prototype, 'invalidateQueries');
    const keysOf = () => spy.mock.calls.map(([filters]) => filters?.queryKey);
    const user = userEvent.setup();
    const block = await openResult(url);
    expect(within(header()).getByRole('button', { name: 'Salvar versão' })).toBeDefined();
    // Só a distância de renderização.
    await user.click(
      within(block).getByRole('checkbox', {
        name: 'Trazer config/sodium-options.json para o pack',
      }),
    );
    await user.click(
      within(block).getByRole('button', { name: 'Trazer 1 selecionado para o pack' }),
    );

    await waitFor(() => {
      expect(backend.callsOf('capture_apply')).toHaveLength(1);
    });
    expect(backend.callsOf('capture_apply')[0]?.args.request).toEqual({
      sessionId: SESSION,
      files: [],
      optionKeys: ['renderDistance'],
      expectedOptionsSha256: 'pack-options.txt',
    });
    const dialog = await screen.findByRole('dialog', { name: '1 mudança trazida para o pack' });
    expect(within(dialog).getByText(/aparecem em Alterações não salvas/)).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Continuar editando' }));
    // Dados atuais: o cabeçalho (linha do pack), a lista de Meus packs e a captura relidos.
    expect(
      await within(header()).findByRole('button', {
        name: 'Salvar versão, 1 alteração não salva',
      }),
    ).toBeDefined();
    // A lista de Meus packs (fora da tela agora) fica marcada para reler.
    expect(keysOf()).toContainEqual(['packs', 'list']);
    expect(
      await screen.findByText('Mudanças deste teste revisadas: 1 mudança foi trazida para o pack.'),
    ).toBeDefined();
    expect(
      within(header()).queryByRole('button', { name: 'Mudanças do teste para revisar' }),
    ).toBeNull();
  });

  it('"Salvar versão" depois de trazer abre o diálogo Salvar versão', async () => {
    let capture: CaptureView | null = makeCapture({ files: [fileView()] });
    const { url } = setup(() => capture, {
      capture_apply: () => {
        capture = {
          ...makeCapture({ files: [fileView()] }),
          status: { kind: 'applied', count: 1 },
        };
        return { count: 1, files: ['config/sodium-options.json'] };
      },
      version_save_preview: () => makeSavePreview(),
    });
    const user = userEvent.setup();
    const block = await openResult(url);
    await user.click(
      within(block).getByRole('button', { name: 'Trazer 1 selecionado para o pack' }),
    );
    const dialog = await screen.findByRole('dialog', { name: '1 mudança trazida para o pack' });
    await user.click(within(dialog).getByRole('button', { name: 'Salvar versão' }));
    expect(await screen.findByRole('dialog', { name: 'Salvar versão' })).toBeDefined();
  });

  it('erro ao trazer (o pack mudou depois da lista) aparece e nada some da tela', async () => {
    const { url } = setup(() => makeCapture(), {
      capture_apply: () =>
        ipcError(
          makeAppError(
            { domain: 'app', code: 'CAPTURE_STALE' },
            { params: { path: 'config/sodium-options.json' } },
          ),
        ),
    });
    const user = userEvent.setup();
    const block = await openResult(url);
    await user.click(
      within(block).getByRole('button', { name: 'Trazer 2 selecionados para o pack' }),
    );
    expect(
      await screen.findByText(
        'O arquivo config/sodium-options.json mudou depois que a lista apareceu. Nada foi gravado: confira a lista atualizada e tente de novo.',
      ),
    ).toBeDefined();
    expect(screen.getByRole('region', { name: 'O que mudou durante o teste' })).toBeDefined();
  });

  it('Descartar tudo pede confirmação, descarta e mostra o resumo', async () => {
    let capture: CaptureView | null = makeCapture();
    const { backend, url } = setup(() => capture, {
      capture_discard: () => {
        capture = { ...makeCapture(), status: { kind: 'discarded' }, needsReview: false };
        return 1;
      },
    });
    const user = userEvent.setup();
    const block = await openResult(url);
    await user.click(within(block).getByRole('button', { name: 'Descartar tudo' }));
    const dialog = await screen.findByRole('alertdialog', {
      name: 'Descartar as mudanças deste teste?',
    });
    await user.click(within(dialog).getByRole('button', { name: 'Descartar mudanças' }));
    expect(
      await screen.findByText(
        'Mudanças deste teste descartadas: no próximo teste, a instância de teste volta a ter as configs do pack.',
      ),
    ).toBeDefined();
    expect(backend.callsOf('capture_discard')[0]?.args).toMatchObject({ sessionId: SESSION });
  });

  it('nada mudou: só uma linha; sessão de outro teste: nada', async () => {
    const empty = setup(() => makeCapture({ files: [], needsReview: false, reformatted: 1 }));
    renderApp(`${empty.url}/teste`);
    expect(
      await screen.findByText(
        /^Durante o teste, nada mudou nas configs nem nas opções do jogo\. 1 arquivo só foi reformatado/,
      ),
    ).toBeDefined();
  });

  it('a captura de outro teste não aparece no resultado desta sessão', async () => {
    const { backend, url } = setup(() => makeCapture({ sessionId: 'outro' }));
    renderApp(`${url}/teste`);
    await screen.findByRole('heading', { level: 1, name: 'O jogo fechou normalmente' });
    await waitFor(() => {
      expect(backend.callsOf('capture_get').length).toBeGreaterThanOrEqual(1);
    });
    expect(screen.queryByRole('region', { name: 'O que mudou durante o teste' })).toBeNull();
  });

  it('Ver diferença mostra o texto antes e depois', async () => {
    const { url } = setup(() => makeCapture(), {
      capture_texts: () => ({
        base: 'a = 1\n',
        instance: 'a = 2\n',
        pack: 'a = 1\n',
        packSha256: 'x',
        lineEnding: '\n',
      }),
    });
    const user = userEvent.setup();
    const block = await openResult(url);
    await user.click(
      within(block).getByRole('button', { name: 'Ver diferença de config/sodium-options.json' }),
    );
    const dialog = await screen.findByRole('dialog', {
      name: 'Diferença de texto: config/sodium-options.json',
    });
    expect(await within(dialog).findByText('a = 2')).toBeDefined();
    const close = within(dialog)
      .getAllByRole('button', { name: 'Fechar' })
      .find((button) => button.textContent === 'Fechar');
    if (!close) throw new Error('botão Fechar do rodapé ausente');
    await user.click(close);
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    // O foco volta para quem abriu.
    expect(document.activeElement?.textContent).toBe('Ver diferença');
  });
});

describe('conflito de três vias (CA-T15-05)', () => {
  const conflict = () =>
    makeCapture({
      files: [
        fileView(
          { path: 'config/create-common.toml' },
          {
            packSha256: 'pack-agora',
            conflict: {
              packValues: [{ key: 'maxRotationSpeed', before: '128', after: '256' }],
              mergeable: true,
            },
          },
        ),
      ],
    });
  const texts = () => ({
    base: 'maxRotationSpeed = 128\nstressMultiplier = 1.0\n',
    pack: 'maxRotationSpeed = 256\nstressMultiplier = 1.0\n',
    instance: 'maxRotationSpeed = 128\nstressMultiplier = 2.0\n',
    packSha256: 'pack-agora',
    lineEnding: '\n',
  });

  it('mostra as duas mudanças e as escolhas; "Juntar as duas" vem marcada e vai no pedido', async () => {
    const { backend, url } = setup(conflict, {
      capture_texts: texts,
      capture_apply: () => ({ count: 1, files: ['config/create-common.toml'] }),
    });
    const user = userEvent.setup();
    const block = await openResult(url);
    expect(within(block).getByText('O mesmo arquivo mudou no pack e no jogo')).toBeDefined();
    expect(
      await within(block).findByText('No pack (você editou depois que o teste começou)'),
    ).toBeDefined();
    expect(within(block).getByText('No jogo (mudou durante o teste)')).toBeDefined();
    const choices = within(block).getByRole('group', {
      name: 'O que fazer com config/create-common.toml',
    });
    expect(within(choices).getByRole('radio', { name: /Juntar as duas/ })).toHaveProperty(
      'checked',
      true,
    );
    await user.click(within(choices).getByRole('radio', { name: /Usar a do jogo/ }));
    await user.click(
      within(block).getByRole('button', { name: 'Trazer 1 selecionado para o pack' }),
    );
    await waitFor(() => {
      expect(backend.callsOf('capture_apply')).toHaveLength(1);
    });
    expect(
      (backend.callsOf('capture_apply')[0]?.args.request as CaptureApplyRequest).files,
    ).toEqual([
      {
        path: 'config/create-common.toml',
        resolution: { kind: 'take' },
        expectedPackSha256: 'pack-agora',
      },
    ]);
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('"Manter a do pack" não manda nada; "Editar e combinar" manda o texto do editor', async () => {
    const { backend, url } = setup(conflict, {
      capture_texts: texts,
      capture_apply: () => ({ count: 1, files: ['config/create-common.toml'] }),
    });
    const user = userEvent.setup();
    const block = await openResult(url);
    const choices = await within(block).findByRole('group', {
      name: 'O que fazer com config/create-common.toml',
    });
    await user.click(within(choices).getByRole('radio', { name: /Manter a do pack/ }));
    expect(
      within(block).getByRole('button', { name: 'Trazer selecionados para o pack' }),
    ).toHaveProperty('disabled', true);

    await waitFor(() => {
      expect(within(choices).getByRole('radio', { name: /Editar e combinar/ })).toHaveProperty(
        'disabled',
        false,
      );
    });
    await user.click(within(choices).getByRole('radio', { name: /Editar e combinar/ }));
    const editor = await screen.findByRole('dialog', {
      name: 'Editar e combinar: config/create-common.toml',
    });
    await user.click(within(editor).getByRole('button', { name: 'Usar este texto' }));
    expect(
      await within(block).findByText('Combinado à mão: o texto do editor vai para o pack.'),
    ).toBeDefined();
    await user.click(
      within(block).getByRole('button', { name: 'Trazer 1 selecionado para o pack' }),
    );
    await waitFor(() => {
      expect(backend.callsOf('capture_apply')).toHaveLength(1);
    });
    expect(
      (backend.callsOf('capture_apply')[0]?.args.request as CaptureApplyRequest).files[0]
        ?.resolution,
    ).toEqual({ kind: 'edited', text: texts().pack });
  });
});

describe('mods colocados à mão (CA-T15-04)', () => {
  it('nunca têm caixa de marcar: o reconhecido oferece adicionar pelo caminho normal', async () => {
    const shader = fileView({
      path: 'shaderpacks/BSL.zip',
      group: 'resource',
      change: 'added',
      selected: false,
      packPath: null,
      values: [],
      text: false,
      managed: false,
    });
    const capture = makeCapture();
    const { backend, url } = setup(() => ({ ...capture, files: [...capture.files, shader] }), {
      add_plan: () => ipcError(makeAppError({ domain: 'app', code: 'INTERNAL' })),
    });
    const user = userEvent.setup();
    const block = await openResult(url);
    const group = within(block)
      .getByText('Mods colocados ou tirados à mão na pasta da instância')
      .closest('section');
    if (!group) throw new Error('grupo dos mods ausente');
    expect(within(group).queryByRole('checkbox')).toBeNull();
    expect(within(group).queryByText(/BSL\.zip/)).toBeNull();
    // Resource packs e shaders no grupo deles, com o título deles.
    const resources = within(block).getByText('Resource packs e shaders novos').closest('section');
    if (!resources) throw new Error('grupo dos resource packs ausente');
    expect(within(resources).getAllByText(/BSL\.zip/).length).toBeGreaterThan(0);
    expect(within(resources).queryByRole('checkbox')).toBeNull();
    expect(
      await within(block).findByText(
        'jei-1.20.1-forge-15.20.0.112.jar é Just Enough Items 15.20.0.112, do Modrinth. O arquivo não é copiado.',
      ),
    ).toBeDefined();
    expect(
      within(block).getByText(
        'desconhecido.jar não foi reconhecido no Modrinth nem na CurseForge. O arquivo não é copiado; para levar ao pack, procure o mod em Adicionar.',
      ),
    ).toBeDefined();
    await user.click(
      within(block).getByRole('button', { name: 'Adicionar ao pack pelo Modrinth' }),
    );
    await waitFor(() => {
      expect(backend.callsOf('add_plan')).toHaveLength(1);
    });
    expect(backend.callsOf('add_plan')[0]?.args.request).toEqual({
      items: [{ source: 'modrinth', projectId: 'u6dRKJwZ', versionId: 'abc123' }],
    });
    // Trazer nunca leva o jar.
    const request = backend.callsOf('capture_apply');
    expect(request).toHaveLength(0);
  });
});

describe('aviso do cabeçalho e testar de novo (T15 "Ações")', () => {
  it('"Mudanças do teste para revisar" no cabeçalho leva ao resultado do teste', async () => {
    const { url } = setup(() => makeCapture());
    renderApp(url);
    const alert = await within(
      await screen.findByRole('banner', { name: 'Pack aberto' }),
    ).findByRole('button', { name: 'Mudanças do teste para revisar' });
    await userEvent.setup().click(alert);
    expect(
      await screen.findByRole('region', { name: 'O que mudou durante o teste' }),
    ).toBeDefined();
    // Na tela do teste o aviso some (a revisão está ali).
    expect(
      within(header()).queryByRole('button', { name: 'Mudanças do teste para revisar' }),
    ).toBeNull();
  });

  it('"Decidir depois" vai para Mods e o cabeçalho passa a avisar', async () => {
    const { url } = setup(() => makeCapture());
    const user = userEvent.setup();
    const block = await openResult(url);
    await user.click(within(block).getByRole('button', { name: 'Decidir depois' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Mods' })).toBeDefined();
    expect(
      await within(header()).findByRole('button', { name: 'Mudanças do teste para revisar' }),
    ).toBeDefined();
  });

  it('sem mudanças para revisar, o cabeçalho não avisa', async () => {
    const { backend, url } = setup(() => makeCapture({ needsReview: false }));
    renderApp(url);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    await waitFor(() => {
      expect(backend.callsOf('capture_get').length).toBeGreaterThanOrEqual(1);
    });
    expect(
      within(header()).queryByRole('button', { name: 'Mudanças do teste para revisar' }),
    ).toBeNull();
  });

  it('Testar com mudanças não revisadas pergunta; "Descartar e testar" descarta e testa', async () => {
    let pending = true;
    const { backend, url } = setup(() => makeCapture(), {
      test_start: () =>
        pending
          ? ipcError(
              makeAppError(
                { domain: 'app', code: 'TEST_CAPTURE_PENDING' },
                { params: { sessionId: SESSION } },
              ),
            )
          : makeSession({ id: 'novo' }),
      capture_discard: () => {
        pending = false;
        return 1;
      },
    });
    renderApp(url);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Testar' }));
    expect(
      await screen.findByRole('heading', {
        level: 1,
        name: 'Mudanças do último teste para revisar',
      }),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Revisar agora' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Cancelar' })).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(screen.getByRole('button', { name: 'Descartar e testar' }));
    await waitFor(() => {
      expect(backend.callsOf('test_start')).toHaveLength(2);
    });
    expect(backend.callsOf('capture_discard')[0]?.args).toMatchObject({ sessionId: SESSION });
    expect((backend.callsOf('test_start')[1]?.args.request as TestRequest).mode).toBe('normal');
  });

  it('"Revisar agora" abre o resultado do teste com as mudanças', async () => {
    const { url } = setup(() => makeCapture(), {
      test_start: () =>
        ipcError(
          makeAppError(
            { domain: 'app', code: 'TEST_CAPTURE_PENDING' },
            { params: { sessionId: SESSION } },
          ),
        ),
    });
    renderApp(url);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Testar' }));
    await user.click(await screen.findByRole('button', { name: 'Revisar agora' }));
    expect(
      await screen.findByRole('region', { name: 'O que mudou durante o teste' }),
    ).toBeDefined();
    expect(
      screen.queryByRole('heading', { level: 1, name: 'Mudanças do último teste para revisar' }),
    ).toBeNull();
  });
});
