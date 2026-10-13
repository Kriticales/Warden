/**
 * Formulário de configs (SPEC T12, parte P1; C-04) no app inteiro, com o editor da C-02:
 *
 * - alternância Formulário/Texto, lembrada entre arquivos; axe nos dois modos;
 * - campos com rótulo do jar e a chave real ao lado, faixa, padrão e "deduzido" (CA-T12-08);
 * - CA-T12-02 pela interface: mudar um valor muda uma linha e grava pelo Salvar de sempre;
 * - número fora da faixa ou inválido fica no campo e não é aplicado;
 * - queda para o texto com o motivo (arquivo só texto e edição não confirmada; ADR-0054);
 * - "Editar este trecho como texto";
 * - "Não substituir se o jogador já tiver" e "Copiar para defaultconfigs/", que atualizam
 *   todas as telas que mostram o pack.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it } from 'vitest';

import type {
  ConfigContent,
  ConfigField,
  ConfigForm,
  CopyOutcome,
  FormEdit,
  FormItem,
  PreserveState,
} from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { makeSettings } from '../../settings/testing';
import { setEditorMode } from './model';

const CREATE = 'config/create-common.toml';
const CREATE_TEXT =
  '#Velocidade\n# Default: 256\n# Range: 64 ~ 1024\n[kinetics]\nmaxRotationSpeed = 256\nenabled = true\n';

function field(
  partial: Partial<ConfigField> & Pick<ConfigField, 'key' | 'value' | 'control'>,
): FormItem {
  return {
    kind: 'field',
    path: partial.key.split('.'),
    line: 1,
    confidence: 'high',
    description: null,
    range: null,
    default: null,
    label: null,
    ...partial,
  };
}

/** O formulário que o Rust montaria para o texto (só o que os testes olham). */
function formOf(text: string): ConfigForm {
  const speed = /maxRotationSpeed = (.*)/.exec(text)?.[1] ?? '256';
  const enabled = /enabled = (.*)/.exec(text)?.[1] ?? 'true';
  return {
    availability: { kind: 'ready' },
    items: [
      { kind: 'section', path: ['kinetics'], key: 'kinetics', line: 4, description: null },
      field({
        key: 'kinetics.maxRotationSpeed',
        line: 5,
        value: { type: 'integer', value: speed },
        control: { type: 'number', integer: true, min: 64, max: 1024 },
        description: 'Velocidade',
        range: { min: 64, max: 1024, text: '64 ~ 1024', inferred: false },
        default: { value: '256', inferred: false },
        label: {
          text: 'Velocidade máxima de rotação',
          tooltip: null,
          translationKey: 'create.configuration.maxRotationSpeed',
          locale: 'ptBr',
        },
      }),
      field({
        key: 'kinetics.enabled',
        line: 6,
        value: { type: 'bool', value: enabled === 'true' },
        control: { type: 'toggle' },
        description: 'Liga. Defaults to true.',
        default: { value: 'true', inferred: true },
      }),
      {
        kind: 'snippet',
        path: ['kinetics', 'lista'],
        key: 'kinetics.lista',
        line: 3,
        reason: 'listOfObjects',
      },
    ],
  };
}

/** Troca o valor da linha `chave = …`, como o Rust (só o valor muda). */
function applyEdits(text: string, edits: FormEdit[]): string {
  let out = text;
  for (const edit of edits) {
    const key = String(edit.path[edit.path.length - 1]);
    const value =
      edit.value.type === 'list' ? edit.value.value.join(',') : String(edit.value.value);
    out = out.replace(new RegExp(`^(${key} = ).*$`, 'm'), `$1${value}`);
  }
  return out;
}

type Disk = Record<string, { text: string; hash: string }>;

interface Options {
  pack: Disk;
  /** `config_form_apply` falha (edição não confirmada). */
  applyFails?: boolean;
  preserve?: PreserveState;
  copy?: CopyOutcome[];
}

let counter = 0;
function entry(text: string) {
  counter += 1;
  return { text, hash: `h${String(counter)}` };
}

function open(path: string, options: Options) {
  const disk = options.pack;
  const preserve = { ...(options.preserve ?? { inIndex: true, preserve: false }) };
  const copies = [...(options.copy ?? [])];
  const { backend, url } = editorBackend({
    pack: { loader: 'neoforge', minecraft: '1.21.1' },
    handlers: {
      settings_get: () => makeSettings({ configDiffBeforeSave: true }),
      config_tree: () => ({
        available: true,
        files: Object.keys(disk).map((file) => ({
          path: file,
          size: 10,
          modifiedAtMs: 1,
          binary: false,
        })),
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
      config_form: (args) => {
        const file = String(args.path);
        if (file.endsWith('.yml')) {
          return {
            availability: { kind: 'textOnly', reason: { code: 'unsupportedFormat' } },
            items: [],
          };
        }
        return formOf(args.text as string);
      },
      config_form_apply: (args) => {
        if (options.applyFails) {
          return ipcError(
            makeAppError(
              { domain: 'configs', code: 'EDIT_NOT_CONFIRMED' },
              { detail: 'releitura' },
            ),
          );
        }
        return applyEdits(args.text as string, args.edits as FormEdit[]);
      },
      config_preserve: () => ({ ...preserve }),
      config_set_preserve: (args) => {
        preserve.preserve = Boolean(args.preserve);
        return true;
      },
      config_copy_to_defaultconfigs: (args) => {
        const outcome = copies.shift() ?? {
          kind: 'copied',
          target: 'defaultconfigs/x-server.toml',
        };
        if (outcome.kind === 'copied') {
          disk[outcome.target] = entry(disk[args.path as string]?.text ?? '');
        }
        return outcome;
      },
    },
  });
  renderApp(`${url}/configs?arquivo=${path}`);
  return { backend, disk };
}

async function toForm(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('radio', { name: 'Formulário' }));
  return screen.findByRole('group', { name: /^Campos de / });
}

beforeEach(() => {
  setEditorMode('text');
});

describe('Formulário de configs (T12, C-04)', () => {
  it('mostra rótulo do jar com a chave real, faixa, padrão e deduzido; axe nos dois modos', async () => {
    const user = userEvent.setup();
    const { backend } = open(CREATE, { pack: { [CREATE]: entry(CREATE_TEXT) } });
    await screen.findByRole('textbox', { name: `Conteúdo de ${CREATE}` }, { timeout: 10_000 });
    expect(await axePage(document.body)).toHaveNoViolations();
    // No texto, o formulário não é montado (cada tecla mudaria a consulta).
    expect(backend.callsOf('config_form')).toHaveLength(0);

    const form = await toForm(user);
    expect(screen.getByRole('radio', { name: 'Formulário' }).getAttribute('aria-checked')).toBe(
      'true',
    );
    expect(within(form).getByText('Velocidade máxima de rotação')).toBeDefined();
    expect(within(form).getByText('kinetics.maxRotationSpeed')).toBeDefined();
    expect(within(form).getByText('Faixa: 64 a 1.024.')).toBeDefined();
    expect(within(form).getByText('padrão: 256')).toBeDefined();
    expect(within(form).getByText(/padrão: true/).textContent).toContain('deduzido');
    expect(within(form).getByRole('switch', { name: 'kinetics.enabled' })).toBeDefined();
    expect(backend.callsOf('config_form')[0]?.args.text).toBe(CREATE_TEXT);
    expect(screen.queryByRole('button', { name: 'Buscar e substituir' })).toBeNull();
    // O aviso do NeoForge (chaves apagadas e valores corrigidos) continua no formulário.
    expect(
      screen.getByText(
        'Valores fora da faixa voltam ao padrão e chaves que o mod não conhece são apagadas quando o jogo abre.',
      ),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T12-02: mudar um valor muda uma linha e grava pelo Salvar de sempre', async () => {
    const user = userEvent.setup();
    const { backend, disk } = open(CREATE, { pack: { [CREATE]: entry(CREATE_TEXT) } });
    const form = await toForm(user);
    const speed = within(form).getByRole('textbox', { name: 'Velocidade máxima de rotação' });
    await user.clear(speed);
    await user.type(speed, '512{Enter}');

    await screen.findByText('1 linha alterada, ainda não salva');
    expect(backend.callsOf('config_form_apply')[0]?.args).toEqual({
      path: CREATE,
      text: CREATE_TEXT,
      edits: [{ path: ['kinetics', 'maxRotationSpeed'], value: { type: 'integer', value: '512' } }],
    });
    // O formulário é refeito do texto novo.
    await waitFor(() => {
      expect(backend.callsOf('config_form').at(-1)?.args.text).toContain('maxRotationSpeed = 512');
    });

    await user.click(screen.getByRole('button', { name: 'Salvar' }));
    const dialog = await screen.findByRole('dialog', { name: 'Salvar create-common.toml?' });
    await user.click(within(dialog).getByRole('button', { name: 'Salvar' }));
    await screen.findByText('create-common.toml salvo.');
    expect(disk[CREATE]?.text).toBe(CREATE_TEXT.replace('= 256', '= 512'));

    // Liga/desliga aplica na hora.
    await user.click(within(form).getByRole('switch', { name: 'kinetics.enabled' }));
    await waitFor(() => {
      expect(backend.callsOf('config_form_apply').at(-1)?.args.edits).toEqual([
        { path: ['kinetics', 'enabled'], value: { type: 'bool', value: false } },
      ]);
    });
  });

  it('número fora da faixa ou que não é número fica no campo e não é aplicado', async () => {
    const user = userEvent.setup();
    const { backend } = open(CREATE, { pack: { [CREATE]: entry(CREATE_TEXT) } });
    const form = await toForm(user);
    const speed = within(form).getByRole('textbox', { name: 'Velocidade máxima de rotação' });
    await user.clear(speed);
    await user.type(speed, '9000{Enter}');
    expect(
      await within(form).findByText(
        '9000 está fora da faixa. O jogo pode trocar por outro valor sem avisar.',
      ),
    ).toBeDefined();
    expect(speed.getAttribute('aria-invalid')).toBe('true');
    await user.clear(speed);
    await user.type(speed, '1.5{Enter}');
    expect(await within(form).findByText('Digite um número inteiro.')).toBeDefined();
    expect(backend.callsOf('config_form_apply')).toHaveLength(0);
    expect(screen.getByText('Sem alterações')).toBeDefined();
  });

  it('edição não confirmada cai para o texto com o motivo, sem mudar nada', async () => {
    const user = userEvent.setup();
    open(CREATE, { pack: { [CREATE]: entry(CREATE_TEXT) }, applyFails: true });
    const form = await toForm(user);
    await user.click(within(form).getByRole('switch', { name: 'kinetics.enabled' }));
    await screen.findByText(/Esta mudança não pôde ser feita pelo formulário/);
    expect(screen.getByText(/O Warden não conseguiu confirmar/)).toBeDefined();
    const textbox = await screen.findByRole('textbox', { name: `Conteúdo de ${CREATE}` });
    expect(textbox.textContent).toContain('enabled = true');
    expect(screen.getByText('Sem alterações')).toBeDefined();
    // Escolher Formulário de novo tenta outra vez.
    await user.click(screen.getByRole('radio', { name: 'Formulário' }));
    await screen.findByRole('group', { name: /^Campos de / });
  });

  it('arquivo só texto mostra o motivo; o modo escolhido vale para o próximo arquivo', async () => {
    const user = userEvent.setup();
    open(CREATE, {
      pack: { [CREATE]: entry(CREATE_TEXT), 'config/a.yml': entry('a: 1\n') },
    });
    await toForm(user);
    await user.click(await screen.findByRole('treeitem', { name: /a\.yml/ }));
    await screen.findByText('Este arquivo só pode ser editado como texto.');
    expect(
      screen.getByText(
        'O formulário funciona com TOML, JSON, JSON5, .properties, .cfg do Forge e options.txt.',
      ),
    ).toBeDefined();
    expect(screen.getByRole('radio', { name: 'Formulário' }).getAttribute('aria-disabled')).toBe(
      'true',
    );
    await screen.findByRole('textbox', { name: 'Conteúdo de config/a.yml' });

    // Voltando ao TOML, ele abre no formulário (o último modo escolhido).
    await user.click(screen.getByRole('treeitem', { name: /create-common\.toml/ }));
    await screen.findByRole('group', { name: `Campos de ${CREATE}` });
  });

  it('"Editar este trecho como texto" abre o texto', async () => {
    const user = userEvent.setup();
    open(CREATE, { pack: { [CREATE]: entry(CREATE_TEXT) } });
    const form = await toForm(user);
    expect(within(form).getByText('Lista de objetos: edite no texto. (linha 3)')).toBeDefined();
    await user.click(
      within(form).getByRole('button', { name: 'Editar este trecho como texto (kinetics.lista)' }),
    );
    await screen.findByRole('textbox', { name: `Conteúdo de ${CREATE}` });
    expect(screen.getByRole('radio', { name: 'Texto' }).getAttribute('aria-checked')).toBe('true');
    await waitFor(() => {
      expect(screen.getByText(/linha 3, coluna 1/)).toBeDefined();
    });
  });

  it('Desfazer as alterações no formulário volta ao texto salvo', async () => {
    const user = userEvent.setup();
    open(CREATE, { pack: { [CREATE]: entry(CREATE_TEXT) } });
    const form = await toForm(user);
    await user.click(within(form).getByRole('switch', { name: 'kinetics.enabled' }));
    await screen.findByText('1 linha alterada, ainda não salva');
    await user.click(screen.getByRole('button', { name: 'Desfazer as alterações' }));
    const dialog = await screen.findByRole('alertdialog');
    await user.click(within(dialog).getByRole('button', { name: 'Desfazer as alterações' }));
    await screen.findByText('Sem alterações');
    await waitFor(() => {
      expect(
        within(form).getByRole('switch', { name: 'kinetics.enabled' }).getAttribute('aria-checked'),
      ).toBe('true');
    });
  });

  it('"Não substituir se o jogador já tiver" grava e atualiza todas as telas do pack', async () => {
    const user = userEvent.setup();
    const { backend } = open('options.txt', { pack: { 'options.txt': entry('fov:0.5\n') } });
    const option = await screen.findByRole('checkbox', {
      name: /Não substituir se o jogador já tiver/,
    });
    expect(option).toHaveProperty('checked', false);
    const before = {
      tree: backend.callsOf('config_tree').length,
      row: backend.callsOf('pack_get').length,
      preserve: backend.callsOf('config_preserve').length,
      read: backend.callsOf('config_read').length,
    };
    await user.click(option);
    await screen.findByText('options.txt não substitui mais o do jogador.');

    expect(backend.callsOf('config_set_preserve')[0]?.args).toMatchObject({
      path: 'options.txt',
      preserve: true,
    });
    await waitFor(() => {
      expect(backend.callsOf('config_tree').length).toBeGreaterThan(before.tree);
      expect(backend.callsOf('pack_get').length).toBeGreaterThan(before.row);
      expect(backend.callsOf('config_preserve').length).toBeGreaterThan(before.preserve);
      expect(backend.callsOf('config_read').length).toBeGreaterThan(before.read);
    });
    await waitFor(() => {
      expect(
        screen.getByRole('checkbox', { name: /Não substituir se o jogador já tiver/ }),
      ).toHaveProperty('checked', true);
    });
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('arquivo fora do índice: a opção fica desligada com a explicação', async () => {
    open('options.txt', {
      pack: { 'options.txt': entry('fov:0.5\n') },
      preserve: { inIndex: false, preserve: false },
    });
    const option = await screen.findByRole('checkbox', {
      name: /Não substituir se o jogador já tiver/,
    });
    expect(option).toHaveProperty('disabled', true);
    expect(screen.getByText('Salve o arquivo uma vez para poder usar esta opção.')).toBeDefined();
  });

  it('"Copiar para defaultconfigs/" pede confirmação para substituir e devolve o foco', async () => {
    const user = userEvent.setup();
    const SERVER = 'config/create-server.toml';
    const { backend } = open(SERVER, {
      pack: { [SERVER]: entry('speed = 2\n') },
      copy: [
        { kind: 'needsOverwrite', target: 'defaultconfigs/create-server.toml' },
        { kind: 'copied', target: 'defaultconfigs/create-server.toml' },
      ],
    });
    const button = await screen.findByRole('button', { name: 'Copiar para defaultconfigs/' });
    const treeCalls = backend.callsOf('config_tree').length;
    await user.click(button);
    const dialog = await screen.findByRole('alertdialog', {
      name: 'Substituir defaultconfigs/create-server.toml?',
    });
    await user.click(within(dialog).getByRole('button', { name: 'Substituir' }));
    await screen.findByText('Copiado para defaultconfigs/create-server.toml.');
    expect(
      backend.callsOf('config_copy_to_defaultconfigs').map((call) => call.args.overwrite),
    ).toEqual([false, true]);
    await waitFor(() => {
      expect(backend.callsOf('config_tree').length).toBeGreaterThan(treeCalls);
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(
        screen.getByRole('button', { name: 'Copiar para defaultconfigs/' }),
      );
    });
    await screen.findByRole('treeitem', { name: /create-server\.toml/ });
  });

  it('com alterações não salvas, a cópia pede para salvar antes', async () => {
    const user = userEvent.setup();
    const SERVER = 'config/create-server.toml';
    const { backend } = open(SERVER, { pack: { [SERVER]: entry('enabled = true\n') } });
    const form = await toForm(user);
    await user.click(within(form).getByRole('switch', { name: 'kinetics.enabled' }));
    await screen.findByText('1 linha alterada, ainda não salva');
    const button = screen.getByRole('button', { name: 'Copiar para defaultconfigs/' });
    expect(button.getAttribute('aria-disabled')).toBe('true');
    await user.hover(button);
    expect(
      await screen.findByRole('tooltip', { name: 'Salve as alterações antes de copiar.' }),
    ).toBeDefined();
    await user.click(button);
    expect(backend.callsOf('config_copy_to_defaultconfigs')).toHaveLength(0);
  });
});
