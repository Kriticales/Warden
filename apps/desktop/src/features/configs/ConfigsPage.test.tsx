/**
 * Seção Configs (SPEC T12) no app inteiro, com um disco simulado que muda como o Rust:
 * `config_write` confere o `hash`, como a concorrência otimista de verdade.
 *
 * - árvore por origem, filtro por nome e arquivo aberto na URL;
 * - Salvar com diferenças (e sem, quando Configurações desliga), `Ctrl+S`, CRLF preservado;
 * - CA-T12-03: arquivo alterado fora do Warden → aviso com Recarregar, Ver diferenças e
 *   Sobrescrever, e nada é gravado sem escolha;
 * - guarda de alterações não salvas ao trocar de arquivo e de seção;
 * - avisos contextuais e arquivos só de leitura.
 */
import { EditorView } from '@codemirror/view';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type {
  ConfigComparison,
  ConfigContent,
  ConfigFile,
  KeyDifference,
} from '../../lib/ipc/bindings';
import { axePage } from '../../test/axe';
import { ipcError } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { editorBackend, overviewFromTree } from '../pack-editor/testing';
import { makeSettings } from '../settings/testing';

type Disk = Record<
  string,
  { text: string; hash: string; lineEnding?: ConfigContent['lineEnding'] }
>;

let counter = 0;
function hashOf(text: string): string {
  counter += 1;
  return `h${String(counter)}-${String(text.length)}`;
}

function entry(text: string, lineEnding: ConfigContent['lineEnding'] = 'lf') {
  return { text, hash: hashOf(text), lineEnding };
}

function listed(path: string, binary = false): ConfigFile {
  return { path, size: 10, modifiedAtMs: 1, binary };
}

interface Options {
  pack?: Disk;
  instance?: Disk | null;
  loader?: string;
  minecraft?: string;
  diff?: boolean;
  readOnly?: Record<string, ConfigContent['readOnly']>;
  /** A primeira leitura de `config_tree` falha. */
  treeErrorOnce?: boolean;
  /** `config_write` falha com um erro que não é de alteração por fora. */
  writeError?: boolean;
  /** Chaves diferentes entre o teste e o pack, por arquivo (`config_compare`). */
  differences?: Record<string, KeyDifference[]>;
}

function isBinary(options: Options, path: string): boolean {
  return options.readOnly?.[path] === 'binary';
}

function openConfigs(path: string, options: Options = {}) {
  const disks = { pack: options.pack ?? {}, instance: options.instance ?? null } as Record<
    'pack' | 'instance',
    Disk | null
  >;
  let treeFailures = 0;
  const tree = (args: Record<string, unknown>) => {
    if (options.treeErrorOnce && treeFailures === 0) {
      treeFailures += 1;
      return ipcError(makeAppError(undefined, { detail: 'disco' }));
    }
    const disk = disks[args.origin as 'pack' | 'instance'];
    return {
      available: disk !== null,
      files: Object.keys(disk ?? {}).map((path) => listed(path, isBinary(options, path))),
      truncated: false,
    };
  };
  const { backend, url } = editorBackend({
    pack: { loader: options.loader ?? 'forge', minecraft: options.minecraft ?? '1.20.1' },
    handlers: {
      settings_get: () => makeSettings({ configDiffBeforeSave: options.diff ?? true }),
      config_overview: async (args) => {
        const overview = await overviewFromTree(tree, args.packId);
        return {
          ...overview,
          files: overview.files.map((file) => {
            const keys = options.differences?.[file.path];
            return keys ? { ...file, differs: true, differences: keys.length } : file;
          }),
        };
      },
      config_compare: (args) =>
        ({
          path: String(args.path),
          byKey: true,
          keys: options.differences?.[String(args.path)] ?? [],
          lines: 0,
          packText: disks.pack?.[String(args.path)]?.text ?? null,
          testText: disks.instance?.[String(args.path)]?.text ?? null,
        }) satisfies ConfigComparison,
      config_bring: () => ({ hash: 'x', size: 1, changed: true }),
      config_revert: () => ({ hash: 'x', size: 1, changed: true }),
      config_mark_personal: () => 0,
      config_unmark_personal: () => true,
      config_keep_personal_out: (args) => (args.paths as string[]).length,
      config_tree: tree,
      config_read: (args) => {
        const disk = disks[args.origin as 'pack' | 'instance'] ?? {};
        const file = disk[args.path as string];
        if (!file) {
          return ipcError(
            makeAppError({ domain: 'project', code: 'CONFIG_NOT_FOUND' }, { detail: 'sumiu' }),
          );
        }
        const reason = options.readOnly?.[args.path as string] ?? null;
        return {
          path: args.path as string,
          text: reason === 'binary' ? null : file.text,
          hash: file.hash,
          size: file.text.length,
          lineEnding: file.lineEnding ?? 'lf',
          readOnly: reason,
          modifiedAtMs: 1,
        } satisfies ConfigContent;
      },
      config_write: (args) => {
        if (options.writeError) {
          return ipcError(makeAppError(undefined, { detail: 'disco' }));
        }
        const disk = disks[args.origin as 'pack' | 'instance'] ?? {};
        const file = disk[args.path as string];
        if (!file || file.hash !== args.expectedHash) {
          return ipcError(makeAppError({ domain: 'project', code: 'FILE_CHANGED_ON_DISK' }));
        }
        const text = args.text as string;
        if (text === file.text) {
          return { hash: file.hash, size: text.length, changed: false };
        }
        disk[args.path as string] = { ...file, text, hash: hashOf(text) };
        return { hash: disk[args.path as string]?.hash, size: text.length, changed: true };
      },
    },
  });
  renderApp(`${url}/configs${path}`);
  return { backend, disks, url };
}

const CREATE = 'config/create-common.toml';
const CREATE_TEXT = '# Create\n[kinetics]\nmaxRotationSpeed = 256\n';

async function editorText(): Promise<HTMLElement> {
  return screen.findByRole('textbox', { name: /^Conteúdo de / });
}

/** Acrescenta texto ao fim do documento, pela API do CodeMirror (o jsdom não digita nele). */
async function type(text: string) {
  const textbox = await editorText();
  const view = EditorView.findFromDOM(textbox.closest<HTMLElement>('.cm-editor') ?? textbox);
  if (!view) throw new Error('editor do CodeMirror não encontrado');
  act(() => {
    view.dispatch({ changes: { from: view.state.doc.length, insert: text } });
  });
}

describe('Configs (T12)', () => {
  it('lista a árvore do pack e abre o arquivo na URL; axe', async () => {
    const user = userEvent.setup();
    openConfigs('', {
      pack: {
        [CREATE]: entry(CREATE_TEXT),
        'config/jei/jei-client.ini': entry('a=1\n'),
      },
    });
    await screen.findByRole('heading', { level: 1, name: 'Configs' }, { timeout: 10_000 });
    // Sem instância de teste, a vista é o pack: só o que vai para os jogadores.
    const tree = await screen.findByRole('tree', { name: 'Configs que estão no pack' });
    expect(within(tree).getByRole('treeitem', { name: /create-common\.toml/ })).toBeDefined();
    expect(screen.getByRole('radio', { name: 'Do pack' }).getAttribute('aria-checked')).toBe(
      'true',
    );
    expect(screen.getByText('Escolha um arquivo')).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();

    await user.click(within(tree).getByRole('treeitem', { name: /^jei/ }));
    await user.click(await within(tree).findByRole('treeitem', { name: /jei-client\.ini/ }));
    await screen.findByRole('textbox', { name: 'Conteúdo de config/jei/jei-client.ini' });
    expect(screen.getByText(/Propriedades · UTF-8 · LF · linha 1, coluna 1/)).toBeDefined();
    // A faixa diz para onde o arquivo vai.
    expect(screen.getByText('Config do pack.')).toBeDefined();
  });

  it('mostra o estado vazio do pack sem configs; o teste local espera o primeiro teste', async () => {
    openConfigs('', { pack: {} });
    await screen.findByText('Este pack ainda não tem configs');
    const test = screen.getByRole('radio', { name: 'Do teste local' });
    expect(test.getAttribute('aria-disabled')).toBe('true');
    expect(test.getAttribute('title')).toBe(
      'O teste local aparece depois do primeiro teste do pack.',
    );
  });

  it('?origem=instancia sem instância: explica que ela nasce no primeiro teste', async () => {
    openConfigs('?origem=instancia', { pack: { [CREATE]: entry(CREATE_TEXT) } });
    await screen.findByText('Ainda não existe uma instância de teste');
  });

  it('estados de erro: lista que não se lê, arquivo que sumiu e gravação que falha', async () => {
    const user = userEvent.setup();
    openConfigs(`?arquivo=${CREATE}`, {
      pack: { [CREATE]: entry(CREATE_TEXT) },
      treeErrorOnce: true,
      writeError: true,
      diff: false,
    });
    // A lista falha e "Tentar de novo" a relê.
    await screen.findByText('Não foi possível ler os arquivos.');
    await user.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    await screen.findByRole('treeitem', { name: /create-common\.toml/ });

    // A gravação falha sem ser por alteração externa: o erro aparece e o texto continua.
    await type('# x');
    await user.click(screen.getByRole('button', { name: 'Salvar' }));
    await screen.findByText('Não foi possível salvar create-common.toml.');
    expect(screen.queryByText('Este arquivo foi alterado fora do Warden.')).toBeNull();
    expect(screen.getByText('1 linha alterada, ainda não salva')).toBeDefined();
  });

  it('arquivo que não existe mais mostra o erro no lugar do editor', async () => {
    openConfigs('?arquivo=config/sumiu.toml', { pack: { [CREATE]: entry(CREATE_TEXT) } });
    await screen.findByText('Não foi possível abrir config/sumiu.toml.');
    expect(screen.queryByRole('textbox', { name: /^Conteúdo de / })).toBeNull();
  });

  it('abre o arquivo, salva mostrando as diferenças e grava com o hash da leitura', async () => {
    const user = userEvent.setup();
    const { backend, disks } = openConfigs(`?arquivo=${CREATE}`, {
      pack: { [CREATE]: entry(CREATE_TEXT) },
    });
    await editorText();
    const save = screen.getByRole('button', { name: 'Salvar' });
    expect(save).toHaveProperty('disabled', true);
    expect(screen.getByText('Sem alterações')).toBeDefined();

    await type('\nextra = 1');
    await waitFor(() => {
      expect(screen.getByText(/alterad[ao]s?, ainda não salvas?/)).toBeDefined();
    });
    await user.click(screen.getByRole('button', { name: 'Salvar' }));
    const dialog = await screen.findByRole('dialog', { name: 'Salvar create-common.toml?' });
    expect(within(dialog).getByText(/extra = 1/)).toBeDefined();
    expect(backend.callsOf('config_write')).toHaveLength(0);
    await user.click(within(dialog).getByRole('button', { name: 'Salvar' }));

    await screen.findByText('create-common.toml salvo.');
    const call = backend.callsOf('config_write')[0];
    expect(call?.args.text).toBe(`${CREATE_TEXT}\nextra = 1`);
    expect(disks.pack?.[CREATE]?.text).toBe(`${CREATE_TEXT}\nextra = 1`);
    await screen.findByText('Sem alterações');
  });

  it('com a confirmação desligada, Salvar grava direto', async () => {
    const user = userEvent.setup();
    const { backend } = openConfigs(`?arquivo=${CREATE}`, {
      pack: { [CREATE]: entry(CREATE_TEXT) },
      diff: false,
    });
    await type('# fim');
    await user.click(screen.getByRole('button', { name: 'Salvar' }));
    await screen.findByText('create-common.toml salvo.');
    expect(backend.callsOf('config_write')).toHaveLength(1);
  });

  it('Ctrl+S grava uma vez só, dentro do editor e fora dele', async () => {
    const user = userEvent.setup();
    const { backend, disks } = openConfigs(`?arquivo=${CREATE}`, {
      pack: { [CREATE]: entry(CREATE_TEXT) },
      diff: false,
    });

    // Dentro do editor: o atalho chega primeiro ao CodeMirror e depois sobe até a janela.
    await type('# um');
    const textbox = await editorText();
    act(() => {
      textbox.focus();
    });
    await user.keyboard('{Control>}s{/Control}');
    await screen.findByText('Sem alterações');
    expect(backend.callsOf('config_write')).toHaveLength(1);
    expect(disks.pack?.[CREATE]?.text).toBe(`${CREATE_TEXT}# um`);

    // Fora do editor: a árvore com o foco.
    await type('\n# dois');
    act(() => {
      screen.getByRole('treeitem', { name: /create-common\.toml/ }).focus();
    });
    await user.keyboard('{Control>}s{/Control}');
    await screen.findByText('Sem alterações');
    expect(backend.callsOf('config_write')).toHaveLength(2);
    expect(disks.pack?.[CREATE]?.text).toBe(`${CREATE_TEXT}# um\n# dois`);
    expect(screen.queryByText('Este arquivo foi alterado fora do Warden.')).toBeNull();
  });

  it('preserva o fim de linha CRLF ao gravar (bytes do que não foi editado)', async () => {
    const user = userEvent.setup();
    const original = '# a\r\nx = 1\r\n';
    const { backend } = openConfigs(`?arquivo=${CREATE}`, {
      pack: { [CREATE]: entry(original, 'crlf') },
      diff: false,
    });
    await type('y = 2');
    await user.click(screen.getByRole('button', { name: 'Salvar' }));
    await screen.findByText('create-common.toml salvo.');
    expect(backend.callsOf('config_write')[0]?.args.text).toBe(`${original}y = 2`);
    expect(screen.getByText(/CRLF/)).toBeDefined();
  });

  it('texto colado com outro fim de linha entra com o fim de linha do arquivo', async () => {
    const user = userEvent.setup();
    const original = '# a\r\nx = 1\r\n';
    const { backend } = openConfigs(`?arquivo=${CREATE}`, {
      pack: { [CREATE]: entry(original, 'crlf') },
      diff: false,
    });
    const textbox = await editorText();
    const view = EditorView.findFromDOM(textbox.closest<HTMLElement>('.cm-editor') ?? textbox);
    if (!view) throw new Error('editor do CodeMirror não encontrado');
    act(() => {
      view.dispatch({ selection: { anchor: view.state.doc.length } });
    });
    // Colar de verdade: o evento passa pelo tratamento de colar do CodeMirror.
    const paste = new Event('paste', { bubbles: true, cancelable: true });
    Object.defineProperty(paste, 'clipboardData', {
      value: { getData: (type: string) => (type === 'text/plain' ? 'y = 2\nz = 3\n' : '') },
    });
    act(() => {
      textbox.dispatchEvent(paste);
    });
    await user.click(screen.getByRole('button', { name: 'Salvar' }));
    await screen.findByText('create-common.toml salvo.');
    expect(backend.callsOf('config_write')[0]?.args.text).toBe(`${original}y = 2\r\nz = 3\r\n`);
  });

  it('CA-T12-03: arquivo alterado por fora → aviso, nada é gravado sem escolha', async () => {
    const user = userEvent.setup();
    const { backend, disks } = openConfigs(`?arquivo=${CREATE}`, {
      pack: { [CREATE]: entry(CREATE_TEXT) },
      diff: false,
    });
    await type('\nmeu = 1');
    // Outro programa grava o arquivo; o Warden só descobre ao tentar salvar.
    const outside = '# Create\n[kinetics]\nmaxRotationSpeed = 999\n';
    disks.pack = { ...disks.pack, [CREATE]: entry(outside) };
    await user.click(screen.getByRole('button', { name: 'Salvar' }));

    await screen.findByText('Este arquivo foi alterado fora do Warden.');
    expect(disks.pack[CREATE]?.text).toBe(outside);
    expect(screen.getByRole('button', { name: 'Salvar' })).toHaveProperty('disabled', true);

    // Ver diferenças mostra o disco contra o texto da pessoa.
    await user.click(screen.getByRole('button', { name: 'Ver diferenças' }));
    const diff = await screen.findByRole('dialog', { name: 'Diferenças com o arquivo no disco' });
    expect(within(diff).getByText(/maxRotationSpeed = 999/)).toBeDefined();
    await user.click(within(diff).getByRole('button', { name: 'Voltar' }));

    // Sobrescrever pede confirmação e só então grava o texto da pessoa.
    await user.click(screen.getByRole('button', { name: 'Sobrescrever' }));
    const confirm = await screen.findByRole('alertdialog', {
      name: 'Sobrescrever create-common.toml?',
    });
    await user.click(within(confirm).getByRole('button', { name: 'Sobrescrever o arquivo' }));
    await screen.findByText('create-common.toml salvo.');
    expect(disks.pack[CREATE]?.text).toBe(`${CREATE_TEXT}\nmeu = 1`);
    expect(backend.callsOf('config_write').length).toBeGreaterThanOrEqual(2);
  });

  it('Recarregar descarta as alterações depois de confirmar', async () => {
    const user = userEvent.setup();
    const { disks } = openConfigs(`?arquivo=${CREATE}`, {
      pack: { [CREATE]: entry(CREATE_TEXT) },
      diff: false,
    });
    await type('\nmeu = 1');
    const outside = '# novo\n';
    disks.pack = { ...disks.pack, [CREATE]: entry(outside) };
    await user.click(screen.getByRole('button', { name: 'Salvar' }));
    await screen.findByText('Este arquivo foi alterado fora do Warden.');

    await user.click(screen.getByRole('button', { name: 'Recarregar' }));
    const confirm = await screen.findByRole('alertdialog', {
      name: 'Recarregar create-common.toml?',
    });
    await user.click(within(confirm).getByRole('button', { name: 'Recarregar e descartar' }));
    await waitFor(() => {
      expect(screen.queryByText('Este arquivo foi alterado fora do Warden.')).toBeNull();
    });
    expect((await editorText()).textContent).toContain('# novo');
    expect(screen.getByText('Sem alterações')).toBeDefined();
  });

  it('com alterações não salvas, trocar de arquivo pede confirmação', async () => {
    const user = userEvent.setup();
    const { backend } = openConfigs(`?arquivo=${CREATE}`, {
      pack: { [CREATE]: entry(CREATE_TEXT), 'config/outro.toml': entry('b = 1\n') },
      diff: false,
    });
    await type('\nx = 1');
    await user.click(screen.getByRole('treeitem', { name: /outro\.toml/ }));
    const ask = await screen.findByRole('alertdialog', { name: 'Sair sem salvar?' });
    expect(within(ask).getByText(/tem alterações que ainda não foram salvas/)).toBeDefined();

    // Continuar editando fica no mesmo arquivo.
    await user.click(within(ask).getByRole('button', { name: 'Continuar editando' }));
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(screen.getByRole('textbox', { name: `Conteúdo de ${CREATE}` })).toBeDefined();

    // Salvar e continuar grava e abre o outro.
    await user.click(screen.getByRole('treeitem', { name: /outro\.toml/ }));
    const again = await screen.findByRole('alertdialog', { name: 'Sair sem salvar?' });
    await user.click(within(again).getByRole('button', { name: 'Salvar e continuar' }));
    await screen.findByRole('textbox', { name: 'Conteúdo de config/outro.toml' });
    expect(backend.callsOf('config_write')).toHaveLength(1);
  });

  it('Descartar alterações segue para o outro arquivo sem gravar', async () => {
    const user = userEvent.setup();
    const { backend } = openConfigs(`?arquivo=${CREATE}`, {
      pack: { [CREATE]: entry(CREATE_TEXT), 'config/outro.toml': entry('b = 1\n') },
    });
    await type('\nx = 1');
    await user.click(screen.getByRole('treeitem', { name: /outro\.toml/ }));
    const ask = await screen.findByRole('alertdialog', { name: 'Sair sem salvar?' });
    await user.click(within(ask).getByRole('button', { name: 'Descartar alterações' }));
    await screen.findByRole('textbox', { name: 'Conteúdo de config/outro.toml' });
    expect(backend.callsOf('config_write')).toHaveLength(0);
  });

  it('mostra os avisos do Forge, do NeoForge, do serverconfig e do options.txt', async () => {
    openConfigs('?arquivo=config/mod-server.toml', {
      pack: { 'config/mod-server.toml': entry('a = 1\n') },
      loader: 'neoforge',
      minecraft: '1.21.1',
    });
    await editorText();
    const list = screen.getByRole('list', { name: 'Avisos sobre este arquivo' });
    expect(list.textContent).toContain('Este tipo de config vale por mundo.');
    expect(list.textContent).toContain('O jogo pode reescrever este arquivo');
    expect(list.textContent).toContain('Valores fora da faixa voltam ao padrão');
  });

  it('options.txt no pack avisa que substitui as preferências de quem joga', async () => {
    openConfigs('?arquivo=options.txt', {
      pack: { 'options.txt': entry('fov:70\n') },
      loader: 'fabric',
    });
    await editorText();
    expect(
      screen.getByText('Este arquivo substitui as preferências de quem já joga o pack.'),
    ).toBeDefined();
  });

  it('arquivo só de leitura abre sem Salvar e explica o motivo', async () => {
    openConfigs('?arquivo=config/grande.json', {
      pack: { 'config/grande.json': entry('{}\n') },
      readOnly: { 'config/grande.json': 'tooLarge' },
    });
    await editorText();
    expect(screen.getByText(/Arquivo acima de 2 MB: abre só para leitura/)).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Salvar' })).toBeNull();
  });

  it('mostra os demais arquivos do pack; servers.dat aparece como binário, sem edição', async () => {
    const user = userEvent.setup();
    openConfigs('', {
      pack: {
        [CREATE]: entry(CREATE_TEXT),
        'notas.txt': entry('lembrar\n'),
        'patchouli_books/guia/livro.json': entry('{}\n'),
        'servers.dat': entry('\u0000'),
      },
      readOnly: { 'servers.dat': 'binary' },
    });
    const tree = await screen.findByRole(
      'tree',
      { name: 'Configs que estão no pack' },
      { timeout: 10_000 },
    );
    expect(within(tree).getByRole('treeitem', { name: /notas\.txt/ })).toBeDefined();
    expect(within(tree).getByRole('treeitem', { name: /patchouli_books\/guia/ })).toBeDefined();
    const servers = within(tree).getByRole('treeitem', { name: /servers\.dat/ });
    expect(servers.textContent).toContain('binário, sem edição');
    expect(within(tree).getByRole('treeitem', { name: /notas\.txt/ }).textContent).not.toContain(
      'binário',
    );

    await user.click(servers);
    expect(
      await screen.findAllByText(
        'Arquivo binário (como servers.dat): o Warden não mostra nem edita.',
      ),
    ).not.toHaveLength(0);
    expect(screen.queryByRole('textbox', { name: /^Conteúdo de / })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Salvar' })).toBeNull();

    // Os de texto abrem no editor.
    await user.click(within(tree).getByRole('treeitem', { name: /notas\.txt/ }));
    await screen.findByRole('textbox', { name: 'Conteúdo de notas.txt' });
  });

  it('instância de teste: a faixa diz que é só do teste e grava na origem da instância', async () => {
    const user = userEvent.setup();
    const { backend } = openConfigs('?origem=instancia&arquivo=config/a.toml', {
      pack: {},
      instance: { 'config/a.toml': entry('a = 1\n') },
      diff: false,
    });
    await screen.findByText('Só no teste local.');
    await type('b = 2');
    await user.click(screen.getByRole('button', { name: 'Salvar' }));
    await screen.findByText('a.toml salvo.');
    expect(backend.callsOf('config_write')[0]?.args.origin).toBe('instance');
  });
});

/** A faixa do lugar que tem esse texto. */
function bandOf(text: string): HTMLElement {
  const band = screen.getByText(text).closest<HTMLElement>('.band');
  if (!band) throw new Error(`faixa com "${text}" não encontrada`);
  return band;
}

const DH = 'config/DistantHorizons.toml';
const DH_PACK = '[quality]\nlodDistance = 256\n[ssao]\nenableSsao = true\n';
const DH_TEST = '[quality]\nlodDistance = 128\n[ssao]\nenableSsao = false\n';
const DH_KEYS: KeyDifference[] = [
  { key: 'quality.lodDistance', pack: '256', test: '128', perKey: true },
  { key: 'ssao.enableSsao', pack: 'true', test: 'false', perKey: true },
];

/** Um pack testado: as configs do pack, as que o jogo criou e o options.txt pessoal. */
function tested(path = '') {
  return openConfigs(path, {
    pack: {
      [DH]: entry(DH_PACK),
      'config/crash_assistant/config.toml': entry('[general]\nx = 1\n'),
    },
    instance: {
      [DH]: entry(DH_TEST),
      'config/crash_assistant/config.toml': entry('[general]\nx = 1\n'),
      [CREATE]: entry(CREATE_TEXT),
      'options.txt': entry('renderDistance:12\n'),
    },
    differences: { [DH]: DH_KEYS },
    diff: false,
  });
}

describe('Configs em três grupos (P9, P7; ADR-0067)', () => {
  it('o teste local junta o pack, o que o jogo criou e as preferências pessoais; axe', async () => {
    tested();
    // Com a instância, a seção abre no teste local, onde os mods criam as configs.
    const pack = await screen.findByRole(
      'tree',
      { name: 'Configs que estão no pack' },
      { timeout: 10_000 },
    );
    expect(screen.getByRole('radio', { name: 'Do teste local' }).getAttribute('aria-checked')).toBe(
      'true',
    );
    const dh = within(pack).getByRole('treeitem', { name: /DistantHorizons\.toml/ });
    expect(dh.textContent).toContain('2 chaves diferentes do pack');
    const onlyTest = screen.getByRole('tree', { name: 'Configs que estão só no teste' });
    expect(within(onlyTest).getByRole('treeitem', { name: /create-common\.toml/ })).toBeDefined();
    const personal = screen.getByRole('tree', { name: 'Preferências pessoais' });
    expect(within(personal).getByRole('treeitem', { name: /Opções do jogo/ })).toBeDefined();
    // As pessoais não estão em nenhum outro grupo.
    expect(within(onlyTest).queryByRole('treeitem', { name: /options\.txt/ })).toBeNull();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('"Diferente do pack" e as setas do teclado andam pela árvore', async () => {
    const user = userEvent.setup();
    tested();
    await screen.findByRole('tree', { name: 'Configs que estão no pack' }, { timeout: 10_000 });
    const filter = screen.getByRole('button', { name: /Só o que difere do pack/ });
    expect(filter.textContent).toContain('1');
    await user.click(filter);
    expect(filter.getAttribute('aria-pressed')).toBe('true');
    expect(screen.queryByRole('treeitem', { name: /crash_assistant/ })).toBeNull();
    expect(screen.queryByRole('treeitem', { name: /create-common/ })).toBeNull();
    expect(screen.getByRole('treeitem', { name: /DistantHorizons/ })).toBeDefined();
    await user.click(filter);

    // Um item só fica no Tab; as setas andam entre os itens.
    const first = screen.getAllByRole('treeitem')[0];
    expect(first?.tabIndex).toBe(0);
    first?.focus();
    await user.keyboard('{ArrowDown}');
    expect(document.activeElement).toBe(screen.getAllByRole('treeitem')[1]);
  });

  it('teste diferente do pack: faixa, Trazer só as chaves e Detalhes com as diferenças', async () => {
    const user = userEvent.setup();
    const { backend } = tested(`?origem=instancia&arquivo=${DH}`);
    await screen.findByText(/2 chaves estão diferentes do pack/, undefined, { timeout: 10_000 });
    const details = screen.getByRole('complementary', { name: 'Detalhes' });
    expect(within(details).getByText('quality.lodDistance')).toBeDefined();
    expect(within(details).getByText('2 diferenças do pack')).toBeDefined();
    // O Texto marca as linhas diferentes do pack.
    await waitFor(() => {
      expect(document.querySelectorAll('.cm-packdiff').length).toBe(2);
    });

    await user.click(screen.getByRole('button', { name: 'Trazer as 2 para o pack' }));
    await waitFor(() => {
      expect(backend.callsOf('config_bring')).toHaveLength(1);
    });
    expect(backend.callsOf('config_bring')[0]?.args).toMatchObject({
      path: DH,
      keys: ['quality.lodDistance', 'ssao.enableSsao'],
    });
    await screen.findByText('DistantHorizons.toml foi para o pack.');

    // Detalhes recolhe pelo botão.
    await user.click(screen.getByRole('button', { name: 'Detalhes' }));
    expect(screen.queryByRole('complementary', { name: 'Detalhes' })).toBeNull();
  });

  it('Comparar: teste e pack lado a lado, → traz a chave e ← volta ao do pack', async () => {
    const user = userEvent.setup();
    const { backend } = tested(`?origem=comparar&arquivo=${DH}`);
    const region = await screen.findByRole(
      'region',
      { name: 'Comparar o teste local com o pack' },
      { timeout: 10_000 },
    );
    expect(
      within(region).getByText('2 diferenças entre o seu teste local e o pack.'),
    ).toBeDefined();
    await user.click(
      within(region).getByRole('button', { name: 'Trazer quality.lodDistance para o pack' }),
    );
    await waitFor(() => {
      expect(backend.callsOf('config_bring')[0]?.args.keys).toEqual(['quality.lodDistance']);
    });
    await user.click(
      within(region).getByRole('button', { name: 'Voltar ssao.enableSsao ao do pack' }),
    );
    await waitFor(() => {
      expect(backend.callsOf('config_revert')[0]?.args.keys).toEqual(['ssao.enableSsao']);
    });
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('só no teste: Trazer para o pack e Marcar como pessoal (com confirmação)', async () => {
    const user = userEvent.setup();
    const { backend } = tested(`?origem=instancia&arquivo=${CREATE}`);
    await screen.findByText('Só no teste local.', undefined, { timeout: 10_000 });
    const band = bandOf('Só no teste local.');
    await user.click(within(band).getByRole('button', { name: 'Trazer para o pack' }));
    await waitFor(() => {
      expect(backend.callsOf('config_bring')[0]?.args).toMatchObject({ path: CREATE, keys: null });
    });

    await user.click(within(band).getByRole('button', { name: 'Marcar como pessoal' }));
    const ask = await screen.findByRole('alertdialog', {
      name: 'Marcar create-common.toml como pessoal?',
    });
    await user.click(within(ask).getByRole('button', { name: 'Marcar como pessoal' }));
    await waitFor(() => {
      expect(backend.callsOf('config_mark_personal')[0]?.args.path).toBe(CREATE);
    });
  });

  it('preferência pessoal: faixa própria e Deixar de ser pessoal…', async () => {
    const user = userEvent.setup();
    const { backend } = tested('?origem=instancia&arquivo=options.txt');
    await screen.findByText('Preferência pessoal.', undefined, { timeout: 10_000 });
    const band = bandOf('Preferência pessoal.');
    await user.click(within(band).getByRole('button', { name: 'Deixar de ser pessoal…' }));
    const ask = await screen.findByRole('alertdialog', {
      name: 'options.txt deixa de ser pessoal?',
    });
    await user.click(within(ask).getByRole('button', { name: 'Deixar de ser pessoal' }));
    await waitFor(() => {
      expect(backend.callsOf('config_unmark_personal')[0]?.args.path).toBe('options.txt');
    });
  });

  it('migração: o options.txt no pack é perguntado ao abrir o pack; nada sai sem escolha', async () => {
    const user = userEvent.setup();
    const { backend } = openConfigs('', {
      pack: { 'options.txt': entry('fov:70\n'), [CREATE]: entry(CREATE_TEXT) },
    });
    const dialog = await screen.findByRole(
      'dialog',
      { name: 'O options.txt agora fica só neste computador' },
      { timeout: 10_000 },
    );
    expect(backend.callsOf('config_keep_personal_out')).toHaveLength(0);
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Tirar do pack e guardar aqui' }));
    await waitFor(() => {
      expect(backend.callsOf('config_keep_personal_out')[0]?.args.paths).toEqual(['options.txt']);
    });
    await screen.findByText('1 arquivo saiu do pack e ficou guardado neste computador.');
  });
});
