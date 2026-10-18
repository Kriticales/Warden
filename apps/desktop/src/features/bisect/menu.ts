/**
 * "Encontrar o mod culpado…" no menu ▾ do Testar (grupo "Outros testes"; SPEC T13 e T25).
 * Registrado em `../test/menu-items.ts`, com o diálogo em `testDialogs`.
 */
import { Target } from 'lucide-react';

import i18n from '../../i18n';
import type { TestDialog, TestMenuItem } from '../test/menu-items';
import { BisectDialog } from './BisectDialog';

const t = i18n.getFixedT(null, 'buscaDoCulpado');

/** Id do diálogo em `testDialogs`. */
export const BISECT_DIALOG = 'bisect';

export const bisectMenuItem: TestMenuItem = {
  id: 'bisect',
  group: 'other',
  order: 400,
  icon: Target,
  label: () => t('menu.item'),
  // Um jogo por vez: a busca abre o jogo várias vezes.
  description: ({ game }) =>
    game ? i18n.t('menu.comJogoAberto', { ns: 'teste' }) : t('menu.descricao'),
  disabled: ({ game }) => game !== null,
  onSelect: ({ openDialog }) => {
    openDialog(BISECT_DIALOG);
  },
};

export const bisectDialog: TestDialog = { id: BISECT_DIALOG, component: BisectDialog };
