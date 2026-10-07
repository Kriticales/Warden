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

import type { ConfigContent, ConfigFile } from '../../lib/ipc/bindings';
import { axePage } from '../../test/axe';
import { ipcError } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { editorBackend } from '../pack-editor/testing';
import { makeSettings } from '../settings/testing';

interface Disk {
  [path: string]: { text: string; hash: string; lineEnding?: ConfigContent['lineEnding'] };
}

let counter = 0;
function hashOf(text: string): string {
  counter += 1;
  return `h${String(counter)}-${String(text.length)}`;
}

function entry(text: string, lineEnding: ConfigContent['lineEnding'] = 'lf') {
  return { text, hash: hashOf(text), lineEnding };
}

function listed(path: string): ConfigFile {
  return { path, size: 10, modifiedAtMs: 1, binary: false };
}

interface Options {
  pack?: Disk;
  instance?: Disk | null;
  loader?: string;
  minecraft?: string;
  diff?: boolean;
  readOnly?: Record<string, ConfigContent['readOnly']>;
}

function openConfigs(path: string, options: Options = {}) {
  const disks = { pack: options.pack ?? {}, instance: options.instance ?? null } as Record<
    'pack' | 'instance',
    Disk | null
  >;
  const { backend, url } = editorBackend({
    pack: { loader: options.loader ?? 'forge', minecraft: options.minecraft ?? '1.20.1' },
    handlers: {
      settings_get: () => makeSettings({ configDiffBeforeSave: options.diff ?? true }),
      config_tree: (args) => {
        const disk = disks[args.origin as 'pack' | 'instance'];
        return {
          available: disk !== null,
          files: Object.keys(disk ?? {}).map(listed),
          truncated: false,
        };
      },
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
          path: args.path,
          text: file.text,
          hash: file.hash,
          size: file.text.length,
          lineEnding: file.lineEnding ?? 'lf',
          readOnly: reason,
          modifiedAtMs: 1,
        } satisfies ConfigContent;
      },
      config_write: (args) => {
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
  it('lista a árvore do pack, filtra por nome e abre o arquivo na URL; axe', async () => {
    const user = userEvent.setup();
    openConfigs('', {
      pack: {
        [CREATE]: entry(CREATE_TEXT),
        'config/jei/jei-client.ini': entry('a=1\n'),
        'options.txt': entry('fov:70\n'),
      },
    });
    await screen.findByRole('heading', { level: 1, name: 'Configs' }, { timeout: 10_000 });
    const tree = await screen.findByRole('tree', { name: 'Arquivos' });
    expect(within(tree).getByRole('treeitem', { name: /create-common\.toml/ })).toBeDefined();
    expect(
      within(tree).getByRole('treeitem', { name: 'Opções do jogo (options.txt)' }),
    ).toBeDefined();
    expect(screen.getByText('Escolha um arquivo')).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();

    await user.type(screen.getByRole('searchbox', { name: 'Filtrar por nome de arquivo' }), 'jei');
    // Com o filtro ligado a árvore é outra (tudo aberto): busca de novo.
    const filtered = await screen.findByRole('tree', { name: 'Arquivos' });
    expect(within(filtered).queryByRole('treeitem', { name: /create-common/ })).toBeNull();
    await user.click(await within(filtered).findByRole('treeitem', { name: /jei-client\.ini/ }));
    await screen.findByRole('textbox', { name: 'Conteúdo de config/jei/jei-client.ini' });
    expect(screen.getByText(/Propriedades · UTF-8 · LF · linha 1, coluna 1/)).toBeDefined();
  });

  it('mostra o estado vazio do pack sem configs e o da instância que não existe', async () => {
    const user = userEvent.setup();
    openConfigs('', { pack: {} });
    await screen.findByText('Este pack ainda não tem configs');
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Origem dos arquivos' }),
      'Mostrando: instância de teste',
    );
    await screen.findByText('Ainda não existe uma instância de teste');
    expect(screen.getByText('Você está editando a instância de teste, não o pack.')).toBeDefined();
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

  it('CA-T12-03: arquivo alterado por fora → aviso, nada é gravado sem escolha', async () => {
    const user = userEvent.setup();
    const { backend, disks } = openConfigs(`?arquivo=${CREATE}`, {
      pack: { [CREATE]: entry(CREATE_TEXT) },
      diff: false,
    });
    await type('\nmeu = 1');
    // Outro programa grava o arquivo; o Warden só descobre ao tentar salvar.
    const outside = '# Create\n[kinetics]\nmaxRotationSpeed = 999\n';
    disks.pack![CREATE] = entry(outside);
    await user.click(screen.getByRole('button', { name: 'Salvar' }));

    await screen.findByText('Este arquivo foi alterado fora do Warden.');
    expect(disks.pack?.[CREATE]?.text).toBe(outside);
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
    expect(disks.pack?.[CREATE]?.text).toBe(`${CREATE_TEXT}\nmeu = 1`);
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
    disks.pack![CREATE] = entry(outside);
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

  it('instância de teste: mostra o aviso fixo e grava na origem da instância', async () => {
    const user = userEvent.setup();
    const { backend } = openConfigs('?origem=instancia&arquivo=config/a.toml', {
      pack: {},
      instance: { 'config/a.toml': entry('a = 1\n') },
      diff: false,
    });
    await screen.findByText('Você está editando a instância de teste, não o pack.');
    await type('b = 2');
    await user.click(screen.getByRole('button', { name: 'Salvar' }));
    await screen.findByText('a.toml salvo.');
    expect(backend.callsOf('config_write')[0]?.args.origin).toBe('instance');
  });
});
