/**
 * Formatos de outros launchers na seção Exportar (E-02; SPEC T19; CA-T19-04 e CA-T19-05 na
 * parte da tela): o que se perde, a troca pelo Modrinth, o bloqueio explicado, a confirmação
 * para embutir arquivos de terceiros e a exigência de `version`. A geração e a conferência do
 * arquivo são provadas no Rust (`warden-export/tests/formats`) e no E2E.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { ExportPreview, FormatAnalysis, FormatItem } from '../../../lib/ipc/bindings';
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

function item(overrides: Partial<FormatItem> & Pick<FormatItem, 'path'>): FormatItem {
  return {
    name: 'Mod',
    filename: 'mod.jar',
    origin: 'curseforge',
    host: null,
    outcome: { kind: 'embed' },
    ...overrides,
  };
}

function makeAnalysis(overrides: Partial<FormatAnalysis> = {}): FormatAnalysis {
  return {
    format: 'mrpack',
    name: 'Vale Sereno',
    version: '1.4.0',
    references: 10,
    embedded: 0,
    losses: [
      { kind: 'optionalTexts', count: 2 },
      { kind: 'configsEverywhere', count: 0 },
      { kind: 'noAutoUpdate', count: 0 },
    ],
    items: [],
    ...overrides,
  };
}

const SWAP: FormatItem = item({
  path: 'mods/jei.pw.toml',
  name: 'JEI',
  filename: 'jei.jar',
  outcome: {
    kind: 'swap',
    required: false,
    project_id: 'u6dRKJwZ',
    version_id: 'v1',
    filename: 'jei-modrinth.jar',
  },
});

async function openFormat(
  analysis: FormatAnalysis | (() => unknown),
  handlers: Record<string, () => unknown> = {},
  label = /^\.mrpack/,
) {
  const { backend, url } = editorBackend({
    handlers: {
      export_preview: () => PREVIEW,
      export_format_analyze: typeof analysis === 'function' ? analysis : () => analysis,
      ...handlers,
    },
  });
  renderApp(`${url}/exportar`);
  const user = userEvent.setup();
  await screen.findByRole('heading', { level: 1, name: 'Exportar' }, { timeout: 10_000 });
  await user.click(await screen.findByRole('radio', { name: label }));
  return { backend, user };
}

describe('Exportar: formatos de outros launchers (T19, P1)', () => {
  it('lista os dois formatos novos, lê o pack para o formato e explica o que se perde; axe', async () => {
    const { backend } = await openFormat(makeAnalysis());
    expect(screen.getByRole('radio', { name: /^\.zip da CurseForge/ })).toBeDefined();
    await screen.findByRole('heading', { name: 'O que este formato perde' });
    expect(backend.callsOf('export_format_analyze')[0]?.args).toMatchObject({
      source: { kind: 'current' },
      format: 'mrpack',
    });
    const losses = screen.getByRole('region', { name: 'O que este formato perde' });
    expect(losses.textContent).toContain('2 mods opcionais perdem o texto e o padrão do opcional');
    expect(losses.textContent).toContain('cada versão nova é um arquivo novo');
    expect(losses.textContent).toContain('As configs vão sempre para todos os lados');
    const summary = screen.getByRole('region', { name: 'O que vai no arquivo' });
    expect(summary.textContent).toContain('10 mods vão por referência');
    expect(summary.textContent).toContain('Nenhum arquivo de terceiros vai dentro');
    // O pack já tem versão: nada a digitar, e Exportar… está liberado.
    expect(screen.queryByRole('region', { name: 'Versão do pack' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Exportar…' })).toHaveProperty('disabled', false);
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('exige a versão quando o pack.toml não tem e a manda ao backend, sem mudar o pack', async () => {
    const { backend, user } = await openFormat(makeAnalysis({ version: null }), {
      export_format_run: () => null,
    });
    const exportButton = await screen.findByRole('button', { name: 'Exportar…' });
    expect(exportButton).toHaveProperty('disabled', true);
    expect(screen.getByText('Digite a versão do pack para exportar.')).toBeDefined();
    await user.type(screen.getByRole('textbox', { name: 'Versão' }), ' 2.0.0 ');
    expect(exportButton).toHaveProperty('disabled', false);
    await user.click(exportButton);
    await waitFor(() => {
      expect(backend.callsOf('export_format_run')).toHaveLength(1);
    });
    expect(backend.callsOf('export_format_run')[0]?.args).toMatchObject({
      format: 'mrpack',
      choices: { swap: [], confirmEmbed: false, version: '2.0.0' },
    });
  });

  it('troca pelo Modrinth: marcada por padrão; desmarcar vira arquivo de terceiros e pede a confirmação', async () => {
    const { backend, user } = await openFormat(makeAnalysis({ items: [SWAP] }), {
      export_format_run: () => null,
    });
    const swap = await screen.findByRole('checkbox', { name: /Trocar pelo Modrinth/ });
    expect(swap).toHaveProperty('checked', true);
    expect(screen.getByText(/Projeto u6dRKJwZ, arquivo jei-modrinth.jar/)).toBeDefined();
    expect(screen.queryByRole('checkbox', { name: /Tenho permissão/ })).toBeNull();

    // Com a troca, vai direto.
    await user.click(screen.getByRole('button', { name: 'Exportar…' }));
    await waitFor(() => {
      expect(backend.callsOf('export_format_run')).toHaveLength(1);
    });
    expect(backend.callsOf('export_format_run')[0]?.args).toMatchObject({
      choices: { swap: ['mods/jei.pw.toml'], confirmEmbed: false },
    });

    // Sem a troca, o jar iria dentro: pede a confirmação de licença.
    await user.click(swap);
    expect(screen.getByRole('button', { name: 'Exportar…' })).toHaveProperty('disabled', true);
    expect(
      screen.getByText('Confirme que pode incluir os arquivos de terceiros para exportar.'),
    ).toBeDefined();
    expect(screen.getByRole('region', { name: 'O que vai no arquivo' }).textContent).toContain(
      '1 arquivo de terceiros vai dentro',
    );
    await user.click(screen.getByRole('checkbox', { name: /Tenho permissão/ }));
    await user.click(screen.getByRole('button', { name: 'Exportar…' }));
    await waitFor(() => {
      expect(backend.callsOf('export_format_run')).toHaveLength(2);
    });
    expect(backend.callsOf('export_format_run')[1]?.args).toMatchObject({
      choices: { swap: [], confirmEmbed: true },
    });
  });

  it('troca obrigatória (CurseForge bloqueou, Modrinth tem): marcada e travada', async () => {
    const required = item({
      ...SWAP,
      outcome: {
        kind: 'swap',
        required: true,
        project_id: 'abc',
        version_id: 'v2',
        filename: 'x.jar',
      },
    });
    await openFormat(makeAnalysis({ items: [required] }));
    const swap = await screen.findByRole('checkbox', { name: /Trocar pelo Modrinth/ });
    expect(swap).toHaveProperty('checked', true);
    expect(swap).toHaveProperty('disabled', true);
    expect(screen.getByText(/não deixa outros apps distribuírem este mod/)).toBeDefined();
    expect(screen.getByRole('button', { name: 'Exportar…' })).toHaveProperty('disabled', false);
  });

  it('mod bloqueado para terceiros sem equivalente: explica e impede de gerar', async () => {
    const blocked = item({
      path: 'mods/x.pw.toml',
      name: 'Mod Fechado',
      outcome: { kind: 'blocked', page_url: 'https://www.curseforge.com/minecraft/mc-mods/x' },
    });
    const gone = item({
      path: 'mods/y.pw.toml',
      name: 'Mod Sumido',
      outcome: { kind: 'unavailable' },
    });
    await openFormat(makeAnalysis({ items: [blocked, gone] }));
    const panel = await screen.findByRole('region', {
      name: '2 arquivos pedem decisão ou explicação',
    });
    expect(within(panel).getByText('Este mod não pode ir neste formato')).toBeDefined();
    expect(within(panel).getByText('Este mod saiu da CurseForge')).toBeDefined();
    expect(
      within(panel).getByRole('button', { name: 'Abrir a página na CurseForge' }),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Exportar…' })).toHaveProperty('disabled', true);
    expect(
      screen.getByText('Resolva os mods que não podem ir neste formato antes de exportar.'),
    ).toBeDefined();
  });

  it('zip da CurseForge: arquivo de fora da CurseForge pede a confirmação com o aviso de aprovação manual', async () => {
    const local = item({
      path: 'mods/local.jar',
      name: 'local.jar',
      filename: 'local.jar',
      origin: 'local',
    });
    const { backend } = await openFormat(
      makeAnalysis({
        format: 'curseforge',
        items: [local],
        losses: [{ kind: 'sideNotKept', count: 3 }],
      }),
      {},
      /^\.zip da CurseForge/,
    );
    await screen.findByText('Arquivos de terceiros dentro do arquivo');
    expect(backend.callsOf('export_format_analyze')[0]?.args).toMatchObject({
      format: 'curseforge',
    });
    expect(screen.getByText(/exige aprovação manual de arquivos que não são dela/)).toBeDefined();
    expect(screen.getByText(/3 mods só de cliente: a CurseForge não guarda o lado/)).toBeDefined();
    expect(screen.getByRole('button', { name: 'Exportar…' })).toHaveProperty('disabled', true);
  });

  it('resultado: tamanho, destino, trocas e a conferência; Mostrar o arquivo usa export_reveal', async () => {
    const { backend, user } = await openFormat(makeAnalysis(), {
      export_format_run: () => ({
        path: 'D:\\Exportados\\vale-sereno.mrpack',
        bytes: 2048,
        swapped: 2,
        validation: { entries: 12, references: 10, overrides: 2, embeddedJars: [] },
      }),
      export_reveal: () => null,
    });
    await user.click(await screen.findByRole('button', { name: 'Exportar…' }));
    const dialog = await screen.findByRole('dialog', { name: 'Arquivo exportado' });
    expect(dialog.textContent).toContain('2,0 KB, em D:\\Exportados\\vale-sereno.mrpack');
    expect(dialog.textContent).toContain('2 mods foram trocados pelo Modrinth');
    expect(dialog.textContent).toContain('Arquivo conferido.');
    expect(dialog.textContent).toContain('12 entradas no zip');
    await user.click(within(dialog).getByRole('button', { name: 'Mostrar o arquivo' }));
    await waitFor(() => {
      expect(backend.callsOf('export_reveal')[0]?.args).toEqual({
        path: 'D:\\Exportados\\vale-sereno.mrpack',
      });
    });
  });

  it('erros de domínio têm frase: chave da CurseForge na leitura e arquivo descartado na geração', async () => {
    const { user } = await openFormat(makeAnalysis(), {
      export_format_run: () => ipcError(makeAppError({ domain: 'export', code: 'INVALID_OUTPUT' })),
    });
    await user.click(await screen.findByRole('button', { name: 'Exportar…' }));
    expect(await screen.findByText('Não foi possível exportar.')).toBeDefined();
    expect(screen.getByText(/não passou na conferência e foi descartado/)).toBeDefined();
  });

  it('leitura recusada pela falta da chave: mostra a frase com o caminho em Configurações', async () => {
    await openFormat(() =>
      ipcError(makeAppError({ domain: 'export', code: 'CURSEFORGE_KEY_MISSING' })),
    );
    expect(
      await screen.findByText('Não foi possível conferir o pack para este formato.'),
    ).toBeDefined();
    expect(screen.getByText(/Cadastre-a em Configurações/)).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Exportar…' })).toBeNull();
  });
});
