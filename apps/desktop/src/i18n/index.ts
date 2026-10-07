/**
 * Configuração do i18next: um único idioma, pt-BR (ADR-0016; ARCHITECTURE §18).
 *
 * Os namespaces não estão listados aqui: todo arquivo de `pt-BR/` é carregado por
 * `import.meta.glob` e cada um exporta uma constante com o nome do namespace
 * (`export const java = {…}`). Os tipos vêm de `catalogo.ts`, que cada arquivo estende com um
 * `declare module`. Para criar um namespace, basta criar o arquivo (ROADMAP §1).
 */
import i18n, { type Resource } from 'i18next';
import { initReactI18next } from 'react-i18next';

import type { Catalogo } from './catalogo';

export const defaultNS = 'comum';

const modules = import.meta.glob<Record<string, unknown>>(['./pt-BR/*.ts', '!./pt-BR/*.test.ts'], {
  eager: true,
});

export const resources = {
  'pt-BR': Object.assign({}, ...Object.values(modules)) as Catalogo,
} as const;

void i18n.use(initReactI18next).init({
  lng: 'pt-BR',
  fallbackLng: 'pt-BR',
  supportedLngs: ['pt-BR'],
  defaultNS,
  ns: Object.keys(resources['pt-BR']),
  resources: resources as unknown as Resource,
  initAsync: false,
  // O React já escapa o texto.
  interpolation: { escapeValue: false },
});

export default i18n;
