/**
 * Configurações → Chaves e contas (T21; esboço `design/bug-01/configuracoes.html`; ADR-0069),
 * com o backend simulado com estado: as 5 chaves na mesma linha, o detalhe da escolhida, os
 * estados (funcionando, recusada, testando, não testada, sem chave), a chave nunca inteira, o
 * aviso de quem assume o Sculk ao remover a chave do provedor em uso e o aviso da migração do
 * `.env` antigo.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { SecretsStatus } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { deferred, type Handler } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { fakeAiHandlers, type FakeAiState } from '../ai/testing';
import { mockSettingsBackend, noSecrets, type FakeState } from '../testing';

const FIRST_PAINT = { timeout: 5000 };
const CF_KEY = 'chave-de-teste-da-curseforge-1234';

function setup(
  initial: Partial<FakeState> = {},
  ai: Partial<FakeAiState> = {},
  handlers: Record<string, Handler> = {},
) {
  let secrets = (): SecretsStatus => ({ ...noSecrets(), ...initial.secrets });
  let refused: (provider: string) => boolean = () => false;
  const fake = fakeAiHandlers(
    () => secrets(),
    ai,
    (provider) => refused(provider),
  );
  const backend = mockSettingsBackend(initial, { ...fake.handlers, ...handlers });
  secrets = () => backend.state.secrets;
  refused = (provider) => backend.state.checks[provider as keyof SecretsStatus] === 'refused';
  return { backend, fake };
}

async function openKeys(search = '') {
  renderApp(`/configuracoes/chaves${search}`);
  await screen.findByRole('heading', { level: 1, name: 'Chaves e contas' }, FIRST_PAINT);
  return screen.findByRole('navigation', { name: 'Chaves' });
}

function detail(name: string) {
  return screen.getByRole('region', { name });
}

/** O item `index` (o teste falha se não existir). */
function nth<T>(items: readonly T[], index: number): T {
  const item = items[index];
  if (item === undefined) throw new Error(`sem o item ${String(index)}`);
  return item;
}

describe('Chaves e contas (ADR-0069)', () => {
  it('as 5 chaves na mesma linha, em dois grupos; o cofre é dito no alto; sem chave, o campo e o passo a passo', async () => {
    setup({}, { order: ['anthropic', 'gemini', 'openai'] });
    const list = await openKeys();
    const groups = within(list).getAllByRole('list');
    expect(
      within(nth(groups, 0))
        .getAllByRole('button')
        .map((b) => b.dataset.testid),
    ).toEqual(['chave-curseforge', 'chave-github']);
    // Os provedores do Sculk, na ordem de preferência.
    expect(
      within(nth(groups, 1))
        .getAllByRole('button')
        .map((b) => b.dataset.testid),
    ).toEqual(['chave-anthropic', 'chave-gemini', 'chave-openai']);
    expect(within(screen.getByTestId('chave-github')).getByText('Sem token')).toBeDefined();
    expect(within(screen.getByTestId('chave-gemini')).getByText('Sem chave')).toBeDefined();
    expect(screen.getByText(/Gerenciador de Credenciais/)).toBeDefined();
    expect(screen.queryByText(/\.env/)).toBeNull();
    expect(screen.queryByText('Onde guardar as chaves')).toBeNull();

    const cf = detail('CurseForge');
    expect(within(cf).getByLabelText('Chave da CurseForge').getAttribute('type')).toBe('password');
    expect(within(cf).getByText(/Vai para o Cofre do Windows/)).toBeDefined();
    expect(
      within(cf).getByRole('heading', { name: 'Como criar a chave da CurseForge' }),
    ).toBeDefined();
    expect(within(cf).getByRole('button', { name: 'Abrir a página no navegador' })).toBeDefined();
  });

  it('salvar testa na hora: "Testando…", depois "Funcionando" com ••••final; a chave inteira nunca aparece', async () => {
    const test = deferred<{ status: 'valid' }>();
    // O teste fica esperando, para ver o "Testando…".
    const { backend } = setup({}, {}, { secrets_test: () => test.promise });
    const user = userEvent.setup();
    await openKeys();
    await user.type(within(detail('CurseForge')).getByLabelText('Chave da CurseForge'), CF_KEY);
    await user.click(within(detail('CurseForge')).getByRole('button', { name: 'Salvar' }));
    expect(await screen.findByText('CurseForge: chave salva no Cofre do Windows.')).toBeDefined();
    const item = screen.getByTestId('chave-curseforge');
    expect(await within(item).findByText('Testando…')).toBeDefined();
    backend.state.checks = { curseforge: 'working' };
    test.resolve({ status: 'valid' });
    expect(await within(item).findByText('Funcionando')).toBeDefined();
    expect(within(item).getByText('••••1234')).toBeDefined();
    expect(within(detail('CurseForge')).getByText(/^Funcionando: testada /)).toBeDefined();
    expect(backend.callsOf('secrets_set')[0]?.args).toEqual({ kind: 'curseforge', value: CF_KEY });
    expect(backend.callsOf('secrets_test')[0]?.args).toEqual({ kind: 'curseforge' });
    expect(document.body.innerHTML).not.toContain(CF_KEY);
    for (const name of ['Testar', 'Substituir', 'Remover']) {
      expect(within(detail('CurseForge')).getByRole('button', { name })).toBeDefined();
    }
  });

  it('CA-T21-02: chave recusada no último teste: "Recusada", a frase do serviço, Substituir primeiro e o passo a passo aberto', async () => {
    setup({
      secrets: { ...noSecrets(), github: true },
      hints: { github: '9c1e' },
      checks: { github: 'refused' },
    });
    await openKeys();
    // Sem chave escolhida, abre na primeira recusada.
    const github = detail('GitHub');
    expect(within(github).getAllByText('Recusada').length).toBeGreaterThan(0);
    expect(
      within(github).getByText(/^O GitHub recusou o token\. Ele pode ter vencido\. Último teste/),
    ).toBeDefined();
    const actions = within(github)
      .getAllByRole('button')
      .map((button) => button.textContent);
    expect(actions.indexOf('Substituir')).toBeLessThan(actions.indexOf('Testar de novo'));
    expect(within(github).getByText('Como criar um token novo').closest('details')?.open).toBe(
      true,
    );
    expect(screen.getByText('1 recusada no último teste')).toBeDefined();
  });

  it('"Testar" com o serviço recusando grava "Recusada"; sem internet, avisa sem mudar a situação; sem testador, explica', async () => {
    const { backend } = setup({
      secrets: { ...noSecrets(), curseforge: true, openai: true, github: true },
      answers: {
        curseforge: { domain: 'curseforge', code: 'KEY_INVALID' },
        openai: { domain: 'core', code: 'NETWORK_UNAVAILABLE' },
        github: 'notTestable',
      },
    });
    const user = userEvent.setup();
    await openKeys('?chave=curseforge');
    await user.click(within(detail('CurseForge')).getByRole('button', { name: 'Testar' }));
    expect(
      await within(detail('CurseForge')).findByText(/^A CurseForge recusou a chave\./),
    ).toBeDefined();
    expect(backend.state.checks.curseforge).toBe('refused');

    await user.click(screen.getByTestId('chave-openai'));
    await user.click(within(detail('OpenAI')).getByRole('button', { name: 'Testar' }));
    expect(await within(detail('OpenAI')).findByText(/^Não deu para testar agora:/)).toBeDefined();
    expect(within(screen.getByTestId('chave-openai')).getByText('Não testada')).toBeDefined();

    await user.click(screen.getByTestId('chave-github'));
    await user.click(within(detail('GitHub')).getByRole('button', { name: 'Testar' }));
    expect(
      await within(detail('GitHub')).findByText(
        'Esta versão do Warden ainda não sabe testar esta chave.',
      ),
    ).toBeDefined();
  });

  it('"Testar todas" testa cada chave guardada', async () => {
    const { backend } = setup({ secrets: { ...noSecrets(), curseforge: true, gemini: true } });
    const user = userEvent.setup();
    await openKeys();
    await user.click(screen.getByRole('button', { name: 'Testar todas' }));
    await waitFor(() => {
      expect(
        backend
          .callsOf('secrets_test')
          .map((call) => call.args.kind)
          .sort(),
      ).toEqual(['curseforge', 'gemini']);
    });
    expect(
      await within(screen.getByTestId('chave-gemini')).findByText('Funcionando'),
    ).toBeDefined();
  });

  it('Substituir mostra o campo "Novo token"; Cancelar volta sem gravar', async () => {
    const { backend } = setup({
      secrets: { ...noSecrets(), github: true },
      hints: { github: '9c1e' },
    });
    const user = userEvent.setup();
    await openKeys('?chave=github');
    await user.click(within(detail('GitHub')).getByRole('button', { name: 'Substituir' }));
    const field = within(detail('GitHub')).getByLabelText('Novo token do GitHub');
    expect(field.getAttribute('type')).toBe('password');
    expect(within(detail('GitHub')).getByText(/^O token atual continua valendo/)).toBeDefined();
    await user.type(field, 'abc');
    await user.click(within(detail('GitHub')).getByRole('button', { name: 'Cancelar' }));
    expect(within(detail('GitHub')).getByText('••••9c1e')).toBeDefined();
    expect(backend.callsOf('secrets_set')).toEqual([]);
  });

  it('ADR-0070: remover a chave do provedor em uso avisa quem assume as conversas novas', async () => {
    const { backend } = setup(
      { secrets: { ...noSecrets(), anthropic: true, openai: true } },
      { order: ['anthropic', 'gemini', 'openai'] },
    );
    const user = userEvent.setup();
    await openKeys('?chave=anthropic');
    const claude = detail('Claude');
    expect(
      await within(claude).findByText(/1º na preferência, usado nas conversas novas/),
    ).toBeDefined();
    expect(await within(claude).findByText('claude-sonnet-9-9')).toBeDefined();
    expect(within(claude).getByText('claude-opus-9-9')).toBeDefined();
    await user.click(within(claude).getByRole('button', { name: 'Remover' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Remover a chave do Claude?' });
    expect(within(dialog).getByText(/As conversas novas passam a usar o/).textContent).toContain(
      'OpenAI',
    );
    await user.click(within(dialog).getByRole('button', { name: 'Remover chave' }));
    expect(await within(detail('Claude')).findByLabelText('Chave do Claude')).toBeDefined();
    expect(backend.callsOf('secrets_remove')[0]?.args).toEqual({ kind: 'anthropic' });
  });

  it('remover a única chave funcionando avisa que o Sculk vai pedir uma chave; reserva não avisa', async () => {
    setup(
      { secrets: { ...noSecrets(), gemini: true, curseforge: true } },
      { order: ['gemini', 'openai', 'anthropic'] },
    );
    const user = userEvent.setup();
    await openKeys('?chave=gemini');
    await user.click(within(detail('Gemini')).getByRole('button', { name: 'Remover' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Remover a chave do Gemini?' });
    expect(within(dialog).getByText(/Nenhum outro provedor tem chave funcionando/)).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Cancelar' }));
    await user.click(screen.getByTestId('chave-curseforge'));
    await user.click(within(detail('CurseForge')).getByRole('button', { name: 'Remover' }));
    const other = await screen.findByRole('alertdialog', {
      name: 'Remover a chave da CurseForge?',
    });
    expect(within(other).queryByText(/conversas novas|Nenhum outro provedor/)).toBeNull();
    expect(within(other).getByText(/A chave sai do Cofre do Windows/)).toBeDefined();
  });

  it('"Substituir a chave" vindo do Sensor Sculk abre já no campo da chave', async () => {
    setup({ secrets: { ...noSecrets(), openai: true }, checks: { openai: 'refused' } });
    await openKeys('?chave=openai&substituir=true');
    expect(await within(detail('OpenAI')).findByLabelText('Nova chave da OpenAI')).toBeDefined();
  });

  it('migração do .env: as chaves foram para o cofre, com aviso; uma falha diz que nada se perdeu e deixa tentar de novo', async () => {
    const { backend } = setup({
      secrets: { ...noSecrets(), curseforge: true },
      migration: {
        kind: 'failed',
        error: makeAppError({ domain: 'secrets', code: 'ENV_MIGRATION_FAILED' }),
      },
    });
    const user = userEvent.setup();
    await openKeys();
    expect(
      await screen.findByText('As chaves do arquivo .env ainda não foram para o cofre.'),
    ).toBeDefined();
    expect(screen.getAllByText(/Nada se perdeu/).length).toBeGreaterThan(0);
    backend.state.migration = null;
    await user.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    await waitFor(() => {
      expect(
        screen.queryByText('As chaves do arquivo .env ainda não foram para o cofre.'),
      ).toBeNull();
    });
    expect(backend.callsOf('secrets_migration_retry')).toHaveLength(1);
  });

  it('migração do .env que deu certo mostra quantas chaves foram e que o arquivo foi apagado', async () => {
    setup({
      secrets: { ...noSecrets(), curseforge: true, github: true },
      migration: { kind: 'moved', moved: ['curseforge', 'github'], fileRemoved: true },
    });
    await openKeys();
    expect(
      await screen.findAllByText('Suas chaves agora ficam no Cofre do Windows.'),
    ).toBeDefined();
    expect(
      screen.getByText(
        /As 2 chaves que estavam no arquivo \.env foram levadas para o cofre e conferidas/,
      ),
    ).toBeDefined();
  });

  it('passa no axe (chaves em vários estados)', async () => {
    setup(
      {
        secrets: { ...noSecrets(), curseforge: true, github: true, anthropic: true, openai: true },
        hints: { curseforge: '3f2a', github: '9c1e', anthropic: 'a91c', openai: '5d0e' },
        checks: { github: 'refused', anthropic: 'working', openai: 'refused' },
      },
      { order: ['anthropic', 'gemini', 'openai'] },
    );
    const { container } = renderApp('/configuracoes/chaves?chave=anthropic');
    await screen.findByRole('heading', { level: 1, name: 'Chaves e contas' }, FIRST_PAINT);
    await within(detail('Claude')).findByText('claude-sonnet-9-9');
    expect(await axePage(container)).toHaveNoViolations();
  });
});
