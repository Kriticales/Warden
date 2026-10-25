/**
 * Padrões das configs (SPEC T12 e T14, parte P1; C-06) no app inteiro, com o backend simulado
 * fazendo o papel do Rust (o mesmo contrato do `bindings.ts`):
 *
 * - "padrão: X" com a origem, "padrão desconhecido", a marca "alterado" e o erro de faixa do
 *   valor que já está no arquivo; axe;
 * - CA-T12-09: "Restaurar padrão" de uma chave muda só aquela linha, com a diferença antes, e
 *   grava pelo Salvar de sempre; o arquivo todo pelo menu ⋯, com Descartar para voltar atrás;
 * - CA-T12-09: o filtro "Só o que mudou do padrão" mostra exatamente as chaves diferentes do
 *   padrão no formulário e na busca; a chave sem padrão conhecido nunca aparece;
 * - CA-T14-10: a correção "Restaurar padrão" de um `W_CONFIG_RANGE` em Problemas mostra a
 *   diferença, grava com o `hash` da leitura e relê tudo o que mostra o pack.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it } from 'vitest';

import type {
  ConfigContent,
  ConfigField,
  ConfigForm,
  ConfigSearchResponse,
  FormItem,
  RestoredDefaults,
} from '../../../../lib/ipc/bindings';
import { axePage } from '../../../../test/axe';
import { ipcError, type Handler } from '../../../../test/backend';
import { makeAppError } from '../../../../test/factories';
import { renderApp } from '../../../../test/render';
import { makeEntry, makeReport } from '../../../diagnostics/diagnostics.fixtures';
import { findingText, findingTitle } from '../../../diagnostics/text';
import { editorBackend } from '../../../pack-editor/testing';
import { makeSettings } from '../../../settings/testing';
import { setEditorMode } from '../model';

const FILE = 'config/chunks-server.toml';
const TEXT =
  '[general]\n#Default: 64\n#Range: 1 ~ 512\nmaxChunks = 900\n#Default: 1\nrate = 3\n#Default: 5\nsame = 5\nfree = 7\n#Defaults to A.\nmode = "A"\n';

/** As chaves com padrão do arquivo (camada A): chave → padrão. */
const DEFAULTS: Record<string, string> = { maxChunks: '64', rate: '1', same: '5' };

function valueOf(text: string, key: string): string {
  return new RegExp(`^${key} = (.*)$`, 'm').exec(text)?.[1] ?? '';
}

function field(key: string, text: string): FormItem {
  const raw = valueOf(text, key);
  const def = DEFAULTS[key];
  const base: ConfigField = {
    path: ['general', key],
    key: `general.${key}`,
    line: text.split('\n').findIndex((line) => line.startsWith(`${key} =`)) + 1,
    value: { type: 'integer', value: raw },
    control: { type: 'number', integer: true, min: null, max: null },
    confidence: 'high',
    description: null,
    range: null,
    default: def ? { value: def, inferred: false, source: 'specComment' } : null,
    defaultState: def ? (def === raw ? 'same' : 'changed') : 'unknown',
    outOfRange: false,
    label: null,
  };
  if (key === 'maxChunks') {
    base.range = { min: 1, max: 512, text: '1 ~ 512', inferred: false };
    base.control = { type: 'number', integer: true, min: 1, max: 512 };
    base.outOfRange = Number(raw) > 512;
  }
  if (key === 'mode') {
    base.value = { type: 'string', value: raw.replaceAll('"', '') };
    base.control = { type: 'text' };
    base.default = { value: 'A', inferred: true, source: 'commentPhrase' };
    base.defaultState = 'unknown';
  }
  return { kind: 'field', ...base };
}

/** O formulário que o Rust montaria. */
function formOf(text: string): ConfigForm {
  return {
    availability: { kind: 'ready' },
    items: [
      { kind: 'section', path: ['general'], key: 'general', line: 1, description: null },
      ...['maxChunks', 'rate', 'same', 'free', 'mode'].map((key) => field(key, text)),
    ],
  };
}

/** `config_restore_default` como o Rust: só o valor das chaves alteradas muda. */
function restore(text: string, keys: string[] | null): RestoredDefaults {
  let out = text;
  const restored: string[] = [];
  for (const [key, def] of Object.entries(DEFAULTS)) {
    const wanted = keys === null || keys.includes(`general.${key}`);
    if (wanted && valueOf(out, key) !== def) {
      out = out.replace(new RegExp(`^(${key} = ).*$`, 'm'), `$1${def}`);
      restored.push(`general.${key}`);
    }
  }
  return { text: out, keys: restored };
}

let counter = 0;
function entry(text: string) {
  counter += 1;
  return { text, hash: `h${String(counter)}` };
}

function setup(extra: Record<string, Handler> = {}) {
  const disk: Record<string, { text: string; hash: string }> = { [FILE]: entry(TEXT) };
  const { backend, url } = editorBackend({
    pack: { loader: 'neoforge', minecraft: '1.21.1' },
    handlers: {
      settings_get: () => makeSettings({ configDiffBeforeSave: true }),
      config_tree: () => ({
        available: true,
        files: [{ path: FILE, size: 10, modifiedAtMs: 1, binary: false }],
        truncated: false,
      }),
      config_read: (args) => {
        const file = disk[args.path as string];
        if (!file) return ipcError(makeAppError({ domain: 'project', code: 'CONFIG_NOT_FOUND' }));
        return {
          path: args.path as string,
          text: file.text,
          hash: file.hash,
          size: file.text.length,
          lineEnding: 'lf',
          readOnly: null,
          modifiedAtMs: 1,
        } satisfies ConfigContent;
      },
      config_write: (args) => {
        const file = disk[args.path as string];
        if (!file || file.hash !== args.expectedHash) {
          return ipcError(makeAppError({ domain: 'project', code: 'FILE_CHANGED_ON_DISK' }));
        }
        disk[args.path as string] = entry(args.text as string);
        return { hash: disk[args.path as string]?.hash, size: 1, changed: true };
      },
      config_form: (args) => formOf(args.text as string),
      config_restore_default: (args) =>
        restore(args.text as string, (args.keys as string[] | null) ?? null),
      ...extra,
    },
  });
  return { backend, url, disk };
}

/** Linhas diferentes entre dois textos de mesmo tamanho. */
function changedLines(before: string, after: string): number[] {
  const old = before.split('\n');
  return after
    .split('\n')
    .map((line, index) => (line === old[index] ? -1 : index + 1))
    .filter((line) => line > 0);
}

async function openForm(url: string) {
  setEditorMode('form');
  renderApp(`${url}/configs?arquivo=${FILE}`);
  return screen.findByRole('group', { name: `Campos de ${FILE}` });
}

function row(form: HTMLElement, key: string): HTMLElement {
  const keyNode = within(form).getByText(key, { selector: '.cfgform__key' });
  const found = keyNode.closest<HTMLElement>('.cfgform__row');
  if (!found) throw new Error(`sem linha para ${key}`);
  return found;
}

beforeEach(() => {
  setEditorMode('text');
});

describe('Padrões das configs (T12, C-06)', () => {
  it('mostra o padrão com a origem, "padrão desconhecido", "alterado" e o erro de faixa; axe', async () => {
    const { url } = setup();
    const form = await openForm(url);

    const chunks = row(form, 'general.maxChunks');
    expect(within(chunks).getByText('alterado')).toBeDefined();
    expect(chunks.className).toContain('cfgform__row--changed');
    expect(chunks.querySelector('.cfgform__def')?.textContent).toBe(
      'padrão: 64 · do comentário do arquivo (Default:)',
    );
    // O valor que já está no arquivo fora da faixa: erro no próprio campo, com o padrão.
    expect(
      within(chunks).getByText('900 está fora da faixa 1 a 512. O jogo troca por 64 sem avisar.'),
    ).toBeDefined();
    expect(
      within(chunks)
        .getByRole('textbox', { name: 'general.maxChunks' })
        .getAttribute('aria-invalid'),
    ).toBe('true');

    const same = row(form, 'general.same');
    expect(within(same).queryByText('alterado')).toBeNull();
    expect(within(same).queryByRole('button', { name: /Restaurar padrão/ })).toBeNull();
    // Sem padrão conhecido: a interface diz, sem inventar, e nunca marca como alterada.
    const free = row(form, 'general.free');
    expect(within(free).getByText('padrão desconhecido')).toBeDefined();
    expect(within(free).queryByText('alterado')).toBeNull();
    const mode = row(form, 'general.mode');
    expect(within(mode).getByText('deduzido')).toBeDefined();
    expect(within(mode).queryByText('alterado')).toBeNull();
    expect(within(mode).queryByRole('button', { name: /Restaurar padrão/ })).toBeNull();

    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T12-09: restaurar uma chave mostra a diferença e muda só aquela linha; grava pelo Salvar', async () => {
    const user = userEvent.setup();
    const { backend, url, disk } = setup();
    const form = await openForm(url);
    const opener = within(row(form, 'general.maxChunks')).getByRole('button', {
      name: 'Restaurar padrão de general.maxChunks',
    });
    await user.click(opener);

    const dialog = await screen.findByRole('dialog', {
      name: 'Restaurar o padrão de general.maxChunks?',
    });
    expect(await within(dialog).findByText('1 chave volta ao padrão.')).toBeDefined();
    expect(within(dialog).getByText('maxChunks = 900')).toBeDefined();
    expect(within(dialog).getByText('maxChunks = 64')).toBeDefined();
    expect(backend.callsOf('config_restore_default').at(-1)?.args).toMatchObject({
      path: FILE,
      text: TEXT,
      keys: ['general.maxChunks'],
    });
    expect(await axePage(document.body)).toHaveNoViolations();
    // Nada foi gravado só por abrir a diferença.
    expect(backend.callsOf('config_write')).toHaveLength(0);
    await user.click(within(dialog).getByRole('button', { name: 'Restaurar' }));

    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(await screen.findByText('1 linha alterada, ainda não salva')).toBeDefined();
    await waitFor(() => {
      expect(
        within(row(form, 'general.maxChunks')).queryByRole('button', { name: /Restaurar padrão/ }),
      ).toBeNull();
    });

    // Salvar: a diferença de sempre e a gravação com o hash da leitura.
    await user.click(screen.getByRole('button', { name: 'Salvar' }));
    const save = await screen.findByRole('dialog', { name: /Salvar chunks-server\.toml/ });
    await user.click(within(save).getByRole('button', { name: 'Salvar' }));
    await waitFor(() => {
      expect(backend.callsOf('config_write')).toHaveLength(1);
    });
    const written = backend.callsOf('config_write')[0]?.args.text as string;
    expect(changedLines(TEXT, written)).toEqual([4]);
    expect(disk[FILE]?.text).toBe(written);
  });

  it('restaurar o arquivo todo pelo menu ⋯, com a diferença; Descartar volta atrás', async () => {
    const user = userEvent.setup();
    const { backend, url } = setup();
    await openForm(url);
    const trigger = screen.getByRole('button', { name: 'Mais ações do arquivo' });
    await user.click(trigger);
    await user.click(
      await screen.findByRole('menuitem', { name: /Restaurar o arquivo todo para o padrão/ }),
    );
    const dialog = await screen.findByRole('dialog', {
      name: 'Restaurar chunks-server.toml para o padrão?',
    });
    expect(await within(dialog).findByText('2 chaves voltam ao padrão.')).toBeDefined();
    expect(backend.callsOf('config_restore_default').at(-1)?.args.keys).toBeNull();
    await user.click(within(dialog).getByRole('button', { name: 'Restaurar' }));
    expect(await screen.findByText('2 linhas alteradas, ainda não salvas')).toBeDefined();
    // O foco volta para quem abriu.
    await waitFor(() => {
      expect(document.activeElement).toBe(trigger);
    });
    const form = screen.getByRole('group', { name: `Campos de ${FILE}` });
    await waitFor(() => {
      expect(within(form).queryByText('alterado')).toBeNull();
    });

    await user.click(screen.getByRole('button', { name: 'Desfazer as alterações' }));
    const undo = await screen.findByRole('alertdialog');
    await user.click(within(undo).getByRole('button', { name: 'Desfazer as alterações' }));
    await waitFor(() => {
      expect(within(row(form, 'general.rate')).getByText('alterado')).toBeDefined();
    });
    expect(backend.callsOf('config_write')).toHaveLength(0);
  });

  it('CA-T12-09: "Só o que mudou do padrão" no formulário mostra exatamente as chaves alteradas', async () => {
    const user = userEvent.setup();
    const { url } = setup();
    const form = await openForm(url);
    // Em botão, sem caixa de seleção (esboço aprovado do bug-01), com a contagem ao lado.
    const filter = within(form).getByRole('button', { name: /Mudou do padrão/ });
    expect(filter.getAttribute('aria-pressed')).toBe('false');
    expect(filter.textContent).toContain('2');
    await user.click(filter);
    expect(filter.getAttribute('aria-pressed')).toBe('true');
    const keys = [...form.querySelectorAll('.cfgform__key')].map((node) =>
      node.textContent.replace('alterado', ''),
    );
    expect(keys).toEqual(['general.maxChunks', 'general.rate']);
    expect(await axePage(document.body)).toHaveNoViolations();
  });
});

describe('Busca em todas as configs com "Só o que mudou do padrão" (C-05 + C-06)', () => {
  const HIT = {
    line: 4,
    field: 'key' as const,
    key: { parts: [{ text: 'general.maxChunks', marked: false }] },
    label: null,
    value: { parts: [{ text: '900', marked: false }] },
    comment: null,
    text: null,
    changed: true,
  };

  function answer(args: Record<string, unknown>): ConfigSearchResponse {
    const status = { available: true, ready: true, indexedFiles: 1, totalFiles: 1, entries: 9 };
    if (args.query === '' && !args.onlyChanged) {
      return { status, results: null, cancelled: false };
    }
    return {
      status,
      cancelled: false,
      results: {
        groups: [{ path: FILE, modId: 'chunks', hits: [HIT], total: 1 }],
        totalHits: 1,
        totalFiles: 1,
        truncated: false,
        mods: [{ id: 'chunks', hits: 1 }],
      },
    };
  }

  it('"Só o que mudou do padrão" na coluna deixa na árvore só os arquivos alterados', async () => {
    const user = userEvent.setup();
    const { backend, url } = setup({
      config_search: answer,
      config_tree: () => ({
        available: true,
        files: [
          { path: FILE, size: 10, modifiedAtMs: 1, binary: false },
          { path: 'config/igual.toml', size: 10, modifiedAtMs: 1, binary: false },
        ],
        truncated: false,
      }),
    });
    renderApp(`${url}/configs`);
    await screen.findByRole('treeitem', { name: /igual\.toml/ }, { timeout: 10_000 });
    const filter = screen.getByRole('button', { name: /Só o que mudou do padrão/ });
    // A contagem é de arquivos com alguma chave alterada (pelo índice da busca).
    await waitFor(() => {
      expect(filter.textContent).toContain('1');
    });
    expect(backend.callsOf('config_search').some((call) => call.args.onlyChanged === true)).toBe(
      true,
    );
    await user.click(filter);
    expect(filter.getAttribute('aria-pressed')).toBe('true');
    expect(screen.queryByRole('treeitem', { name: /igual\.toml/ })).toBeNull();
    expect(screen.getByRole('treeitem', { name: /chunks/ })).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });
});

describe('Restaurar padrão em Problemas (T14, CA-T14-10)', () => {
  const RANGE = makeEntry({
    rule: 'W_CONFIG_RANGE',
    severity: 'warning',
    titleKey: 'diagnostico.achados.configForaDaFaixa',
    params: {
      config_file: FILE,
      config_key: 'general.maxChunks',
      config_value: '900',
      config_limit: '1 ~ 512',
      config_default: '64',
    },
    evidence: [{ kind: 'packFile', path: FILE, excerpt: 'maxChunks = 900' }],
    fixes: [{ kind: 'restoreDefault', path: FILE, key: 'general.maxChunks' }],
  });

  it('mostra a diferença, grava com o hash da leitura e relê o pack e o relatório', async () => {
    const user = userEvent.setup();
    const { backend, url, disk } = setup({
      diagnostics_report_get: () => makeReport({ findings: [RANGE] }),
    });
    renderApp(`${url}/problemas`);
    const card = await screen.findByRole(
      'article',
      { name: 'general.maxChunks = 900 está fora do que o mod aceita (1 ~ 512)' },
      { timeout: 10_000 },
    );
    expect(
      within(card).getByText(
        `Em ${FILE}. Quando o jogo abrir, ele troca o valor por 64, o padrão, sem avisar. Restaure o padrão ou corrija o valor em Configs.`,
      ),
    ).toBeDefined();
    const readHash = disk[FILE]?.hash;
    const reports = backend.callsOf('diagnostics_report_get').length;
    const rows = backend.callsOf('pack_get').length;

    await user.click(
      within(card).getByRole('button', { name: 'Restaurar padrão de general.maxChunks' }),
    );
    const dialog = await screen.findByRole('dialog', {
      name: 'Restaurar o padrão de general.maxChunks?',
    });
    expect(await within(dialog).findByText('maxChunks = 64')).toBeDefined();
    expect(backend.callsOf('config_write')).toHaveLength(0);
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Restaurar e salvar' }));

    await waitFor(() => {
      expect(backend.callsOf('config_write')).toHaveLength(1);
    });
    expect(backend.callsOf('config_write')[0]?.args).toMatchObject({
      origin: 'pack',
      path: FILE,
      expectedHash: readHash,
    });
    expect(changedLines(TEXT, disk[FILE]?.text ?? '')).toEqual([4]);
    expect(await screen.findByText(`general.maxChunks voltou ao padrão em ${FILE}.`)).toBeDefined();
    // O que está na tela é relido: o relatório de Problemas e a linha do pack (as demais
    // consultas do pack ficam invalidadas; teste do `usePackRestoreWrite` abaixo).
    await waitFor(() => {
      expect(backend.callsOf('diagnostics_report_get').length).toBeGreaterThan(reports);
      expect(backend.callsOf('pack_get').length).toBeGreaterThan(rows);
    });
  });

  it('arquivo mudado por fora: nada é gravado e o erro aparece no diálogo', async () => {
    const user = userEvent.setup();
    const { backend, url } = setup({
      diagnostics_report_get: () => makeReport({ findings: [RANGE] }),
      config_write: () =>
        ipcError(makeAppError({ domain: 'project', code: 'FILE_CHANGED_ON_DISK' })),
    });
    renderApp(`${url}/problemas`);
    const card = await screen.findByRole(
      'article',
      { name: /general\.maxChunks = 900/ },
      { timeout: 10_000 },
    );
    await user.click(
      within(card).getByRole('button', { name: 'Restaurar padrão de general.maxChunks' }),
    );
    const dialog = await screen.findByRole('dialog');
    await within(dialog).findByText('maxChunks = 64');
    await user.click(within(dialog).getByRole('button', { name: 'Restaurar e salvar' }));
    expect(await within(dialog).findByText('Não foi possível restaurar o padrão.')).toBeDefined();
    expect(backend.callsOf('config_write')).toHaveLength(1);
  });
});

describe('Config resetada pelo jogo (T14, CA-T14-10)', () => {
  it('o achado do log diz, em português, que o jogo trocou o valor pelo padrão', () => {
    const reset = makeEntry({
      rule: 'W_CONFIG_RESET',
      severity: 'warning',
      titleKey: 'diagnostico.achados.configResetada',
      params: {
        config_key: 'server.zombieBaseSummonChance',
        config_value: '5.0',
        config_default: '1.0',
      },
      evidence: [
        {
          kind: 'log',
          file: 'logs/latest.log',
          line: 32,
          excerpt:
            'Incorrect key server.zombieBaseSummonChance was corrected from 5.0 to its default, 1.0.',
        },
      ],
    });
    expect(findingTitle(reset)).toBe(
      'O jogo trocou um valor da sua config pelo padrão (server.zombieBaseSummonChance)',
    );
    expect(findingText(reset)).toContain(
      'O valor 5.0 estava fora da faixa ou do tipo que o mod aceita, e o jogo usou 1.0 no lugar.',
    );
  });
});
