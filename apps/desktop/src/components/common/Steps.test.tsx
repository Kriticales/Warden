import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { axeComponent } from '../../test/axe';
import { NameTile, tilePixels } from './PixelArt';
import { Steps } from './Steps';

describe('Steps', () => {
  it('marca as concluídas com ✓ e a atual com aria-current; passa no axe', async () => {
    const { container } = render(
      <Steps items={['Um', 'Dois', 'Três']} current={1} label="Etapas de teste" />,
    );
    const list = screen.getByRole('list', { name: 'Etapas de teste' });
    const items = within(list).getAllByRole('listitem');
    expect(items.map((item) => item.textContent)).toEqual([
      'Um (concluída)',
      '2Dois (agora)',
      '3Três',
    ]);
    expect(items[0]?.className).toContain('steps__item--done');
    expect(items[1]?.getAttribute('aria-current')).toBe('step');
    expect(items[2]?.getAttribute('aria-current')).toBeNull();
    expect(await axeComponent(container)).toHaveNoViolations();
  });
});

describe('NameTile', () => {
  it('o mesmo nome dá o mesmo desenho, simétrico; nomes diferentes, desenhos diferentes', () => {
    const a = tilePixels('Vale Sereno');
    expect(tilePixels('Vale Sereno')).toEqual(a);
    expect(tilePixels('Cobre Antigo')).not.toEqual(a);
    // Fundo 8×8 e pares espelhados (x e 7 − x).
    expect(a[0]?.slice(0, 4)).toEqual([0, 0, 8, 8]);
    const lit = a.slice(1);
    expect(lit.length % 2).toBe(0);
    for (let index = 0; index < lit.length; index += 2) {
      const [left, right] = [lit[index], lit[index + 1]];
      expect(right?.[0]).toBe(7 - (left?.[0] ?? 0));
      expect(right?.[1]).toBe(left?.[1]);
    }
  });

  it('é decorativo e respeita o tamanho', () => {
    const { container } = render(<NameTile seed="Vale Sereno" size="lg" />);
    const tile = container.querySelector('.tile');
    expect(tile?.className).toBe('tile tile--lg');
    expect(tile?.getAttribute('aria-hidden')).toBe('true');
  });
});
