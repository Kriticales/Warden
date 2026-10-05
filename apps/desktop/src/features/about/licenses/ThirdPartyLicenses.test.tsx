import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { axeComponent } from '../../../test/axe';
import { deferred } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderWithProviders } from '../../../test/render';
import type { Notices } from './notices';
import { makeNotices } from './notices.fixture';
import { ThirdPartyLicenses } from './ThirdPartyLicenses';

async function openDialog(load: () => Promise<Notices | null>) {
  const user = userEvent.setup();
  const view = renderWithProviders(<ThirdPartyLicenses load={load} />);
  await user.click(screen.getByRole('button', { name: 'Licenças de terceiros' }));
  const dialog = await screen.findByRole('dialog', { name: 'Licenças de terceiros' });
  return { user, view, dialog };
}

function groupToggle(dialog: HTMLElement, name: RegExp) {
  return within(dialog).getByRole('button', { name });
}

describe('Licenças de terceiros (T23, A-02)', () => {
  it('Sobre lista todas as licenças: grupos, contagem, créditos e o texto de cada uma; passa no axe', async () => {
    const { user, dialog } = await openDialog(() => Promise.resolve(makeNotices()));
    expect(
      await within(dialog).findByText(
        'O Warden é feito com 6 componentes de código aberto. Aqui estão as licenças de cada um.',
      ),
    ).toBeDefined();

    // Créditos: Forge sempre; portablemc só quando o motor foi compilado.
    expect(
      within(dialog).getByText(/^Forge: o Warden baixa o Forge sempre do Maven oficial/),
    ).toBeDefined();
    expect(within(dialog).getByText(/^packwiz: o formato e a ferramenta/)).toBeDefined();
    expect(within(dialog).getByText(/^portablemc: o motor/)).toBeDefined();

    for (const [name, count] of [
      [/Rust \(o app\)/, '2 componentes'],
      [/JavaScript \(a interface\)/, '2 componentes'],
      [/packwiz embutido/, '1 componentes'],
      [/Fontes/, '1 componentes'],
    ] as const) {
      const toggle = groupToggle(dialog, name);
      expect(toggle.getAttribute('aria-expanded')).toBe('false');
      expect(toggle.closest('.listgroup')?.textContent).toContain(count);
    }
    // Fechados, os grupos não montam as linhas.
    expect(within(dialog).queryByText('serde')).toBeNull();

    await user.click(groupToggle(dialog, /Rust \(o app\)/));
    expect(groupToggle(dialog, /Rust \(o app\)/).getAttribute('aria-expanded')).toBe('true');
    const serde = within(dialog).getByText('serde').closest('li');
    if (!serde) throw new Error('linha do serde');
    expect(within(serde).getByText('1.0.228')).toBeDefined();
    expect(within(serde).getByText('MIT OR Apache-2.0')).toBeDefined();
    await user.click(within(serde).getByText('Ver a licença'));
    const texts = serde.querySelectorAll('pre');
    expect([...texts].map((pre) => pre.textContent)).toEqual([
      'MIT License\n\nPermission is hereby granted',
      'Apache License\nVersion 2.0',
    ]);

    await user.click(groupToggle(dialog, /JavaScript \(a interface\)/));
    const semTexto = within(dialog).getByText('sem-texto').closest('li');
    if (!semTexto) throw new Error('linha sem texto');
    expect(within(semTexto).getByText('Licença não identificada: veja o texto')).toBeDefined();
    expect(within(semTexto).getByText('O pacote não traz o texto da licença.')).toBeDefined();

    // Recolher de novo.
    await user.click(groupToggle(dialog, /Rust \(o app\)/));
    expect(within(dialog).queryByText('serde')).toBeNull();

    expect(await axeComponent(dialog)).toHaveNoViolations();
  });

  it('filtro abre os grupos com resultado e avisa quando nada combina', async () => {
    const { user, dialog } = await openDialog(() => Promise.resolve(makeNotices()));
    const filter = await within(dialog).findByLabelText('Filtrar por nome ou licença');
    await user.type(filter, 'apache');
    expect(groupToggle(dialog, /Rust \(o app\)/).getAttribute('aria-expanded')).toBe('true');
    expect(within(dialog).getByText('portablemc')).toBeDefined();
    expect(within(dialog).queryByRole('button', { name: /JavaScript/ })).toBeNull();

    await user.clear(filter);
    await user.type(filter, 'xyz');
    expect(within(dialog).getByRole('status').textContent).toBe('Nenhum componente com "xyz".');

    await user.clear(filter);
    expect(groupToggle(dialog, /Rust \(o app\)/).getAttribute('aria-expanded')).toBe('false');
  });

  it('sem o motor do launcher compilado, não credita o portablemc', async () => {
    const notices = makeNotices();
    notices.groups = notices.groups.filter((group) => group.id !== 'rust');
    const { dialog } = await openDialog(() => Promise.resolve(notices));
    expect(await within(dialog).findByText(/^Forge:/)).toBeDefined();
    expect(within(dialog).queryByText(/^portablemc:/)).toBeNull();
  });

  it('carregando, lista não gerada e erro com "Tentar de novo"', async () => {
    const pending = deferred<Notices | null>();
    const first = await openDialog(() => pending.promise);
    expect(within(first.dialog).getByRole('status').textContent).toBe('Lendo as licenças…');
    pending.resolve(null);
    expect(await within(first.dialog).findByText('Lista de licenças indisponível')).toBeDefined();
    expect(
      within(first.dialog).getByText(
        'Esta compilação do Warden foi feita sem a lista de licenças. Para incluir, rode "cargo xtask notices" antes de compilar.',
      ),
    ).toBeDefined();
    first.view.unmount();

    let calls = 0;
    const second = await openDialog(() => {
      calls += 1;
      if (calls > 1) return Promise.resolve(makeNotices());
      const failing = deferred<Notices | null>();
      failing.fail(makeAppError({ domain: 'core', code: 'TIMEOUT' }));
      return failing.promise;
    });
    await second.user.click(
      await within(second.dialog).findByRole('button', { name: 'Tentar de novo' }),
    );
    expect(await within(second.dialog).findByText(/componentes de código aberto/)).toBeDefined();
  });

  it('carregador padrão: abre a lista gerada ou explica que ela falta, nunca dá erro', async () => {
    const user = userEvent.setup();
    renderWithProviders(<ThirdPartyLicenses />);
    await user.click(screen.getByRole('button', { name: 'Licenças de terceiros' }));
    const dialog = await screen.findByRole('dialog', { name: 'Licenças de terceiros' });
    expect(
      await within(dialog).findByText(
        /componentes de código aberto|Esta compilação do Warden foi feita sem a lista/,
      ),
    ).toBeDefined();
    expect(within(dialog).queryByRole('button', { name: 'Tentar de novo' })).toBeNull();
  });

  it('"Fechar" fecha o diálogo e devolve o foco ao link', async () => {
    const { user, dialog } = await openDialog(() => Promise.resolve(makeNotices()));
    const close = within(dialog).getAllByRole('button', { name: 'Fechar' });
    const footerClose = close.at(-1);
    if (!footerClose) throw new Error('botão Fechar do rodapé');
    await user.click(footerClose);
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.activeElement).toBe(
      screen.getByRole('button', { name: 'Licenças de terceiros' }),
    );
  });
});
