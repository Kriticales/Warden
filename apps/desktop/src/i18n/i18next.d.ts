// Chaves do catálogo tipadas: chave inexistente não compila (ARCHITECTURE §18).
import 'i18next';

import type { defaultNS, resources } from './index';

declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: typeof defaultNS;
    resources: (typeof resources)['pt-BR'];
  }
}
