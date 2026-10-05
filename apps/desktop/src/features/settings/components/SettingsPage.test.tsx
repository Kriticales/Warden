import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { axePage } from '../../../test/axe';
import { ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { DEFAULT_PACKS_DIR, makeSettings, mockSettingsBackend } from '../testing';

/** Primeira tela de cada caso: roteador e consultas, numa máquina ocupada. */
const FIRST_PAINT = { timeout: 5000 };

const CF_KEY = 'chave-de-teste-123';

async function openSettings() {
  renderApp('/configuracoes');
  const heading = await screen.findByRole('heading', { level: 2, name: 'Geral' }, FIRST_PAINT);
  // A pasta vem de `settings_status`, lido à parte.
  await screen.findByRole('button', { name: 'Escolher…' });
  return heading;
}

function section(name: string) {
  return screen.getByRole('region', { name });
}

describe('Configurações', () => {
  it('abre pela barra do app e mostra as seções na ordem da SPEC, com "Sobre o Warden" por último', async () => {
    mockSettingsBackend();
    const user = userEvent.setup();
    renderApp('/');
    const header = await screen.findByRole('banner', {}, FIRST_PAINT);
    await user.click(within(header).getByRole('link', { name: 'Configurações' }));

    expect(await screen.findByRole('heading', { level: 1, name: 'Configurações' })).toBeDefined();
    await screen.findByRole('heading', { level: 2, name: 'Geral' });
    const titles = screen.getAllByRole('heading', { level: 2 }).map((h) => h.textContent);
    expect(titles).toEqual([
      'Geral',
      'Chaves e contas',
      'Teste',
      'Editor de configs',
      'Privacidade e registros',
      'Sobre o Warden',
    ]);
    // A própria página não repete o link para si; "Meus packs" volta para o início.
    expect(
      within(screen.getByRole('banner')).queryByRole('link', { name: 'Configurações' }),
    ).toBeNull();
    expect(
      within(screen.getByRole('banner'))
        .getByRole('link', { name: 'Meus packs' })
        .getAttribute('href'),
    ).toBe('/');
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
      await openSettings();
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
      await openSettings();
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
      await openSettings();
      await user.click(screen.getByRole('button', { name: 'Escolher…' }));
      const error = await screen.findByText(new RegExp(`^${text.replace(/[.()]/g, '\\$&')}`));
      expect(error).toBeDefined();
      const field = screen.getByRole<HTMLInputElement>('textbox', { name: 'Pasta dos packs' });
      expect(field.getAttribute('aria-invalid')).toBe('true');
    });

    it('CA-T01-02 também aqui: "Zé" é recusado com a regra; "Ze_123" é salvo', async () => {
      const backend = mockSettingsBackend();
      const user = userEvent.setup();
      await openSettings();
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
      await openSettings();
      await user.click(screen.getByRole('link', { name: 'Rever boas-vindas' }));
      expect(
        await screen.findByRole('list', { name: 'Etapas da primeira execução' }),
      ).toBeDefined();
    });
  });

  describe('Chaves e contas', () => {
    it('salva uma chave: o valor some e fica só "Configurada", com Testar, Substituir e Remover', async () => {
      const backend = mockSettingsBackend();
      const user = userEvent.setup();
      await openSettings();
      const keys = section('Chaves e contas');
      const row = await within(keys).findByTestId('chave-curseforge');
      await user.type(within(row).getByLabelText('Chave da CurseForge'), CF_KEY);
      await user.click(within(row).getByRole('button', { name: 'Salvar' }));

      expect(await within(row).findByText('Configurada')).toBeDefined();
      expect(backend.callsOf('secrets_set')[0]?.args).toEqual({
        kind: 'curseforge',
        value: CF_KEY,
      });
      expect(document.body.innerHTML).not.toContain(CF_KEY);
      for (const name of ['Testar', 'Substituir', 'Remover']) {
        expect(within(row).getByRole('button', { name })).toBeDefined();
      }
      expect(within(row).queryByLabelText('Chave da CurseForge')).toBeNull();
    });

    it('CA-T21-02: "Testar" com chave inválida mostra "A CurseForge recusou a chave."', async () => {
      mockSettingsBackend(
        { secrets: { backend: 'keyring', curseforge: true, gemini: false, github: false } },
        {
          secrets_test: () =>
            ipcError(makeAppError({ domain: 'curseforge', code: 'CURSEFORGE_KEY_INVALID' })),
        },
      );
      const user = userEvent.setup();
      await openSettings();
      const row = await screen.findByTestId('chave-curseforge');
      await user.click(within(row).getByRole('button', { name: 'Testar' }));
      expect(await within(row).findByText(/^A CurseForge recusou a chave\./)).toBeDefined();
    });

    it('"Testar" com chave aceita mostra "Funcionando"; sem testador, explica', async () => {
      mockSettingsBackend(
        { secrets: { backend: 'keyring', curseforge: true, gemini: true, github: false } },
        {
          secrets_test: ({ kind }) =>
            kind === 'curseforge' ? { status: 'valid' } : { status: 'notTestable' },
        },
      );
      const user = userEvent.setup();
      await openSettings();
      const cf = await screen.findByTestId('chave-curseforge');
      await user.click(within(cf).getByRole('button', { name: 'Testar' }));
      expect(await within(cf).findByText('Funcionando')).toBeDefined();

      const gemini = screen.getByTestId('chave-gemini');
      await user.click(within(gemini).getByRole('button', { name: 'Testar' }));
      expect(
        await within(gemini).findByText('Esta versão do Warden ainda não sabe testar esta chave.'),
      ).toBeDefined();
    });

    it('Substituir mostra o campo de novo; Cancelar volta a "Configurada" sem gravar', async () => {
      const backend = mockSettingsBackend({
        secrets: { backend: 'keyring', curseforge: false, gemini: false, github: true },
      });
      const user = userEvent.setup();
      await openSettings();
      const row = await screen.findByTestId('chave-github');
      await user.click(within(row).getByRole('button', { name: 'Substituir' }));
      const field = within(row).getByLabelText('Token do GitHub');
      expect(field.getAttribute('type')).toBe('password');
      await user.type(field, 'abc');
      await user.click(within(row).getByRole('button', { name: 'Cancelar' }));
      expect(within(row).getByText('Configurada')).toBeDefined();
      expect(backend.callsOf('secrets_set')).toEqual([]);
    });

    it('Remover pede confirmação e apaga a chave', async () => {
      const backend = mockSettingsBackend({
        secrets: { backend: 'keyring', curseforge: false, gemini: true, github: false },
      });
      const user = userEvent.setup();
      await openSettings();
      const row = await screen.findByTestId('chave-gemini');
      await user.click(within(row).getByRole('button', { name: 'Remover' }));
      const dialog = await screen.findByRole('alertdialog', { name: 'Remover a chave da Gemini?' });
      await user.click(within(dialog).getByRole('button', { name: 'Remover chave' }));
      expect(await within(row).findByLabelText('Chave do Gemini')).toBeDefined();
      expect(backend.callsOf('secrets_remove')[0]?.args).toEqual({ kind: 'gemini' });
    });

    it('"Como criar" mostra o passo a passo; o do GitHub diz as permissões para publicar', async () => {
      mockSettingsBackend();
      const user = userEvent.setup();
      await openSettings();
      const row = await screen.findByTestId('chave-github');
      const toggle = within(row).getByRole('button', { name: 'Como criar' });
      expect(toggle.getAttribute('aria-expanded')).toBe('false');
      await user.click(toggle);
      expect(toggle.getAttribute('aria-expanded')).toBe('true');
      expect(within(row).getByText(/Administration \(leitura e escrita\)/)).toBeDefined();
      expect(within(row).getByText(/Contents \(leitura e escrita\)/)).toBeDefined();
      expect(
        within(row).getByRole('button', { name: 'Abrir a página no navegador' }),
      ).toBeDefined();
    });

    it('trocar para o .env pede confirmação com o aviso; depois o aviso fica fixo com "Voltar para o cofre"', async () => {
      const backend = mockSettingsBackend();
      const user = userEvent.setup();
      await openSettings();
      const keys = section('Chaves e contas');
      await user.click(await within(keys).findByRole('radio', { name: /Arquivo \.env/ }));

      const dialog = await screen.findByRole('alertdialog', {
        name: 'Guardar as chaves num arquivo .env?',
      });
      expect(
        within(dialog).getByText(
          'Qualquer programa no seu computador, e qualquer backup ou sincronização dessa pasta, consegue ler as chaves.',
        ),
      ).toBeDefined();
      expect(backend.callsOf('secrets_backend_set')).toEqual([]);
      await user.click(within(dialog).getByRole('button', { name: 'Usar arquivo .env' }));

      const warning = await within(keys).findByText('As chaves estão num arquivo de texto.');
      expect(warning).toBeDefined();
      expect(backend.callsOf('secrets_backend_set')[0]?.args).toEqual({ backend: 'envfile' });
      expect(
        within(keys).getByRole<HTMLInputElement>('radio', { name: /Arquivo \.env/ }).checked,
      ).toBe(true);

      await user.click(within(keys).getByRole('button', { name: 'Voltar para o cofre' }));
      await waitFor(() => {
        expect(within(keys).queryByText('As chaves estão num arquivo de texto.')).toBeNull();
      });
      expect(backend.callsOf('secrets_backend_set')[1]?.args).toEqual({ backend: 'keyring' });
    });

    it('"Manter no cofre" desiste sem mover as chaves', async () => {
      const backend = mockSettingsBackend();
      const user = userEvent.setup();
      await openSettings();
      const keys = section('Chaves e contas');
      await user.click(await within(keys).findByRole('radio', { name: /Arquivo \.env/ }));
      const dialog = await screen.findByRole('alertdialog');
      await user.click(within(dialog).getByRole('button', { name: 'Manter no cofre' }));
      await waitFor(() => {
        expect(screen.queryByRole('alertdialog')).toBeNull();
      });
      expect(backend.callsOf('secrets_backend_set')).toEqual([]);
      expect(
        within(keys).getByRole<HTMLInputElement>('radio', { name: /Cofre do Windows/ }).checked,
      ).toBe(true);
    });

    it('falha ao trocar de modo mostra o erro e as chaves continuam onde estavam', async () => {
      mockSettingsBackend(
        {},
        {
          secrets_backend_set: () =>
            ipcError(makeAppError({ domain: 'secrets', code: 'BACKEND_SWITCH_FAILED' })),
        },
      );
      const user = userEvent.setup();
      await openSettings();
      const keys = section('Chaves e contas');
      await user.click(await within(keys).findByRole('radio', { name: /Arquivo \.env/ }));
      const dialog = await screen.findByRole('alertdialog');
      await user.click(within(dialog).getByRole('button', { name: 'Usar arquivo .env' }));
      expect(
        await within(dialog).findByText(/^Não foi possível trocar onde as chaves ficam guardadas/),
      ).toBeDefined();
    });
  });

  describe('Teste, Editor de configs e registros', () => {
    it('grava memória, versões beta, intervalo, diferenças e nível dos registros', async () => {
      const backend = mockSettingsBackend();
      const user = userEvent.setup();
      await openSettings();

      await user.selectOptions(screen.getByRole('combobox', { name: 'Memória padrão' }), '8 GB');
      await user.click(
        screen.getByRole('switch', { name: 'Mostrar versões beta e alpha dos mods' }),
      );
      await user.selectOptions(
        screen.getByRole('combobox', { name: 'Verificar atualizações dos mods' }),
        'Só quando eu pedir',
      );
      await user.click(
        screen.getByRole('checkbox', { name: /Mostrar as diferenças antes de salvar/ }),
      );
      await user.selectOptions(
        screen.getByRole('combobox', { name: 'Nível de detalhe dos registros' }),
        'Detalhado (para relatar um problema)',
      );

      await waitFor(() => {
        expect(backend.callsOf('settings_update')).toHaveLength(5);
      });
      expect(backend.callsOf('settings_update').map((call) => call.args.patch)).toEqual([
        { testMemory: { mode: 'fixed', mb: 8192 } },
        { showPrereleaseVersions: true },
        { updateCheckIntervalHours: 0 },
        { configDiffBeforeSave: false },
        { logLevel: 'detailed' },
      ]);
      expect(
        screen
          .getByRole('switch', { name: 'Mostrar versões beta e alpha dos mods' })
          .getAttribute('aria-checked'),
      ).toBe('true');
    });

    it('valor fora da lista continua visível', async () => {
      mockSettingsBackend({
        settings: makeSettings({
          testMemory: { mode: 'fixed', mb: 5000 },
          updateCheckIntervalHours: 12,
        }),
      });
      await openSettings();
      const memory = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Memória padrão' });
      expect(memory.selectedOptions[0]?.textContent).toBe('5000 MB');
      const interval = screen.getByRole<HTMLSelectElement>('combobox', {
        name: 'Verificar atualizações dos mods',
      });
      expect(interval.selectedOptions[0]?.textContent).toBe('A cada 12 horas');
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
      await openSettings();
      await user.click(screen.getByRole('checkbox', { name: /Mostrar as diferenças/ }));
      expect(await screen.findByText('A configuração não foi salva')).toBeDefined();
    });

    it('"Abrir pasta de registros" pede ao Rust (sem caminho vindo da interface)', async () => {
      const backend = mockSettingsBackend();
      const user = userEvent.setup();
      await openSettings();
      await user.click(screen.getByRole('button', { name: 'Abrir pasta de registros' }));
      await waitFor(() => {
        expect(backend.callsOf('logs_reveal_folder')).toEqual([
          { command: 'logs_reveal_folder', args: {} },
        ]);
      });
    });
  });

  it('erro ao ler as configurações mostra "Tentar de novo"', async () => {
    mockSettingsBackend(
      {},
      { settings_get: () => ipcError(makeAppError({ domain: 'core', code: 'TIMEOUT' })) },
    );
    renderApp('/configuracoes');
    expect(
      await screen.findByText('A operação demorou demais e foi interrompida. Tente de novo.'),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Tentar de novo' })).toBeDefined();
  });

  it('passa no axe (página inteira, com chaves configuradas e o aviso do .env)', async () => {
    mockSettingsBackend({
      secrets: { backend: 'envfile', curseforge: true, gemini: false, github: true },
    });
    const { container } = renderApp('/configuracoes');
    await screen.findByText('As chaves estão num arquivo de texto.');
    await screen.findByText('abc123def456');
    expect(await axePage(container)).toHaveNoViolations();
  });
});
