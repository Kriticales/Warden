/**
 * Configuração do i18next: um único idioma, pt-BR (ADR-0016; ARCHITECTURE §18).
 *
 * Registro acréscimo-apenas (ROADMAP §1): cada área acrescenta uma linha de import e uma
 * linha em `resources`.
 */
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import { boasVindas } from './pt-BR/boas-vindas';
import { comum } from './pt-BR/comum';
import { configuracoes } from './pt-BR/configuracoes';
import { licencas } from './pt-BR/licencas';
import { navegacao } from './pt-BR/navegacao';
import { sobre } from './pt-BR/sobre';
import { tarefas } from './pt-BR/tarefas';
import { java } from './pt-BR/java';
import { packs } from './pt-BR/packs';
import { editor } from './pt-BR/editor';
import { exportar } from './pt-BR/exportar';

export const defaultNS = 'comum';

export const resources = {
  'pt-BR': {
    comum,
    licencas,
    navegacao,
    configuracoes,
    boasVindas,
    sobre,
    tarefas,
    java,
    packs,
    editor,
    exportar,
  },
} as const;

void i18n.use(initReactI18next).init({
  lng: 'pt-BR',
  fallbackLng: 'pt-BR',
  supportedLngs: ['pt-BR'],
  defaultNS,
  ns: Object.keys(resources['pt-BR']),
  resources,
  initAsync: false,
  // O React já escapa o texto.
  interpolation: { escapeValue: false },
});

export default i18n;
