/**
 * Instância pronta para o Prism em Exportar (SPEC T19; CA-T19-09): análise, Trocar pelo
 * Modrinth, confirmação de arquivos de terceiros, geração e resultado, com o backend simulado.
 * O conteúdo do arquivo gerado é provado no Rust (`warden-export`, `tests/prism`).
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { ExportPreview, PrismAnalysis } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';

const PREVIEW: ExportPreview = {
  files: [],
  folders: [],
  bytes: 0,
  references: 0,
  preflight: { hygiene: [], unsavedChanges: false, diagnosticErrors: null },
};

function analysis(overrides: Partial<PrismAnalysis> = {}): PrismAnalysis {
  return {
    version: '1.2.0',
    minecraft: '1.21.1',
    loader: 'fabric 0.16.5',
    modrinth: 40,
    curseforge: 3,
    links: 1,
    overrides: 12,
    untrusted: 4,
    curseforgeKeyMissing: false,
    blocked: [],
    localFiles: [],
    ...overrides,
  };
}

const BLOCKED = {
  path: 'mods/entityculling.pw.toml',
  name: 'Entity Culling',
  fileName: 'entityculling.jar',
  pageUrl: null,
  swap: {
    projectId: 'NNAgCjsB',
    versionId: 'abc',
    versionNumber: '1.7.0',
    fileName: 'entityculling-fabric-1.7.0.jar',
  },
};

type Options = NonNullable<Parameters<typeof editorBackend>[0]>;

async function openPrism(handlers: Options['handlers'] = {}) {
  const { backend, url } = editorBackend({
    handlers: {
      export_preview: () => PREVIEW,
      prism_analyze: () => analysis(),
      ...handlers,
    },
  });
  renderApp(`${url}/exportar`);
  await screen.findByRole('heading', { level: 1, name: 'Exportar' }, { timeout: 10_000 });
  const user = userEvent.setup();
  await user.click(await screen.findByRole('radio', { name: /^Instância pronta para o Prism/ }));
  return { backend, user };
}

describe('Instância pronta para o Prism (T19)', () => {
  it('mostra o que vai no arquivo e o aviso do que o jogador vai ver; axe', async () => {
    const { backend } = await openPrism();
    const summary = await screen.findByRole('region', { name: 'O que vai no arquivo' });
    expect(backend.callsOf('prism_analyze')[0]?.args.source).toEqual({ kind: 'current' });
    expect(summary.textContent).toContain(
      'Minecraft 1.21.1 com fabric 0.16.5, versão 1.2.0 do pack.',
    );
    expect(summary.textContent).toContain('40 arquivos do Modrinth');
    expect(summary.textContent).toContain(
      '3 arquivos da CurseForge, baixados pelo Prism com a chave do próprio jogador',
    );
    expect(summary.textContent).toContain('O Prism vai pedir confirmação de 4 itens.');
    expect(summary.textContent).toContain('cada versão nova é um arquivo novo');
    expect(screen.getByRole('button', { name: 'Gerar instância…' })).toHaveProperty(
      'disabled',
      false,
    );
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('mod bloqueado impede gerar até trocar pelo Modrinth; a troca vai nas opções', async () => {
    const { backend, user } = await openPrism({
      prism_analyze: () => analysis({ blocked: [BLOCKED] }),
      prism_run: () => null,
    });
    const generate = await screen.findByRole('button', { name: 'Gerar instância…' });
    expect(generate).toHaveProperty('disabled', true);
    expect(screen.getByText(/Resolva os mods bloqueados acima/)).toBeDefined();

    await user.click(screen.getByRole('button', { name: 'Trocar Entity Culling pelo Modrinth' }));
    expect(
      screen.getByText('Será trocado por entityculling-fabric-1.7.0.jar (1.7.0) do Modrinth.'),
    ).toBeDefined();
    expect(generate).toHaveProperty('disabled', false);
    await user.click(generate);
    await waitFor(() => {
      expect(backend.callsOf('prism_run')[0]?.args).toMatchObject({
        source: { kind: 'current' },
        options: { confirmLocalFiles: false, swaps: ['mods/entityculling.pw.toml'] },
      });
    });
  });

  it('mod bloqueado sem equivalente no Modrinth explica e não oferece troca', async () => {
    await openPrism({
      prism_analyze: () => analysis({ blocked: [{ ...BLOCKED, swap: null }] }),
    });
    await screen.findByText('Este arquivo não existe no Modrinth.');
    expect(screen.queryByRole('button', { name: /pelo Modrinth/ })).toBeNull();
    expect(screen.getByRole('button', { name: 'Gerar instância…' })).toHaveProperty(
      'disabled',
      true,
    );
  });

  it('arquivos de terceiros pedem a confirmação de licença antes de gerar', async () => {
    const { backend, user } = await openPrism({
      prism_analyze: () => analysis({ localFiles: [{ path: 'mods/local.jar', bytes: 2048 }] }),
      prism_run: () => null,
    });
    const generate = await screen.findByRole('button', { name: 'Gerar instância…' });
    expect(generate).toHaveProperty('disabled', true);
    expect(screen.getByText('mods/local.jar')).toBeDefined();
    await user.click(screen.getByRole('checkbox', { name: /Tenho o direito de distribuir/ }));
    await user.click(generate);
    await waitFor(() => {
      expect(backend.callsOf('prism_run')[0]?.args).toMatchObject({
        options: { confirmLocalFiles: true, swaps: [] },
      });
    });
  });

  it('sem versão e sem chave da CurseForge: avisa e não deixa gerar', async () => {
    await openPrism({
      prism_analyze: () => analysis({ version: '', curseforgeKeyMissing: true }),
    });
    await screen.findByText('O pack ainda não tem versão');
    expect(screen.getByText('Falta a chave da CurseForge')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Gerar instância…' })).toHaveProperty(
      'disabled',
      true,
    );
  });

  it('resultado: tamanho, SHA-256, conferência e o passo a passo com a memória do teste', async () => {
    const { backend, user } = await openPrism({
      prism_run: () => ({
        path: 'D:\\Exportados\\vale-sereno-1.2.0-prism.mrpack',
        bytes: 5120,
        sha256: 'ab'.repeat(32),
        files: 44,
        overrides: 12,
        untrusted: 4,
      }),
      prism_reveal: () => null,
      pack_test_settings_get: () => ({
        settings: { memory: { mode: 'fixed', mb: 8192 }, java: null, jvmArgs: '' },
        instanceExists: false,
        hasWorlds: false,
      }),
    });
    await user.click(await screen.findByRole('button', { name: 'Gerar instância…' }));
    const dialog = await screen.findByRole('dialog', {
      name: 'Instância pronta para o Prism gerada',
    });
    expect(dialog.textContent).toContain(
      '44 arquivos para o Prism baixar e 12 arquivos dentro do arquivo',
    );
    expect(dialog.textContent).toContain('ab'.repeat(32));
    expect(dialog.textContent).toContain(
      'Baixe o arquivo e arraste para a janela do Prism Launcher.',
    );
    expect(dialog.textContent).toContain('Use 8 GB de memória');
    expect(dialog.textContent).toContain('não se atualiza sozinha');
    await user.click(within(dialog).getByRole('button', { name: 'Mostrar arquivo' }));
    await waitFor(() => {
      expect(backend.callsOf('prism_reveal')[0]?.args).toEqual({
        path: 'D:\\Exportados\\vale-sereno-1.2.0-prism.mrpack',
      });
    });
  });

  it('erro da geração aparece com a frase do domínio export', async () => {
    const { user } = await openPrism({
      prism_run: () =>
        ipcError(
          makeAppError(
            { domain: 'export', code: 'PRISM_BLOCKED' },
            { params: { mods: 'Entity Culling', count: '1' } },
          ),
        ),
    });
    await user.click(await screen.findByRole('button', { name: 'Gerar instância…' }));
    expect(await screen.findByText(/Entity Culling/)).toBeDefined();
    expect(
      screen.getByText('Não foi possível gerar a instância pronta para o Prism.'),
    ).toBeDefined();
  });

  it('falha ao analisar mostra o erro com Tentar de novo', async () => {
    await openPrism({
      prism_analyze: () =>
        ipcError(makeAppError({ domain: 'export', code: 'PRISM_LOOKUP_FAILED' })),
    });
    expect(await screen.findByText('Não foi possível conferir os mods do pack.')).toBeDefined();
    expect(screen.getByRole('button', { name: /Tentar de novo/ })).toBeDefined();
  });
});
