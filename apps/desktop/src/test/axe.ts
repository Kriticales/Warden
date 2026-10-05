/**
 * Acessibilidade nos testes de componente (QUALITY §4.1; HANDOFF §7): `axe-core` pelo
 * `vitest-axe`, com o matcher `toHaveNoViolations`.
 *
 * Desligadas no jsdom, de propósito:
 * - `color-contrast`: o jsdom não calcula cores; o contraste dos tokens é medido por
 *   `node design/system/tools/contraste.mjs` (102 pares) e o E2E roda o axe no app real.
 * - `region` só em componentes soltos (`axeComponent`): fora da moldura do app eles não estão
 *   dentro de landmarks. A tela inteira (`axePage`) confere também as regiões.
 */
import { configureAxe } from 'vitest-axe';

export const axeComponent = configureAxe({
  rules: {
    'color-contrast': { enabled: false },
    region: { enabled: false },
  },
});

export const axePage = configureAxe({
  rules: {
    'color-contrast': { enabled: false },
  },
});
