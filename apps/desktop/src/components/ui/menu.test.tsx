import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Ellipsis, Trash2 } from 'lucide-react';
import { describe, expect, it } from 'vitest';

import { axeComponent } from '../../test/axe';
import { renderWithProviders } from '../../test/render';
import { Menu, MenuContent, MenuIconTrigger, MenuItem, MenuSeparator } from './menu';

function Example({ onDelete }: { onDelete: () => void }) {
  return (
    <Menu>
      <MenuIconTrigger icon={Ellipsis} label="Mais ações" />
      <MenuContent>
        <MenuItem onSelect={() => undefined}>Primeiro</MenuItem>
        <MenuSeparator />
        <MenuItem icon={Trash2} danger description="Vai para a Lixeira" onSelect={onDelete}>
          Apagar
        </MenuItem>
        <MenuItem disabled onSelect={() => undefined}>
          Indisponível
        </MenuItem>
      </MenuContent>
    </Menu>
  );
}

describe('Menu', () => {
  it('abre pelo teclado, passa pelos itens com ↓ e escolhe com Enter; Esc devolve o foco', async () => {
    const user = userEvent.setup();
    let deleted = 0;
    const { baseElement } = renderWithProviders(
      <Example
        onDelete={() => {
          deleted += 1;
        }}
      />,
    );
    const trigger = screen.getByRole('button', { name: 'Mais ações' });
    trigger.focus();
    await user.keyboard('{Enter}');
    const menu = await screen.findByRole('menu', { name: 'Mais ações' });
    expect(menu.className).toBe('menu');
    expect(screen.getByRole('menuitem', { name: /Apagar/ }).className).toContain(
      'menu__item--danger',
    );
    expect(screen.getByText('Vai para a Lixeira').className).toBe('menu__desc');
    expect(
      screen.getByRole('menuitem', { name: 'Indisponível' }).getAttribute('aria-disabled'),
    ).toBe('true');
    expect(await axeComponent(baseElement)).toHaveNoViolations();
    await user.keyboard('{ArrowDown}');
    await waitFor(() => {
      expect(document.activeElement?.textContent).toContain('Apagar');
    });
    await user.keyboard('{Enter}');
    expect(deleted).toBe(1);
    await waitFor(() => {
      expect(screen.queryByRole('menu')).toBeNull();
    });

    await user.keyboard('{Enter}');
    await screen.findByRole('menu');
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(document.activeElement).toBe(trigger);
    });
  });
});
