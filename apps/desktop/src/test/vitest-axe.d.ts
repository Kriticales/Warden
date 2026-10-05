// O `vitest-axe` 0.1 declara o matcher no namespace antigo (`Vi`); aqui ele entra no
// `Assertion` do Vitest atual.
import 'vitest';

import type { AxeMatchers } from 'vitest-axe/matchers';

declare module 'vitest' {
  // eslint-disable-next-line @typescript-eslint/no-empty-object-type
  interface Assertion extends AxeMatchers {}
  // eslint-disable-next-line @typescript-eslint/no-empty-object-type
  interface AsymmetricMatchersContaining extends AxeMatchers {}
}
