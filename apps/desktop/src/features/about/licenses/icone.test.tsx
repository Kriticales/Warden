/**
 * Ícone do app (A-02): `src-tauri/icons/warden.svg`, de onde saem os ícones do executável e do
 * instalador (`gerar-icones.mjs`), tem de ser a mesma marca que a interface desenha
 * (`BrandMark`, igual ao `brandMark` do design system aprovado).
 */
import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { BrandMark } from '../../../components/common/PixelArt';
import iconSvg from '../../../../src-tauri/icons/warden.svg?raw';

const ATTRIBUTES = ['x', 'y', 'width', 'height', 'fill', 'fill-opacity'] as const;

function rects(root: ParentNode): string[] {
  return Array.from(root.querySelectorAll('rect'), (rect) =>
    ATTRIBUTES.map((name) => {
      const value = rect.getAttribute(name);
      // `.6` e `0.6` são a mesma opacidade; sem o atributo, opaco.
      return name === 'fill-opacity' ? String(Number(value ?? '1')) : (value ?? '');
    }).join(' '),
  );
}

describe('ícone do app', () => {
  it('é a marca do Warden desenhada pela interface', () => {
    const icon = new DOMParser().parseFromString(iconSvg, 'image/svg+xml');
    const { container } = render(<BrandMark />);
    const fromIcon = rects(icon);
    expect(fromIcon.length).toBeGreaterThan(0);
    expect(fromIcon).toEqual(rects(container));
    expect(icon.documentElement.getAttribute('viewBox')).toBe('0 0 16 16');
  });
});
