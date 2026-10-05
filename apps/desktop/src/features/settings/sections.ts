/**
 * Seções de Configurações (T21), na ordem da SPEC: Geral, Chaves e contas, Teste, Editor de
 * configs, Privacidade e registros, Armazenamento (P1) e Sobre o Warden (T23), sempre a última.
 *
 * Registro acréscimo-apenas (ROADMAP §1): a tarefa dona de uma seção nova acrescenta a linha
 * dela no lugar certo da ordem, e uma seção só aparece quando a tarefa que a faz existe
 * (Armazenamento entra com a A-06, em `storage/`).
 *
 * Partes de outras tarefas dentro de uma seção (a tabela de Java da L-01, o modelo do Gemini
 * da D-04) entram pelos encaixes de `slots.ts`.
 */
import type { ComponentType } from 'react';

import { AboutWarden } from '../about/components/AboutWarden';
import { ConfigEditorSection } from './components/ConfigEditorSection';
import { GeneralSection } from './components/GeneralSection';
import { KeysSection } from './components/KeysSection';
import { LogsSection } from './components/LogsSection';
import { TestSection } from './components/TestSection';

export interface SettingsSection {
  /** Identificador estável (também a âncora `#<id>` da seção). */
  id: string;
  /** A seção inteira: `<section className="panel">` com o próprio título `<h2>`. */
  Component: ComponentType;
}

export const settingsSections: readonly SettingsSection[] = [
  { id: 'geral', Component: GeneralSection },
  { id: 'chaves', Component: KeysSection },
  { id: 'teste', Component: TestSection },
  { id: 'editor', Component: ConfigEditorSection },
  { id: 'registros', Component: LogsSection },
  // A-06: { id: 'armazenamento', Component: StorageSection },
  { id: 'sobre', Component: AboutWarden },
];
