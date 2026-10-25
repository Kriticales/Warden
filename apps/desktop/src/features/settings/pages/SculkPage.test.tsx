/**
 * Configurações → Sensor Sculk (ADR-0070; CA-T21-06; esboço `design/bug-01/configuracoes.html`):
 * a ordem de preferência dos provedores (setas, Alt+↑/↓ e arrastar), o papel de cada um na
 * conversa nova (o 1º com chave funcionando; reserva; pulado sem chave ou com a chave recusada),
 * os modelos de cada modo só para consulta e a prévia do campo de pergunta.
 */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { SecretsStatus } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { renderApp } from '../../../test/render';
import { fakeAiHandlers, type FakeAiState } from '../ai/testing';
import { mockSettingsBackend, noSecrets, type FakeState } from '../testing';

const FIRST_PAINT = { timeout: 5000 };

function setup(initial: Partial<FakeState> = {}, ai: Partial<FakeAiState> = {}) {
  let secrets = (): SecretsStatus => ({ ...noSecrets(), ...initial.secrets });
  let refused: (provider: string) => boolean = () => false;
  const fake = fakeAiHandlers(
    () => secrets(),
    ai,
    (provider) => refused(provider),
  );
  const backend = mockSettingsBackend(initial, fake.handlers);
  secrets = () => backend.state.secrets;
  refused = (provider) => backend.state.checks[provider as keyof SecretsStatus] === 'refused';
  return { backend, fake };
}

async function openSculk() {
  renderApp('/configuracoes/sensor-sculk');
  await screen.findByRole('heading', { level: 1, name: 'Sensor Sculk' }, FIRST_PAINT);
  return screen.findByRole('list', { name: 'Ordem de preferência dos provedores' });
}

function rows(list: HTMLElement) {
  return within(list).getAllByRole('listitem');
}

function names(list: HTMLElement) {
  return rows(list).map((row) => row.getAttribute('data-provider'));
}

/** O item `index` (o teste falha se não existir). */
function nth<T>(items: readonly T[], index: number): T {
  const item = items[index];
  if (item === undefined) throw new Error(`sem o item ${String(index)}`);
  return item;
}

describe('Sensor Sculk (ADR-0070)', () => {
  it('cada provedor com o seu papel: conversas novas, reserva, pulado sem chave e pulado com a chave recusada', async () => {
    setup(
      {
        secrets: { ...noSecrets(), anthropic: true, openai: true },
        checks: { openai: 'refused' },
      },
      { order: ['anthropic', 'gemini', 'openai'] },
    );
    const list = await openSculk();
    expect(names(list)).toEqual(['anthropic', 'gemini', 'openai']);
    const [claude, gemini, openai] = rows(list) as [HTMLElement, HTMLElement, HTMLElement];
    expect(within(claude).getByText('Conversas novas')).toBeDefined();
    expect(within(gemini).getByText('Pulado: sem chave')).toBeDefined();
    expect(within(openai).getByText('Pulado: chave recusada')).toBeDefined();
    // Os modelos de cada modo, só para consulta, sem quebrar o nome.
    expect(await within(claude).findByText('claude-sonnet-9-9')).toBeDefined();
    expect(within(claude).getByText('claude-opus-9-9')).toBeDefined();
    expect(
      within(gemini).getByText(/Sem escolha de modo: usa o Gemini Flash mais novo/),
    ).toBeDefined();
    expect(
      within(openai).getByText('Os nomes aparecem quando a chave voltar a funcionar.'),
    ).toBeDefined();
    // O aviso da chave recusada, com o botão para trocar a chave.
    expect(screen.getByText(/A chave da OpenAI foi recusada no último teste/)).toBeDefined();
    expect(screen.getByText(/o Sculk pula a OpenAI e usa o próximo da lista/)).toBeDefined();
    expect(screen.getByRole('button', { name: 'Substituir a chave' })).toBeDefined();
  });

  it('com a reserva funcionando, ela aparece como "Reserva"; o Gemini mostra um modelo só e a troca no limite do dia', async () => {
    setup(
      { secrets: { ...noSecrets(), gemini: true, openai: true } },
      { order: ['gemini', 'openai', 'anthropic'] },
    );
    const list = await openSculk();
    const [gemini, openai] = rows(list) as [HTMLElement, HTMLElement];
    expect(within(gemini).getByText('Conversas novas')).toBeDefined();
    expect(within(openai).getByText('Reserva')).toBeDefined();
    expect(await within(gemini).findByText('gemini-9.1-flash')).toBeDefined();
    expect(within(gemini).getByText(/Troca sozinho para o/).textContent).toContain(
      'gemini-9.1-flash-lite',
    );
    // A prévia: com o Gemini, não há seletor de modo.
    expect(screen.queryByRole('radiogroup', { name: 'Como o Sculk procura' })).toBeNull();
    expect(screen.getByText('Com o Gemini, o seletor de modo não aparece.')).toBeDefined();
  });

  it('setas e Alt+↑/↓ mudam a ordem e gravam; a conversa nova passa a ser do 1º com chave', async () => {
    const { backend, fake } = setup(
      { secrets: { ...noSecrets(), gemini: true, anthropic: true } },
      { order: ['gemini', 'openai', 'anthropic'] },
    );
    const user = userEvent.setup();
    const list = await openSculk();
    expect(
      within(list).getByRole('button', { name: 'Subir Gemini' }).hasAttribute('disabled'),
    ).toBe(true);
    await user.click(within(list).getByRole('button', { name: 'Subir Claude' }));
    await waitFor(() => {
      expect(names(list)).toEqual(['gemini', 'anthropic', 'openai']);
    });
    expect(backend.callsOf('ai_order_set')[0]?.args).toEqual({
      order: ['gemini', 'anthropic', 'openai'],
    });

    within(nth(rows(list), 1))
      .getByRole('button', { name: /^Mover Claude/ })
      .focus();
    await user.keyboard('{Alt>}{ArrowUp}{/Alt}');
    await waitFor(() => {
      expect(names(list)).toEqual(['anthropic', 'gemini', 'openai']);
    });
    expect(fake.state.order).toEqual(['anthropic', 'gemini', 'openai']);
    // O foco segue a linha movida.
    await waitFor(() => {
      expect(document.activeElement?.getAttribute('aria-label')).toMatch(/^Mover Claude/);
    });
    expect(
      await screen.findByText(
        'Ordem salva: 1º Claude, 2º Gemini, 3º OpenAI. Conversas novas: Claude.',
      ),
    ).toBeDefined();
    expect(within(nth(rows(list), 0)).getByText('Conversas novas')).toBeDefined();
    expect(within(nth(rows(list), 1)).getByText('Reserva')).toBeDefined();
  });

  it('arrastar uma linha para cima de outra muda a ordem', async () => {
    const { backend } = setup({}, { order: ['gemini', 'openai', 'anthropic'] });
    const list = await openSculk();
    const [gemini, , anthropic] = rows(list) as [HTMLElement, HTMLElement, HTMLElement];
    fireEvent.dragStart(anthropic, { dataTransfer: { setData: () => undefined } });
    fireEvent.dragOver(gemini);
    fireEvent.drop(gemini);
    await waitFor(() => {
      expect(backend.callsOf('ai_order_set')[0]?.args).toEqual({
        order: ['anthropic', 'gemini', 'openai'],
      });
    });
  });

  it('a prévia mostra o seletor de modo e o modelo de cada modo do provedor em uso', async () => {
    setup(
      { secrets: { ...noSecrets(), anthropic: true } },
      { order: ['anthropic', 'gemini', 'openai'] },
    );
    const user = userEvent.setup();
    await openSculk();
    const modes = screen.getByRole('radiogroup', { name: 'Como o Sculk procura' });
    expect(
      within(modes).getByRole('radio', { name: 'Procurar a fundo' }).getAttribute('aria-checked'),
    ).toBe('true');
    const meta = () => document.querySelector('.qpreview__meta')?.textContent ?? '';
    await waitFor(() => {
      expect(meta()).toBe('usa o claude-opus-9-9');
    });
    await user.click(within(modes).getByRole('radio', { name: 'Procurar rápido' }));
    expect(meta()).toBe('usa o claude-sonnet-9-9');
  });

  it('sem nenhuma chave funcionando, a prévia avisa que o Sculk pede uma chave', async () => {
    setup();
    await openSculk();
    expect(
      screen.getByText(
        'Nenhum provedor com chave funcionando. O Sensor Sculk pede uma chave antes de começar.',
      ),
    ).toBeDefined();
  });

  it('"Substituir a chave" leva a Chaves e contas, já no campo da chave recusada', async () => {
    setup(
      { secrets: { ...noSecrets(), openai: true }, checks: { openai: 'refused' } },
      { order: ['openai', 'gemini', 'anthropic'] },
    );
    const user = userEvent.setup();
    await openSculk();
    await user.click(screen.getByRole('button', { name: 'Substituir a chave' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Chaves e contas' })).toBeDefined();
    expect(await screen.findByLabelText('Nova chave da OpenAI')).toBeDefined();
  });

  it('passa no axe', async () => {
    setup(
      {
        secrets: { ...noSecrets(), anthropic: true, openai: true },
        checks: { openai: 'refused' },
      },
      { order: ['anthropic', 'gemini', 'openai'] },
    );
    const { container } = renderApp('/configuracoes/sensor-sculk');
    await screen.findByRole('heading', { level: 1, name: 'Sensor Sculk' }, FIRST_PAINT);
    await screen.findAllByText('claude-sonnet-9-9');
    expect(await axePage(container)).toHaveNoViolations();
  });
});
