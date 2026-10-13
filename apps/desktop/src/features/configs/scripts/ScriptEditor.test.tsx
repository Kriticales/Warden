/**
 * Editor de scripts KubeJS e CraftTweaker dentro de Configs (SPEC T26; C-07), no app inteiro
 * com o backend simulado:
 *
 * - CA-T26-01: pack Forge 1.20.1 com KubeJS 6, `kubejs/server_scripts/receitas.js` abre com a
 *   barra "KubeJS 6 · Forge 1.20.1", os trechos prontos da versão e o autocompletar de IDs
 *   (`'minecraft:copper_` lista `minecraft:copper_ingot`);
 * - CA-T26-02: o painel Erros dos scripts mostra o erro da linha 16 e clicar leva à linha (e a
 *   outro arquivo, quando o erro é dele);
 * - CA-T26-03 (interface): com o servidor web do KubeJS 7, Recarregar no jogo chama
 *   `scripts_reload`, avisa e relê os erros; o token nunca chega à interface;
 * - CA-T26-04: com o KubeJS 6 e só o cliente aberto, mostra `/kubejs reload server_scripts` com
 *   Copiar; com o servidor local, mostra a resposta do servidor;
 * - sem jogo aberto, Recarregar fica indisponível com o motivo; Abrir no VS Code;
 * - Salvar e recarregar na instância de teste; aviso no pack com o jogo aberto;
 * - pack sem KubeJS: o arquivo abre como texto comum; CraftTweaker com ZenScript.
 */
import { startCompletion } from '@codemirror/autocomplete';
import { EditorView } from '@codemirror/view';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type {
  ConfigContent,
  GameState,
  PackId,
  ReloadOutcome,
  ReloadRoute,
  ScriptIds,
  ScriptProblem,
  ScriptProblems,
  ScriptsInfo,
} from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { makeSettings } from '../../settings/testing';
import { makeGame } from '../../test/test.fixtures';
import { useTestStore } from '../../test/store';

const RECEITAS = 'kubejs/server_scripts/receitas.js';
const TAGS = 'kubejs/server_scripts/tags.js';
const ZS = 'scripts/receitas.zs';
const TOKEN = 'dG9rZW4tc2VjcmV0by1kby1rdWJlanM';

const RECEITAS_TEXT = [
  '// Receitas do Vale Sereno',
  'ServerEvents.recipes(event => {',
  "  event.shaped('minecraft:copper_block', [",
  "    'CCC',",
  "    'CCC',",
  "    'CCC'",
  '  ], {',
  "    C: 'minecraft:copper_ingot'",
  '  })',
  '',
  '  // Para-raios',
  "  event.shapeless('minecraft:lightning_rod', ['minecraft:copper_ingot'])",
  '',
  '  // Lingotes de volta',
  '  ',
  "  event.shapeless('9x minecraft:copper_ingott', ['minecraft:copper_block'])",
  '})',
  '',
].join('\n');

const KUBEJS_6: ScriptsInfo['env']['kubejs'] = {
  tool: 'kubejs',
  major: 6,
  version: '2001.6.5-build.26',
};

function snippetsKubejs6(): ScriptsInfo['kubejsSnippets'] {
  return [
    {
      id: 'receita-com-forma',
      scriptType: 'server',
      body: "ServerEvents.recipes(event => {\n  event.shaped('${minecraft:stone}', ['A'], { A: '${minecraft:cobblestone}' })\n})\n",
    },
    {
      id: 'receita-sem-forma',
      scriptType: 'server',
      body: "ServerEvents.recipes(event => {\n  event.shapeless('${minecraft:stone}', [])\n})\n",
    },
    { id: 'remover-receita', scriptType: 'server', body: 'x' },
    { id: 'tag-de-item', scriptType: 'server', body: 'x' },
    { id: 'item-novo', scriptType: 'startup', body: 'x' },
    { id: 'dica-de-item', scriptType: 'client', body: 'x' },
  ];
}

const IDS: ScriptIds = {
  groups: [
    {
      source: 'Minecraft',
      items: ['minecraft:copper_block', 'minecraft:copper_ingot', 'minecraft:stone'],
      blocks: ['minecraft:copper_block', 'minecraft:copper_ore'],
      itemTags: ['forge:ingots/copper'],
      blockTags: [],
      fluidTags: [],
      recipes: [],
    },
  ],
  unread: [],
};

function problem(file: string | null, line: number | null, message: string): ScriptProblem {
  return {
    level: 'error',
    file,
    line,
    column: null,
    message,
    logLine: `[06:03:25] [ERROR] ! ${file?.split('/').pop() ?? '?'}#${String(line)}: ${message}`,
    logFile: 'logs/kubejs/server.log',
  };
}

interface Options {
  info?: Partial<ScriptsInfo>;
  files?: Record<string, string>;
  instanceFiles?: Record<string, string> | null;
  problems?: ScriptProblems;
  route?: ReloadRoute;
  outcome?: (path: string) => ReloadOutcome;
  game?: GameState['state'] | null;
  loader?: string;
  /** `scripts_open_vscode` falha sem o VS Code. */
  noVsCode?: boolean;
}

function openScript(search: string, options: Options = {}) {
  const disks = {
    pack: { ...(options.files ?? { [RECEITAS]: RECEITAS_TEXT, [TAGS]: 'a\nb\nc\nd\n' }) },
    instance: options.instanceFiles === undefined ? null : options.instanceFiles,
  } as Record<'pack' | 'instance', Record<string, string> | null>;
  const hashes: Record<string, number> = {};
  const hashOf = (origin: string, path: string) =>
    `${origin}:${path}:${String(hashes[`${origin}:${path}`] ?? 0)}`;
  let packId = '' as PackId;
  const { backend, url, row } = editorBackend({
    pack: { loader: options.loader ?? 'forge', minecraft: '1.20.1' },
    handlers: {
      settings_get: () => makeSettings({ configDiffBeforeSave: false }),
      test_game_state: () => (options.game ? makeGame(packId, options.game) : null),
      config_tree: (args) => {
        const disk = disks[args.origin as 'pack' | 'instance'];
        return {
          available: disk !== null,
          files: Object.keys(disk ?? {}).map((path) => ({
            path,
            size: 10,
            modifiedAtMs: 1,
            binary: false,
          })),
          truncated: false,
        };
      },
      config_read: (args) => {
        const origin = args.origin as 'pack' | 'instance';
        const path = args.path as string;
        const text = disks[origin]?.[path];
        if (text === undefined) {
          return ipcError(makeAppError({ domain: 'project', code: 'CONFIG_NOT_FOUND' }));
        }
        return {
          path,
          text,
          hash: hashOf(origin, path),
          size: text.length,
          lineEnding: 'lf',
          readOnly: null,
          modifiedAtMs: 1,
        } satisfies ConfigContent;
      },
      config_write: (args) => {
        const origin = args.origin as 'pack' | 'instance';
        const path = args.path as string;
        const disk = disks[origin] ?? {};
        disk[path] = args.text as string;
        hashes[`${origin}:${path}`] = (hashes[`${origin}:${path}`] ?? 0) + 1;
        return { hash: hashOf(origin, path), size: 1, changed: true };
      },
      scripts_info: () =>
        ({
          env: { kubejs: KUBEJS_6, crafttweaker: null },
          kubejsSnippets: snippetsKubejs6(),
          crafttweakerSnippets: [],
          instanceReady: true,
          ...options.info,
        }) satisfies ScriptsInfo,
      scripts_ids: () => IDS,
      scripts_problems: () =>
        options.problems ?? {
          problems: [],
          logs: [],
          live: false,
        },
      scripts_reload_route: () =>
        options.route ?? (options.game === 'running' ? copy() : { kind: 'noGame' }),
      scripts_reload: (args) =>
        options.outcome?.(args.path as string) ?? {
          route: options.route ?? { kind: 'noGame' },
          response: [],
        },
      scripts_open_vscode: () =>
        options.noVsCode
          ? ipcError(makeAppError({ domain: 'scripts', code: 'VS_CODE_NOT_FOUND' }))
          : null,
    },
  });
  packId = row.id;
  if (options.game) {
    useTestStore.setState({ game: makeGame(row.id, options.game) });
  }
  renderApp(`${url}/configs${search}`);
  return { backend, disks, row };
}

function copy(): ReloadRoute {
  return { kind: 'copyCommand', command: '/kubejs reload server_scripts' };
}

async function view(name = RECEITAS): Promise<EditorView> {
  const textbox = await screen.findByRole(
    'textbox',
    { name: `Conteúdo de ${name}` },
    { timeout: 10_000 },
  );
  const found = EditorView.findFromDOM(textbox.closest<HTMLElement>('.cm-editor') ?? textbox);
  if (!found) throw new Error('editor do CodeMirror não encontrado');
  return found;
}

beforeEach(() => {
  useTestStore.setState({ game: null, runs: {}, packChanged: {} });
});

describe('editor de scripts (T26)', () => {
  it('CA-T26-01: barra com a versão, trechos da versão e IDs dos jars no autocompletar; axe', async () => {
    const user = userEvent.setup();
    openScript(`?arquivo=${RECEITAS}`);
    const editor = await view();
    await screen.findByText('KubeJS 6 · Forge 1.20.1');
    expect(
      screen.getByText(/JavaScript · KubeJS 6 · UTF-8 · LF · linha 1, coluna 1/),
    ).toBeDefined();

    // Trechos prontos: só os de server_scripts desta versão.
    await user.click(screen.getByRole('button', { name: 'Trechos prontos' }));
    const menu = await screen.findByRole('menu');
    expect(
      within(menu)
        .getAllByRole('menuitem')
        .map((item) => item.textContent),
    ).toEqual(['Receita com forma', 'Receita sem forma', 'Remover receita', 'Tag de item']);
    await user.click(within(menu).getByRole('menuitem', { name: 'Receita com forma' }));
    await waitFor(() => {
      expect(editor.state.sliceDoc()).toContain(
        "event.shaped('minecraft:stone', ['A'], { A: 'minecraft:cobblestone' })",
      );
    });

    // Autocompletar: `'minecraft:copper_` lista os IDs dos jars do pack.
    await waitFor(() => {
      expect(screen.queryByText('Lendo os logs…')).toBeNull();
    });
    act(() => {
      editor.dispatch({
        changes: { from: editor.state.doc.length, insert: "\n'minecraft:copper_" },
        selection: { anchor: editor.state.doc.length + "\n'minecraft:copper_".length },
      });
      startCompletion(editor);
    });
    const list = await screen.findByRole('listbox', {}, { timeout: 5000 });
    const labels = within(list)
      .getAllByRole('option')
      .map((option) => option.querySelector('.cm-completionLabel')?.textContent);
    expect(labels).toContain('minecraft:copper_ingot');
    expect(labels).toContain('minecraft:copper_ore');
    expect(labels).not.toContain('minecraft:stone');
    expect(list.textContent).toContain('item');

    act(() => {
      editor.contentDOM.blur();
    });
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T26-02: o painel mostra o erro da linha 16 e clicar leva à linha e a outro arquivo', async () => {
    const user = userEvent.setup();
    openScript(`?arquivo=${RECEITAS}`, {
      problems: {
        problems: [
          problem(TAGS, 3, 'TypeError: Cannot find function add'),
          problem(
            RECEITAS,
            16,
            "Failed to create recipe for type 'kubejs:shapeless': ItemStack 'result' can't be empty!",
          ),
          { ...problem(null, null, 'Error parsing recipe minecraft:kjs/x'), level: 'warning' },
        ],
        logs: [{ file: 'logs/kubejs/server.log', modifiedMs: Date.now() }],
        live: false,
      },
    });
    await view();
    const panel = await screen.findByRole('region', { name: 'Erros dos scripts' });
    expect(
      within(panel).getByText(/do último teste, hoje às .* · lido de logs\/kubejs\/server\.log/),
    ).toBeDefined();
    const items = within(panel).getAllByRole('listitem');
    // Os do arquivo aberto primeiro.
    expect(items[0]?.textContent).toContain('receitas.js, linha 16');
    expect(items[0]?.textContent).toContain('ItemStack');
    expect(items[0]?.querySelector('pre')?.textContent).toContain('receitas.js#16');
    expect(items[2]?.textContent).toContain('Aviso');

    // A linha 16 fica marcada no editor.
    await waitFor(() => {
      expect(document.querySelectorAll('.cm-line.cm-script-error')).toHaveLength(1);
    });

    await user.click(within(panel).getByRole('button', { name: 'receitas.js, linha 16' }));
    await screen.findByText(/linha 16, coluna 1/);

    // Erro de outro script: abre o arquivo na linha.
    await user.click(within(panel).getByRole('button', { name: 'tags.js, linha 3' }));
    await view(TAGS);
    await screen.findByText(/linha 3, coluna 1/);
  });

  it('CA-T26-04: com o KubeJS 6 e só o cliente aberto, mostra o comando com Copiar', async () => {
    const user = userEvent.setup();
    const writeText = vi.spyOn(navigator.clipboard, 'writeText');
    const { backend } = openScript(`?arquivo=${RECEITAS}`, {
      game: 'running',
      route: copy(),
      outcome: () => ({ route: copy(), response: [] }),
    });
    await view();
    const reload = await screen.findByRole('button', { name: 'Recarregar no jogo' });
    await waitFor(() => {
      expect(reload.getAttribute('aria-disabled')).toBeNull();
    });
    await user.click(reload);
    await screen.findByText('No KubeJS 6, recarregar precisa de um comando no jogo.');
    expect(backend.callsOf('scripts_reload')[0]?.args).toMatchObject({ path: RECEITAS });
    await user.click(screen.getByRole('button', { name: 'Copiar /kubejs reload server_scripts' }));
    expect(writeText).toHaveBeenCalledWith('/kubejs reload server_scripts');
    await screen.findByText('Comando copiado');
    await user.click(screen.getByRole('button', { name: 'Fechar o aviso' }));
    expect(screen.queryByText('No KubeJS 6, recarregar precisa de um comando no jogo.')).toBeNull();
  });

  it('CA-T26-04: com o servidor local aberto, envia pelo console e mostra a resposta', async () => {
    const user = userEvent.setup();
    const route: ReloadRoute = { kind: 'serverConsole', command: 'kubejs reload server_scripts' };
    openScript(`?arquivo=${RECEITAS}`, {
      game: 'running',
      route,
      outcome: () => ({ route, response: ['[15:12:03] [Server thread/INFO]: Reloading!'] }),
    });
    await view();
    await user.click(await screen.findByRole('button', { name: 'Recarregar no jogo' }));
    await screen.findByText('Scripts recarregados no servidor');
    expect(screen.getByText(/O Warden enviou kubejs reload server_scripts\./)).toBeDefined();
    expect(screen.getByText(/Reloading!/)).toBeDefined();
  });

  it('CA-T26-03 (interface): KubeJS 7 pelo servidor web relê os erros; o token não aparece', async () => {
    const user = userEvent.setup();
    const route: ReloadRoute = { kind: 'webServer', target: 'server' };
    const { backend } = openScript(`?arquivo=${RECEITAS}`, {
      game: 'running',
      info: {
        env: { kubejs: { tool: 'kubejs', major: 7, version: '2101.7.2' }, crafttweaker: null },
      },
      route,
      outcome: () => ({ route, response: [] }),
      problems: { problems: [], logs: [], live: true },
    });
    await view();
    await screen.findByText('KubeJS 7 · Forge 1.20.1');
    await screen.findByText('ao vivo, do jogo aberto');
    const before = backend.callsOf('scripts_problems').length;
    await user.click(await screen.findByRole('button', { name: 'Recarregar no jogo' }));
    await screen.findByText('Scripts recarregados no jogo');
    await waitFor(() => {
      expect(backend.callsOf('scripts_problems').length).toBeGreaterThan(before);
    });
    expect(document.body.innerHTML).not.toContain(TOKEN);
  });

  it('sem jogo aberto: Recarregar fica indisponível com o motivo; Abrir no VS Code', async () => {
    const user = userEvent.setup();
    const { backend } = openScript(`?arquivo=${RECEITAS}`);
    await view();
    const reload = await screen.findByRole('button', {
      name: 'Recarregar no jogo',
      description: 'Abra o jogo pelo Testar para recarregar os scripts sem reiniciar.',
    });
    expect(reload.getAttribute('aria-disabled')).toBe('true');
    await user.click(reload);
    expect(backend.callsOf('scripts_reload')).toHaveLength(0);

    await user.click(screen.getByRole('button', { name: 'Abrir no VS Code' }));
    await waitFor(() => {
      expect(backend.callsOf('scripts_open_vscode')).toHaveLength(1);
    });
  });

  it('VS Code sem instância de teste fica indisponível; sem VS Code, explica', async () => {
    const user = userEvent.setup();
    const { backend } = openScript(`?arquivo=${RECEITAS}`, { info: { instanceReady: false } });
    await view();
    const button = await screen.findByRole('button', {
      name: 'Abrir no VS Code',
      description: 'Teste o pack uma vez para criar a instância de teste.',
    });
    await user.click(button);
    expect(backend.callsOf('scripts_open_vscode')).toHaveLength(0);
  });

  it('erro ao abrir o VS Code vira aviso', async () => {
    const user = userEvent.setup();
    openScript(`?arquivo=${RECEITAS}`, { noVsCode: true });
    await view();
    await user.click(await screen.findByRole('button', { name: 'Abrir no VS Code' }));
    await screen.findByText(
      'O VS Code não foi encontrado neste computador. Instale o VS Code e tente de novo.',
    );
  });

  it('na instância de teste com o jogo aberto: Salvar e recarregar grava e recarrega', async () => {
    const user = userEvent.setup();
    const { backend, disks } = openScript(`?origem=instancia&arquivo=${RECEITAS}`, {
      game: 'running',
      instanceFiles: { [RECEITAS]: RECEITAS_TEXT },
      route: copy(),
      outcome: () => ({ route: copy(), response: [] }),
    });
    const editor = await view();
    act(() => {
      editor.dispatch({ changes: { from: editor.state.doc.length, insert: '// novo\n' } });
    });
    const treeCalls = backend.callsOf('config_tree').length;
    await user.click(await screen.findByRole('button', { name: 'Salvar e recarregar' }));
    await waitFor(() => {
      expect(backend.callsOf('scripts_reload')).toHaveLength(1);
    });
    expect(disks.instance?.[RECEITAS]).toContain('// novo');
    expect(backend.callsOf('config_write')[0]?.args).toMatchObject({ origin: 'instance' });
    await screen.findByText('No KubeJS 6, recarregar precisa de um comando no jogo.');
    // A lista de arquivos é relida depois de gravar (dados na tela sempre atuais).
    await waitFor(() => {
      expect(backend.callsOf('config_tree').length).toBeGreaterThan(treeCalls);
    });
  });

  it('no pack com o jogo aberto: avisa e leva ao mesmo script na instância', async () => {
    const user = userEvent.setup();
    openScript(`?arquivo=${RECEITAS}`, {
      game: 'running',
      instanceFiles: { [RECEITAS]: RECEITAS_TEXT },
    });
    await view();
    await screen.findByText('O jogo está aberto com a instância de teste.');
    await user.click(screen.getByRole('button', { name: 'Editar na instância de teste' }));
    await screen.findByText('Você está editando a instância de teste, não o pack.');
    await view();
  });

  it('pack sem KubeJS: o script abre como texto comum', async () => {
    const { backend } = openScript(`?arquivo=${RECEITAS}`, {
      info: { env: { kubejs: null, crafttweaker: null }, kubejsSnippets: [] },
    });
    await view();
    await waitFor(() => {
      expect(backend.callsOf('scripts_info')).toHaveLength(1);
    });
    expect(screen.queryByRole('button', { name: 'Trechos prontos' })).toBeNull();
    expect(screen.queryByRole('region', { name: 'Erros dos scripts' })).toBeNull();
    expect(backend.callsOf('scripts_ids')).toHaveLength(0);
  });

  it('config comum não consulta nada de scripts', async () => {
    const { backend } = openScript('?arquivo=config/a.toml', {
      files: { 'config/a.toml': 'a = 1\n' },
    });
    await view('config/a.toml');
    expect(backend.callsOf('scripts_info')).toHaveLength(0);
  });

  it('CraftTweaker: ZenScript com realce, versão na barra e IDs no colchete', async () => {
    openScript(`?arquivo=${ZS}`, {
      files: { [ZS]: '// Receitas\nrecipes.remove(<item:minecraft:stone>);\n' },
      info: {
        env: {
          kubejs: null,
          crafttweaker: { tool: 'crafttweaker', major: 14, version: '14.0.60' },
        },
        kubejsSnippets: [],
        crafttweakerSnippets: [{ id: 'fornalha', scriptType: null, body: 'furnace;' }],
      },
    });
    const editor = await view(ZS);
    await screen.findByText('CraftTweaker 14 · Forge 1.20.1');
    expect(screen.getByText(/ZenScript · CraftTweaker 14/)).toBeDefined();
    // O comentário e o colchete têm cor própria (spans do realce).
    await waitFor(() => {
      expect(editor.contentDOM.querySelectorAll('.cm-line span').length).toBeGreaterThan(1);
    });
    act(() => {
      editor.dispatch({
        changes: { from: editor.state.doc.length, insert: '<item:minecraft:cop' },
        selection: { anchor: editor.state.doc.length + '<item:minecraft:cop'.length },
      });
      startCompletion(editor);
    });
    const list = await screen.findByRole('listbox', {}, { timeout: 5000 });
    const labels = within(list)
      .getAllByRole('option')
      .map((option) => option.querySelector('.cm-completionLabel')?.textContent);
    expect(labels).toEqual(['minecraft:copper_block', 'minecraft:copper_ingot']);
  });
});
