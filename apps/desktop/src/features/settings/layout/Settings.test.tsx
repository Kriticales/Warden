import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { InstalledRuntime, JavaOverview } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { fakeAiHandlers } from '../ai/testing';
import { SETTINGS_PAGE_IDS, type SettingsPageId } from '../pages';
import { DEFAULT_PACKS_DIR, makeSettings, mockSettingsBackend } from '../testing';

/** Primeira tela de cada caso: roteador e consultas, numa máquina ocupada. */
const FIRST_PAINT = { timeout: 5000 };

const TITLES: Record<SettingsPageId, string> = {
  geral: 'Geral',
  chaves: 'Chaves e contas',
  'sensor-sculk': 'Sensor Sculk',
  teste: 'Teste',
  editor: 'Editor de configs',
  registros: 'Privacidade e registros',
  armazenamento: 'Armazenamento',
  sobre: 'Sobre o Warden',
};

async function openPage(page: SettingsPageId) {
  renderApp(`/configuracoes/${page}`);
  const heading = await screen.findByRole('heading', { level: 1, name: TITLES[page] }, FIRST_PAINT);
  if (page === 'geral') {
    // A pasta vem de `settings_status`, lido à parte.
    await screen.findByRole('button', { name: 'Escolher…' });
  }
  return heading;
}

function menu() {
  return screen.getByRole('navigation', { name: 'Páginas de Configurações' });
}

const java21: InstalledRuntime = {
  id: 'temurin-21-jdk-21.0.8+9-x64',
  source: 'temurin',
  version: { major: 21, minor: 0, security: 8, patch: 0, build: 9 },
  vendor: 'Eclipse Adoptium',
  releaseName: 'jdk-21.0.8+9',
  arch: 'x64',
  home: 'C:/dados/shared/runtimes/temurin-21',
  launcher: 'C:/dados/shared/runtimes/temurin-21/bin/javaw.exe',
  java: 'C:/dados/shared/runtimes/temurin-21/bin/java.exe',
  installedAtMs: 0,
  updateCap: null,
  mojangComponent: null,
  archiveSha256: null,
  supersededBy: null,
};

const JAVAS: JavaOverview = {
  runtimes: [{ runtime: java21, packs: [], inGame: false }],
  packsToDownload: [],
  unresolved: [],
  brokenCount: 0,
};

/** O item `index` (o teste falha se não existir). */
function nth<T>(items: readonly T[], index: number): T {
  const item = items[index];
  if (item === undefined) throw new Error(`sem o item ${String(index)}`);
  return item;
}

describe('Configurações (menu lateral; esboço bug-01)', () => {
  it('abre pela barra do app na Geral; o menu tem as 8 páginas na ordem, a atual marcada, e cada uma tem endereço', async () => {
    const backend = mockSettingsBackend();
    const user = userEvent.setup();
    renderApp('/');
    const header = await screen.findByRole('banner', {}, FIRST_PAINT);
    await user.click(within(header).getByRole('link', { name: 'Configurações' }));

    expect(await screen.findByRole('heading', { level: 1, name: 'Geral' })).toBeDefined();
    const links = within(menu()).getAllByRole('link');
    expect(links.map((link) => link.querySelector('.sidenav__name')?.textContent)).toEqual([
      'Geral',
      'Chaves e contas',
      'Sensor Sculk',
      'Teste',
      'Editor de configs',
      'Privacidade e registros',
      'Armazenamento',
      'Sobre o Warden',
    ]);
    expect(links.map((link) => link.getAttribute('href'))).toEqual(
      SETTINGS_PAGE_IDS.map((id) => `/configuracoes/${id}`),
    );
    expect(links[0]?.getAttribute('aria-current')).toBe('page');
    expect(within(menu()).getByText('Cofre do Windows')).toBeDefined();
    // A versão aparece no item "Sobre o Warden".
    expect(within(nth(links, 7)).getByText('0.1.0')).toBeDefined();

    await user.click(within(menu()).getByRole('link', { name: /Privacidade e registros/ }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Privacidade e registros' }),
    ).toBeDefined();
    expect(
      within(menu())
        .getByRole('link', { name: /Privacidade e registros/ })
        .getAttribute('aria-current'),
    ).toBe('page');
    expect(links[0]?.getAttribute('aria-current')).toBeNull();
    // "Meus packs" volta para o início; a página não repete o link para si.
    const banner = screen.getByRole('banner');
    expect(within(banner).queryByRole('link', { name: 'Configurações' })).toBeNull();
    expect(within(banner).getByRole('link', { name: 'Meus packs' }).getAttribute('href')).toBe('/');
    expect(backend.callsOf('settings_get').length).toBeGreaterThan(0);
  });

  it('contadores do menu: chaves recusadas, o provedor das conversas novas e o espaço usado', async () => {
    const secrets = {
      curseforge: true,
      gemini: false,
      openai: true,
      anthropic: true,
      github: true,
    };
    const ai = fakeAiHandlers(() => secrets, { order: ['anthropic', 'gemini', 'openai'] });
    mockSettingsBackend(
      { secrets, checks: { github: 'refused', openai: 'refused', anthropic: 'working' } },
      {
        ...ai.handlers,
        storage_usage: () => ({
          cache: 1024 ** 3,
          clearable: 1024 ** 3,
          minecraft: 1024 ** 3,
          javas: 0,
          instances: 1024 ** 3,
        }),
      },
    );
    await openPage('geral');
    const keys = within(menu()).getByRole('link', { name: /Chaves e contas/ });
    expect(await within(keys).findByText('2')).toBeDefined();
    expect(within(keys).getByLabelText('2 chaves recusadas no último teste')).toBeDefined();
    const sculk = within(menu()).getByRole('link', { name: /Sensor Sculk/ });
    expect(await within(sculk).findByText('Claude')).toBeDefined();
    const storage = within(menu()).getByRole('link', { name: /Armazenamento/ });
    expect(await within(storage).findByText('3,0 GB')).toBeDefined();
  });

  it('endereço de página que não existe mostra "não encontrada"', async () => {
    mockSettingsBackend();
    renderApp('/configuracoes/nao-existe');
    expect(
      (await screen.findAllByText(/não existe|não encontrada/i, {}, FIRST_PAINT)).length,
    ).toBeGreaterThan(0);
  });

  it('erro ao ler as configurações: diz o que falhou (A-01) e "Tentar de novo" lê de novo', async () => {
    let fail = true;
    const user = userEvent.setup();
    const real = makeSettings();
    mockSettingsBackend(
      {},
      {
        settings_get: () =>
          fail ? ipcError(makeAppError({ domain: 'app', code: 'INTERNAL' })) : real,
      },
    );
    renderApp('/configuracoes');
    const retry = await screen.findByRole('button', { name: 'Tentar de novo' }, FIRST_PAINT);
    expect(
      within(screen.getByRole('alert')).getByText('Não foi possível ler as configurações.'),
    ).toBeDefined();
    // O menu continua lá para ir a outra página.
    expect(menu()).toBeDefined();
    fail = false;
    await user.click(retry);
    expect(await screen.findByRole('heading', { level: 1, name: 'Geral' })).toBeDefined();
  });

  describe('Geral', () => {
    it('mostra a pasta em uso e troca pelo diálogo nativo', async () => {
      const backend = mockSettingsBackend(
        {},
        {
          settings_choose_packs_dir: () => {
            backend.state.settings = makeSettings({ packsDir: 'D:\\Packs' });
            return { firstRun: false, packsDir: 'D:\\Packs', defaultPacksDir: DEFAULT_PACKS_DIR };
          },
        },
      );
      const user = userEvent.setup();
      await openPage('geral');
      const field = screen.getByRole<HTMLInputElement>('textbox', { name: 'Pasta dos packs' });
      expect(field.value).toBe(DEFAULT_PACKS_DIR);
      expect(field.hasAttribute('readonly')).toBe(true);

      await user.click(screen.getByRole('button', { name: 'Escolher…' }));
      await waitFor(() => {
        expect(field.value).toBe('D:\\Packs');
      });
      // O caminho nunca vai da interface para o Rust (ARCHITECTURE §4.1).
      expect(backend.callsOf('settings_choose_packs_dir')[0]?.args).toEqual({});
    });

    it('desistir do diálogo não muda nada', async () => {
      mockSettingsBackend({}, { settings_choose_packs_dir: () => null });
      const user = userEvent.setup();
      await openPage('geral');
      await user.click(screen.getByRole('button', { name: 'Escolher…' }));
      const field = screen.getByRole<HTMLInputElement>('textbox', { name: 'Pasta dos packs' });
      await waitFor(() => {
        expect(screen.getByRole('button', { name: 'Escolher…' }).hasAttribute('disabled')).toBe(
          false,
        );
      });
      expect(field.value).toBe(DEFAULT_PACKS_DIR);
      expect(field.getAttribute('aria-invalid')).toBeNull();
    });

    it.each([
      ['insideAppData', 'Essa pasta fica dentro dos dados do próprio Warden'],
      ['notWritable', 'O Warden não tem permissão para gravar nessa pasta.'],
      ['symlink', 'Essa pasta é um atalho (link ou junção) para outro lugar.'],
    ])('pasta recusada (%s) mostra o motivo no campo', async (reason, text) => {
      mockSettingsBackend(
        {},
        {
          settings_choose_packs_dir: () =>
            ipcError(
              makeAppError(
                { domain: 'app', code: 'SETTINGS_INVALID' },
                { params: { field: 'packsDir', reason } },
              ),
            ),
        },
      );
      const user = userEvent.setup();
      await openPage('geral');
      await user.click(screen.getByRole('button', { name: 'Escolher…' }));
      const error = await screen.findByText(new RegExp(`^${text.replace(/[.()]/g, '\\$&')}`));
      expect(error).toBeDefined();
      const field = screen.getByRole<HTMLInputElement>('textbox', { name: 'Pasta dos packs' });
      expect(field.getAttribute('aria-invalid')).toBe('true');
    });

    it('CA-T01-02 também aqui: "Zé" é recusado com a regra; "Ze_123" é salvo', async () => {
      const backend = mockSettingsBackend();
      const user = userEvent.setup();
      await openPage('geral');
      const field = screen.getByRole('textbox', { name: 'Nome do jogador nos testes' });
      await user.clear(field);
      await user.type(field, 'Zé');
      expect(
        screen.getByText(
          'Use só letras sem acento, números e _ (sem espaço), de 3 a 16 caracteres.',
        ),
      ).toBeDefined();
      expect(screen.getByRole('button', { name: 'Salvar nome' }).hasAttribute('disabled')).toBe(
        true,
      );

      await user.clear(field);
      await user.type(field, 'Ze_123{Enter}');
      await screen.findByText('Nome do jogador salvo.');
      expect(backend.callsOf('settings_update').map((call) => call.args)).toEqual([
        { patch: { playerName: 'Ze_123' } },
      ]);
      expect(screen.queryByRole('button', { name: 'Salvar nome' })).toBeNull();
    });

    it('"Rever boas-vindas" abre a primeira execução', async () => {
      mockSettingsBackend();
      const user = userEvent.setup();
      await openPage('geral');
      await user.click(screen.getByRole('link', { name: 'Rever boas-vindas' }));
      expect(
        await screen.findByRole('list', { name: 'Etapas da primeira execução' }),
      ).toBeDefined();
    });
  });

  describe('Teste', () => {
    it('CA-T21-04: a tabela de Java aparece em Teste; grava memória, versões beta e intervalo', async () => {
      const backend = mockSettingsBackend({}, { java_runtimes_list: () => JAVAS });
      const user = userEvent.setup();
      await openPage('teste');
      expect(
        await screen.findByRole('table', {
          name: 'Javas instalados pelo Warden e os packs que usam cada um',
        }),
      ).toBeDefined();
      expect(screen.getByRole('rowheader', { name: /Java 21/ })).toBeDefined();
      expect(screen.getByRole('button', { name: 'Procurar atualizações do Java' })).toBeDefined();
      expect(screen.getByText('EULA do Minecraft')).toBeDefined();

      await user.selectOptions(screen.getByRole('combobox', { name: 'Memória padrão' }), '8 GB');
      await user.click(
        screen.getByRole('switch', { name: 'Mostrar versões beta e alpha dos mods' }),
      );
      await user.selectOptions(
        screen.getByRole('combobox', { name: 'Verificar atualizações dos mods' }),
        'Só quando eu pedir',
      );
      await waitFor(() => {
        expect(backend.callsOf('settings_update')).toHaveLength(3);
      });
      expect(backend.callsOf('settings_update').map((call) => call.args.patch)).toEqual([
        { testMemory: { mode: 'fixed', mb: 8192 } },
        { showPrereleaseVersions: true },
        { updateCheckIntervalHours: 0 },
      ]);
      expect(
        screen
          .getByRole('switch', { name: 'Mostrar versões beta e alpha dos mods' })
          .getAttribute('aria-checked'),
      ).toBe('true');
    });

    it('valor fora da lista continua visível', async () => {
      mockSettingsBackend(
        {
          settings: makeSettings({
            testMemory: { mode: 'fixed', mb: 5000 },
            updateCheckIntervalHours: 12,
          }),
        },
        { java_runtimes_list: () => JAVAS },
      );
      await openPage('teste');
      const memory = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Memória padrão' });
      expect(memory.selectedOptions[0]?.textContent).toBe('5000 MB');
      const interval = screen.getByRole<HTMLSelectElement>('combobox', {
        name: 'Verificar atualizações dos mods',
      });
      expect(interval.selectedOptions[0]?.textContent).toBe('A cada 12 horas');
    });
  });

  describe('Editor de configs e Privacidade e registros', () => {
    it('grava as diferenças antes de salvar e o nível dos registros', async () => {
      const backend = mockSettingsBackend();
      const user = userEvent.setup();
      await openPage('editor');
      await user.click(
        screen.getByRole('switch', { name: 'Mostrar as diferenças antes de salvar' }),
      );
      expect(screen.getByText(/ficam na seção/)).toBeDefined();
      await user.click(within(menu()).getByRole('link', { name: /Privacidade e registros/ }));
      await user.selectOptions(
        await screen.findByRole('combobox', { name: 'Nível de detalhe dos registros' }),
        'Detalhado (para relatar um problema)',
      );
      await waitFor(() => {
        expect(backend.callsOf('settings_update').map((call) => call.args.patch)).toEqual([
          { configDiffBeforeSave: false },
          { logLevel: 'detailed' },
        ]);
      });
      expect(screen.getByText(/Os registros do Warden nunca guardam chaves/)).toBeDefined();
    });

    it('recusa do Warden ao salvar vira aviso', async () => {
      mockSettingsBackend(
        {},
        {
          settings_update: () =>
            ipcError(makeAppError({ domain: 'app', code: 'SETTINGS_NEWER_VERSION' })),
        },
      );
      const user = userEvent.setup();
      await openPage('editor');
      await user.click(screen.getByRole('switch', { name: /Mostrar as diferenças/ }));
      expect(await screen.findByText('A configuração não foi salva')).toBeDefined();
    });

    it('"Abrir pasta de registros" pede ao Rust (sem caminho vindo da interface)', async () => {
      const backend = mockSettingsBackend();
      const user = userEvent.setup();
      await openPage('registros');
      await user.click(screen.getByRole('button', { name: 'Abrir pasta de registros' }));
      await waitFor(() => {
        expect(backend.callsOf('logs_reveal_folder')).toEqual([
          { command: 'logs_reveal_folder', args: {} },
        ]);
      });
    });
  });

  it('T23: Sobre mostra a versão, o commit e o commit do packwiz embutido', async () => {
    mockSettingsBackend();
    await openPage('sobre');
    expect(await screen.findByText('abc123def456')).toBeDefined();
    expect(screen.getByText('packwiz embutido')).toBeDefined();
    expect(screen.getByText('ef87d96')).toBeDefined();
  });

  it.each(['geral', 'teste', 'editor', 'registros', 'armazenamento', 'sobre'] as const)(
    'a página %s passa no axe',
    async (page) => {
      mockSettingsBackend({}, { java_runtimes_list: () => JAVAS });
      const { container } = renderApp(`/configuracoes/${page}`);
      await screen.findByRole('heading', { level: 1, name: TITLES[page] }, FIRST_PAINT);
      if (page === 'sobre') await screen.findByText('abc123def456');
      if (page === 'teste') await screen.findByRole('rowheader', { name: /Java 21/ });
      expect(await axePage(container)).toHaveNoViolations();
    },
  );
});
