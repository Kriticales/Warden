import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { axeComponent } from '../../test/axe';
import { DiffView } from './DiffView';

describe('DiffView', () => {
  it('sinal visível, texto para leitor de tela e contagem no cabeçalho', async () => {
    const { container } = render(
      <DiffView
        file="config/jei/jei-client.ini"
        before={'[advanced]\ncheatMode = false\nmaxColumns = 9\n'}
        after={'[advanced]\ncheatMode = true\nmaxColumns = 9\n'}
      />,
    );
    expect(screen.getByText('config/jei/jei-client.ini')).toBeDefined();
    expect(screen.getByRole('img', { name: '1 linhas adicionadas e 1 removidas' })).toBeDefined();
    const added = container.querySelector('.diff__line--add');
    const removed = container.querySelector('.diff__line--del');
    expect(added?.textContent).toBe('2+Adicionada: cheatMode = true');
    expect(removed?.textContent).toBe('2−Removida: cheatMode = false');
    expect(added?.querySelector('.diff__sign')?.getAttribute('aria-hidden')).toBe('true');
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('trechos iguais longos ficam dobrados com a quantidade', () => {
    const before = Array.from({ length: 20 }, (_, i) => `k${String(i)}=0`).join('\n');
    const after = before.replace('k10=0', 'k10=1');
    render(<DiffView file="options.txt" before={before} after={after} context={2} />);
    expect(screen.getByText('8 linhas iguais ocultas')).toBeDefined();
    expect(screen.getByText('7 linhas iguais ocultas')).toBeDefined();
  });

  it('aceita as linhas prontas', () => {
    render(
      <DiffView
        file="a.toml"
        lines={[
          { kind: 'fold', count: 1 },
          { kind: 'add', newNo: 2, text: 'x = 1' },
        ]}
      />,
    );
    expect(screen.getByText('1 linha igual oculta')).toBeDefined();
    expect(screen.getByText('x = 1')).toBeDefined();
  });
});
