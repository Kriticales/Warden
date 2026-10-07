/**
 * Seção Exportar (SPEC T19) no app inteiro, com o backend simulado. A conformidade da saída
 * (CA-T19-01 e CA-T19-02) é provada no Rust (`warden-export`) e no E2E; aqui ficam a tela, os
 * alertas, "Excluir do pack", a limpeza da higiene (CA-T19-03) e o fluxo de Exportar.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { ExportPreview, PreviewFile } from '../../lib/ipc/bindings';
import { axePage } from '../../test/axe';
import { ipcError } from '../../test/backend';
import { makeAppError, makeHygieneFinding, makeOperation, nextPackId } from '../../test/factories';
import { renderApp } from '../../test/render';
import { editorBackend } from '../pack-editor/testing';

function file(path: string, overrides: Partial<PreviewFile> = {}): PreviewFile {
  const reference = path.endsWith('.pw.toml');
  return {
    path,
    bytes: reference ? 400 : 2048,
    reference,
    local: !reference && path !== 'pack.toml' && path !== 'index.toml',
    config: path.startsWith('config/'),
    alerts: [],
    ...overrides,
  };
}

function makePreview(overrides: Partial<ExportPreview> = {}): ExportPreview {
  const files = [
    file('config/create.toml'),
    file('config/jei/jei-client.ini'),
    file('index.toml'),
    file('mods/jei.pw.toml'),
    file('mods/sodium.pw.toml'),
    file('pack.toml'),
  ];
  return {
    files,
    folders: [
      { path: 'config', files: 2, bytes: 4096 },
      { path: 'mods', files: 2, bytes: 800 },
    ],
    bytes: files.reduce((sum, f) => sum + f.bytes, 0),
    references: 2,
    preflight: { hygiene: [], unsavedChanges: false, diagnosticErrors: null },
    ...overrides,
  };
}

type Options = NonNullable<Parameters<typeof editorBackend>[0]>;

async function openExport(handlers: Options['handlers'] = {}, pack: Options['pack'] = {}) {
  const { backend, row, url } = editorBackend({
    pack,
    handlers: { export_preview: () => makePreview(), ...handlers },
  });
  renderApp(`${url}/exportar`);
  await screen.findByRole('heading', { level: 1, name: 'Exportar' }, { timeout: 10_000 });
  await screen.findByRole('heading', { name: '3. O que vai no pack' });
  return { backend, row };
}

describe('Exportar (T19)', () => {
  it('mostra antes de exportar, formato e a árvore com contagem e tamanho por pasta; axe', async () => {
    const { backend } = await openExport();

    expect(backend.callsOf('export_preview')[0]?.args.source).toEqual({ kind: 'current' });
    expect(
      screen.getByText(
        'Nenhum arquivo que não deveria ir para quem joga (registros, cópias de segurança, caches).',
      ),
    ).toBeDefined();
    expect(
      screen.getByText('Sem alterações não salvas: a exportação é igual à última versão salva.'),
    ).toBeDefined();
    expect(screen.getByRole('radio', { name: /^Pasta packwiz/ })).toHaveProperty('checked', true);
    expect(screen.getByRole('radio', { name: /^Arquivo .zip do pack packwiz/ })).toBeDefined();

    const tree = screen.getByRole('region', { name: '3. O que vai no pack' });
    expect(tree.textContent).toContain('pack.toml · index.toml');
    expect(tree.textContent).toContain('config/2 arquivos, 4,0 KB');
    expect(tree.textContent).toContain('mods/2 referências (os .jar não vão: quem joga baixa)');
    expect(tree.textContent).toContain('6 arquivos');
    // Sem alertas, o painel "pedem atenção" não aparece.
    expect(screen.queryByRole('region', { name: /pedem? atenção/ })).toBeNull();

    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('a seção Exportar fica habilitada no menu do pack e marcada como atual', async () => {
    await openExport();
    const menu = screen.getByRole('navigation', { name: 'Seções do pack' });
    const link = within(menu).getByRole('link', { name: /Exportar/ });
    expect(link.getAttribute('aria-current')).toBe('page');
  });

  it('cada alerta da prévia tem a sua frase, e a higiene usa o motivo de Abrir pack', async () => {
    const big = file('mods/modelos.zip', {
      bytes: 30 * 1024 * 1024,
      alerts: [{ kind: 'largeFile' }],
    });
    const options = file('options.txt', {
      alerts: [{ kind: 'optionsOverridesPreferences' }, { kind: 'looseRootFile' }],
    });
    const backup = file('config/create.toml.bak', {
      alerts: [
        { kind: 'hygiene', cause: { kind: 'pattern', pattern: '*.bak', group: 'anyDepth' } },
      ],
    });
    await openExport({
      export_preview: () =>
        makePreview({ files: [file('index.toml'), file('pack.toml'), big, options, backup] }),
    });

    const panel = screen.getByRole('region', { name: '3 arquivos pedem atenção' });
    const text = panel.textContent;
    expect(text).toContain('Arquivo grande (mais de 20 MB): quem joga vai baixar tudo isso.');
    expect(text).toContain(
      'Substitui as preferências de quem já joga (teclas, gráficos, som) a cada atualização.',
    );
    expect(text).toContain(
      'Arquivo solto na raiz do pack: confira se ele deveria mesmo ir para quem joga.',
    );
    expect(text).toContain('Parece não ser para quem joga: ');
    // Os soltos na raiz aparecem na árvore, abertos.
    const tree = screen.getByRole('region', { name: '3. O que vai no pack' });
    expect(tree.textContent).toContain('Na raiz do pack');
  });

  it('Excluir do pack mostra a linha do .packwizignore e chama export_exclude', async () => {
    const user = userEvent.setup();
    const options = file('options.txt', { alerts: [{ kind: 'optionsOverridesPreferences' }] });
    let excluded = false;
    const { backend } = await openExport({
      export_preview: () =>
        makePreview({
          files: excluded
            ? [file('index.toml'), file('pack.toml')]
            : [file('index.toml'), file('pack.toml'), options],
        }),
      export_exclude: () => {
        excluded = true;
        return null;
      },
    });

    const panel = screen.getByRole('region', { name: '1 arquivo pede atenção' });
    await user.click(within(panel).getByRole('button', { name: 'Excluir do pack: options.txt' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Excluir do pack' });
    expect(within(dialog).getByText('/options.txt')).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Excluir do pack' }));

    await waitFor(() => {
      expect(backend.callsOf('export_exclude')[0]?.args).toMatchObject({ path: 'options.txt' });
    });
    await waitFor(() => {
      expect(screen.queryByRole('region', { name: '1 arquivo pede atenção' })).toBeNull();
    });
    expect(await screen.findByText('Excluído do pack: options.txt')).toBeDefined();
  });

  it('excluir uma pasta inteira grava a regra com barra no fim', async () => {
    const user = userEvent.setup();
    const { backend } = await openExport({ export_exclude: () => null });
    await user.click(screen.getByText('config/'));
    await user.click(
      await screen.findByRole('button', { name: 'Excluir a pasta config/ do pack' }),
    );
    const dialog = await screen.findByRole('alertdialog', { name: 'Excluir do pack' });
    expect(within(dialog).getByText('/config/')).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Excluir do pack' }));
    await waitFor(() => {
      expect(backend.callsOf('export_exclude')[0]?.args).toMatchObject({ path: 'config' });
    });
  });

  it('CA-T19-03: crash-reports/x.txt aparece na higiene e some depois de Limpar', async () => {
    const user = userEvent.setup();
    let cleaned = false;
    const crash = makeHygieneFinding({
      path: 'crash-reports/x.txt',
      inIndex: false,
      bytes: 120,
      reasons: [{ kind: 'pattern', pattern: 'crash-reports/', group: 'runtime' }],
    });
    const { backend } = await openExport({
      export_preview: () =>
        makePreview({
          preflight: {
            hygiene: cleaned ? [] : [crash],
            unsavedChanges: true,
            diagnosticErrors: null,
          },
        }),
      pack_hygiene_fix: () => {
        cleaned = true;
        return ['crash-reports/x.txt'];
      },
    });

    expect(
      screen.getByText('1 arquivo na pasta do pack não deveria ir para quem joga.', {
        exact: false,
      }),
    ).toBeDefined();
    expect(screen.getByText(/Há alterações não salvas/)).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Revisar e limpar' }));
    const dialog = await screen.findByRole('dialog', { name: 'Limpar a pasta do pack' });
    expect(within(dialog).getByText('crash-reports/x.txt')).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Limpar 1 arquivo' }));

    await waitFor(() => {
      expect(backend.callsOf('pack_hygiene_fix')[0]?.args).toMatchObject({
        paths: ['crash-reports/x.txt'],
      });
    });
    expect(
      await screen.findByText(
        'Nenhum arquivo que não deveria ir para quem joga (registros, cópias de segurança, caches).',
      ),
    ).toBeDefined();
  });

  it('Exportar em .zip: chama export_run com o formato e mostra o resultado com Mostrar o arquivo', async () => {
    const user = userEvent.setup();
    const { backend } = await openExport({
      export_run: () => ({
        path: 'D:\\Exportados\\vale-sereno.zip',
        files: ['config/create.toml', 'index.toml', 'pack.toml'],
        bytes: 1300,
      }),
      export_reveal: () => null,
    });

    await user.click(screen.getByRole('radio', { name: /^Arquivo .zip do pack packwiz/ }));
    expect(screen.getByText('A seguir, você escolhe onde salvar o arquivo .zip.')).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Exportar…' }));

    const dialog = await screen.findByRole('dialog', { name: 'Pack exportado' });
    expect(backend.callsOf('export_run')[0]?.args).toMatchObject({
      source: { kind: 'current' },
      format: 'zip',
    });
    expect(dialog.textContent).toContain('3 arquivos, 1,3 KB, em D:\\Exportados\\vale-sereno.zip');
    expect(dialog.textContent).toContain('Conferido com o packwiz.');
    await user.click(within(dialog).getByRole('button', { name: 'Mostrar o arquivo' }));
    await waitFor(() => {
      expect(backend.callsOf('export_reveal')[0]?.args).toEqual({
        path: 'D:\\Exportados\\vale-sereno.zip',
      });
    });
  });

  it('fechar o diálogo de destino sem escolher não mostra nada', async () => {
    const user = userEvent.setup();
    const { backend } = await openExport({ export_run: () => null });
    await user.click(screen.getByRole('button', { name: 'Exportar…' }));
    await waitFor(() => {
      expect(backend.callsOf('export_run')).toHaveLength(1);
    });
    expect(screen.queryByRole('dialog', { name: 'Pack exportado' })).toBeNull();
    expect(screen.queryByText('Não foi possível exportar.')).toBeNull();
  });

  it('destino ocupado: a frase do erro explica o que fazer', async () => {
    const user = userEvent.setup();
    await openExport({
      export_run: () => ipcError(makeAppError({ domain: 'export', code: 'DESTINATION_NOT_EMPTY' })),
    });
    await user.click(screen.getByRole('button', { name: 'Exportar…' }));
    expect(await screen.findByText('Não foi possível exportar.')).toBeDefined();
    expect(
      screen.getByText(
        'O destino escolhido já existe. Escolha uma pasta vazia ou um nome de arquivo novo. Nada foi gravado.',
      ),
    ).toBeDefined();
  });

  it('cancelada pela pessoa: sem painel de erro', async () => {
    const user = userEvent.setup();
    await openExport({
      export_run: () => ipcError(makeAppError({ domain: 'core', code: 'CANCELLED' })),
    });
    await user.click(screen.getByRole('button', { name: 'Exportar…' }));
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Exportar…' })).toHaveProperty('disabled', false);
    });
    expect(screen.queryByText('Não foi possível exportar.')).toBeNull();
  });

  it('durante a exportação: progresso e Cancelar pela operação export.run', async () => {
    const user = userEvent.setup();
    const packId = nextPackId();
    const operation = makeOperation({ kind: 'export.run', packId, cancellable: true });
    const { backend } = await openExport(
      { operations_list: () => [operation], operation_cancel: () => null },
      { id: packId },
    );
    expect(screen.getByRole('progressbar', { name: 'Exportando' })).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Exportar…' })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Cancelar' }));
    await waitFor(() => {
      expect(backend.callsOf('operation_cancel')[0]?.args).toEqual({ operationId: operation.id });
    });
  });

  it('com erros no diagnóstico, pede Exportar mesmo assim', async () => {
    const user = userEvent.setup();
    const { backend } = await openExport({
      export_preview: () =>
        makePreview({
          preflight: { hygiene: [], unsavedChanges: false, diagnosticErrors: 2 },
        }),
      export_run: () => null,
    });
    expect(
      screen.getByText('O pack tem 2 erros. Veja em Problemas antes de exportar.'),
    ).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Exportar…' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'O pack tem 2 erros.' });
    expect(backend.callsOf('export_run')).toHaveLength(0);
    await user.click(within(dialog).getByRole('button', { name: 'Exportar mesmo assim' }));
    await waitFor(() => {
      expect(backend.callsOf('export_run')).toHaveLength(1);
    });
  });

  it('pack só para leitura: exporta, mas sem Excluir do pack nem limpar', async () => {
    const options = file('options.txt', { alerts: [{ kind: 'looseRootFile' }] });
    await openExport(
      {
        export_preview: () =>
          makePreview({
            files: [file('index.toml'), file('pack.toml'), options],
            preflight: {
              hygiene: [makeHygieneFinding()],
              unsavedChanges: false,
              diagnosticErrors: null,
            },
          }),
      },
      { readOnlyReason: 'newerFormat' },
    );
    expect(screen.getByRole('button', { name: 'Exportar…' })).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Revisar e limpar' })).toBeNull();
    expect(screen.queryByRole('button', { name: /Excluir/ })).toBeNull();
    expect(
      screen.getByText(
        'Este pack está só para leitura: dá para exportar, mas não excluir nem limpar.',
      ),
    ).toBeDefined();
  });
});
