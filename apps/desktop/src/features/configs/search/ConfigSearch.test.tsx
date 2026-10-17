/**
 * Buscar em todas as configs (SPEC T12; C-05) na seção Configs, com o `config_search`
 * simulado:
 *
 * - CA-T12-06 (componente): a busca espera a última tecla, mostra "N resultados em M arquivos"
 *   agrupados por arquivo com os trechos destacados, e clicar abre o arquivo na linha da chave;
 * - leitura em andamento, sem resultado, instância que não existe, erro, filtro "Só deste mod",
 *   resultados cortados, Esc e Limpar busca;
 * - dados sempre atuais: o `config-index-changed` e o `pack-changed` de uma edição por fora
 *   (CA-T12-07) relêem a busca, a árvore, o arquivo aberto e o cabeçalho do pack;
 * - o editor com alterações não salvas continua montado enquanto os resultados aparecem.
 */
import { EditorView } from '@codemirror/view';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import type {
  ConfigSearchGroup,
  ConfigSearchHit,
  ConfigSearchResponse,
  IndexStatus,
  MarkedText,
} from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { makeSettings } from '../../settings/testing';

const CREATE = 'config/create-common.toml';
const CREATE_TEXT = '# Create\n[worldgen]\n\t#Desliga perto do spawn\n\tdisableWorldGen = false\n';
const SUPP = 'config/supplementaries-common.toml';
const SUPP_TEXT = 'bird_spawn = true\nother = 1\njar_spawn_rate = 0.05\n';

const READY: IndexStatus = {
  available: true,
  ready: true,
  indexedFiles: 2,
  totalFiles: 2,
  entries: 7,
};

/** Texto com `marked` destacado onde aparece. */
function mark(text: string, marked = 'spawn'): MarkedText {
  const at = text.toLowerCase().indexOf(marked);
  if (at < 0) return { parts: [{ text, marked: false }] };
  return {
    parts: [
      { text: text.slice(0, at), marked: false },
      { text: text.slice(at, at + marked.length), marked: true },
      { text: text.slice(at + marked.length), marked: false },
    ].filter((part) => part.text !== ''),
  };
}

function hit(line: number, key: string, value: string, comment?: string): ConfigSearchHit {
  return {
    line,
    field: 'key',
    key: mark(key),
    label: null,
    value: { parts: [{ text: value, marked: false }] },
    comment: comment ? mark(comment) : null,
    text: null,
    changed: false,
  };
}

const GROUPS: ConfigSearchGroup[] = [
  {
    path: SUPP,
    modId: 'supplementaries',
    hits: [hit(1, 'bird_spawn', 'true'), hit(3, 'jar_spawn_rate', '0.05')],
    total: 2,
  },
  {
    path: CREATE,
    modId: 'create',
    hits: [hit(4, 'worldgen.disableWorldGen', 'false', 'Desliga perto do spawn')],
    total: 1,
  },
];

interface Options {
  /** Resposta de `config_search` para cada busca (a vazia só prepara). */
  answer?: (args: Record<string, unknown>) => ConfigSearchResponse;
  instance?: boolean;
}

function results(groups: ConfigSearchGroup[], extra: Partial<ConfigSearchResponse> = {}) {
  return {
    status: READY,
    results: {
      groups,
      totalHits: groups.reduce((sum, group) => sum + group.total, 0),
      totalFiles: groups.length,
      truncated: false,
      mods: [
        { id: 'create', hits: 1 },
        { id: 'supplementaries', hits: 2 },
      ],
    },
    cancelled: false,
    ...extra,
  } satisfies ConfigSearchResponse;
}

function defaultAnswer(args: Record<string, unknown>): ConfigSearchResponse {
  if (args.query === '') return { status: READY, results: null, cancelled: false };
  const groups =
    args.modId === 'create' ? GROUPS.filter((group) => group.modId === 'create') : GROUPS;
  return results(args.query === 'spawn' ? groups : []);
}

function openConfigs(path = '', options: Options = {}) {
  const disk: Record<string, string> = { [CREATE]: CREATE_TEXT, [SUPP]: SUPP_TEXT };
  const { backend, url } = editorBackend({
    pack: { loader: 'forge', minecraft: '1.20.1' },
    handlers: {
      settings_get: () => makeSettings({ configDiffBeforeSave: true }),
      config_tree: (args) =>
        args.origin === 'instance' && !options.instance
          ? { available: false, files: [], truncated: false }
          : {
              available: true,
              files: Object.keys(disk).map((file) => ({
                path: file,
                size: 10,
                modifiedAtMs: 1,
                binary: false,
              })),
              truncated: false,
            },
      config_read: (args) => {
        const text = disk[args.path as string] ?? '';
        return {
          path: args.path as string,
          text,
          hash: `h-${String(text.length)}`,
          size: text.length,
          lineEnding: 'lf',
          readOnly: null,
          modifiedAtMs: 1,
        };
      },
      config_search: (args) => (options.answer ?? defaultAnswer)(args),
    },
  });
  renderApp(`${url}/configs${path}`);
  return { backend, url, disk };
}

function searchBox() {
  return screen.findByRole('searchbox', { name: 'Buscar em todas as configs' });
}

function searches(backend: ReturnType<typeof openConfigs>['backend']) {
  return backend.callsOf('config_search').map((call) => call.args.query);
}

async function editorView(): Promise<EditorView> {
  const textbox = await screen.findByRole('textbox', { name: /^Conteúdo de / });
  const view = EditorView.findFromDOM(textbox.closest<HTMLElement>('.cm-editor') ?? textbox);
  if (!view) throw new Error('editor do CodeMirror não encontrado');
  return view;
}

describe('Buscar em todas as configs (C-05)', () => {
  it('CA-T12-06: espera a última tecla, agrupa por arquivo, destaca e abre na linha; axe', async () => {
    const user = userEvent.setup();
    const { backend } = openConfigs();
    await screen.findByRole('treeitem', { name: /create-common\.toml/ });
    // O índice é preparado ao abrir a seção, antes da primeira tecla.
    await waitFor(() => {
      expect(searches(backend)).toContain('');
    });

    await user.type(await searchBox(), 'spawn');
    const title = await screen.findByRole('heading', {
      level: 2,
      name: '3 resultados em 2 arquivos para “spawn”',
    });
    // Uma busca só, depois da última tecla (nada de "s", "sp"…).
    expect(searches(backend).filter((query) => query !== '')).toEqual(['spawn']);

    const panel = title.closest('section');
    if (!panel) throw new Error('painel de resultados não encontrado');
    const supp = within(panel).getByRole('region', { name: SUPP });
    const create = within(panel).getByRole('region', { name: CREATE });
    expect(within(supp).getAllByRole('button')).toHaveLength(2);
    expect(within(create).getByRole('button').textContent).toContain('Desliga perto do spawn');
    const marks = [...panel.querySelectorAll('mark')].map((node) => node.textContent);
    expect(marks).toEqual(['spawn', 'spawn', 'spawn']);
    expect(await axePage(document.body)).toHaveNoViolations();

    // Clicar abre o arquivo na linha da chave, com o foco no editor.
    await user.click(within(create).getByRole('button', { name: /disableWorldGen/ }));
    await screen.findByRole('textbox', { name: `Conteúdo de ${CREATE}` });
    await screen.findByText(/linha 4, coluna 1/);
    const view = await editorView();
    await waitFor(() => {
      expect(view.hasFocus).toBe(true);
    });
    expect(view.state.doc.lineAt(view.state.selection.main.head).number).toBe(4);
    expect(screen.queryByRole('heading', { name: /resultados em/ })).toBeNull();

    // Voltar ao campo mostra os resultados de novo; outro resultado do mesmo arquivo também vai.
    await user.click(await searchBox());
    const again = await screen.findByRole('region', { name: SUPP });
    await user.click(within(again).getByRole('button', { name: /jar_spawn_rate/ }));
    await screen.findByText(/linha 3, coluna 1/);
  });

  it('mostra a leitura em andamento e busca de novo até o índice ficar pronto', async () => {
    const user = userEvent.setup();
    let calls = 0;
    const { backend } = openConfigs('', {
      answer: (args) => {
        if (args.query === '') return { status: READY, results: null, cancelled: false };
        calls += 1;
        return calls === 1
          ? results(GROUPS.slice(1), {
              status: { ...READY, ready: false, indexedFiles: 10, totalFiles: 40 },
            })
          : results(GROUPS);
      },
    });
    await user.type(await searchBox(), 'spawn');
    await screen.findByText(
      'Lendo as configs: 10 de 40 arquivos. Os resultados aparecem enquanto a leitura continua.',
    );
    await screen.findByRole('heading', { name: '1 resultado em 1 arquivo para “spawn”' });
    // Sem o evento: a situação "lendo" faz a tela perguntar de novo.
    await screen.findByRole('heading', { name: '3 resultados em 2 arquivos para “spawn”' });
    expect(screen.queryByText(/Lendo as configs/)).toBeNull();
    expect(searches(backend).filter((query) => query === 'spawn').length).toBeGreaterThan(1);
  });

  it('filtro "Só deste mod", resultados cortados e mais resultados no arquivo', async () => {
    const user = userEvent.setup();
    const { backend } = openConfigs('', {
      answer: (args) => {
        const answer = defaultAnswer(args);
        if (answer.results && args.modId === null) {
          answer.results.truncated = true;
          answer.results.groups = [{ ...GROUPS[0], total: 7 } as ConfigSearchGroup];
          answer.results.totalHits = 12;
          answer.results.totalFiles = 3;
        }
        return answer;
      },
    });
    await user.type(await searchBox(), 'spawn');
    await screen.findByRole('heading', { name: '12 resultados em 3 arquivos para “spawn”' });
    expect(screen.getByText('Mais 5 resultados neste arquivo')).toBeDefined();
    expect(
      screen.getByText(
        'Mostrando os primeiros 2 resultados. Use mais palavras ou o filtro por mod para ver o resto.',
      ),
    ).toBeDefined();

    const filter = screen.getByRole('combobox', { name: 'Só deste mod' });
    expect(
      within(filter)
        .getAllByRole('option')
        .map((option) => option.textContent),
    ).toEqual(['Todos os mods', 'Só do mod create', 'Só do mod supplementaries']);
    await user.selectOptions(filter, 'Só do mod create');
    await screen.findByRole('heading', { name: '1 resultado em 1 arquivo para “spawn”' });
    expect(backend.callsOf('config_search').at(-1)?.args.modId).toBe('create');
    expect(screen.queryByRole('region', { name: SUPP })).toBeNull();
  });

  it('sem resultado, Esc e Limpar busca', async () => {
    const user = userEvent.setup();
    openConfigs();
    const box = await searchBox();
    await user.type(box, 'zumbi');
    await screen.findByText('Nenhuma config com “zumbi”.');
    expect(
      screen.getByText(
        'Confira a grafia ou use menos palavras: cada palavra precisa aparecer na mesma chave, no comentário ou no valor dela.',
      ),
    ).toBeDefined();
    await user.keyboard('{Escape}');
    expect((box as HTMLInputElement).value).toBe('');
    expect(screen.queryByText(/Nenhuma config/)).toBeNull();
    expect(screen.getByText('Escolha um arquivo')).toBeDefined();

    await user.type(box, 'zumbi');
    await screen.findByText('Nenhuma config com “zumbi”.');
    // Dois "Limpar busca": no campo e no painel; o do painel limpa igual.
    const [, panelClear, ...rest] = screen.getAllByRole('button', { name: 'Limpar busca' });
    expect(rest).toHaveLength(0);
    if (!panelClear) throw new Error('Limpar busca do painel não encontrado');
    await user.click(panelClear);
    expect((box as HTMLInputElement).value).toBe('');
  });

  it('instância que ainda não existe e erro da busca com Tentar de novo', async () => {
    const user = userEvent.setup();
    let failures = 0;
    openConfigs('?origem=instancia', {
      answer: (args) => {
        if (args.origin === 'instance' && args.query !== '' && failures === 0) {
          failures += 1;
          return ipcError(makeAppError(undefined, { detail: 'disco' }));
        }
        return {
          status: { ...READY, available: false, totalFiles: 0, indexedFiles: 0 },
          results: args.query === '' ? null : results([]).results,
          cancelled: false,
        };
      },
    });
    await user.type(await searchBox(), 'spawn');
    await screen.findByText('Não foi possível buscar nas configs.');
    await user.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    await screen.findByText(
      'A instância de teste ainda não existe. Teste o pack uma vez para buscar nas configs dela.',
    );
  });

  it('edição por fora: o aviso do Rust relê a busca, a árvore, o arquivo e o cabeçalho', async () => {
    const user = userEvent.setup();
    const { backend, url } = openConfigs(`?arquivo=${CREATE}`);
    await screen.findByRole('textbox', { name: `Conteúdo de ${CREATE}` });
    await user.type(await searchBox(), 'spawn');
    await screen.findByRole('heading', { name: '3 resultados em 2 arquivos para “spawn”' });
    const before = {
      search: searches(backend).filter((query) => query === 'spawn').length,
      tree: backend.callsOf('config_tree').length,
      file: backend.callsOf('config_read').length,
      row: backend.callsOf('pack_get').length,
    };
    const packId = url.split('/').at(-1) ?? '';
    // O que o Rust emite quando um arquivo do pack muda por fora (configs_search.rs).
    await act(async () => {
      await backend.emit('config-index-changed', {
        packId,
        origin: 'pack',
        status: READY,
        changedFiles: 1,
        first: false,
      });
      await backend.emit('pack-changed', { packId, areas: [] });
    });
    await waitFor(() => {
      expect(searches(backend).filter((query) => query === 'spawn').length).toBeGreaterThan(
        before.search,
      );
      expect(backend.callsOf('config_tree').length).toBeGreaterThan(before.tree);
      expect(backend.callsOf('config_read').length).toBeGreaterThan(before.file);
      expect(backend.callsOf('pack_get').length).toBeGreaterThan(before.row);
    });

    // O índice pronto (sem mudança de arquivo) só relê a busca.
    const tree = backend.callsOf('config_tree').length;
    const count = searches(backend).filter((query) => query === 'spawn').length;
    await act(async () => {
      await backend.emit('config-index-changed', {
        packId,
        origin: 'pack',
        status: READY,
        changedFiles: 0,
        first: true,
      });
    });
    await waitFor(() => {
      expect(searches(backend).filter((query) => query === 'spawn').length).toBeGreaterThan(count);
    });
    expect(backend.callsOf('config_tree').length).toBe(tree);
  });

  it('várias chaves na mesma linha (tabela inline) aparecem todas, sem chave repetida', async () => {
    const user = userEvent.setup();
    const errors = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const inline: ConfigSearchGroup = {
      path: CREATE,
      modId: 'create',
      hits: [hit(2, 'spawn.spawnA', '1'), hit(2, 'spawn.spawnB', '2')],
      total: 2,
    };
    openConfigs('', { answer: (args) => results(args.query === 'spawn' ? [inline] : []) });
    await user.type(await searchBox(), 'spawn');
    const group = await screen.findByRole('region', { name: CREATE });
    expect(within(group).getAllByRole('button')).toHaveLength(2);
    const repeated = errors.mock.calls.some((call) => String(call[0]).includes('same key'));
    errors.mockRestore();
    expect(repeated).toBe(false);
  });

  it('o arquivo relido depois de uma edição por fora não volta o cursor nem o foco para a linha', async () => {
    const user = userEvent.setup();
    const { backend, url, disk } = openConfigs(`?arquivo=${CREATE}&linha=4`);
    await screen.findByText(/linha 4, coluna 1/);
    const box = await searchBox();
    await user.click(box);
    expect(document.activeElement).toBe(box);

    disk[CREATE] = `${CREATE_TEXT}novo = 1\n`;
    const packId = url.split('/').at(-1) ?? '';
    await act(async () => {
      await backend.emit('pack-changed', { packId, areas: [] });
    });
    await waitFor(async () => {
      expect((await editorView()).state.sliceDoc()).toContain('novo = 1');
    });
    const view = await editorView();
    expect(view.state.doc.lineAt(view.state.selection.main.head).number).toBe(1);
    expect(document.activeElement).toBe(box);
  });

  it('o editor com alterações não salvas continua montado enquanto a busca está aberta', async () => {
    const user = userEvent.setup();
    openConfigs(`?arquivo=${CREATE}`);
    const view = await editorView();
    act(() => {
      view.dispatch({ changes: { from: view.state.doc.length, insert: '# meu\n' } });
    });
    await screen.findByText('1 linha alterada, ainda não salva');

    await user.type(await searchBox(), 'spawn');
    await screen.findByRole('heading', { name: '3 resultados em 2 arquivos para “spawn”' });
    // Outra linha do mesmo arquivo: não sai dele, então não pergunta nada.
    await user.click(screen.getByRole('button', { name: /disableWorldGen/ }));
    await screen.findByText(/linha 4, coluna 1/);
    expect(screen.queryByText('Sair sem salvar?')).toBeNull();
    expect(screen.getByText('1 linha alterada, ainda não salva')).toBeDefined();
    expect((await editorView()).state.sliceDoc()).toContain('# meu');

    // Outro arquivo com alterações não salvas: a guarda da C-02 pergunta antes.
    await user.click(await searchBox());
    await user.click(await screen.findByRole('button', { name: /bird_spawn/ }));
    await screen.findByText('Sair sem salvar?');
  });
});
